//! JSON-Schema → Rust type-string mapping.

use crate::imports::Import;
use bo4e_schemas::models::json_schema::{PrimitiveValue, SchemaType, StringSchemaFormat};
use std::collections::BTreeSet;

/// The result of mapping a JSON Schema fragment to a Rust type expression.
#[derive(Debug, Clone, PartialEq, Eq)]
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
/// Follows the schema's nullability directly: `anyOf:[T, null]` returns
/// `Option<T>`. Non-nullable schemas return their plain Rust type. The
/// struct renderer no longer auto-wraps fields based on `required` —
/// optionality is expressed by the field's default expression (driven
/// by the strict required/default invariant in `crate::validate`).
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
pub struct UnsupportedShape(pub String);

impl std::fmt::Display for UnsupportedShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Render a JSON Schema `default` (when present) as a Rust expression
/// matching the schema's mapped Rust type.
///
/// Type-aware: for typed string formats (`date`, `time`, `date-time`,
/// `uuid`), the default is parsed at generate time and emitted as a typed
/// constructor (`chrono::NaiveDate::from_ymd_opt`, `uuid::uuid!`, etc.).
/// For `DecimalSchema`, the default is emitted via the
/// `rust_decimal_macros::dec!` macro. The validator
/// (`crate::validate::object_invariants`) guarantees the default's
/// primitive kind matches the schema type and that typed-format strings
/// parse — so the `unwrap()` paths here cannot fail at runtime.
///
/// Returns `None` when the schema has no declared `default`.
pub fn literal_default_rust(schema: &SchemaType) -> Option<String> {
    let prim = crate::refs::schema_base(schema).default.as_ref()?;
    Some(render_typed_default(schema, prim))
}

/// Render a primitive literal in the most permissive (type-unaware) way.
/// Used as the fallback for schema shapes whose mapped Rust type matches
/// the primitive directly (e.g. `bool`, `i64`, `f64`, plain `String`).
fn render_primitive_default(prim: &PrimitiveValue) -> String {
    match prim {
        PrimitiveValue::Null => "None".into(),
        PrimitiveValue::Bool(true) => "true".into(),
        PrimitiveValue::Bool(false) => "false".into(),
        PrimitiveValue::Integer(i) => format!("{i}i64"),
        PrimitiveValue::Float(f) => format!("{f}f64"),
        PrimitiveValue::String(s) => format!("{s:?}.to_string()"),
    }
}

