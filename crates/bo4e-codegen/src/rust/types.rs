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

/// Rewrites every `enum` segment of a `parse_ref` module to `enums`, at
/// any depth, so Rust paths use the keyword-safe directory name.
/// Delegates to [`crate::rust::path_segments`] — the single source of
/// truth for Rust output path rewriting (used by per-file path
/// computation in [`crate::rust::module_paths`], the `mod.rs` writer
/// in `rust::plain`, and import-path building here).
fn rewrite_enum_dir(module: Vec<String>) -> Vec<String> {
    crate::rust::path_segments(&module)
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
/// (`crate::validate::all_schemas`) guarantees the default's primitive
/// kind matches the schema type and that typed-format strings parse —
/// so the `unwrap()` paths here cannot fail at runtime.
///
/// Returns `None` when the schema has no declared `default`.
pub fn literal_default_rust(schema: &SchemaType) -> Option<String> {
    let prim = crate::refs::schema_base(schema).default.as_ref()?;
    Some(render_typed_default(schema, prim))
}

/// Type-aware default rendering. Dispatches strictly on the (schema,
/// primitive) pair to emit a Rust expression that's a value of the
/// *mapped* Rust type. No fallback: every accepted pair has an explicit
/// arm; unmatched pairs are a validator bug and panic via
/// `unreachable!`.
///
/// The [`crate::validate::all_schemas`] gate is the single source of
/// truth for what's reachable here.
fn render_typed_default(schema: &SchemaType, prim: &PrimitiveValue) -> String {
    use bo4e_schemas::models::json_schema::StringSchemaFormat;

    match (schema, prim) {
        // ── Nullable wrappers: descend into the non-null branch (or
        // emit `None` when the default itself is null and the mapped
        // type is `Option<T>`). ────────────────────────────────────
        (SchemaType::AnyOf(_), PrimitiveValue::Null) => "None".into(),
        (SchemaType::AnyOf(a), p) => {
            let non_null = a
                .any_of
                .iter()
                .find(|t| !matches!(t, SchemaType::NullSchema(_)))
                .expect("validator: anyOf with default must have a non-null branch");
            render_typed_default(non_null, p)
        }
        (SchemaType::AllOf(a), p) => {
            let only = a
                .all_of
                .first()
                .expect("validator: allOf must have exactly one element");
            render_typed_default(only, p)
        }

        // ── Bool / number / decimal primitives. ───────────────────
        (SchemaType::BooleanSchema(_), PrimitiveValue::Bool(b)) => b.to_string(),
        (SchemaType::IntegerSchema(_), PrimitiveValue::Integer(i)) => format!("{i}i64"),
        (SchemaType::NumberSchema(_), PrimitiveValue::Integer(i)) => format!("{i}_f64"),
        (SchemaType::NumberSchema(_), PrimitiveValue::Float(f)) => format!("{f}_f64"),
        (SchemaType::DecimalSchema(_), PrimitiveValue::Integer(i)) => {
            format!("rust_decimal_macros::dec!({i})")
        }
        (SchemaType::DecimalSchema(_), PrimitiveValue::Float(f)) => {
            format!("rust_decimal_macros::dec!({f})")
        }
        (SchemaType::DecimalSchema(_), PrimitiveValue::String(s)) => {
            format!("rust_decimal_macros::dec!({s})")
        }

        // ── String: plain and typed-format. ───────────────────────
        (SchemaType::StringSchema(s), PrimitiveValue::String(v)) if s.format.is_none() => {
            format!("{v:?}.to_string()")
        }
        (SchemaType::StringSchema(s), PrimitiveValue::String(v)) => match &s.format {
            Some(StringSchemaFormat::Date) => render_date_default(v),
            Some(StringSchemaFormat::Time) => render_time_default(v),
            Some(StringSchemaFormat::DateTime) => render_datetime_default(v),
            Some(StringSchemaFormat::Uuid) => format!("uuid::uuid!({v:?})"),
            _ => format!("{v:?}.to_string()"),
        },

        // ── Enum / const / $ref string defaults. The enclosing
        // renderer prefers `enum_variant_default_rust` for $ref-to-
        // enum cases; this fallback covers inline `StrEnum` / `const`
        // and is also reachable when `enum_variant_default_rust`
        // declines (rare). ────────────────────────────────────────
        (SchemaType::ConstantSchema(_), PrimitiveValue::String(v))
        | (SchemaType::StrEnum(_), PrimitiveValue::String(v))
        | (SchemaType::ReferenceSchema(_), PrimitiveValue::String(v)) => {
            format!("{v:?}.to_string()")
        }

        // ── Any / Object: the validator only accepts `null` here
        // (Any field types render as `serde_json::Value`, so the
        // default expression is the JSON null variant). Non-null
        // defaults are validator-rejected and never reach this arm.
        (SchemaType::AnySchema(_), PrimitiveValue::Null)
        | (SchemaType::Object(_), PrimitiveValue::Null) => "serde_json::Value::Null".into(),

        // ── Unreachable per validator. Any pair landing here is a
        // gap in `validate::all_schemas` that must be fixed there,
        // not papered over with a primitive fallback. ─────────────
        (schema, prim) => unreachable!(
            "render_typed_default: validator should have rejected this pair \
             before code generation. schema={schema:?} default={prim:?}"
        ),
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
/// Uses `from_hms_nano_opt` so fractional-second defaults
/// (e.g. `"14:30:00.123"`) round-trip without nanoseconds being dropped.
fn render_time_default(value: &str) -> String {
    use chrono::Timelike;
    let parsed = chrono::NaiveTime::parse_from_str(value, "%H:%M:%S")
        .or_else(|_| chrono::NaiveTime::parse_from_str(value, "%H:%M:%S%.f"));
    match parsed {
        Ok(t) => format!(
            "chrono::NaiveTime::from_hms_nano_opt({}, {}, {}, {}).unwrap()",
            t.hour(),
            t.minute(),
            t.second(),
            t.nanosecond(),
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
        // `Import::Sibling::module` holds the lowercased, keyword-rewritten
        // segments (single source of truth via `rust::path_segments`); the
        // class name is preserved in PascalCase via `Import::Sibling::name`.
        assert!(m.imports.contains(&Import::Sibling {
            module: vec!["com".into(), "adresse".into()],
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
            module: vec!["enums".into(), "typ".into()],
            name: "Typ".into(),
        }));
    }

    /// Recursive enum-segment rewriting: an `enum` segment appearing at
    /// non-first depth must also be rewritten to `enums`. Pin this with
    /// a `$ref` like `../foo/enum/Color.json` — the import path must be
    /// `foo::enums::color`.
    #[test]
    fn map_ref_with_nested_enum_segment_rewrites_at_any_depth() {
        let schema = SchemaType::ReferenceSchema(ReferenceSchema {
            base: TypeBase::default(),
            r#ref: "../foo/enum/Color.json".into(),
        });
        let m = map_rust(&schema).unwrap();
        assert!(m.imports.contains(&Import::Sibling {
            module: vec!["foo".into(), "enums".into(), "color".into()],
            name: "Color".into(),
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
            "chrono::NaiveTime::from_hms_nano_opt(14, 30, 0, 0).unwrap()"
        );
    }

    /// Fractional-second time defaults must round-trip: the renderer
    /// uses `from_hms_nano_opt` so the parsed nanoseconds aren't dropped.
    #[test]
    fn literal_default_time_with_fractional_seconds_preserves_nanos() {
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("14:30:00.123".into())),
                ..Default::default()
            },
            r#type: LiteralTypeString::String,
            format: Some(StringSchemaFormat::Time),
        });
        // 0.123 seconds = 123_000_000 nanoseconds.
        assert_eq!(
            literal_default_rust(&schema).unwrap(),
            "chrono::NaiveTime::from_hms_nano_opt(14, 30, 0, 123000000).unwrap()"
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
