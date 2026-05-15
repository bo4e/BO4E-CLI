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

/// Render a JSON Schema `default` (when present) as a Python expression of
/// the schema's *mapped* Python type — symmetric with Rust's
/// `literal_default_rust`.
///
/// Typed-format string defaults emit immutable typed constructors:
/// - `format: date` → `date(y, m, d)`
/// - `format: time` → `time(h, m, s, microsecond=N)`
/// - `format: date-time` → `datetime.fromisoformat("…")`
/// - `format: uuid` → `UUID("…")`
/// - `DecimalSchema` → `Decimal("…")` (always string form, even for
///   numeric primitive defaults — preserves precision)
///
/// All five constructors return immutable values, so passing them as
/// `Field(default=…)` is safe (mutable defaults shared across pydantic
/// instances would be a bug; these aren't). For all other primitives
/// we fall through to the raw Python literal — pydantic v2's automatic
/// coercion handles any remaining type mapping.
///
/// The constructors reuse types already imported by [`map_pydantic`]
/// for the field's *type* annotation; no extra imports are needed
/// beyond what the type mapper already collects.
pub(crate) fn literal_default(schema: &SchemaType) -> Option<String> {
    let prim = schema_base(schema).default.as_ref()?;
    Some(render_typed_default(schema, prim))
}

/// Dispatches strictly on the `(schema, primitive)` pair. Mirrors the
/// Rust side's strict matching; unmatched pairs are validator gaps
/// and panic via `unreachable!`. The validator
/// (`crate::validate::all_schemas`) is the single source of truth for
/// what's reachable here.
fn render_typed_default(schema: &SchemaType, prim: &PrimitiveValue) -> String {
    match (schema, prim) {
        // ── Null at any level renders as Python `None`. ───────────
        (_, PrimitiveValue::Null) => "None".into(),

        // ── Nullable wrappers: descend into the non-null branch. ──
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

        // ── Bool / number primitives (no type widening needed). ───
        (SchemaType::BooleanSchema(_), PrimitiveValue::Bool(true)) => "True".into(),
        (SchemaType::BooleanSchema(_), PrimitiveValue::Bool(false)) => "False".into(),
        (SchemaType::IntegerSchema(_), PrimitiveValue::Integer(i)) => i.to_string(),
        (SchemaType::NumberSchema(_), PrimitiveValue::Integer(i)) => format!("{i}.0"),
        (SchemaType::NumberSchema(_), PrimitiveValue::Float(f)) => f.to_string(),

        // ── Decimal: always string form for precision. All values
        // pass through `python_string_literal` so a quote / backslash
        // in the original (validator already proved parseable) can't
        // produce invalid Python source. ──────────────────────────
        (SchemaType::DecimalSchema(_), PrimitiveValue::Integer(i)) => {
            format!(
                "Decimal({})",
                crate::python::python_string_literal(&i.to_string())
            )
        }
        (SchemaType::DecimalSchema(_), PrimitiveValue::Float(f)) => {
            format!(
                "Decimal({})",
                crate::python::python_string_literal(&f.to_string())
            )
        }
        (SchemaType::DecimalSchema(_), PrimitiveValue::String(s)) => {
            format!("Decimal({})", crate::python::python_string_literal(s))
        }

        // ── String: plain and typed-format. ───────────────────────
        (SchemaType::StringSchema(s), PrimitiveValue::String(v)) if s.format.is_none() => {
            crate::python::python_string_literal(v)
        }
        (SchemaType::StringSchema(s), PrimitiveValue::String(v)) => match &s.format {
            Some(StringSchemaFormat::Date) => render_python_date(v),
            Some(StringSchemaFormat::Time) => render_python_time(v),
            Some(StringSchemaFormat::DateTime) => render_python_datetime(v),
            Some(StringSchemaFormat::Uuid) => {
                format!("UUID({})", crate::python::python_string_literal(v))
            }
            _ => crate::python::python_string_literal(v),
        },

        // ── Enum / const / $ref string defaults. The pydantic
        // generator's `qualify_enum_default` wraps a quoted-string
        // default to `EnumName.<member>` when the schema $refs an
        // enum; this base rendering covers the inline cases. The
        // string passes through `python_string_literal` so that
        // arbitrary enum-member shapes (rare but allowed by JSON
        // Schema) escape cleanly. ─────────────────────────────────
        (SchemaType::ConstantSchema(_), PrimitiveValue::String(v))
        | (SchemaType::StrEnum(_), PrimitiveValue::String(v))
        | (SchemaType::ReferenceSchema(_), PrimitiveValue::String(v)) => {
            crate::python::python_string_literal(v)
        }

        // ── Any / Object: validator only accepts Null here; the
        // PrimitiveValue::Null arm at the top already handles it. ─
        (SchemaType::AnySchema(_), _) | (SchemaType::Object(_), _) => "None".into(),

        // ── Unreachable per validator. Any pair landing here is a
        // gap in `validate::all_schemas`. ─────────────────────────
        (schema, prim) => unreachable!(
            "literal_default: validator should have rejected this pair before code generation. \
             schema={schema:?} default={prim:?}"
        ),
    }
}