/// Type-aware default rendering. Dispatches on the schema variant (and
/// `format` where relevant) to emit a Rust expression that's a value of
/// the *mapped* Rust type, not just the JSON primitive.
fn render_typed_default(schema: &SchemaType, prim: &PrimitiveValue) -> String {
    use bo4e_schemas::models::json_schema::StringSchemaFormat;

    // Null is the universal "absent" expression.
    if matches!(prim, PrimitiveValue::Null) {
        return "None".into();
    }

    match (schema, prim) {
        (SchemaType::BooleanSchema(_), PrimitiveValue::Bool(b)) => b.to_string(),
        (SchemaType::IntegerSchema(_), PrimitiveValue::Integer(i)) => format!("{i}i64"),
        (SchemaType::NumberSchema(_), PrimitiveValue::Integer(i)) => format!("{i}_f64"),
        (SchemaType::NumberSchema(_), PrimitiveValue::Float(f)) => format!("{f}_f64"),

        // Decimal: use the dec! macro for all numeric forms. dec!() accepts
        // any literal token (integer, float, parsed string) — the validator
        // has already proven the value is well-formed.
        (SchemaType::DecimalSchema(_), PrimitiveValue::Integer(i)) => {
            format!("rust_decimal_macros::dec!({i})")
        }
        (SchemaType::DecimalSchema(_), PrimitiveValue::Float(f)) => {
            format!("rust_decimal_macros::dec!({f})")
        }
        (SchemaType::DecimalSchema(_), PrimitiveValue::String(s)) => {
            format!("rust_decimal_macros::dec!({s})")
        }

        // Plain string (no format) — `String::to_string()` from the literal.
        (SchemaType::StringSchema(s), PrimitiveValue::String(v)) if s.format.is_none() => {
            format!("{v:?}.to_string()")
        }

        // Typed string formats. The validator has parse-checked the value.
        (SchemaType::StringSchema(s), PrimitiveValue::String(v)) => match &s.format {
            Some(StringSchemaFormat::Date) => render_date_default(v),
            Some(StringSchemaFormat::Time) => render_time_default(v),
            Some(StringSchemaFormat::DateTime) => render_datetime_default(v),
            Some(StringSchemaFormat::Uuid) => format!("uuid::uuid!({v:?})"),
            _ => format!("{v:?}.to_string()"),
        },

        // ConstantSchema / StrEnum / $ref-to-enum: the renderer's
        // `enum_variant_default_rust` path is preferred. Fall back to a
        // plain string literal for cases where the enum context isn't
        // resolvable (e.g. inline const without a $ref).
        (SchemaType::ConstantSchema(_), PrimitiveValue::String(v)) => format!("{v:?}.to_string()"),
        (SchemaType::StrEnum(_), PrimitiveValue::String(v)) => format!("{v:?}.to_string()"),

        // anyOf:[T, null] — descend into the non-null branch. We've already
        // handled PrimitiveValue::Null above; here we know prim is non-null
        // and at least one branch is non-null (validator enforces this).
        (SchemaType::AnyOf(a), _) => {
            if let Some(non_null) = a
                .any_of
                .iter()
                .find(|t| !matches!(t, SchemaType::NullSchema(_)))
            {
                return render_typed_default(non_null, prim);
            }
            render_primitive_default(prim)
        }
        (SchemaType::AllOf(a), _) => {
            if let Some(only) = a.all_of.first() {
                return render_typed_default(only, prim);
            }
            render_primitive_default(prim)
        }

        // Catch-all (Object, Array, AnySchema, ReferenceSchema, NullSchema):
        // emit the primitive directly. Validator gates most of these out.
        _ => render_primitive_default(prim),
    }
}

/// Render a `date` default as `chrono::NaiveDate::from_ymd_opt(y, m, d).unwrap()`.
/// The validator has confirmed `value` parses as `%Y-%m-%d`.
fn render_date_default(value: &str) -> String {
    use chrono::Datelike;
    match chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        Ok(d) => format!(
            "chrono::NaiveDate::from_ymd_opt({}, {}, {}).unwrap()",
            d.year(),
            d.month(),
            d.day()
        ),
        // Validator failure — keep going with a runtime parse so the
        // generator never panics on malformed input that slipped through.
        Err(_) => format!("chrono::NaiveDate::parse_from_str({value:?}, \"%Y-%m-%d\").unwrap()"),
    }
}

/// Render a `time` default. Validator accepts `%H:%M:%S` or `%H:%M:%S%.f`.
fn render_time_default(value: &str) -> String {
    use chrono::Timelike;
    let parsed = chrono::NaiveTime::parse_from_str(value, "%H:%M:%S")
        .or_else(|_| chrono::NaiveTime::parse_from_str(value, "%H:%M:%S%.f"));
    match parsed {
        Ok(t) => format!(
            "chrono::NaiveTime::from_hms_opt({}, {}, {}).unwrap()",
            t.hour(),
            t.minute(),
            t.second()
        ),
        Err(_) => format!("chrono::NaiveTime::parse_from_str({value:?}, \"%H:%M:%S\").unwrap()"),
    }
}

/// Render a `date-time` default as a parsed RFC3339 expression in `Utc`.
/// We keep this as a runtime parse expression rather than reconstructing
/// from components: chrono has no const-fn constructor for `DateTime<Utc>`
/// and the validator has confirmed the value is RFC3339-parseable.
fn render_datetime_default(value: &str) -> String {
    format!("chrono::DateTime::parse_from_rfc3339({value:?}).unwrap().with_timezone(&chrono::Utc)")
}

