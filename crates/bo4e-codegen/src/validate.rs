//! Schema-consistency invariants checked at generate-time.
//!
//! BO4E schemas pass through `bo4e edit` and any intermediate tooling, so
//! the generator can't assume the upstream definitions are internally
//! consistent. The checks here are the *gate* between "schema in" and
//! "code out": if a property's `required` membership doesn't match its
//! declared default, the generator refuses to produce code rather than
//! silently picking a behaviour that may surprise the caller.

use crate::Error;
use bo4e_schemas::models::json_schema::ObjectSchema;

/// Enforce the **required ⇔ no-default** invariant on every property of an
/// object schema. The rule has two directions:
///
/// - **required + default declared** is rejected: the default is
///   structurally unreachable (the JSON key is always present, so the
///   runtime never falls back to the default).
/// - **optional + no default** is rejected: the JSON key may be absent and
///   the runtime has nothing to fall back on, so the generated code would
///   need to invent a default (and *which* default is a design call that
///   only the schema can answer).
///
/// `schema_name` is used for error messages only (typically the class
/// name).
pub(crate) fn object_invariants(schema_name: &str, obj: &ObjectSchema) -> Result<(), Error> {
    use std::collections::BTreeSet;

    let property_names: BTreeSet<&str> = obj.properties.keys().map(String::as_str).collect();
    for required_name in &obj.required {
        if !property_names.contains(required_name.as_str()) {
            return Err(Error::InconsistentSchema {
                schema: schema_name.to_string(),
                property: required_name.clone(),
                reason: "name appears in `required` but has no entry in \
                     `properties`; the generator can't emit a field for it"
                    .to_string(),
            });
        }
    }

    let required: BTreeSet<&str> = obj.required.iter().map(String::as_str).collect();
    for (prop_name, prop_schema) in &obj.properties {
        validate_property_name(schema_name, prop_name)?;
        reject_pure_null_property(schema_name, prop_name, prop_schema)?;
        let is_required = required.contains(prop_name.as_str());
        let has_default = crate::refs::schema_base(prop_schema).default.is_some();
        match (is_required, has_default) {
            (true, true) => {
                return Err(Error::InconsistentSchema {
                    schema: schema_name.to_string(),
                    property: prop_name.clone(),
                    reason: "field is in `required` but declares a default value; \
                         defaults on required fields are unreachable (the JSON \
                         key is always present)"
                        .to_string(),
                });
            }
            (false, false) => {
                return Err(Error::InconsistentSchema {
                    schema: schema_name.to_string(),
                    property: prop_name.clone(),
                    reason: "field is optional (not in `required`) but declares no \
                         default value; the runtime has nothing to fall back on \
                         when the JSON key is absent"
                        .to_string(),
                });
            }
            _ => {}
        }

        default_matches_schema_type(schema_name, prop_name, prop_schema)?;
    }
    Ok(())
}

/// Reject property names that aren't a legal identifier source. BO4E
/// property names are expected to be camelCase / snake_case identifiers —
/// matching `[A-Za-z_][A-Za-z0-9_]*`. Names with hyphens, spaces, dots, or
/// other punctuation can't round-trip through `to_snake_case` →
/// `rust_field_name` / `python_attr_name` without producing invalid
/// identifiers in the generated code; reject them up front instead.
fn validate_property_name(schema_name: &str, prop_name: &str) -> Result<(), Error> {
    fn is_first_char_ok(c: char) -> bool {
        c.is_ascii_alphabetic() || c == '_'
    }
    fn is_rest_char_ok(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }
    let mut chars = prop_name.chars();
    let first_ok = chars.next().is_some_and(is_first_char_ok);
    let rest_ok = chars.all(is_rest_char_ok);
    if !first_ok || !rest_ok {
        return Err(Error::InconsistentSchema {
            schema: schema_name.to_string(),
            property: prop_name.to_string(),
            reason: "property name is not a valid identifier source; expected \
                 `[A-Za-z_][A-Za-z0-9_]*` (camelCase or snake_case shape)"
                .to_string(),
        });
    }
    Ok(())
}