/// Render a `date` default as `date(y, m, d)`. Validator confirms the
/// value parses as `%Y-%m-%d`.
fn render_python_date(value: &str) -> String {
    use chrono::Datelike;
    match chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        Ok(d) => format!("date({}, {}, {})", d.year(), d.month(), d.day()),
        Err(_) => format!(
            "date.fromisoformat({})",
            crate::python::python_string_literal(value)
        ),
    }
}

/// Render a `time` default as `time(h, m, s, microsecond=N)` (Python
/// time has microsecond resolution, not nanosecond — chrono nanoseconds
/// are truncated to microseconds, which matches what Python could
/// round-trip anyway).
fn render_python_time(value: &str) -> String {
    use chrono::Timelike;
    let parsed = chrono::NaiveTime::parse_from_str(value, "%H:%M:%S")
        .or_else(|_| chrono::NaiveTime::parse_from_str(value, "%H:%M:%S%.f"));
    match parsed {
        Ok(t) => {
            let micro = t.nanosecond() / 1_000;
            if micro == 0 {
                format!("time({}, {}, {})", t.hour(), t.minute(), t.second())
            } else {
                format!(
                    "time({}, {}, {}, microsecond={micro})",
                    t.hour(),
                    t.minute(),
                    t.second()
                )
            }
        }
        Err(_) => format!(
            "time.fromisoformat({})",
            crate::python::python_string_literal(value)
        ),
    }
}

/// Render a `date-time` default. We use `datetime.fromisoformat(…)`
/// because the constructor form would need an extra `from datetime
/// import timezone` import that the type mapper doesn't otherwise
/// add, and the parse method returns an immutable `datetime` — so the
/// "no mutable defaults" rule is still satisfied. Normalises the
/// trailing `Z` to `+00:00` so the call works on pre-3.11 Pythons.
fn render_python_datetime(value: &str) -> String {
    let normalised = if let Some(stripped) = value.strip_suffix('Z') {
        format!("{stripped}+00:00")
    } else {
        value.to_string()
    };
    format!(
        "datetime.fromisoformat({})",
        crate::python::python_string_literal(&normalised)
    )
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

        // ── Single-variant inline string enum ────────────────────────────────
        // Schema-shape narrowing: a `StrEnum` with exactly one declared
        // member constrains the value to that one literal, so emit
        // `Literal["X"]` instead of bare `str` — symmetric with Rust's
        // synthetic single-variant enum narrowing in
        // `rust::render::single_variant_discriminator`. Multi-member
        // enums stay as the loose `str` fallback (they're accessed via
        // `$ref` to an enum module, which the ReferenceSchema arm
        // above handles).
        SchemaType::StrEnum(e) if e.enum_values.len() == 1 => literal_str_type(&e.enum_values[0]),
        SchemaType::StrEnum(_) => simple("str"),

        // ── Inline object ────────────────────────────────────────────────────
        // Inline object definitions inside another schema are rare; map to Any.
        SchemaType::Object(_) => with_import("Any", "typing", "Any"),

        // ── Constant: single-value `Literal["X"]` narrowing. ─────────────────
        // Mirrors the Rust single_variant_discriminator path. Multi-member
        // schemas (e.g. an unconstrained $ref to an enum) are handled by the
        // ReferenceSchema arm above; this arm covers inline `const` values.
        SchemaType::ConstantSchema(c) => literal_str_type(&c.constant),
    })
}

