//! JSON-Schema `$ref` parsing and small accessors shared by every code generator.

use crate::imports::Import;
use bo4e_schemas::models::json_schema::{SchemaType, TypeBase};

/// Parse a `$ref` string into `(module_segments, class_name)`.
///
/// Accepts both relative paths (`"../bo/Geschaeftspartner.json"`) and absolute URLs
/// (the form that appears before the normalisation pass). The last path component
/// (stripped of `.json`) becomes the class name; preceding path components (stripped of
/// leading `../` traversals) form the module. The class name is also appended as the
/// final segment of the module so the renderer can form
/// `from ..<sub>.<file> import <Class>` (Python) or
/// `use super::super::<sub>::<file>::<Class>;` (Rust).
pub fn parse_ref(ref_str: &str) -> (Vec<String>, String) {
    let path_part = if let Some(idx) = ref_str.find("bo4e_schemas/") {
        &ref_str[idx + "bo4e_schemas/".len()..]
    } else {
        let mut s = ref_str;
        while let Some(rest) = s.strip_prefix("../") {
            s = rest;
        }
        s
    };

    let path_part = path_part.split('#').next().unwrap_or(path_part);

    let mut segments: Vec<String> = path_part
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let file_name = segments.pop().unwrap_or_default();
    let class_name = file_name
        .strip_suffix(".json")
        .unwrap_or(&file_name)
        .to_string();

    segments.push(class_name.clone());

    (segments, class_name)
}

/// Extract the [`TypeBase`] (default/title/description) common to every schema variant.
pub fn schema_base(schema: &SchemaType) -> &TypeBase {
    match schema {
        SchemaType::StringSchema(s) => &s.base,
        SchemaType::IntegerSchema(s) => &s.base,
        SchemaType::NumberSchema(s) => &s.base,
        SchemaType::BooleanSchema(s) => &s.base,
        SchemaType::DecimalSchema(s) => &s.base,
        SchemaType::NullSchema(s) => &s.base,
        SchemaType::AnySchema(s) => &s.base,
        SchemaType::Array(s) => &s.base,
        SchemaType::AnyOf(s) => &s.base,
        SchemaType::AllOf(s) => &s.base,
        SchemaType::ConstantSchema(s) => &s.base,
        SchemaType::ReferenceSchema(s) => &s.base,
        SchemaType::Object(s) => &s.base,
        SchemaType::StrEnum(s) => &s.base,
    }
}

/// If `schema` is (or wraps in `anyOf:[…, null]`) a `$ref` to an `enum/…` schema,
/// return `(EnumClassName, sibling_module_path)`.
pub fn enum_ref_target(schema: &SchemaType) -> Option<(String, Vec<String>)> {
    let r = match schema {
        SchemaType::ReferenceSchema(r) if !r.r#ref.is_empty() => r,
        SchemaType::AnyOf(a) => {
            let non_null: Vec<&SchemaType> = a
                .any_of
                .iter()
                .filter(|t| !matches!(t, SchemaType::NullSchema(_)))
                .collect();
            if non_null.len() == 1 {
                if let SchemaType::ReferenceSchema(r) = non_null[0] {
                    if r.r#ref.is_empty() {
                        return None;
                    }
                    r
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        _ => return None,
    };
    let (module, class_name) = parse_ref(&r.r#ref);
    if module.first().map(|s| s.as_str()) == Some("enum") {
        Some((class_name, module))
    } else {
        None
    }
}

/// Helper that callers use to inject the matching sibling `Import` for an `enum_ref_target`.
/// Lives here because both language renderers need the same data shape.
pub fn enum_import(name: String, module: Vec<String>) -> Import {
    Import::Sibling { module, name }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ref_relative() {
        let (module, name) = parse_ref("../bo/Geschaeftspartner.json");
        assert_eq!(module, vec!["bo", "Geschaeftspartner"]);
        assert_eq!(name, "Geschaeftspartner");
    }

    #[test]
    fn parse_ref_relative_enum() {
        let (module, name) = parse_ref("../enum/Typ.json");
        assert_eq!(module, vec!["enum", "Typ"]);
        assert_eq!(name, "Typ");
    }

    #[test]
    fn parse_ref_absolute_url() {
        let (module, name) = parse_ref(
            "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202501.1.0-rc1/src/bo4e_schemas/bo/Geschaeftspartner.json",
        );
        assert_eq!(module, vec!["bo", "Geschaeftspartner"]);
        assert_eq!(name, "Geschaeftspartner");
    }
}