/// Reject pure `type: null` property schemas. They have no use in BO4E and
/// the code generators have no sensible Rust/Python type to emit for them.
fn reject_pure_null_property(
    schema_name: &str,
    prop_name: &str,
    prop_schema: &bo4e_schemas::models::json_schema::SchemaType,
) -> Result<(), Error> {
    use bo4e_schemas::models::json_schema::SchemaType;
    if matches!(prop_schema, SchemaType::NullSchema(_)) {
        return Err(Error::InconsistentSchema {
            schema: schema_name.to_string(),
            property: prop_name.to_string(),
            reason: "property has pure `type: null`; null-only fields are not \
                 supported (use `anyOf: [T, null]` for nullable fields)"
                .to_string(),
        });
    }
    Ok(())
}

/// Reject defaults whose primitive kind does not match the property's
/// declared schema type. The match is structural: e.g. a `string`-typed
/// property may only declare a `String`-kind default; an `anyOf:[T, null]`
/// property accepts the union of `T`'s allowed kinds plus `Null`.
///
/// For string-format properties (`date`, `date-time`, `time`, `uuid`)
/// the default's literal is also parse-checked at generate time, so
/// e.g. `{type: "string", format: "date", default: "not-a-date"}`
/// is rejected before reaching the renderer.
fn default_matches_schema_type(
    schema_name: &str,
    prop_name: &str,
    prop_schema: &bo4e_schemas::models::json_schema::SchemaType,
) -> Result<(), Error> {
    use bo4e_schemas::models::json_schema::PrimitiveValue;

    let Some(default) = crate::refs::schema_base(prop_schema).default.as_ref() else {
        return Ok(());
    };

    let allowed = allowed_default_kinds(prop_schema);
    let actual = primitive_kind(default);

    if !allowed.contains(&actual) {
        return Err(Error::InconsistentSchema {
            schema: schema_name.to_string(),
            property: prop_name.to_string(),
            reason: format!(
                "default value kind `{actual:?}` is not compatible with the \
                 declared schema type (allowed kinds: {allowed:?})"
            ),
        });
    }

    // Format-specific content check: a `String` default for a typed-format
    // property must parse as that format.
    if let PrimitiveValue::String(s) = default
        && let Some(format_error) = check_string_format(prop_schema, s)
    {
        return Err(Error::InconsistentSchema {
            schema: schema_name.to_string(),
            property: prop_name.to_string(),
            reason: format_error,
        });
    }

    Ok(())
}

/// The set of `PrimitiveValue` kinds a default may have, given a property's
/// declared schema type. Used by [`default_matches_schema_type`].
fn allowed_default_kinds(
    schema: &bo4e_schemas::models::json_schema::SchemaType,
) -> std::collections::BTreeSet<PrimitiveKind> {
    use bo4e_schemas::models::json_schema::SchemaType;
    let mut out = std::collections::BTreeSet::new();
    match schema {
        SchemaType::StringSchema(_) => {
            out.insert(PrimitiveKind::String);
        }
        SchemaType::IntegerSchema(_) => {
            out.insert(PrimitiveKind::Integer);
        }
        SchemaType::NumberSchema(_) => {
            out.insert(PrimitiveKind::Integer);
            out.insert(PrimitiveKind::Float);
        }
        SchemaType::DecimalSchema(_) => {
            out.insert(PrimitiveKind::Integer);
            out.insert(PrimitiveKind::Float);
            out.insert(PrimitiveKind::String);
        }
        SchemaType::BooleanSchema(_) => {
            out.insert(PrimitiveKind::Bool);
        }
        SchemaType::NullSchema(_) => {
            out.insert(PrimitiveKind::Null);
        }
        SchemaType::AnySchema(_) | SchemaType::Object(_) => {
            // Permissive: any primitive kind, including null.
            out.insert(PrimitiveKind::Null);
            out.insert(PrimitiveKind::Bool);
            out.insert(PrimitiveKind::Integer);
            out.insert(PrimitiveKind::Float);
            out.insert(PrimitiveKind::String);
        }
        SchemaType::Array(_) => {
            // Array defaults aren't supported; the validator's
            // required ⇔ no-default rule and the schema_base default
            // type means this path is hit only via a structural default
            // that the renderer rejects elsewhere.
            out.insert(PrimitiveKind::Null);
        }
        SchemaType::AnyOf(a) => {
            for branch in &a.any_of {
                out.extend(allowed_default_kinds(branch));
            }
        }
        SchemaType::AllOf(a) => {
            if let Some(only) = a.all_of.first() {
                out.extend(allowed_default_kinds(only));
            }
        }
        SchemaType::ReferenceSchema(_) | SchemaType::StrEnum(_) | SchemaType::ConstantSchema(_) => {
            // `$ref` to an enum or inline str-enum / const → default is a
            // member name (string). The validator can't check membership
            // without resolving the ref; the renderer enforces that.
            out.insert(PrimitiveKind::String);
        }
    }
    out
}