/// Build a `Literal["X"]` type expression with the matching
/// `from typing import Literal` import. Used by the
/// single-variant-discriminator narrowing path so the generated
/// Python types reflect the schema's single-value constraint.
fn literal_str_type(value: &str) -> MappedType {
    let mut imports = BTreeSet::new();
    imports.insert(Import::Named {
        module: "typing".to_string(),
        name: "Literal".to_string(),
    });
    MappedType {
        rendered: format!("Literal[{}]", crate::python::python_string_literal(value)),
        imports,
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

    // ── Single-variant Literal narrowing (mirror of Rust side) ───────────────
    #[test]
    fn map_constant_narrows_to_literal() {
        use bo4e_schemas::models::json_schema::ConstantSchema;
        let schema = SchemaType::ConstantSchema(ConstantSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            format: None,
            constant: "ANGEBOT".to_string(),
        });
        let m = map_pydantic(&schema).unwrap();
        assert_eq!(m.rendered, "Literal[\"ANGEBOT\"]");
        assert!(m.imports.contains(&Import::Named {
            module: "typing".into(),
            name: "Literal".into(),
        }));
    }

    #[test]
    fn map_single_member_strenum_narrows_to_literal() {
        use bo4e_schemas::models::json_schema::StrEnumSchema;
        let schema = SchemaType::StrEnum(StrEnumSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            enum_values: vec!["ANGEBOT".into()],
        });
        let m = map_pydantic(&schema).unwrap();
        assert_eq!(m.rendered, "Literal[\"ANGEBOT\"]");
        assert!(m.imports.contains(&Import::Named {
            module: "typing".into(),
            name: "Literal".into(),
        }));
    }

    #[test]
    fn map_multi_member_strenum_stays_str() {
        use bo4e_schemas::models::json_schema::StrEnumSchema;
        let schema = SchemaType::StrEnum(StrEnumSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            enum_values: vec!["A".into(), "B".into()],
        });
        let m = map_pydantic(&schema).unwrap();
        assert_eq!(m.rendered, "str");
    }

    #[test]
    fn anyof_const_and_null_renders_optional_literal() {
        use bo4e_schemas::models::json_schema::ConstantSchema;
        let schema = SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![
                SchemaType::ConstantSchema(ConstantSchema {
                    base: TypeBase::default(),
                    r#type: LiteralTypeString::String,
                    format: None,
                    constant: "ANGEBOT".to_string(),
                }),
                SchemaType::NullSchema(NullSchema::default()),
            ],
        });
        let m = map_pydantic(&schema).unwrap();
        assert_eq!(m.rendered, "Literal[\"ANGEBOT\"] | None");
    }

    // ── Typed-format defaults: immutable constructors ────────────────────────
    fn s_string(format: Option<StringSchemaFormat>, default: &str) -> SchemaType {
        SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String(default.to_string())),
                ..TypeBase::default()
            },
            r#type: LiteralTypeString::String,
            format,
        })
    }

    #[test]
    fn literal_default_date_emits_constructor() {
        let s = s_string(Some(StringSchemaFormat::Date), "2024-01-15");
        assert_eq!(literal_default(&s).unwrap(), "date(2024, 1, 15)");
    }

    #[test]
    fn literal_default_time_no_fractional() {
        let s = s_string(Some(StringSchemaFormat::Time), "14:30:00");
        assert_eq!(literal_default(&s).unwrap(), "time(14, 30, 0)");
    }

    #[test]
    fn literal_default_time_with_microseconds() {
        let s = s_string(Some(StringSchemaFormat::Time), "14:30:00.123");
        assert_eq!(
            literal_default(&s).unwrap(),
            "time(14, 30, 0, microsecond=123000)"
        );
    }

    #[test]
    fn literal_default_datetime_normalises_z_suffix() {
        let s = s_string(Some(StringSchemaFormat::DateTime), "2024-01-15T12:00:00Z");
        assert_eq!(
            literal_default(&s).unwrap(),
            "datetime.fromisoformat(\"2024-01-15T12:00:00+00:00\")"
        );
    }

    #[test]
    fn literal_default_datetime_keeps_explicit_offset() {
        let s = s_string(
            Some(StringSchemaFormat::DateTime),
            "2024-01-15T12:00:00+02:00",
        );
        assert_eq!(
            literal_default(&s).unwrap(),
            "datetime.fromisoformat(\"2024-01-15T12:00:00+02:00\")"
        );
    }

    #[test]
    fn literal_default_uuid_emits_constructor() {
        let s = s_string(
            Some(StringSchemaFormat::Uuid),
            "550e8400-e29b-41d4-a716-446655440000",
        );
        assert_eq!(
            literal_default(&s).unwrap(),
            "UUID(\"550e8400-e29b-41d4-a716-446655440000\")"
        );
    }

    #[test]
    fn literal_default_decimal_string_form_for_precision() {
        let schema = SchemaType::DecimalSchema(DecimalSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::Float(1.23)),
                ..TypeBase::default()
            },
            r#type: LiteralTypeDecimal::Number,
            format: LiteralFormatDecimal::Decimal,
        });
        assert_eq!(literal_default(&schema).unwrap(), "Decimal(\"1.23\")");
    }
}
