//! Pure render helpers consumed by `rust::plain` and `rust::crate_` orchestrators.

use crate::imports::Import;
use crate::naming::{sanitize_member_name, to_pascal_case};
use crate::rust::imports::UseBlock;

/// Render a docstring block as outer `///` lines. Empty input → empty string.
/// Preserves embedded line breaks verbatim — Sphinx RST is not stripped.
#[allow(dead_code)] // consumed by render_object in Task 21
pub(crate) fn render_doc_comment(description: Option<&str>, indent: &str) -> String {
    let Some(text) = description.map(str::trim).filter(|s| !s.is_empty()) else {
        return String::new();
    };
    text.lines()
        .map(|line| format!("{indent}/// {line}").trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the single-variant enum that types a const-valued property.
/// `class_name` = e.g. `"AngebotTyp"`; `wire_value` = the JSON literal, e.g. `"ANGEBOT"`.
#[allow(dead_code)] // consumed by render_object in Task 21
pub(crate) fn render_single_variant_enum(
    class_name: &str,
    wire_value: &str,
    docstring: Option<&str>,
) -> String {
    let variant_ident = to_pascal_case(&sanitize_member_name(wire_value));
    let doc = render_doc_comment(docstring, "");
    let mut out = String::new();
    if !doc.is_empty() {
        out.push_str(&doc);
        out.push('\n');
    }
    out.push_str(
        "#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]\n",
    );
    out.push_str(&format!("pub enum {class_name} {{\n"));
    out.push_str("    #[default]\n");
    out.push_str(&format!("    #[serde(rename = \"{wire_value}\")]\n"));
    out.push_str(&format!("    {variant_ident},\n"));
    out.push_str("}\n");
    out
}

/// Render the plain string-enum form (`enum Typ { Angebot, Ausschreibung, … }`).
#[allow(dead_code)] // consumed by rust::plain orchestrator in Task 22
pub(crate) fn render_str_enum(
    class_name: &str,
    members: &[String],
    docstring: Option<&str>,
) -> String {
    let doc = render_doc_comment(docstring, "");
    let mut out = String::new();
    if !doc.is_empty() {
        out.push_str(&doc);
        out.push('\n');
    }
    out.push_str("#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]\n");
    out.push_str(&format!("pub enum {class_name} {{\n"));
    for m in members {
        let variant = to_pascal_case(&sanitize_member_name(m));
        out.push_str(&format!("    #[serde(rename = \"{m}\")]\n"));
        out.push_str(&format!("    {variant},\n"));
    }
    out.push_str("}\n");
    out
}

/// Render the `use` block for a file at module depth `depth`.
#[allow(dead_code)] // consumed by render_object in Task 21
pub(crate) fn render_use_block(imports: impl IntoIterator<Item = Import>, depth: usize) -> String {
    let mut b = UseBlock::new();
    b.extend(imports);
    b.render(depth)
}

use bo4e_schemas::models::json_schema::{ObjectSchema, SchemaType};
use minijinja::{Environment, context};
use serde::Serialize;
use std::collections::BTreeSet;

use crate::Error;
use crate::refs::{enum_ref_target, schema_base};
use crate::rust::types::{UnsupportedShape, literal_default_rust, map_rust};

/// Per-field context for the Struct.jinja2 template.
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub(crate) struct RustField {
    pub name: String,
    pub type_hint: String,
    pub serde_attrs: String,
    pub doc: String,
    /// Default expression for the field, used by `render_default_impl`.
    #[serde(skip)]
    pub default_expr: Option<String>,
}

/// Outcome of rendering an object schema: the file body, ready to write.
#[allow(dead_code)]
pub(crate) struct RenderedObject {
    pub body: String,
    /// Brief per-file decision summary suitable for verbose CLI output.
    pub diagnostic: String,
}

#[derive(Debug)]
pub(crate) enum DefaultImplOutcome {
    Emitted,
    Skipped { missing: Vec<String> },
}

fn default_impl_outcome(fields: &[RustField]) -> DefaultImplOutcome {
    let missing: Vec<String> = fields
        .iter()
        .filter(|f| f.default_expr.is_none())
        .map(|f| f.name.clone())
        .collect();
    if missing.is_empty() {
        DefaultImplOutcome::Emitted
    } else {
        DefaultImplOutcome::Skipped { missing }
    }
}

/// Render a single object schema as a `.rs` file body.
///
/// `parent_module` is the schema's module path *without* the class name appended.
/// `depth` is the file's directory depth from the output root (1 = root-level, 2 = under `bo/`, …).
#[allow(dead_code)]
pub(crate) fn render_object(
    env: &Environment<'static>,
    class_name: &str,
    parent_module: &[String],
    obj: &ObjectSchema,
    depth: usize,
) -> Result<RenderedObject, Error> {
    let mut imports: BTreeSet<Import> = BTreeSet::new();

    let required: BTreeSet<&str> = obj.required.iter().map(|s| s.as_str()).collect();
    let mut fields: Vec<RustField> = Vec::new();
    let mut extra_enums: Vec<String> = Vec::new();
    let mut needs_default_version_fn = false;

    for (prop_name, prop_schema) in &obj.properties {
        let (rust_name, needs_rename) = crate::rust::rust_field_name(prop_name);
        let is_required = required.contains(prop_name.as_str());

        // Detect `_typ`-style discriminator: ConstantSchema or anyOf:[const,null] or
        // anyOf:[$ref to enum/Typ, null] with TypeBase.default
        let (type_hint, default_expr) =
            if let Some(disc) = single_variant_discriminator(prop_name, prop_schema) {
                let enum_name = format!(
                    "{class_name}{}",
                    to_pascal_case(&strip_leading_underscore(prop_name))
                );
                extra_enums.push(render_single_variant_enum(
                    &enum_name,
                    &disc.wire_value,
                    schema_base(prop_schema).description.as_deref(),
                ));
                // When the schema permits null we expose `Option<EnumName>` so
                // schema-valid `null` payloads deserialize as `None` instead of
                // erroring. The default still carries the single variant so
                // that `Struct::default()` produces a fully-populated,
                // idiomatic BO4E object.
                if disc.nullable {
                    (
                        format!("Option<{enum_name}>"),
                        Some(format!("Some({enum_name}::default())")),
                    )
                } else {
                    (enum_name.clone(), Some(format!("{enum_name}::default()")))
                }
            } else {
                let mapped = map_rust(prop_schema).map_err(|UnsupportedShape(shape)| {
                    Error::UnsupportedSchemaShape {
                        schema_name: class_name.to_string(),
                        property: prop_name.clone(),
                        shape,
                    }
                })?;
                imports.extend(mapped.imports.iter().cloned());

                let json_default = literal_default_rust(prop_schema);

                // `_version` is always non-optional per design — it must hold a string
                // value (typically the live BO4E version constant). This matters when
                // bo4e edit marks `_version` as required, in which case `Option<String>`
                // would defeat the contract.
                let is_version_field = prop_name == "_version";

                // A field is `Option<T>` when either the JSON value can be null
                // (`map_rust` returns `Option<T>` for `anyOf:[T, null]`) OR the JSON key
                // can be absent (i.e. the field is not in `required`). The only override
                // is `_version`, which is kept as a plain `String` with a generated
                // `default_version()` helper.
                let schema_is_nullable = mapped.rendered.starts_with("Option<");
                let render_as_option = !is_version_field && (schema_is_nullable || !is_required);

                // Strip any outer `Option<>` from the mapped type so we can re-wrap
                // consistently below — `map_rust` only adds it for `anyOf:[T, null]`,
                // never recursively, so this only touches the outermost layer.
                let inner = mapped
                    .rendered
                    .strip_prefix("Option<")
                    .and_then(|s| s.strip_suffix('>'))
                    .unwrap_or(&mapped.rendered)
                    .to_string();
                let type_hint = if render_as_option {
                    format!("Option<{inner}>")
                } else {
                    inner
                };

                let default_expr = if is_version_field {
                    needs_default_version_fn = true;
                    Some("default_version()".to_string())
                } else if render_as_option {
                    Some("None".to_string())
                } else {
                    json_default
                };

                (type_hint, default_expr)
            };

        let serde_attrs = build_serde_attrs(
            prop_name,
            needs_rename,
            &type_hint,
            default_expr.as_deref(),
            is_required,
        );
        let doc = render_doc_comment(schema_base(prop_schema).description.as_deref(), "");

        fields.push(RustField {
            name: rust_name,
            type_hint,
            serde_attrs,
            doc,
            default_expr,
        });
    }

    let mut uses = render_use_block(imports.iter().cloned(), depth);
    if needs_default_version_fn {
        let supers = "super::".repeat(depth);
        let extra_use = format!("use {supers}default_version;");
        uses = if uses.is_empty() {
            extra_use
        } else {
            format!("{uses}\n{extra_use}")
        };
    }
    let doc = render_doc_comment(obj.base.description.as_deref(), "");

    let outcome = default_impl_outcome(&fields);
    let default_impl = render_default_impl(class_name, &fields);
    let default_version_fn = String::new(); // No longer emitted per-file; lives in root mod.rs / lib.rs.

    let n_fields = fields.len();
    let n_synth = extra_enums.len();
    let diagnostic = match &outcome {
        DefaultImplOutcome::Emitted => format!(
            "struct {class_name} ({n_fields} fields, Default impl emitted{synth})",
            synth = if n_synth > 0 {
                format!(", {n_synth} synthetic enums")
            } else {
                String::new()
            }
        ),
        DefaultImplOutcome::Skipped { missing } => format!(
            "struct {class_name} ({n_fields} fields, Default impl SKIPPED: field `{}` has no default expression{synth})",
            missing.join("`, `"),
            synth = if n_synth > 0 {
                format!(", {n_synth} synthetic enums")
            } else {
                String::new()
            }
        ),
    };

    let tpl = env.get_template("rust/plain/Struct.jinja2")?;
    let body = tpl.render(context! {
        uses => uses,
        extra_enums => extra_enums,
        doc => doc,
        class_name => class_name,
        fields => &fields,
        default_impl => default_impl,
        default_version_fn => default_version_fn,
    })?;

    let _ = parent_module; // silence unused warning until cross-module diagnostics use it
    Ok(RenderedObject { body, diagnostic })
}

fn strip_leading_underscore(s: &str) -> String {
    s.strip_prefix('_').unwrap_or(s).to_string()
}

fn build_serde_attrs(
    prop_name: &str,
    needs_rename: bool,
    type_hint: &str,
    default_expr: Option<&str>,
    is_required: bool,
) -> String {
    let mut parts = Vec::<String>::new();
    if needs_rename {
        parts.push(format!("rename = \"{prop_name}\""));
    }
    let is_option = type_hint.starts_with("Option<");
    if is_option {
        if is_required {
            // Required + nullable: the JSON key must be present (no `default`), but the
            // value may be null. We still skip emitting `null` on serialization — most
            // BO4E consumers prefer absent over `null` for clean round-trips, and a
            // future consumer who wants `null` explicitly can override the serde attr.
            parts.push("skip_serializing_if = \"Option::is_none\"".to_string());
        } else {
            parts.push("default".to_string());
            parts.push("skip_serializing_if = \"Option::is_none\"".to_string());
        }
    } else if let Some(d) = default_expr {
        if prop_name == "_version" {
            // `_version` is the only non-Option field that gets a serde-callable
            // default *unconditionally* (even when the schema marks it as
            // required): the root `default_version()` helper is generated in
            // `RootModRs.jinja2` and pulled in via `use super::…default_version;`
            // (see the `needs_default_version_fn` plumbing above). The contract is
            // "version is always populated by the live BO4E version constant",
            // schema `required` flag notwithstanding.
            parts.push("default = \"default_version\"".to_string());
        } else if !is_required && (d == "Default::default()" || d.ends_with("::default()")) {
            // Non-required + has a `::default()`-shaped expression (e.g. a
            // synthetic discriminator enum like `AngebotTyp::default()`):
            // tell serde to use the field type's Default on a missing key.
            // For *required* fields we intentionally omit this so a missing
            // key fails to deserialize — matches the required + `Option<T>`
            // branch above, and matches pydantic's strict-required semantics.
            // The literal default still lives in `impl Default for Struct`
            // (for `Struct::default()` callers).
            parts.push("default".to_string());
        }
    }
    parts.join(", ")
}

fn render_default_impl(class_name: &str, fields: &[RustField]) -> String {
    if fields.iter().any(|f| f.default_expr.is_none()) {
        let missing: Vec<&str> = fields
            .iter()
            .filter(|f| f.default_expr.is_none())
            .map(|f| f.name.as_str())
            .collect();
        return format!(
            "// Default impl omitted: field `{}` has no default expression.",
            missing.join("`, `")
        );
    }
    let mut out = String::new();
    out.push_str(&format!("impl Default for {class_name} {{\n"));
    out.push_str("    fn default() -> Self {\n");
    out.push_str("        Self {\n");
    for f in fields {
        let expr = f.default_expr.as_deref().unwrap();
        out.push_str(&format!("            {}: {},\n", f.name, expr));
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");
    out
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
fn single_variant_discriminator(
    _prop_name: &str,
    prop_schema: &SchemaType,
) -> Option<DiscriminatorMatch> {
    use bo4e_schemas::models::json_schema::PrimitiveValue;

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

    // anyOf: [const, null] OR anyOf: [$ref to enum, null] with TypeBase.default
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
        if non_null.len() == 1 {
            if let Some(v) = const_value(non_null[0]) {
                return Some(DiscriminatorMatch {
                    wire_value: v.to_string(),
                    nullable,
                });
            }
            if let Some(PrimitiveValue::String(default_val)) = &schema_base(prop_schema).default
                && enum_ref_target(non_null[0]).is_some()
            {
                return Some(DiscriminatorMatch {
                    wire_value: default_val.clone(),
                    nullable,
                });
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_comment_single_line() {
        let s = render_doc_comment(Some("Hello world"), "");
        assert_eq!(s, "/// Hello world");
    }

    #[test]
    fn doc_comment_multi_line_preserves_breaks() {
        let s = render_doc_comment(Some("Line one\nLine two"), "");
        assert_eq!(s, "/// Line one\n/// Line two");
    }

    #[test]
    fn doc_comment_empty_returns_empty() {
        assert_eq!(render_doc_comment(None, ""), "");
        assert_eq!(render_doc_comment(Some(""), ""), "");
        assert_eq!(render_doc_comment(Some("   "), ""), "");
    }

    #[test]
    fn doc_comment_indent_applied() {
        let s = render_doc_comment(Some("Hi"), "    ");
        assert_eq!(s, "    /// Hi");
    }

    #[test]
    fn single_variant_enum_shape() {
        let s = render_single_variant_enum("AngebotTyp", "ANGEBOT", Some("Angebot discriminator"));
        assert!(s.contains("/// Angebot discriminator"));
        assert!(s.contains("pub enum AngebotTyp"));
        assert!(s.contains("#[default]"));
        assert!(s.contains("#[serde(rename = \"ANGEBOT\")]"));
        assert!(s.contains("Angebot,"));
    }

    #[test]
    fn str_enum_one_variant_per_member() {
        let members = vec!["ANGEBOT".to_string(), "AUSSCHREIBUNG".to_string()];
        let s = render_str_enum("Typ", &members, None);
        assert!(s.contains("pub enum Typ"));
        assert!(s.contains("#[serde(rename = \"ANGEBOT\")]"));
        assert!(s.contains("Angebot,"));
        assert!(s.contains("#[serde(rename = \"AUSSCHREIBUNG\")]"));
        assert!(s.contains("Ausschreibung,"));
    }

    #[test]
    fn str_enum_handles_hyphenated_member() {
        let members = vec!["2-01-7-001".to_string()];
        let s = render_str_enum("Code", &members, None);
        assert!(s.contains("#[serde(rename = \"2-01-7-001\")]"));
        assert!(s.contains("_2_01_7_001,"));
    }

    #[test]
    fn use_block_round_trip() {
        let imports = vec![
            Import::Named {
                module: "serde".into(),
                name: "Serialize".into(),
            },
            Import::Sibling {
                module: vec!["com".into(), "Adresse".into()],
                name: "Adresse".into(),
            },
        ];
        let s = render_use_block(imports, 2);
        assert!(s.contains("use serde::Serialize;"));
        assert!(s.contains("use super::super::com::adresse::Adresse;"));
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
        let r = render_object(&env, "Angebot", &["bo".to_string()], &schema, 2).unwrap();
        assert!(r.body.contains("pub struct Angebot"), "got:\n{}", r.body);
        assert!(
            r.body.contains("pub id: Option<String>"),
            "got:\n{}",
            r.body
        );
        assert!(r.body.contains("rename = \"_id\""), "got:\n{}", r.body);
        assert!(r.body.contains("pub angebotsnummer: Option<String>"));
    }

    /// `Lastgang.zeitIntervallLaenge`-style regression: the schema is
    /// `anyOf:[$ref to Menge, null]` AND the field appears in `required`.
    /// "Required" means the JSON key must be present, NOT that null is forbidden,
    /// so the Rust type must stay `Option<Menge>`. The serde attrs drop `default`
    /// (forcing presence on deserialization) but keep `skip_serializing_if` so
    /// `None` round-trips as absent for ergonomic output.
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
        let r = render_object(&env, "Lastgang", &["bo".to_string()], &schema, 2).unwrap();
        assert!(
            r.body.contains("pub zeit_intervall_laenge: Option<Menge>"),
            "required+nullable field must stay Option<T>, got:\n{}",
            r.body
        );
        assert!(
            !r.body.contains("pub zeit_intervall_laenge: Menge"),
            "field must not have been stripped to bare `Menge`, got:\n{}",
            r.body
        );
        // Required: no `default` attr (must be present in JSON).
        assert!(
            !r.body.contains("default,"),
            "required+nullable field must not have `default` in its serde attrs, got:\n{}",
            r.body
        );
        // But `skip_serializing_if` is fine for clean output.
        assert!(
            r.body.contains("skip_serializing_if = \"Option::is_none\""),
            "expected skip_serializing_if, got:\n{}",
            r.body
        );
    }

    /// Regression: a property that is *not* in `required` and is *not* nullable
    /// (e.g. `type: "array"` without a `null` branch — `adressen` in BO4E's sql
    /// fixture) must still be wrapped in `Option<T>` so deserialization survives
    /// the JSON key being absent. Previously the renderer kept the raw type and
    /// emitted no `default` attr, breaking serde on missing keys.
    #[test]
    #[cfg(feature = "rust-plain")]
    fn render_object_optional_non_nullable_array_is_option() {
        use bo4e_schemas::models::json_schema::{ArraySchema, LiteralTypeArray, ReferenceSchema};

        let array_of_adresse = SchemaType::Array(ArraySchema {
            base: TypeBase::default(),
            r#type: LiteralTypeArray::Array,
            items: Box::new(SchemaType::ReferenceSchema(ReferenceSchema {
                base: TypeBase::default(),
                r#ref: "../com/Adresse.json".to_string(),
            })),
        });

        let env = make_env();
        let schema = obj_with_props(
            vec![("adressen", array_of_adresse)],
            vec![], // not required
            Some("An offer."),
        );
        let r = render_object(&env, "Angebot", &["bo".to_string()], &schema, 2).unwrap();
        assert!(
            r.body.contains("pub adressen: Option<Vec<Adresse>>"),
            "optional non-nullable array must be Option<Vec<_>>, got:\n{}",
            r.body
        );
        assert!(
            r.body.contains("default")
                && r.body.contains("skip_serializing_if = \"Option::is_none\""),
            "expected `default, skip_serializing_if = \"Option::is_none\"` on optional field, got:\n{}",
            r.body
        );
    }

    /// Regression: a `_typ`-style discriminator whose schema permits null
    /// (`anyOf:[$ref to enum, null]`) must be wrapped in `Option<EnumName>` so
    /// schema-valid `null` payloads deserialize as `None` instead of erroring.
    /// The default still seeds `Some(default)` so `Struct::default()` produces
    /// a fully-populated, idiomatic BO4E object.
    #[test]
    #[cfg(feature = "rust-plain")]
    fn render_object_nullable_discriminator_is_option() {
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
        let r = render_object(&env, "Angebot", &["bo".to_string()], &schema, 2).unwrap();
        assert!(
            r.body.contains("pub enum AngebotTyp"),
            "expected synthetic discriminator enum, got:\n{}",
            r.body
        );
        assert!(
            r.body.contains("pub typ: Option<AngebotTyp>"),
            "nullable discriminator must be Option<EnumName>, got:\n{}",
            r.body
        );
        assert!(
            r.body.contains("typ: Some(AngebotTyp::default())"),
            "Struct::default() must seed Some(EnumName::default()), got:\n{}",
            r.body
        );
    }

    /// A discriminator whose schema is a *direct* `const`/single-`StrEnum` (no
    /// null branch) stays as a bare `EnumName` — only the nullable shape gets
    /// the `Option<…>` wrap.
    #[test]
    #[cfg(feature = "rust-plain")]
    fn render_object_non_nullable_discriminator_is_bare() {
        use bo4e_schemas::models::json_schema::ConstantSchema;

        let const_typ = SchemaType::ConstantSchema(ConstantSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            format: None,
            constant: "ANGEBOT".to_string(),
        });

        let env = make_env();
        let schema = obj_with_props(vec![("_typ", const_typ)], vec!["_typ"], None);
        let r = render_object(&env, "Angebot", &["bo".to_string()], &schema, 2).unwrap();
        assert!(
            r.body.contains("pub typ: AngebotTyp"),
            "non-nullable discriminator must stay bare, got:\n{}",
            r.body
        );
        assert!(
            !r.body.contains("pub typ: Option<AngebotTyp>"),
            "non-nullable discriminator must NOT be wrapped in Option, got:\n{}",
            r.body
        );
        // Regression: required + non-Option fields must NOT carry a serde
        // `default` attr — that would let a missing JSON key silently
        // deserialize via `AngebotTyp::default()`, contradicting the
        // schema's `required` contract. (`#[serde(rename = "_typ")]` is
        // allowed; only the standalone `default` token is forbidden here.)
        let body_lines: Vec<&str> = r.body.lines().collect();
        let typ_attr_line = body_lines
            .iter()
            .find(|l| l.contains("#[serde(") && l.contains("rename = \"_typ\""))
            .copied()
            .unwrap_or("");
        assert!(
            !typ_attr_line.contains("default"),
            "required + non-Option discriminator must not emit a serde `default` attr, got line:\n{typ_attr_line}",
        );
    }

    /// Optional + non-Option fields with a `::default()`-shaped default
    /// expression (e.g. a synthetic single-variant enum on an optional
    /// `_typ`) still get the bare `default` serde attr so missing JSON
    /// keys fall back to `EnumName::default()`. This is the inverse of
    /// the required case above.
    #[test]
    #[cfg(feature = "rust-plain")]
    fn render_object_optional_non_nullable_discriminator_keeps_serde_default() {
        use bo4e_schemas::models::json_schema::ConstantSchema;

        let const_typ = SchemaType::ConstantSchema(ConstantSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            format: None,
            constant: "ANGEBOT".to_string(),
        });

        let env = make_env();
        // `_typ` is NOT in required.
        let schema = obj_with_props(vec![("_typ", const_typ)], vec![], None);
        let r = render_object(&env, "Angebot", &["bo".to_string()], &schema, 2).unwrap();
        let body_lines: Vec<&str> = r.body.lines().collect();
        let typ_attr_line = body_lines
            .iter()
            .find(|l| l.contains("#[serde(") && l.contains("rename = \"_typ\""))
            .copied()
            .unwrap_or("");
        assert!(
            typ_attr_line.contains("default"),
            "optional + non-Option discriminator should emit `default` so missing JSON keys fall back, got line:\n{typ_attr_line}",
        );
    }
}