/// Parse-check that a string default is well-formed for typed string formats.
/// Returns `Some(reason)` when the default is not parseable.
fn check_string_format(
    schema: &bo4e_schemas::models::json_schema::SchemaType,
    value: &str,
) -> Option<String> {
    use bo4e_schemas::models::json_schema::{SchemaType, StringSchemaFormat};
    let format = match schema {
        SchemaType::StringSchema(s) => s.format.as_ref()?,
        SchemaType::AnyOf(a) => {
            for branch in &a.any_of {
                if let Some(err) = check_string_format(branch, value) {
                    return Some(err);
                }
            }
            return None;
        }
        SchemaType::AllOf(a) => {
            if let Some(only) = a.all_of.first() {
                return check_string_format(only, value);
            }
            return None;
        }
        _ => return None,
    };
    let parsed = match format {
        StringSchemaFormat::Date => chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_ok(),
        StringSchemaFormat::Time => {
            chrono::NaiveTime::parse_from_str(value, "%H:%M:%S").is_ok()
                || chrono::NaiveTime::parse_from_str(value, "%H:%M:%S%.f").is_ok()
        }
        StringSchemaFormat::DateTime => chrono::DateTime::parse_from_rfc3339(value).is_ok(),
        StringSchemaFormat::Uuid => uuid::Uuid::parse_str(value).is_ok(),
        _ => true,
    };
    if parsed {
        None
    } else {
        Some(format!(
            "default value `{value:?}` is not parseable as format {format:?}"
        ))
    }
}

/// The five `PrimitiveValue` variants reduced to comparable tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PrimitiveKind {
    Null,
    Bool,
    Integer,
    Float,
    String,
}

