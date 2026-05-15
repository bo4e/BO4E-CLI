//! Pure render helpers consumed by `rust::plain` and `rust::crate_` orchestrators.

use bo4e_schemas::models::json_schema::{ObjectSchema, SchemaType};
use minijinja::{Environment, context};
use serde::Serialize;
use std::collections::BTreeSet;

use crate::Error;
use crate::imports::Import;
use crate::naming::{sanitize_member_name, to_pascal_case};
use crate::refs::schema_base;
use crate::rust::imports::UseBlock;
use crate::rust::types::{
    UnsupportedShape, enum_variant_default_rust, literal_default_rust, map_rust,
};

/// Render a docstring block as outer `///` lines. Empty input → empty string.
/// Preserves embedded line breaks verbatim — Sphinx RST is not stripped.
/// The struct template applies its own `{{ doc | indent(4) }}` when needed,
/// so this helper always emits flush-left lines.
fn render_doc_comment(description: Option<&str>) -> String {
    let Some(text) = description.map(str::trim).filter(|s| !s.is_empty()) else {
        return String::new();
    };
    text.lines()
        .map(|line| format!("/// {line}").trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the single-variant enum that types a const-valued property.
/// `class_name` = e.g. `"AngebotTyp"`; `wire_value` = the JSON literal, e.g. `"ANGEBOT"`.
fn render_single_variant_enum(
    env: &Environment<'static>,
    class_name: &str,
    wire_value: &str,
    docstring: Option<&str>,
) -> Result<String, Error> {
    let variant_ident = to_pascal_case(&sanitize_member_name(wire_value));
    let tpl = env.get_template("rust/plain/Enum.jinja2")?;
    Ok(tpl.render(context! {
        doc => render_doc_comment(docstring),
        single_variant => true,
        class_name => class_name,
        variants => vec![context! {
            wire_quoted => rust_string_literal(wire_value),
            name => variant_ident,
        }],
    })?)
}

/// Render the plain string-enum form (`enum Typ { Angebot, Ausschreibung, … }`).
pub(crate) fn render_str_enum(
    env: &Environment<'static>,
    class_name: &str,
    members: &[String],
    docstring: Option<&str>,
) -> Result<String, Error> {
    let variants: Vec<_> = members
        .iter()
        .map(|m| {
            context! {
                wire_quoted => rust_string_literal(m),
                name => to_pascal_case(&sanitize_member_name(m)),
            }
        })
        .collect();
    let tpl = env.get_template("rust/plain/Enum.jinja2")?;
    Ok(tpl.render(context! {
        doc => render_doc_comment(docstring),
        single_variant => false,
        class_name => class_name,
        variants => variants,
    })?)
}

/// Render a string as a Rust double-quoted string literal — handles
/// quotes, backslashes, and control characters via the standard
/// Debug format. Used wherever a runtime string flows into Rust
/// source via a template (currently `serde(rename = …)` on enum
/// variants). Mirrors [`crate::python::python_string_literal`] on
/// the Python side so both flavours emit safe literals from
/// arbitrary JSON-schema values.
fn rust_string_literal(s: &str) -> String {
    format!("{s:?}")
}

/// Per-field context for the Struct.jinja2 template.
#[derive(Debug, Serialize)]
struct RustField {
    pub name: String,
    pub type_hint: String,
    pub serde_attrs: String,
    pub doc: String,
    /// Default expression for the field, used by `render_default_impl`.
    #[serde(skip)]
    pub default_expr: Option<String>,
}

/// A per-field serde-`default` helper, emitted alongside the struct it
/// belongs to. Required for any optional field whose schema default does
/// not match what bare `#[serde(default)]` would produce — i.e. row 3
/// (optional + non-nullable + literal) and row 5 (optional + nullable +
/// non-null literal) of the strict default matrix.
#[derive(Debug, Serialize)]
struct HelperFn {
    /// `default_<rust_field_name>`.
    pub name: String,
    /// Return type matches the field's `type_hint`.
    pub return_type: String,
    /// Body expression (`Some(Typ::Angebot)`, `42i64`, etc.).
    pub body: String,
}

/// Outcome of rendering an object schema: the file body, ready to write.
pub(crate) struct RenderedObject {
    pub body: String,
    /// Brief per-file decision summary suitable for verbose CLI output.
    pub diagnostic: String,
}

#[derive(Debug)]
enum DefaultImplOutcome {
    /// Every field has a default expression. Carries the validated
    /// `(name, expr)` pairs so the renderer doesn't need to re-unwrap
    /// `default_expr` and parallel-track invariants with the field slice.
    Emitted { pairs: Vec<(String, String)> },
    /// At least one field is missing a default; we emit a comment instead
    /// of an `impl Default for X` block.
    Skipped { missing: Vec<String> },
}

impl DefaultImplOutcome {
    /// Inspect `fields` and decide whether a `Default` impl can be emitted —
    /// a field with no `default_expr` blocks the block. The two-pass version
    /// (collect missing, then map fields) is fused into one walk here.
    fn from_fields(fields: &[RustField]) -> Self {
        let mut pairs: Vec<(String, String)> = Vec::with_capacity(fields.len());
        let mut missing: Vec<String> = Vec::new();
        for f in fields {
            match f.default_expr.as_deref() {
                Some(expr) => pairs.push((f.name.clone(), expr.to_string())),
                None => missing.push(f.name.clone()),
            }
        }
        if missing.is_empty() {
            Self::Emitted { pairs }
        } else {
            Self::Skipped { missing }
        }
    }
}

impl std::fmt::Display for DefaultImplOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Emitted { .. } => write!(f, "Default impl emitted"),
            Self::Skipped { missing } => {
                let (subject, verb) = if missing.len() > 1 {
                    ("fields", "have")
                } else {
                    ("field", "has")
                };
                write!(
                    f,
                    "Default impl SKIPPED: {subject} `{}` {verb} no default expression",
                    missing.join("`, `")
                )
            }
        }
    }
}

