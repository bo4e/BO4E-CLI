//! JSON Schema → Python type-string mapping.
//!
//! Each function returns a [`MappedType`] where [`MappedType::rendered`] is the type as it
//! should appear inline in generated code, and [`MappedType::imports`] is the set of imports it
//! depends on.  The caller (the per-output-type generator) merges these imports into the file's
//! import block.
//!
//! The pydantic dialect emits:
//! - PEP 604 union syntax (`T | None`, `A | B`) — not `Optional[T]` / `Union[A, B]`.
//! - PEP 585 generics (`list[T]`, `dict[K, V]`) — not `List[T]` / `Dict[K, V]`.

// Public items are consumed by Tasks 6+ (import collector, template renderer).
// Until those crates reference this module, the compiler sees them as dead code.
#![allow(dead_code)]

use bo4e_schemas::models::json_schema::{PrimitiveValue, SchemaType, StringSchemaFormat};
use std::collections::BTreeSet;

pub use crate::imports::Import;

// ── Public types ──────────────────────────────────────────────────────────────

/// The result of mapping a JSON Schema fragment to a Python type expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedType {
    /// The type expression as it appears in generated source code, e.g. `"list[Adresse]"`.
    pub rendered: String,
    /// Imports that `rendered` depends on.
    pub imports: BTreeSet<Import>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse a `$ref` string into `(module_segments, class_name)`.
///
/// Accepts both relative paths (`"../bo/Geschaeftspartner.json"`) and absolute URLs
/// (the form that appears before the normalisation pass).  The last path component
/// (stripped of `.json`) becomes the class name; preceding path components (stripped of
/// leading `../` traversals) form the module.
///
/// # Examples
///
/// ```
/// // "../bo/Geschaeftspartner.json" → (["bo"], "Geschaeftspartner")
/// // "https://.../bo4e_schemas/bo/Geschaeftspartner.json" → (["bo"], "Geschaeftspartner")
/// // "../enum/Typ.json" → (["enum"], "Typ")
/// ```
pub(crate) fn parse_ref(ref_str: &str) -> (Vec<String>, String) {
    // Strip URL scheme + host + path-prefix if this is a full URL.
    let path_part = if let Some(idx) = ref_str.find("bo4e_schemas/") {
        &ref_str[idx + "bo4e_schemas/".len()..]
    } else {
        // Relative path: strip leading `../` sequences.
        let mut s = ref_str;
        while let Some(rest) = s.strip_prefix("../") {
            s = rest;
        }
        s
    };

    // Strip a trailing `#` fragment if present.
    let path_part = path_part.split('#').next().unwrap_or(path_part);

    let mut segments: Vec<String> = path_part
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    // The last segment is the file name; strip `.json`.
    let file_name = segments.pop().unwrap_or_default();
    let class_name = file_name
        .strip_suffix(".json")
        .unwrap_or(&file_name)
        .to_string();

    // Combine remaining path segments with the class name as the final module segment.
    // Convention: module = [...path_segs, class_name] so the renderer can form
    // `from ..<sub>.<file> import <Class>`.
    segments.push(class_name.clone());

    (segments, class_name)
}

fn simple(rendered: impl Into<String>) -> MappedType {
    MappedType {
        rendered: rendered.into(),
        imports: BTreeSet::new(),
    }
}

fn with_import(rendered: impl Into<String>, module: &str, name: &str) -> MappedType {
    let mut imports = BTreeSet::new();
    imports.insert(Import::Named {
        module: module.to_string(),
        name: name.to_string(),
    });
    MappedType {
        rendered: rendered.into(),
        imports,
    }
}

// ── Public helper functions ───────────────────────────────────────────────────