fn primitive_kind(v: &bo4e_schemas::models::json_schema::PrimitiveValue) -> PrimitiveKind {
    use bo4e_schemas::models::json_schema::PrimitiveValue;
    match v {
        PrimitiveValue::Null => PrimitiveKind::Null,
        PrimitiveValue::Bool(_) => PrimitiveKind::Bool,
        PrimitiveValue::Integer(_) => PrimitiveKind::Integer,
        PrimitiveValue::Float(_) => PrimitiveKind::Float,
        PrimitiveValue::String(_) => PrimitiveKind::String,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::models::json_schema::{
        AnyOfSchema, LiteralTypeObject, NullSchema, PrimitiveValue, SchemaType, StringSchema,
        TypeBase,
    };
    use std::collections::BTreeMap;

    /// A `string`-typed schema with a default. The schema is *not* nullable —
    /// any `Null` default will be rejected by the type-compat check.
    fn s_string_with_default(default: Option<PrimitiveValue>) -> SchemaType {
        SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default,
                ..TypeBase::default()
            },
            r#type: bo4e_schemas::models::json_schema::LiteralTypeString::String,
            format: None,
        })
    }

    /// A truly nullable string schema: `anyOf: [string, null]`, optionally
    /// carrying a default on the outer schema (as BO4E sets it).
    fn s_nullable_string_with_default(default: Option<PrimitiveValue>) -> SchemaType {
        SchemaType::AnyOf(AnyOfSchema {
            base: TypeBase {
                default,
                ..TypeBase::default()
            },
            any_of: vec![
                SchemaType::StringSchema(StringSchema {
                    base: TypeBase::default(),
                    r#type: bo4e_schemas::models::json_schema::LiteralTypeString::String,
                    format: None,
                }),
                SchemaType::NullSchema(NullSchema::default()),
            ],
        })
    }

    fn s_string_no_default() -> SchemaType {
        SchemaType::StringSchema(StringSchema {
            base: TypeBase::default(),
            r#type: bo4e_schemas::models::json_schema::LiteralTypeString::String,
            format: None,
        })
    }

    fn obj(props: &[(&str, SchemaType)], required: &[&str]) -> ObjectSchema {
        let mut map = BTreeMap::new();
        for (k, v) in props {
            map.insert(k.to_string(), v.clone());
        }
        ObjectSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeObject::Object,
            additional_properties: true,
            properties: map,
            required: required.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn required_without_default_is_valid() {
        let o = obj(&[("name", s_string_no_default())], &["name"]);
        assert!(object_invariants("Foo", &o).is_ok());
    }

    #[test]
    fn optional_with_default_is_valid() {
        let o = obj(
            &[(
                "name",
                s_nullable_string_with_default(Some(PrimitiveValue::Null)),
            )],
            &[],
        );
        assert!(object_invariants("Foo", &o).is_ok());
    }

    #[test]
    fn required_with_default_is_rejected() {
        let o = obj(
            &[(
                "name",
                s_string_with_default(Some(PrimitiveValue::String("X".into()))),
            )],
            &["name"],
        );
        match object_invariants("Foo", &o) {
            Err(Error::InconsistentSchema {
                schema,
                property,
                reason,
            }) => {
                assert_eq!(schema, "Foo");
                assert_eq!(property, "name");
                assert!(reason.contains("required"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn optional_without_default_is_rejected() {
        let o = obj(&[("name", s_string_no_default())], &[]);
        match object_invariants("Foo", &o) {
            Err(Error::InconsistentSchema {
                schema,
                property,
                reason,
            }) => {
                assert_eq!(schema, "Foo");
                assert_eq!(property, "name");
                assert!(reason.contains("optional"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn null_default_counts_as_a_default() {
        // The optional-with-default-null shape is the common BO4E pattern
        // for "key may be absent or null".
        let o = obj(
            &[(
                "name",
                s_nullable_string_with_default(Some(PrimitiveValue::Null)),
            )],
            &[],
        );
        assert!(object_invariants("Foo", &o).is_ok());
    }

    #[test]
    fn required_name_missing_from_properties_is_rejected() {
        let o = obj(&[], &["ghost"]);
        match object_invariants("Foo", &o) {
            Err(Error::InconsistentSchema {
                property, reason, ..
            }) => {
                assert_eq!(property, "ghost");
                assert!(reason.contains("required"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn null_default_on_non_nullable_string_is_rejected() {
        let o = obj(
            &[("name", s_string_with_default(Some(PrimitiveValue::Null)))],
            &[],
        );
        match object_invariants("Foo", &o) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("kind"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn pure_null_property_is_rejected() {
        let o = obj(
            &[("name", SchemaType::NullSchema(NullSchema::default()))],
            &[],
        );
        match object_invariants("Foo", &o) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("pure"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn property_name_with_hyphen_is_rejected() {
        let o = obj(&[("foo-bar", s_string_no_default())], &["foo-bar"]);
        match object_invariants("Foo", &o) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("identifier"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn malformed_date_default_is_rejected() {
        use bo4e_schemas::models::json_schema::StringSchemaFormat;
        let schema = SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("not-a-date".into())),
                ..TypeBase::default()
            },
            r#type: bo4e_schemas::models::json_schema::LiteralTypeString::String,
            format: Some(StringSchemaFormat::Date),
        });
        let o = obj(&[("birthday", schema)], &[]);
        match object_invariants("Foo", &o) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("parseable"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }
}
