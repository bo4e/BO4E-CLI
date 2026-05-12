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

/// Render a module-level `//!` docstring. Empty input → empty string.
#[allow(dead_code)] // consumed by render_object in Task 21
pub(crate) fn render_module_doc(description: Option<&str>) -> String {
    let Some(text) = description.map(str::trim).filter(|s| !s.is_empty()) else {
        return String::new();
    };
    text.lines()
        .map(|line| format!("//! {line}").trim_end().to_string())
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
            if let Some(wire) = single_variant_discriminator(prop_name, prop_schema) {
                let enum_name = format!(
                    "{class_name}{}",
                    to_pascal_case(&strip_leading_underscore(prop_name))
                );
                extra_enums.push(render_single_variant_enum(
                    &enum_name,
                    &wire,
                    schema_base(prop_schema).description.as_deref(),
                ));
                (enum_name.clone(), Some(format!("{enum_name}::default()")))
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
                let has_non_null_default = json_default
                    .as_deref()
                    .is_some_and(|d| d != "None" && !d.is_empty());

                // `_version` is always non-optional per design — it must hold a string
                // value (typically the live BO4E version constant). This matters when
                // bo4e edit marks `_version` as required, in which case `Option<String>`
                // would defeat the contract.
                let is_version_field = prop_name == "_version";

                // `map_rust` already returns `Option<T>` when the schema is `anyOf:[T, null]`.
                // We unwrap when the field should be non-optional (required, or has a
                // non-null default, or is `_version`).
                let already_optional = mapped.rendered.starts_with("Option<");
                let should_be_optional = !is_version_field && !is_required && !has_non_null_default;
                let inner = mapped
                    .rendered
                    .strip_prefix("Option<")
                    .and_then(|s| s.strip_suffix('>'))
                    .unwrap_or(&mapped.rendered)
                    .to_string();
                let type_hint = if should_be_optional {
                    if already_optional {
                        mapped.rendered.clone()
                    } else {
                        format!("Option<{}>", mapped.rendered)
                    }
                } else {
                    inner
                };

                let default_expr = if is_version_field {
                    needs_default_version_fn = true;
                    Some("default_version()".to_string())
                } else if should_be_optional {
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
    let module_doc = render_module_doc(obj.base.description.as_deref());
    let doc = render_doc_comment(obj.base.description.as_deref(), "");

    let default_impl = render_default_impl(class_name, &fields);
    let default_version_fn = String::new(); // No longer emitted per-file; lives in root mod.rs / lib.rs.

    let tpl = env.get_template("rust/plain/Struct.jinja2")?;
    let body = tpl.render(context! {
        module_doc => module_doc,
        uses => uses,
        extra_enums => extra_enums,
        doc => doc,
        class_name => class_name,
        fields => &fields,
        default_impl => default_impl,
        default_version_fn => default_version_fn,
    })?;

    let _ = parent_module; // silence unused warning until cross-module diagnostics use it
    Ok(RenderedObject { body })
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
        parts.push("default".to_string());
        parts.push("skip_serializing_if = \"Option::is_none\"".to_string());
    } else if let Some(d) = default_expr {
        if d == "Default::default()" || d.ends_with("::default()") {
            parts.push("default".to_string());
        } else if !is_required || prop_name == "_version" {
            // `_version` always emits its default fn — the renderer guarantees one is
            // generated, and we want it to fire whether or not the schema marks the
            // field as required.
            let stripped = prop_name.strip_prefix('_').unwrap_or(prop_name);
            let fn_name = format!("default_{}", crate::naming::to_snake_case(stripped));
            parts.push(format!("default = \"{fn_name}\""));
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

/// Detect a `_typ`-style discriminator. Returns `Some(wire_value)` when the
/// property's schema fixes one constant string value.
fn single_variant_discriminator(_prop_name: &str, prop_schema: &SchemaType) -> Option<String> {
    use bo4e_schemas::models::json_schema::PrimitiveValue;

    fn const_value(s: &SchemaType) -> Option<&str> {
        match s {
            SchemaType::ConstantSchema(c) => Some(c.constant.as_str()),
            SchemaType::StrEnum(e) if e.enum_values.len() == 1 => Some(e.enum_values[0].as_str()),
            _ => None,
        }
    }

    // direct
    if let Some(v) = const_value(prop_schema) {
        return Some(v.to_string());
    }

    // anyOf: [const, null] OR anyOf: [$ref to enum, null] with TypeBase.default
    if let SchemaType::AnyOf(a) = prop_schema {
        let non_null: Vec<&SchemaType> = a
            .any_of
            .iter()
            .filter(|t| !matches!(t, SchemaType::NullSchema(_)))
            .collect();
        if non_null.len() == 1 {
            if let Some(v) = const_value(non_null[0]) {
                return Some(v.to_string());
            }
            if let Some(PrimitiveValue::String(default_val)) = &schema_base(prop_schema).default
                && enum_ref_target(non_null[0]).is_some()
            {
                return Some(default_val.clone());
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
    fn module_doc_renders_bangs() {
        let s = render_module_doc(Some("First\nSecond"));
        assert_eq!(s, "//! First\n//! Second");
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
}
