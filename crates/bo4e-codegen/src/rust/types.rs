//! JSON-Schema → Rust type-string mapping.

use crate::imports::Import;
use bo4e_schemas::models::json_schema::{PrimitiveValue, SchemaType, StringSchemaFormat};
use std::collections::BTreeSet;

/// The result of mapping a JSON Schema fragment to a Rust type expression.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // consumed by render_object in Task 21
pub struct MappedType {
    pub rendered: String,
    pub imports: BTreeSet<Import>,
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

fn with_imports(rendered: impl Into<String>, imports: Vec<Import>) -> MappedType {
    MappedType {
        rendered: rendered.into(),
        imports: imports.into_iter().collect(),
    }
}

/// Map a JSON Schema fragment to its Rust type expression.
///
/// Returns the "non-optional" form. The struct renderer wraps in `Option<T>`
/// when the owning property is not in `required`.
#[allow(dead_code)] // consumed by render_object in Task 21
pub fn map_rust(schema_type: &SchemaType) -> Result<MappedType, UnsupportedShape> {
    Ok(match schema_type {
        SchemaType::StringSchema(s) => match &s.format {
            None => simple("String"),
            Some(StringSchemaFormat::DateTime) => with_imports(
                "DateTime<Utc>",
                vec![
                    Import::Named {
                        module: "chrono".into(),
                        name: "DateTime".into(),
                    },
                    Import::Named {
                        module: "chrono".into(),
                        name: "Utc".into(),
                    },
                ],
            ),
            Some(StringSchemaFormat::Date) => with_import("NaiveDate", "chrono", "NaiveDate"),
            Some(StringSchemaFormat::Time) => with_import("NaiveTime", "chrono", "NaiveTime"),
            Some(StringSchemaFormat::Uuid) => with_import("Uuid", "uuid", "Uuid"),
            Some(_) => simple("String"),
        },
        SchemaType::IntegerSchema(_) => simple("i64"),
        SchemaType::NumberSchema(_) => simple("f64"),
        SchemaType::BooleanSchema(_) => simple("bool"),
        SchemaType::DecimalSchema(_) => with_import("Decimal", "rust_decimal", "Decimal"),
        SchemaType::NullSchema(_) => simple("()"),
        SchemaType::AnySchema(_) => with_import("Value", "serde_json", "Value"),

        SchemaType::Array(a) => {
            let inner = map_rust(&a.items)?;
            MappedType {
                rendered: format!("Vec<{}>", inner.rendered),
                imports: inner.imports,
            }
        }

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
            let inner = map_rust(non_null_branches[0])?;
            MappedType {
                rendered: format!("Option<{}>", inner.rendered),
                imports: inner.imports,
            }
        }

        SchemaType::AllOf(a) => {
            if a.all_of.len() == 1 {
                map_rust(&a.all_of[0])?
            } else {
                return Err(UnsupportedShape(
                    "multi-element allOf (intersection)".into(),
                ));
            }
        }

        SchemaType::ReferenceSchema(r) => {
            if r.r#ref.is_empty() {
                with_import("Value", "serde_json", "Value")
            } else {
                let (module, class_name) = crate::refs::parse_ref(&r.r#ref);
                let mut imports = BTreeSet::new();
                imports.insert(Import::Sibling {
                    module: rewrite_enum_dir(module),
                    name: class_name.clone(),
                });
                MappedType {
                    rendered: class_name,
                    imports,
                }
            }
        }

        SchemaType::StrEnum(_) => simple("String"),
        SchemaType::Object(_) => with_import("Value", "serde_json", "Value"),
        SchemaType::ConstantSchema(_) => simple("String"),
    })
}

/// Rewrites the first segment `"enum"` of a `parse_ref` module to `"enums"` so
/// Rust paths use the keyword-safe directory name.
fn rewrite_enum_dir(mut module: Vec<String>) -> Vec<String> {
    if module.first().map(String::as_str) == Some("enum") {
        module[0] = "enums".to_string();
    }
    module
}

/// Returned by [`map_rust`] when the schema has a shape BO4E declares unused.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // wrapped into Error::UnsupportedSchemaShape by render_object
pub struct UnsupportedShape(pub String);