/// Render a single object schema as a `.rs` file body.
///
/// `depth` is the file's directory depth from the output root (1 = root-level, 2 = under `bo/`, …).
pub(crate) fn render_object(
    env: &Environment<'static>,
    class_name: &str,
    obj: &ObjectSchema,
    depth: usize,
) -> Result<RenderedObject, Error> {
    let mut imports: BTreeSet<Import> = BTreeSet::new();

    let required: BTreeSet<&str> = obj.required.iter().map(|s| s.as_str()).collect();
    let mut fields: Vec<RustField> = Vec::new();
    let mut extra_enums: Vec<String> = Vec::new();
    let mut helpers: Vec<HelperFn> = Vec::new();

    for (prop_name, prop_schema) in &obj.properties {
        let (rust_name, needs_rename) = crate::rust::rust_field_name(prop_name);
        let is_required = required.contains(prop_name.as_str());

        // Detect `_typ`-style discriminator: ConstantSchema or anyOf:[const,null] or
        // anyOf:[$ref to enum/Typ, null] with TypeBase.default
        let (type_hint, default_expr) =
            if let Some(disc) = single_variant_discriminator(prop_schema) {
                let enum_name = format!(
                    "{class_name}{}",
                    to_pascal_case(&strip_leading_underscore(prop_name))
                );
                extra_enums.push(render_single_variant_enum(
                    env,
                    &enum_name,
                    &disc.wire_value,
                    schema_base(prop_schema).description.as_deref(),
                )?);
                // Type follows the schema's nullability. Default expression
                // follows the strict required/default invariant: emit one
                // only when the schema declares a `default` (validated
                // upstream — required ⇔ no default).
                let type_hint = if disc.nullable {
                    format!("Option<{enum_name}>")
                } else {
                    enum_name.clone()
                };
                let default_expr = if schema_base(prop_schema).default.is_some() {
                    if disc.nullable {
                        Some(format!("Some({enum_name}::default())"))
                    } else {
                        Some(format!("{enum_name}::default()"))
                    }
                } else {
                    None
                };
                (type_hint, default_expr)
            } else {
                let mapped = map_rust(prop_schema).map_err(|UnsupportedShape(shape)| {
                    Error::UnsupportedSchemaShape {
                        schema_name: class_name.to_string(),
                        property: prop_name.clone(),
                        shape,
                    }
                })?;
                imports.extend(mapped.imports.iter().cloned());

                // The Rust type follows the schema's nullability ONLY: no
                // auto-wrapping in `Option<T>` based on `required`. The strict
                // required/default invariant (validated in `crate::validate`)
                // guarantees we never have an optional field without a
                // default, so we don't need to invent `Option<T>` to express
                // "may be absent".
                let inner = mapped
                    .rendered
                    .strip_prefix("Option<")
                    .and_then(|s| s.strip_suffix('>'))
                    .unwrap_or(&mapped.rendered)
                    .to_string();
                let type_hint = mapped.rendered.clone();
                let is_option = is_option_typed(&type_hint);

                // Pick the default expression:
                //   - $ref-to-enum + string default → `EnumName::Variant`
                //     (renders e.g. `Some(Typ::Angebot)` after wrapping).
                //   - otherwise → the primitive literal from
                //     `literal_default_rust` (already escaped).
                let raw = enum_variant_default_rust(prop_schema, &inner)
                    .or_else(|| literal_default_rust(prop_schema));

                // Wrap in `Some(...)` for `Option<T>` non-null literal defaults
                // so the expression's type matches the field. A `null` schema
                // default is already rendered as `"None"` and stays as-is.
                let default_expr = raw.map(|d| {
                    if is_option && d != "None" {
                        format!("Some({d})")
                    } else {
                        d
                    }
                });

                (type_hint, default_expr)
            };

        let attrs_decision = decide_serde_attrs(
            prop_name,
            &rust_name,
            needs_rename,
            &type_hint,
            default_expr.as_deref(),
            is_required,
        );
        if attrs_decision.needs_helper
            && let Some(body) = default_expr.as_deref()
        {
            helpers.push(HelperFn {
                name: format!("default_{rust_name}"),
                return_type: type_hint.clone(),
                body: body.to_string(),
            });
        }
        let serde_attrs = attrs_decision.attrs;
        let doc = render_doc_comment(schema_base(prop_schema).description.as_deref());

        fields.push(RustField {
            name: rust_name,
            type_hint,
            serde_attrs,
            doc,
            default_expr,
        });
    }

    let uses = {
        let mut b = UseBlock::new();
        b.extend(imports.iter().cloned());
        b.render(depth)
    };
    let doc = render_doc_comment(obj.base.description.as_deref());

    let outcome = DefaultImplOutcome::from_fields(&fields);
    let default_impl = render_default_impl(env, class_name, &outcome)?;

    let n_fields = fields.len();
    let n_synth = extra_enums.len();
    let synth = if n_synth > 0 {
        format!(", {n_synth} synthetic enums")
    } else {
        String::new()
    };
    let diagnostic = format!("struct {class_name} ({n_fields} fields, {outcome}{synth})");

    let tpl = env.get_template("rust/plain/Struct.jinja2")?;
    let body = tpl.render(context! {
        uses => uses,
        extra_enums => extra_enums,
        doc => doc,
        class_name => class_name,
        fields => &fields,
        default_impl => default_impl,
        helpers => &helpers,
    })?;

    Ok(RenderedObject { body, diagnostic })
}