/// Extract the [`TypeBase`] (default/title/description) common to every schema variant.
pub(crate) fn schema_base(schema: &SchemaType) -> &bo4e_schemas::models::json_schema::TypeBase {
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
/// return `(EnumClassName, sibling_module_path)`. Used by the pydantic renderer to
/// qualify a string default as an enum member instead of emitting a bare string
/// literal — e.g. `default="DE"` for an enum-typed `landescode` field becomes
/// `default=Landescode.DE`.
pub(crate) fn enum_ref_target(schema: &SchemaType) -> Option<(String, Vec<String>)> {
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

/// Render a JSON Schema `default` (when present, primitive) as a Python literal expression.
pub(crate) fn literal_default(schema: &SchemaType) -> Option<String> {
    schema_base(schema).default.as_ref().map(|v| match v {
        PrimitiveValue::Null => "None".into(),
        PrimitiveValue::Bool(true) => "True".into(),
        PrimitiveValue::Bool(false) => "False".into(),
        PrimitiveValue::Integer(i) => i.to_string(),
        PrimitiveValue::Float(f) => f.to_string(),
        PrimitiveValue::String(s) => format!("\"{s}\""),
    })
}

// ── Public mapping function ───────────────────────────────────────────────────

/// Map a JSON Schema fragment to its pydantic Python type expression.
///
/// The returned [`MappedType::rendered`] is the type string that should appear inline in
/// generated code.  [`MappedType::imports`] contains the import statements that `rendered`
/// depends on.
pub fn map_pydantic(schema_type: &SchemaType) -> MappedType {
    match schema_type {
        // ── Scalar primitives ────────────────────────────────────────────────
        SchemaType::StringSchema(s) => match &s.format {
            None => simple("str"),
            Some(StringSchemaFormat::DateTime) => with_import("datetime", "datetime", "datetime"),
            Some(StringSchemaFormat::Date) => with_import("date", "datetime", "date"),
            Some(StringSchemaFormat::Time) => with_import("time", "datetime", "time"),
            Some(StringSchemaFormat::Uuid) => with_import("UUID", "uuid", "UUID"),
            // All other formats fall back to plain str.
            Some(_) => simple("str"),
        },
        SchemaType::IntegerSchema(_) => simple("int"),
        SchemaType::NumberSchema(_) => simple("float"),
        SchemaType::BooleanSchema(_) => simple("bool"),

        // ── Decimal (BO4E extension: type=number|string + format=decimal) ────
        SchemaType::DecimalSchema(_) => with_import("Decimal", "decimal", "Decimal"),

        // ── Null ─────────────────────────────────────────────────────────────
        SchemaType::NullSchema(_) => simple("None"),

        // ── Any ──────────────────────────────────────────────────────────────
        SchemaType::AnySchema(_) => with_import("Any", "typing", "Any"),

        // ── Array ────────────────────────────────────────────────────────────
        SchemaType::Array(a) => {
            let inner = map_pydantic(&a.items);
            let rendered = format!("list[{}]", inner.rendered);
            MappedType {
                rendered,
                imports: inner.imports,
            }
        }

        // ── AnyOf — optional or real union ───────────────────────────────────
        SchemaType::AnyOf(a) => {
            // Partition branches into null and non-null.
            let (null_branches, non_null_branches): (Vec<_>, Vec<_>) = a
                .any_of
                .iter()
                .partition(|t| matches!(t, SchemaType::NullSchema(_)));

            let is_optional = !null_branches.is_empty();

            // Map each non-null branch.
            let mapped: Vec<MappedType> =
                non_null_branches.iter().map(|t| map_pydantic(t)).collect();

            let mut all_imports: BTreeSet<Import> = BTreeSet::new();
            for m in &mapped {
                all_imports.extend(m.imports.iter().cloned());
            }

            // `Any` subsumes every other type (including None), so a union containing
            // `Any` collapses to just `Any`.
            if mapped.iter().any(|m| m.rendered == "Any") {
                return MappedType {
                    rendered: "Any".into(),
                    imports: all_imports,
                };
            }

            let inner_rendered: Vec<&str> = mapped.iter().map(|m| m.rendered.as_str()).collect();
            let type_str = inner_rendered.join(" | ");

            let rendered = if is_optional {
                if type_str.is_empty() {
                    "None".to_string()
                } else {
                    format!("{} | None", type_str)
                }
            } else {
                type_str
            };

            MappedType {
                rendered,
                imports: all_imports,
            }
        }

        // ── AllOf — treated as a single-item wrapper (pydantic inheritance) ──
        SchemaType::AllOf(a) => {
            if a.all_of.len() == 1 {
                map_pydantic(&a.all_of[0])
            } else {
                // Multi-branch allOf is rare in BO4E; emit an intersection approximation.
                let mapped: Vec<MappedType> = a.all_of.iter().map(map_pydantic).collect();
                let mut all_imports: BTreeSet<Import> = BTreeSet::new();
                for m in &mapped {
                    all_imports.extend(m.imports.iter().cloned());
                }
                let rendered = mapped
                    .iter()
                    .map(|m| m.rendered.as_str())
                    .collect::<Vec<_>>()
                    .join(" & ");
                MappedType {
                    rendered,
                    imports: all_imports,
                }
            }
        }

        // ── $ref ─────────────────────────────────────────────────────────────
        SchemaType::ReferenceSchema(r) => {
            // Empty $ref (from deserializing bare `{}`) should map to Any, not an empty string.
            if r.r#ref.is_empty() {
                with_import("Any", "typing", "Any")
            } else {
                let (module, class_name) = parse_ref(&r.r#ref);
                let mut imports = BTreeSet::new();
                imports.insert(Import::Sibling {
                    module,
                    name: class_name.clone(),
                });
                MappedType {
                    rendered: class_name,
                    imports,
                }
            }
        }

        // ── Inline enum (string enum as a schema-type fragment) ──────────────
        // A StrEnum used as a field type is always accessed via $ref; meeting it
        // inline here is unusual but we map it to `str` as a conservative fallback.
        SchemaType::StrEnum(_) => simple("str"),

        // ── Inline object ────────────────────────────────────────────────────
        // Inline object definitions inside another schema are rare; map to Any.
        SchemaType::Object(_) => with_import("Any", "typing", "Any"),

        // ── Constant ─────────────────────────────────────────────────────────
        // TODO(generate plan task 8): render as Literal["<value>"] with typing.Literal import
        SchemaType::ConstantSchema(_) => simple("str"),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::models::json_schema::{
        AnyOfSchema, AnySchema, ArraySchema, BooleanSchema, DecimalSchema, IntegerSchema,
        LiteralFormatDecimal, LiteralTypeArray, LiteralTypeDecimal, LiteralTypeString, NullSchema,
        NumberSchema, ReferenceSchema, StringSchema, StringSchemaFormat, TypeBase,
    };

    // ── Case 1: plain string ──────────────────────────────────────────────────
    #[test]
    fn map_string() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            format: None,
        });
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "str");
        assert!(result.imports.is_empty());
    }

    // ── Case 2: integer ───────────────────────────────────────────────────────
    #[test]
    fn map_integer() {
        let schema = SchemaType::IntegerSchema(IntegerSchema::default());
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "int");
        assert!(result.imports.is_empty());
    }

    // ── Case 3: number (float) ────────────────────────────────────────────────
    #[test]
    fn map_number() {
        let schema = SchemaType::NumberSchema(NumberSchema::default());
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "float");
        assert!(result.imports.is_empty());
    }

    // ── Case 4: boolean ───────────────────────────────────────────────────────
    #[test]
    fn map_boolean() {
        let schema = SchemaType::BooleanSchema(BooleanSchema::default());
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "bool");
        assert!(result.imports.is_empty());
    }

    // ── Case 5: optional string (anyOf: [string, null]) ───────────────────────
    #[test]
    fn map_optional_string() {
        let schema = SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![
                SchemaType::StringSchema(StringSchema {
                    base: TypeBase::default(),
                    r#type: LiteralTypeString::String,
                    format: None,
                }),
                SchemaType::NullSchema(NullSchema::default()),
            ],
        });
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "str | None");
        assert!(result.imports.is_empty());
    }

    // ── Case 6: array of strings ──────────────────────────────────────────────
    #[test]
    fn map_array_of_strings() {
        let schema = SchemaType::Array(ArraySchema {
            base: TypeBase::default(),
            r#type: LiteralTypeArray::Array,
            items: Box::new(SchemaType::StringSchema(StringSchema {
                base: TypeBase::default(),
                r#type: LiteralTypeString::String,
                format: None,
            })),
        });
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "list[str]");
        assert!(result.imports.is_empty());
    }

    // ── Case 7: Decimal (type=number, format=decimal) ─────────────────────────
    #[test]
    fn map_decimal() {
        let schema = SchemaType::DecimalSchema(DecimalSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeDecimal::Number,
            format: LiteralFormatDecimal::Decimal,
        });
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "Decimal");
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports.contains(&Import::Named {
            module: "decimal".to_string(),
            name: "Decimal".to_string(),
        }));
    }

    // ── Case 8: datetime ──────────────────────────────────────────────────────
    #[test]
    fn map_datetime() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            format: Some(StringSchemaFormat::DateTime),
        });
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "datetime");
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports.contains(&Import::Named {
            module: "datetime".to_string(),
            name: "datetime".to_string(),
        }));
    }

    // ── Case 9: $ref to sibling module ────────────────────────────────────────
    #[test]
    fn map_ref_to_sibling() {
        let schema = SchemaType::ReferenceSchema(ReferenceSchema {
            base: TypeBase::default(),
            r#ref: "../bo/Geschaeftspartner.json".to_string(),
        });
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "Geschaeftspartner");
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports.contains(&Import::Sibling {
            module: vec!["bo".to_string(), "Geschaeftspartner".to_string()],
            name: "Geschaeftspartner".to_string(),
        }));
    }

    // ── Additional: parse_ref helper ──────────────────────────────────────────

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

    #[test]
    fn map_uuid() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            format: Some(StringSchemaFormat::Uuid),
        });
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "UUID");
        assert!(result.imports.contains(&Import::Named {
            module: "uuid".to_string(),
            name: "UUID".to_string(),
        }));
    }

    #[test]
    fn map_optional_ref() {
        let schema = SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![
                SchemaType::ReferenceSchema(ReferenceSchema {
                    base: TypeBase::default(),
                    r#ref: "../bo/Adresse.json".to_string(),
                }),
                SchemaType::NullSchema(NullSchema::default()),
            ],
        });
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "Adresse | None");
        assert!(result.imports.contains(&Import::Sibling {
            module: vec!["bo".to_string(), "Adresse".to_string()],
            name: "Adresse".to_string(),
        }));
    }

    #[test]
    fn map_any_includes_typing_import() {
        let result = map_pydantic(&SchemaType::AnySchema(AnySchema::default()));
        assert_eq!(result.rendered, "Any");
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports.contains(&Import::Named {
            module: "typing".into(),
            name: "Any".into(),
        }));
    }

    #[test]
    fn map_anyof_any_with_null_collapses_to_any() {
        // `anyOf: [Any, null]` → `Any` (not `Any | None`, since Any subsumes None).
        let schema = SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![
                SchemaType::AnySchema(AnySchema::default()),
                SchemaType::NullSchema(NullSchema::default()),
            ],
        });
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "Any");
        assert!(result.imports.contains(&Import::Named {
            module: "typing".into(),
            name: "Any".into(),
        }));
    }

    #[test]
    fn map_anyof_str_and_any_collapses_to_any() {
        // `anyOf: [str, Any]` → `Any` (Any swallows the rest).
        let schema = SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![
                SchemaType::StringSchema(StringSchema {
                    base: TypeBase::default(),
                    r#type: LiteralTypeString::String,
                    format: None,
                }),
                SchemaType::AnySchema(AnySchema::default()),
            ],
        });
        let result = map_pydantic(&schema);
        assert_eq!(result.rendered, "Any");
    }
}
