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
pub use crate::refs::{enum_ref_target, parse_ref, schema_base};

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

/// Returned by [`map_pydantic`] when the schema has a shape BO4E declares
/// unused (e.g. real `anyOf` unions, multi-element `allOf` intersections,
/// pure `type: null`). Mirrors the Rust mapper's `UnsupportedShape` so both
/// flavours reject the same set of shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedShape(pub String);

impl std::fmt::Display for UnsupportedShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Map a JSON Schema fragment to its pydantic Python type expression.
///
/// The returned [`MappedType::rendered`] is the type string that should appear inline in
/// generated code.  [`MappedType::imports`] contains the import statements that `rendered`
/// depends on.
///
/// Returns `Err(UnsupportedShape)` for the same shapes the Rust mapper
/// rejects:
/// - `allOf` with more than one element (intersection)
/// - `anyOf` with more than one non-null branch (real union)
/// - `anyOf` with no `null` branch (real union)
/// - pure `type: null` outside an `anyOf` branch
pub fn map_pydantic(schema_type: &SchemaType) -> Result<MappedType, UnsupportedShape> {
    Ok(match schema_type {
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
        // Pure `type: null` outside an `anyOf` branch has no use in BO4E.
        // The validator rejects such properties up front, but the type mapper
        // is also called from Array `items` and other nested positions, so
        // mirror the rejection here.
        SchemaType::NullSchema(_) => {
            return Err(UnsupportedShape(
                "pure `type: null` schema has no use".into(),
            ));
        }

        // ── Any ──────────────────────────────────────────────────────────────
        SchemaType::AnySchema(_) => with_import("Any", "typing", "Any"),

        // ── Array ────────────────────────────────────────────────────────────
        SchemaType::Array(a) => {
            let inner = map_pydantic(&a.items)?;
            let rendered = format!("list[{}]", inner.rendered);
            MappedType {
                rendered,
                imports: inner.imports,
            }
        }

        // ── AnyOf — only the nullable pattern (one non-null + null) ─────────
        SchemaType::AnyOf(a) => {
            let (null_branches, non_null_branches): (Vec<_>, Vec<_>) = a
                .any_of
                .iter()
                .partition(|t| matches!(t, SchemaType::NullSchema(_)));

            if null_branches.is_empty() {
                return Err(UnsupportedShape(
                    "anyOf without null branch (real union)".into(),
                ));
            }
            if non_null_branches.len() != 1 {
                return Err(UnsupportedShape(
                    "anyOf with more than one non-null branch (real union)".into(),
                ));
            }

            let inner = map_pydantic(non_null_branches[0])?;
            MappedType {
                rendered: format!("{} | None", inner.rendered),
                imports: inner.imports,
            }
        }

        // ── AllOf — one-element wrapper only ─────────────────────────────────
        SchemaType::AllOf(a) => {
            if a.all_of.len() == 1 {
                map_pydantic(&a.all_of[0])?
            } else {
                return Err(UnsupportedShape(
                    "multi-element allOf (intersection)".into(),
                ));
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
        // Loose fallback. A stricter mapping would be `Literal["X"]` (with a
        // `typing.Literal` import) since the schema actually constrains the
        // value to that single string, but the per-field pydantic renderer
        // no longer narrows constant defaults specially — they fall through
        // here and inherit the value via `Field(default=…)` instead.
        SchemaType::ConstantSchema(_) => simple("str"),
    })
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
        let result = map_pydantic(&schema).unwrap();
        assert_eq!(result.rendered, "str");
        assert!(result.imports.is_empty());
    }

    // ── Case 2: integer ───────────────────────────────────────────────────────
    #[test]
    fn map_integer() {
        let schema = SchemaType::IntegerSchema(IntegerSchema::default());
        let result = map_pydantic(&schema).unwrap();
        assert_eq!(result.rendered, "int");
        assert!(result.imports.is_empty());
    }

    // ── Case 3: number (float) ────────────────────────────────────────────────
    #[test]
    fn map_number() {
        let schema = SchemaType::NumberSchema(NumberSchema::default());
        let result = map_pydantic(&schema).unwrap();
        assert_eq!(result.rendered, "float");
        assert!(result.imports.is_empty());
    }

    // ── Case 4: boolean ───────────────────────────────────────────────────────
    #[test]
    fn map_boolean() {
        let schema = SchemaType::BooleanSchema(BooleanSchema::default());
        let result = map_pydantic(&schema).unwrap();
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
        let result = map_pydantic(&schema).unwrap();
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
        let result = map_pydantic(&schema).unwrap();
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
        let result = map_pydantic(&schema).unwrap();
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
        let result = map_pydantic(&schema).unwrap();
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
        let result = map_pydantic(&schema).unwrap();
        assert_eq!(result.rendered, "Geschaeftspartner");
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports.contains(&Import::Sibling {
            module: vec!["bo".to_string(), "Geschaeftspartner".to_string()],
            name: "Geschaeftspartner".to_string(),
        }));
    }

    #[test]
    fn map_uuid() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            format: Some(StringSchemaFormat::Uuid),
        });
        let result = map_pydantic(&schema).unwrap();
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
        let result = map_pydantic(&schema).unwrap();
        assert_eq!(result.rendered, "Adresse | None");
        assert!(result.imports.contains(&Import::Sibling {
            module: vec!["bo".to_string(), "Adresse".to_string()],
            name: "Adresse".to_string(),
        }));
    }

    #[test]
    fn map_any_includes_typing_import() {
        let result = map_pydantic(&SchemaType::AnySchema(AnySchema::default())).unwrap();
        assert_eq!(result.rendered, "Any");
        assert_eq!(result.imports.len(), 1);
        assert!(result.imports.contains(&Import::Named {
            module: "typing".into(),
            name: "Any".into(),
        }));
    }

    #[test]
    fn map_anyof_any_with_null_renders_optional_any() {
        // `anyOf: [Any, null]` is the nullable pattern; emit `Any | None`.
        // (Python `Any` semantically subsumes None, but the schema asked for
        // explicit nullability so we surface it in the type expression.)
        let schema = SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![
                SchemaType::AnySchema(AnySchema::default()),
                SchemaType::NullSchema(NullSchema::default()),
            ],
        });
        let result = map_pydantic(&schema).unwrap();
        assert_eq!(result.rendered, "Any | None");
        assert!(result.imports.contains(&Import::Named {
            module: "typing".into(),
            name: "Any".into(),
        }));
    }

    #[test]
    fn map_anyof_str_and_any_is_rejected_as_real_union() {
        // `anyOf: [str, Any]` is a two-non-null-branch union — rejected.
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
        let err = map_pydantic(&schema).unwrap_err();
        assert!(err.0.contains("real union"), "got: {}", err.0);
    }

    #[test]
    fn map_anyof_without_null_is_rejected() {
        let schema = SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![
                SchemaType::StringSchema(StringSchema {
                    base: TypeBase::default(),
                    r#type: LiteralTypeString::String,
                    format: None,
                }),
                SchemaType::IntegerSchema(IntegerSchema::default()),
            ],
        });
        let err = map_pydantic(&schema).unwrap_err();
        assert!(err.0.contains("null branch"), "got: {}", err.0);
    }

    #[test]
    fn map_all_of_multi_is_rejected() {
        use bo4e_schemas::models::json_schema::AllOfSchema;
        let schema = SchemaType::AllOf(AllOfSchema {
            base: TypeBase::default(),
            all_of: vec![
                SchemaType::StringSchema(StringSchema {
                    base: TypeBase::default(),
                    r#type: LiteralTypeString::String,
                    format: None,
                }),
                SchemaType::IntegerSchema(IntegerSchema::default()),
            ],
        });
        let err = map_pydantic(&schema).unwrap_err();
        assert!(err.0.contains("allOf"), "got: {}", err.0);
    }

    #[test]
    fn map_pure_null_is_rejected() {
        let err = map_pydantic(&SchemaType::NullSchema(NullSchema::default())).unwrap_err();
        assert!(err.0.contains("null"), "got: {}", err.0);
    }
}