impl std::fmt::Display for UnsupportedShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Render a JSON Schema `default` (when present, primitive) as a Rust literal expression.
#[allow(dead_code)]
pub fn literal_default_rust(schema: &SchemaType) -> Option<String> {
    crate::refs::schema_base(schema)
        .default
        .as_ref()
        .map(|v| match v {
            PrimitiveValue::Null => "None".into(),
            PrimitiveValue::Bool(true) => "true".into(),
            PrimitiveValue::Bool(false) => "false".into(),
            PrimitiveValue::Integer(i) => format!("{i}i64"),
            PrimitiveValue::Float(f) => format!("{f}f64"),
            PrimitiveValue::String(s) => format!("\"{s}\".to_string()"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::models::json_schema::{
        AnyOfSchema, AnySchema, ArraySchema, BooleanSchema, DecimalSchema, IntegerSchema,
        LiteralFormatDecimal, LiteralTypeArray, LiteralTypeDecimal, LiteralTypeString, NullSchema,
        NumberSchema, ReferenceSchema, StringSchema, StringSchemaFormat, TypeBase,
    };

    fn s_string(format: Option<StringSchemaFormat>) -> SchemaType {
        SchemaType::StringSchema(StringSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            format,
        })
    }
    fn s_null() -> SchemaType {
        SchemaType::NullSchema(NullSchema::default())
    }

    #[test]
    fn map_string() {
        let m = map_rust(&s_string(None)).unwrap();
        assert_eq!(m.rendered, "String");
        assert!(m.imports.is_empty());
    }

    #[test]
    fn map_integer_i64() {
        let m = map_rust(&SchemaType::IntegerSchema(IntegerSchema::default())).unwrap();
        assert_eq!(m.rendered, "i64");
    }

    #[test]
    fn map_number_f64() {
        let m = map_rust(&SchemaType::NumberSchema(NumberSchema::default())).unwrap();
        assert_eq!(m.rendered, "f64");
    }

    #[test]
    fn map_boolean() {
        let m = map_rust(&SchemaType::BooleanSchema(BooleanSchema::default())).unwrap();
        assert_eq!(m.rendered, "bool");
    }

    #[test]
    fn map_decimal_imports_rust_decimal() {
        let m = map_rust(&SchemaType::DecimalSchema(DecimalSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeDecimal::Number,
            format: LiteralFormatDecimal::Decimal,
        }))
        .unwrap();
        assert_eq!(m.rendered, "Decimal");
        assert!(m.imports.contains(&Import::Named {
            module: "rust_decimal".into(),
            name: "Decimal".into(),
        }));
    }

    #[test]
    fn map_datetime_imports_chrono() {
        let m = map_rust(&s_string(Some(StringSchemaFormat::DateTime))).unwrap();
        assert_eq!(m.rendered, "DateTime<Utc>");
        assert!(m.imports.contains(&Import::Named {
            module: "chrono".into(),
            name: "DateTime".into(),
        }));
        assert!(m.imports.contains(&Import::Named {
            module: "chrono".into(),
            name: "Utc".into(),
        }));
    }

    #[test]
    fn map_uuid_imports_uuid() {
        let m = map_rust(&s_string(Some(StringSchemaFormat::Uuid))).unwrap();
        assert_eq!(m.rendered, "Uuid");
        assert!(m.imports.contains(&Import::Named {
            module: "uuid".into(),
            name: "Uuid".into(),
        }));
    }

    #[test]
    fn map_optional_string_via_any_of_null() {
        let schema = SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![s_string(None), s_null()],
        });
        let m = map_rust(&schema).unwrap();
        assert_eq!(m.rendered, "Option<String>");
    }

    #[test]
    fn map_array_of_strings() {
        let schema = SchemaType::Array(ArraySchema {
            base: TypeBase::default(),
            r#type: LiteralTypeArray::Array,
            items: Box::new(s_string(None)),
        });
        let m = map_rust(&schema).unwrap();
        assert_eq!(m.rendered, "Vec<String>");
    }

    #[test]
    fn map_ref_sibling_with_super_path_data() {
        let schema = SchemaType::ReferenceSchema(ReferenceSchema {
            base: TypeBase::default(),
            r#ref: "../com/Adresse.json".into(),
        });
        let m = map_rust(&schema).unwrap();
        assert_eq!(m.rendered, "Adresse");
        assert!(m.imports.contains(&Import::Sibling {
            module: vec!["com".into(), "Adresse".into()],
            name: "Adresse".into(),
        }));
    }

    #[test]
    fn map_ref_to_enum_directory_rewrites_to_enums() {
        let schema = SchemaType::ReferenceSchema(ReferenceSchema {
            base: TypeBase::default(),
            r#ref: "../enum/Typ.json".into(),
        });
        let m = map_rust(&schema).unwrap();
        assert!(m.imports.contains(&Import::Sibling {
            module: vec!["enums".into(), "Typ".into()],
            name: "Typ".into(),
        }));
    }

    #[test]
    fn map_any_uses_serde_json_value() {
        let m = map_rust(&SchemaType::AnySchema(AnySchema::default())).unwrap();
        assert_eq!(m.rendered, "Value");
        assert!(m.imports.contains(&Import::Named {
            module: "serde_json".into(),
            name: "Value".into(),
        }));
    }

    #[test]
    fn map_any_of_without_null_errs() {
        let schema = SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![s_string(None), s_string(None)],
        });
        let err = map_rust(&schema).unwrap_err();
        assert!(err.0.contains("real union"));
    }

    #[test]
    fn map_all_of_multi_errs() {
        use bo4e_schemas::models::json_schema::AllOfSchema;
        let schema = SchemaType::AllOf(AllOfSchema {
            base: TypeBase::default(),
            all_of: vec![s_string(None), s_string(None)],
        });
        let err = map_rust(&schema).unwrap_err();
        assert!(err.0.contains("allOf"));
    }

    #[test]
    fn map_all_of_single_unwraps() {
        use bo4e_schemas::models::json_schema::AllOfSchema;
        let schema = SchemaType::AllOf(AllOfSchema {
            base: TypeBase::default(),
            all_of: vec![s_string(None)],
        });
        let m = map_rust(&schema).unwrap();
        assert_eq!(m.rendered, "String");
    }

    #[test]
    fn literal_default_string_renders_to_string() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("DE".into())),
                ..Default::default()
            },
            r#type: LiteralTypeString::String,
            format: None,
        });
        assert_eq!(literal_default_rust(&schema).unwrap(), "\"DE\".to_string()");
    }

    #[test]
    fn literal_default_int_renders_typed() {
        let schema = SchemaType::IntegerSchema(IntegerSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::Integer(7)),
                ..Default::default()
            },
            ..Default::default()
        });
        assert_eq!(literal_default_rust(&schema).unwrap(), "7i64");
    }
}