fn strip_leading_underscore(s: &str) -> String {
    s.strip_prefix('_').unwrap_or(s).to_string()
}

/// Whether a rendered Rust type string represents an `Option<T>`. We detect
/// this by string prefix because `MappedType.rendered` is a `String` rather
/// than a structured type — centralised here so the convention has one site
/// to update if the rendering ever drifts (e.g. nested `Option`, custom
/// spacing).
fn is_option_typed(rendered: &str) -> bool {
    rendered.starts_with("Option<")
}

struct SerdeAttrsDecision {
    /// The comma-joined `serde(...)` argument list, e.g.
    /// `rename = "_id", default, skip_serializing_if = "Option::is_none"`.
    attrs: String,
    /// Whether the field also needs a per-field `default_<name>()` helper
    /// emitted alongside the struct. The helper is referenced by name from
    /// the serde attrs above (`default = "default_<name>"`).
    needs_helper: bool,
}

/// Pick the per-field serde attribute list for the strict required/default
/// matrix, plus whether a `default_<field>()` helper is needed:
///
/// | required | nullable | default | attrs                              | helper |
/// |----------|----------|---------|------------------------------------|--------|
/// | ✓        | ✗        | ✗       | —                                  | no     |
/// | ✓        | ✓        | ✗       | —  *(field always serialised)*     | no     |
/// | ✗        | ✗        | literal | `default = "default_<field>"`      | yes    |
/// | ✗        | ✓        | null    | `default, skip_serializing_if = …` | no     |
/// | ✗        | ✓        | literal | `default = "default_<field>"`      | yes    |
///
/// "literal" includes both primitive literals (`"X"`, `42i64`, `true`) and
/// `T::default()`-shaped expressions (the synthetic single-variant
/// discriminator emits this — bare `#[serde(default)]` actually works for
/// the non-Option case there since the field type's `Default::default()`
/// matches the desired value, so no helper is emitted).
fn decide_serde_attrs(
    prop_name: &str,
    rust_name: &str,
    needs_rename: bool,
    type_hint: &str,
    default_expr: Option<&str>,
    is_required: bool,
) -> SerdeAttrsDecision {
    let mut parts = Vec::<String>::new();
    if needs_rename {
        parts.push(format!("rename = \"{prop_name}\""));
    }
    let is_option = is_option_typed(type_hint);
    let mut needs_helper = false;

    if let Some(d) = default_expr {
        debug_assert!(
            !is_required,
            "validator should reject required+default; field `{prop_name}`",
        );
        // Decide whether bare `#[serde(default)]` already produces the
        // schema's declared default. If so, emit bare `default` and skip
        // the helper. Otherwise generate a `default_<name>` helper and
        // reference it.
        let bare_default_matches = if is_option {
            // `Option::<T>::default()` is `None`, which only matches when
            // the schema's literal default is itself null.
            d == "None"
        } else {
            // `T::default()` matches only the `::default()`-shaped expression
            // produced by the synthetic single-variant discriminator branch.
            d == "Default::default()" || d.ends_with("::default()")
        };
        if bare_default_matches {
            parts.push("default".to_string());
        } else {
            parts.push(format!("default = \"default_{rust_name}\""));
            needs_helper = true;
        }
        // Skip-serializing-if is intentionally narrow: row 4 only. The
        // language's `Option::is_none` matches the schema's null default,
        // so serialised JSON omits the key when it's at its default —
        // a clean round-trip. For all other rows we *always* serialise
        // the field (required keys must be present; non-null literal
        // defaults still get echoed back since serde has no built-in
        // "skip if equals literal" predicate).
        if is_option && d == "None" {
            parts.push("skip_serializing_if = \"Option::is_none\"".to_string());
        }
    }
    SerdeAttrsDecision {
        attrs: parts.join(", "),
        needs_helper,
    }
}