/// If `schema` is a `$ref` to an enum (directly or via `anyOf:[$ref, null]`)
/// AND carries a string `default`, render that default as the variant
/// reference `<inner_type>::<Sanitised>`. Otherwise returns `None`, leaving
/// the caller to fall back to [`literal_default_rust`].
///
/// `inner_type` is the rendered Rust type name **without** any `Option<>`
/// wrapper (e.g. `"Typ"` for both `Typ` and `Option<Typ>` fields). The
/// caller re-wraps in `Some(...)` afterwards as the matrix demands.
pub fn enum_variant_default_rust(schema: &SchemaType, inner_type: &str) -> Option<String> {
    use crate::naming::{sanitize_member_name, to_pascal_case};

    let PrimitiveValue::String(s) = crate::refs::schema_base(schema).default.as_ref()? else {
        return None;
    };
    crate::refs::enum_ref_target(schema)?;
    let variant = to_pascal_case(&sanitize_member_name(s));
    Some(format!("{inner_type}::{variant}"))
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

    /// Regression: string defaults must be escaped into valid Rust string
    /// literals. Plain interpolation broke on quotes/backslashes/newlines.
    #[test]
    fn literal_default_string_escapes_quotes_and_backslashes() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String(r#"He said "hi" \n"#.into())),
                ..Default::default()
            },
            r#type: LiteralTypeString::String,
            format: None,
        });
        // `{:?}` on a `&str` produces a Rust-syntax string literal.
        assert_eq!(
            literal_default_rust(&schema).unwrap(),
            r#""He said \"hi\" \\n".to_string()"#
        );
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

    #[test]
    fn literal_default_date_renders_typed_constructor() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("2024-01-15".into())),
                ..Default::default()
            },
            r#type: LiteralTypeString::String,
            format: Some(StringSchemaFormat::Date),
        });
        assert_eq!(
            literal_default_rust(&schema).unwrap(),
            "chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()"
        );
    }

    #[test]
    fn literal_default_time_renders_typed_constructor() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("14:30:00".into())),
                ..Default::default()
            },
            r#type: LiteralTypeString::String,
            format: Some(StringSchemaFormat::Time),
        });
        assert_eq!(
            literal_default_rust(&schema).unwrap(),
            "chrono::NaiveTime::from_hms_opt(14, 30, 0).unwrap()"
        );
    }

    #[test]
    fn literal_default_datetime_renders_parse_from_rfc3339() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("2024-01-15T12:00:00Z".into())),
                ..Default::default()
            },
            r#type: LiteralTypeString::String,
            format: Some(StringSchemaFormat::DateTime),
        });
        let s = literal_default_rust(&schema).unwrap();
        assert!(s.contains("parse_from_rfc3339"), "got: {s}");
        assert!(s.contains("Utc"), "got: {s}");
    }

    #[test]
    fn literal_default_uuid_renders_uuid_macro() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String(
                    "550e8400-e29b-41d4-a716-446655440000".into(),
                )),
                ..Default::default()
            },
            r#type: LiteralTypeString::String,
            format: Some(StringSchemaFormat::Uuid),
        });
        assert_eq!(
            literal_default_rust(&schema).unwrap(),
            "uuid::uuid!(\"550e8400-e29b-41d4-a716-446655440000\")"
        );
    }

    #[test]
    fn literal_default_decimal_renders_dec_macro() {
        let schema = SchemaType::DecimalSchema(DecimalSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::Float(1.23)),
                ..Default::default()
            },
            r#type: LiteralTypeDecimal::Number,
            format: LiteralFormatDecimal::Decimal,
        });
        assert_eq!(
            literal_default_rust(&schema).unwrap(),
            "rust_decimal_macros::dec!(1.23)"
        );
    }
}