fn render_default_impl(
    env: &Environment<'static>,
    class_name: &str,
    outcome: &DefaultImplOutcome,
) -> Result<String, Error> {
    let (missing, field_ctx) = match outcome {
        DefaultImplOutcome::Emitted { pairs } => {
            let ctx: Vec<_> = pairs
                .iter()
                .map(|(name, expr)| context! { name => name, expr => expr })
                .collect();
            (Vec::<&str>::new(), ctx)
        }
        DefaultImplOutcome::Skipped { missing } => {
            (missing.iter().map(String::as_str).collect(), Vec::new())
        }
    };
    let tpl = env.get_template("rust/plain/DefaultImpl.jinja2")?;
    Ok(tpl.render(context! {
        class_name => class_name,
        missing => missing,
        fields => field_ctx,
    })?)
}

/// Outcome of `single_variant_discriminator`: the constant wire value plus a
/// flag for whether the schema permits `null`. Nullable discriminators are
/// wrapped in `Option<EnumName>` by the caller so that schema-valid `null`
/// payloads deserialize as `None` instead of erroring (matches pydantic's
/// looser typing for `anyOf:[const|$ref, null]`).
struct DiscriminatorMatch {
    wire_value: String,
    nullable: bool,
}

/// Detect a `_typ`-style discriminator. Returns `Some` with the constant wire
/// value and a flag indicating whether the schema's discriminator branch is
/// nullable (`anyOf: […, null]`).
fn single_variant_discriminator(prop_schema: &SchemaType) -> Option<DiscriminatorMatch> {
    fn const_value(s: &SchemaType) -> Option<&str> {
        match s {
            SchemaType::ConstantSchema(c) => Some(c.constant.as_str()),
            SchemaType::StrEnum(e) if e.enum_values.len() == 1 => Some(e.enum_values[0].as_str()),
            _ => None,
        }
    }

    // direct (non-nullable by construction)
    if let Some(v) = const_value(prop_schema) {
        return Some(DiscriminatorMatch {
            wire_value: v.to_string(),
            nullable: false,
        });
    }

    // anyOf:[const, null] or anyOf:[single-element-StrEnum, null] — these are
    // *true* single-value schemas: the type system literally permits only one
    // wire value (plus null when nullable). The earlier `anyOf:[$ref to a
    // multi-element enum, null] + default` arm has been removed — a default
    // does not narrow the type, it just provides an absent-key fallback, so
    // those fields should be rendered as `Option<RefedEnum>` (handled by the
    // general map_rust path).
    if let SchemaType::AnyOf(a) = prop_schema {
        let nullable = a
            .any_of
            .iter()
            .any(|t| matches!(t, SchemaType::NullSchema(_)));
        let non_null: Vec<&SchemaType> = a
            .any_of
            .iter()
            .filter(|t| !matches!(t, SchemaType::NullSchema(_)))
            .collect();
        if non_null.len() == 1
            && let Some(v) = const_value(non_null[0])
        {
            return Some(DiscriminatorMatch {
                wire_value: v.to_string(),
                nullable,
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_comment_single_line() {
        let s = render_doc_comment(Some("Hello world"));
        assert_eq!(s, "/// Hello world");
    }

    #[test]
    fn doc_comment_multi_line_preserves_breaks() {
        let s = render_doc_comment(Some("Line one\nLine two"));
        assert_eq!(s, "/// Line one\n/// Line two");
    }

    #[test]
    fn doc_comment_empty_returns_empty() {
        assert_eq!(render_doc_comment(None), "");
        assert_eq!(render_doc_comment(Some("")), "");
        assert_eq!(render_doc_comment(Some("   ")), "");
    }

    #[test]
    #[cfg(feature = "rust-plain")]
    fn single_variant_enum_shape() {
        let env = make_env();
        let s = render_single_variant_enum(
            &env,
            "AngebotTyp",
            "ANGEBOT",
            Some("Angebot discriminator"),
        )
        .unwrap();
        assert!(s.contains("/// Angebot discriminator"));
        assert!(s.contains("pub enum AngebotTyp"));
        assert!(s.contains("#[default]"));
        assert!(s.contains("#[serde(rename = \"ANGEBOT\")]"));
        assert!(s.contains("Angebot,"));
    }

    #[test]
    #[cfg(feature = "rust-plain")]
    fn str_enum_one_variant_per_member() {
        let env = make_env();
        let members = vec!["ANGEBOT".to_string(), "AUSSCHREIBUNG".to_string()];
        let s = render_str_enum(&env, "Typ", &members, None).unwrap();
        assert!(s.contains("pub enum Typ"));
        assert!(s.contains("#[serde(rename = \"ANGEBOT\")]"));
        assert!(s.contains("Angebot,"));
        assert!(s.contains("#[serde(rename = \"AUSSCHREIBUNG\")]"));
        assert!(s.contains("Ausschreibung,"));
    }

    #[test]
    #[cfg(feature = "rust-plain")]
    fn str_enum_handles_hyphenated_member() {
        let env = make_env();
        let members = vec!["2-01-7-001".to_string()];
        let s = render_str_enum(&env, "Code", &members, None).unwrap();
        assert!(s.contains("#[serde(rename = \"2-01-7-001\")]"));
        assert!(s.contains("_2_01_7_001,"));
    }

    use bo4e_schemas::models::json_schema::{
        AnyOfSchema, LiteralTypeObject, LiteralTypeString, NullSchema, ObjectSchema,
        PrimitiveValue, SchemaType, StringSchema, TypeBase,
    };
    use std::collections::BTreeMap;

    fn make_env() -> minijinja::Environment<'static> {
        crate::env::make_environment(None).unwrap()
    }

    fn obj_with_props(
        props: Vec<(&str, SchemaType)>,
        required: Vec<&str>,
        desc: Option<&str>,
    ) -> ObjectSchema {
        let mut base = TypeBase::default();
        if let Some(d) = desc {
            base.description = Some(d.to_string());
        }
        let mut map: BTreeMap<String, SchemaType> = BTreeMap::new();
        for (k, v) in props {
            map.insert(k.to_string(), v);
        }
        ObjectSchema {
            base,
            r#type: LiteralTypeObject::Object,
            additional_properties: true,
            properties: map,
            required: required.into_iter().map(String::from).collect(),
        }
    }

    fn p_string_or_null(default: Option<&str>) -> SchemaType {
        let mut base = TypeBase::default();
        if let Some(d) = default {
            base.default = Some(PrimitiveValue::String(d.to_string()));
        }
        SchemaType::AnyOf(AnyOfSchema {
            base,
            any_of: vec![
                SchemaType::StringSchema(StringSchema {
                    base: TypeBase::default(),
                    r#type: LiteralTypeString::String,
                    format: None,
                }),
                SchemaType::NullSchema(NullSchema::default()),
            ],
        })
    }

    #[test]
    #[cfg(feature = "rust-plain")]
    fn render_object_emits_struct_and_field_renames() {
        let env = make_env();
        let schema = obj_with_props(
            vec![
                ("_id", p_string_or_null(None)),
                ("angebotsnummer", p_string_or_null(None)),
            ],
            vec![],
            Some("An offer."),
        );
        let r = render_object(&env, "Angebot", &schema, 2).unwrap();
        assert!(r.body.contains("pub struct Angebot"), "got:\n{}", r.body);
        assert!(
            r.body.contains("pub id: Option<String>"),
            "got:\n{}",
            r.body
        );
        assert!(r.body.contains("rename = \"_id\""), "got:\n{}", r.body);
        assert!(r.body.contains("pub angebotsnummer: Option<String>"));
    }

    /// Row 2 of the strict matrix: `required + nullable + no default`. The
    /// `anyOf:[$ref to Menge, null]` field is in `required`, so the JSON key
    /// must always be present (the value may be null). The Rust type stays
    /// `Option<Menge>` and serde gets **no attributes at all** — neither
    /// `default` (required keys must not have a missing-key fallback) nor
    /// `skip_serializing_if` (the schema says the key is always present, so
    /// serialised JSON must always include it, even when value is None).
    #[test]
    #[cfg(feature = "rust-plain")]
    fn render_object_required_nullable_ref_stays_option() {
        use bo4e_schemas::models::json_schema::ReferenceSchema;

        fn p_ref_or_null(target_ref: &str) -> SchemaType {
            SchemaType::AnyOf(AnyOfSchema {
                base: TypeBase::default(),
                any_of: vec![
                    SchemaType::ReferenceSchema(ReferenceSchema {
                        base: TypeBase::default(),
                        r#ref: target_ref.to_string(),
                    }),
                    SchemaType::NullSchema(NullSchema::default()),
                ],
            })
        }

        let env = make_env();
        let schema = obj_with_props(
            vec![("zeitIntervallLaenge", p_ref_or_null("../com/Menge.json"))],
            vec!["zeitIntervallLaenge"],
            Some("A load profile."),
        );
        let r = render_object(&env, "Lastgang", &schema, 2).unwrap();
        assert!(
            r.body.contains("pub zeit_intervall_laenge: Option<Menge>"),
            "required+nullable field must stay Option<T>, got:\n{}",
            r.body
        );
        // Required → no `default` attr (missing key must error). Required
        // → no `skip_serializing_if` either (key must always be present in
        // serialised JSON). Look only at serde-attr lines; the
        // "Default impl omitted" comment harmlessly mentions "default".
        let serde_lines: Vec<&str> = r.body.lines().filter(|l| l.contains("#[serde(")).collect();
        for line in &serde_lines {
            assert!(
                !line.contains("default"),
                "required field must not have a `default` serde attr, got line:\n{line}",
            );
            assert!(
                !line.contains("skip_serializing_if"),
                "required field must not have skip_serializing_if, got line:\n{line}",
            );
        }
    }

    /// Row 3 of the strict matrix: `optional + non-nullable + literal
    /// default`. The Rust type stays the underlying primitive (no
    /// `Option<…>` wrap) and serde gets `default = "default_<field>"`
    /// referencing a generated per-field helper. The helper function is
    /// emitted alongside the struct.
    #[test]
    #[cfg(feature = "rust-plain")]
    fn render_object_optional_non_nullable_string_with_literal_default() {
        let env = make_env();
        let schema = obj_with_props(
            vec![("anrede", s_string_with_default(Some("Herr")))],
            vec![], // not required, but has a default → row 3 (consistent)
            Some("A salutation."),
        );
        let r = render_object(&env, "Angebot", &schema, 2).unwrap();
        // Type stays `String` (schema is non-nullable, optionality is
        // expressed via the serde `default` attribute, not the type).
        assert!(
            r.body.contains("pub anrede: String"),
            "optional non-nullable string must stay `String` (no Option wrap), got:\n{}",
            r.body
        );
        assert!(
            !r.body.contains("pub anrede: Option<"),
            "must NOT wrap in Option, got:\n{}",
            r.body
        );
        // Serde attrs reference the per-field helper.
        assert!(
            r.body.contains("default = \"default_anrede\""),
            "expected `default = \"default_anrede\"`, got:\n{}",
            r.body
        );
        // The helper itself is emitted.
        assert!(
            r.body.contains("fn default_anrede() -> String"),
            "expected helper fn, got:\n{}",
            r.body
        );
        assert!(
            r.body.contains("\"Herr\".to_string()"),
            "helper body should return the literal default, got:\n{}",
            r.body
        );
    }

    fn s_string_with_default(default_str: Option<&str>) -> SchemaType {
        SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default: default_str.map(|s| PrimitiveValue::String(s.to_string())),
                ..TypeBase::default()
            },
            r#type: LiteralTypeString::String,
            format: None,
        })
    }

    /// Row 5: an optional `$ref`-to-enum field with a string default no
    /// longer triggers the synthetic-discriminator narrowing — schema says
    /// `anyOf:[$ref to multi-element BoTyp, null]` so the value can be any
    /// `BoTyp` variant at runtime. Renders as `Option<BoTyp>` with the
    /// default literal qualified to `Some(BoTyp::Angebot)` via a per-field
    /// helper.
    #[test]
    #[cfg(feature = "rust-plain")]
    fn render_object_optional_enum_ref_with_default_uses_full_enum_type() {
        use bo4e_schemas::models::json_schema::ReferenceSchema;

        let typ_schema = SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("ANGEBOT".into())),
                ..TypeBase::default()
            },
            any_of: vec![
                SchemaType::ReferenceSchema(ReferenceSchema {
                    base: TypeBase::default(),
                    r#ref: "../enum/BoTyp.json".to_string(),
                }),
                SchemaType::NullSchema(NullSchema::default()),
            ],
        });

        let env = make_env();
        let schema = obj_with_props(vec![("_typ", typ_schema)], vec![], Some("An offer."));
        let r = render_object(&env, "Angebot", &schema, 2).unwrap();
        // No synthetic AngebotTyp any more — the type is the referenced enum.
        assert!(
            !r.body.contains("pub enum AngebotTyp"),
            "no synthetic single-variant enum for $ref-to-multi-element, got:\n{}",
            r.body
        );
        assert!(
            r.body.contains("pub typ: Option<BoTyp>"),
            "expected `Option<BoTyp>`, got:\n{}",
            r.body
        );
        // The default gets resolved to the enum variant via the helper.
        assert!(
            r.body.contains("fn default_typ() -> Option<BoTyp>"),
            "expected default_typ helper, got:\n{}",
            r.body
        );
        assert!(
            r.body.contains("Some(BoTyp::Angebot)"),
            "helper should return Some(BoTyp::Angebot), got:\n{}",
            r.body
        );
        // serde attrs reference the helper.
        assert!(
            r.body.contains("default = \"default_typ\""),
            "serde attrs should reference the helper, got:\n{}",
            r.body
        );
    }

    /// Row 1: required + non-nullable + no default. A `ConstantSchema` field
    /// (which restricts the value to a single string at the type level) in
    /// `required` produces the synthetic single-variant enum as the type but
    /// emits **no** serde default attr — the JSON key must be present,
    /// missing key must error.
    #[test]
    #[cfg(feature = "rust-plain")]
    fn render_object_required_const_discriminator_has_no_default() {
        use bo4e_schemas::models::json_schema::ConstantSchema;

        let const_typ = SchemaType::ConstantSchema(ConstantSchema {
            base: TypeBase::default(), // no schema-declared default
            r#type: LiteralTypeString::String,
            format: None,
            constant: "ANGEBOT".to_string(),
        });

        let env = make_env();
        let schema = obj_with_props(vec![("_typ", const_typ)], vec!["_typ"], None);
        let r = render_object(&env, "Angebot", &schema, 2).unwrap();
        // Synthetic enum is still emitted (the schema constrains the type
        // to a single value).
        assert!(
            r.body.contains("pub enum AngebotTyp"),
            "synthetic discriminator enum missing, got:\n{}",
            r.body
        );
        assert!(
            r.body.contains("pub typ: AngebotTyp"),
            "non-nullable discriminator must stay bare, got:\n{}",
            r.body
        );
        // Required + no-default → no `default` attr anywhere on the field.
        let body_lines: Vec<&str> = r.body.lines().collect();
        let typ_attr_line = body_lines
            .iter()
            .find(|l| l.contains("#[serde(") && l.contains("rename = \"_typ\""))
            .copied()
            .unwrap_or("");
        assert!(
            !typ_attr_line.contains("default"),
            "required + non-Option without schema default must not emit `default`, got line:\n{typ_attr_line}",
        );
        // The Default impl for the struct is skipped because the field has
        // no default expression (no `T::default()` fallback path).
        assert!(
            !r.body.contains("impl Default for Angebot"),
            "Default impl must be skipped when a required field lacks any default, got:\n{}",
            r.body
        );
    }

    /// Row 3 variant: an optional `ConstantSchema` field with a matching
    /// schema-declared default. The synthetic enum is emitted; its
    /// `EnumName::default()` matches the schema's literal, so bare
    /// `#[serde(default)]` works and no `default_<field>` helper is
    /// generated.
    #[test]
    #[cfg(feature = "rust-plain")]
    fn render_object_optional_const_discriminator_uses_bare_serde_default() {
        use bo4e_schemas::models::json_schema::ConstantSchema;

        let const_typ = SchemaType::ConstantSchema(ConstantSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("ANGEBOT".into())),
                ..TypeBase::default()
            },
            r#type: LiteralTypeString::String,
            format: None,
            constant: "ANGEBOT".to_string(),
        });

        let env = make_env();
        // `_typ` is NOT in required → optional → must have default → ✓.
        let schema = obj_with_props(vec![("_typ", const_typ)], vec![], None);
        let r = render_object(&env, "Angebot", &schema, 2).unwrap();
        let body_lines: Vec<&str> = r.body.lines().collect();
        let typ_attr_line = body_lines
            .iter()
            .find(|l| l.contains("#[serde(") && l.contains("rename = \"_typ\""))
            .copied()
            .unwrap_or("");
        // Bare `default` is sufficient: T::default() on the synthetic
        // single-variant enum returns the only variant.
        assert!(
            typ_attr_line.contains("default"),
            "optional + non-Option discriminator should emit `default`, got line:\n{typ_attr_line}",
        );
        assert!(
            !typ_attr_line.contains("default = \""),
            "bare `default` is enough; no helper-fn reference expected, got line:\n{typ_attr_line}",
        );
        // And therefore no `fn default_typ()` helper is emitted.
        assert!(
            !r.body.contains("fn default_typ()"),
            "no helper expected for the bare-default case, got:\n{}",
            r.body
        );
    }
}
