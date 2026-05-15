//! Schema-consistency invariants checked at generate-time.
//!
//! Validation is decoupled from generation: the public entry point
//! [`all_schemas`] runs once over the entire [`bo4e_schemas::Schemas`]
//! collection, before any flavour-specific renderer touches a file.
//! Each `generate()` calls it at the top so a failed schema can never
//! produce a half-written output tree.
//!
//! The validator has access to the full schema set so cross-schema
//! checks (e.g. `$ref` defaults must reference a real enum variant)
//! happen here rather than being deferred to the renderer.
//!
//! All violations surface as [`Error::InconsistentSchema`] with the
//! offending schema/property pair and a human-readable reason.

use crate::Error;
use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::{ObjectSchema, PrimitiveValue, SchemaRootType, SchemaType};

/// Run every schema-consistency invariant against `schemas`. Should be
/// called once at the top of each `generate()` so the file-writing
/// phase can assume validity.
pub fn all_schemas(schemas: &Schemas) -> Result<(), Error> {
    for schema_rc in schemas {
        let mut schema = schema_rc.borrow_mut();
        let class_name = schema.name().to_string();
        let parsed = schema.schema().map_err(Error::Schema)?.clone();
        drop(schema);
        if let SchemaRootType::Object(o) = &parsed {
            object_invariants(&class_name, &o.object, schemas)?;
        }
    }
    Ok(())
}

/// Enforce schema-consistency invariants on one object schema.
///
/// `schemas` is used to resolve `$ref` defaults to their target
/// schema so we can verify a string default actually names a real
/// enum variant.
pub(crate) fn object_invariants(
    schema_name: &str,
    obj: &ObjectSchema,
    schemas: &Schemas,
) -> Result<(), Error> {
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

        default_matches_schema_type(schema_name, prop_name, prop_schema, schemas)?;
    }
    Ok(())
}

/// Reject property names that aren't a legal identifier source. BO4E
/// property names are expected to be camelCase / snake_case identifiers —
/// matching `[A-Za-z_][A-Za-z0-9_]*`. Names with hyphens, spaces, dots, or
/// other punctuation can't round-trip through `to_snake_case` →
/// `rust_field_name` / `python_attr_name` without producing invalid
/// identifiers in the generated code; reject them up front instead.
///
/// Also rejects names whose post-leading-underscore-strip result is
/// empty or `_` — the generators strip one leading `_` (BO4E's `_id`,
/// `_typ`, `_version` convention), so `_` would become an empty Rust /
/// Python field name and `__` would become the placeholder `_`. Both
/// are unusable identifiers.
fn validate_property_name(schema_name: &str, prop_name: &str) -> Result<(), Error> {
    fn is_first_char_ok(c: char) -> bool {
        c.is_ascii_alphabetic() || c == '_'
    }
    fn is_rest_char_ok(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }
    let invalid_charset = {
        let mut chars = prop_name.chars();
        let first_ok = chars.next().is_some_and(is_first_char_ok);
        let rest_ok = chars.all(is_rest_char_ok);
        !first_ok || !rest_ok
    };
    if invalid_charset {
        return Err(Error::InconsistentSchema {
            schema: schema_name.to_string(),
            property: prop_name.to_string(),
            reason: "property name is not a valid identifier source; expected \
                 `[A-Za-z_][A-Za-z0-9_]*` (camelCase or snake_case shape)"
                .to_string(),
        });
    }
    // After stripping one leading underscore (the generator convention
    // for `_id`/`_typ`/`_version`), the result must be a non-empty,
    // non-underscore-only identifier. `_` becomes empty; `__` becomes
    // `_` (used as Rust's "ignore" placeholder).
    let stripped = prop_name.strip_prefix('_').unwrap_or(prop_name);
    if stripped.is_empty() || stripped.chars().all(|c| c == '_') {
        return Err(Error::InconsistentSchema {
            schema: schema_name.to_string(),
            property: prop_name.to_string(),
            reason: "property name has no meaningful identifier after stripping a \
                 leading underscore; got `_` / `__` / similar"
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
    prop_schema: &SchemaType,
) -> Result<(), Error> {
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
/// declared schema type. The match is structural and exhaustive: the
/// validator either accepts a (schema, primitive) pair or rejects it —
/// the renderer can rely on every accepted pair having an explicit
/// rendering arm.
///
/// For typed string formats (`date`, `date-time`, `time`, `uuid`) the
/// default literal is parse-checked at generate time. For
/// `DecimalSchema` string defaults the string is parsed as a decimal.
/// For `$ref` defaults the target is resolved via `schemas`; only
/// `$ref` to a `StrEnum` is acceptable, and the default value must be
/// one of the enum's declared members.
fn default_matches_schema_type(
    schema_name: &str,
    prop_name: &str,
    prop_schema: &SchemaType,
    schemas: &Schemas,
) -> Result<(), Error> {
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

    // Decimal string default: the renderer injects it into
    // `rust_decimal_macros::dec!(...)`, which only accepts a valid
    // decimal literal — parse-check it here.
    if let PrimitiveValue::String(s) = default
        && schema_targets_decimal(prop_schema)
        && s.parse::<rust_decimal::Decimal>().is_err()
    {
        return Err(Error::InconsistentSchema {
            schema: schema_name.to_string(),
            property: prop_name.to_string(),
            reason: format!("decimal string default `{s}` is not a parseable decimal literal"),
        });
    }

    // `$ref` defaults: only `$ref` to a `StrEnum` is acceptable, and
    // the default must be one of the enum's declared members.
    if let Some(ref_module) = extract_ref_target_module(prop_schema) {
        check_ref_default(schema_name, prop_name, default, &ref_module, schemas)?;
    }

    Ok(())
}

/// The set of `PrimitiveValue` kinds a default may have, given a
/// property's declared schema type. Strict: arms that don't accept any
/// default (e.g. `Array`) return an empty set so the type-compat check
/// rejects every default value on those shapes.
fn allowed_default_kinds(schema: &SchemaType) -> std::collections::BTreeSet<PrimitiveKind> {
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
        // `Any`/object fields: only `null` is sensible as a default
        // (renders as `serde_json::Value::Null`); other primitive
        // defaults would need bespoke `serde_json::json!(…)` rendering
        // which BO4E does not use.
        SchemaType::AnySchema(_) | SchemaType::Object(_) => {
            out.insert(PrimitiveKind::Null);
        }
        // `null`-typed branch (only reached via `anyOf` recursion —
        // pure-null properties are rejected upstream by
        // `reject_pure_null_property`). Contributes the `Null` kind to
        // any `anyOf` that includes a null branch.
        SchemaType::NullSchema(_) => {
            out.insert(PrimitiveKind::Null);
        }
        // Arrays have no representable default — `default: [1, 2]` and
        // the like are not part of the BO4E schema dialect. Empty set
        // rejects every default value on direct array properties.
        SchemaType::Array(_) => {}
        // `anyOf:[T, null]` accepts `T`'s kinds plus `Null` (the
        // explicit null branch contributes Null). Real unions are
        // rejected upstream by the type mappers.
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
        // `$ref` (or inline str-enum / const): default is a member
        // name (string). Membership is checked separately via
        // `check_ref_default`.
        SchemaType::ReferenceSchema(_) | SchemaType::StrEnum(_) | SchemaType::ConstantSchema(_) => {
            out.insert(PrimitiveKind::String);
        }
    }
    out
}

/// Parse-check that a string default is well-formed for typed string formats.
/// Returns `Some(reason)` when the default is not parseable.
fn check_string_format(schema: &SchemaType, value: &str) -> Option<String> {
    use bo4e_schemas::models::json_schema::StringSchemaFormat;
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

/// True iff `schema` is (or wraps in `anyOf:[…, null]` / single-element
/// `allOf`) a `DecimalSchema`. Used to gate the decimal-string parse
/// check.
fn schema_targets_decimal(schema: &SchemaType) -> bool {
    match schema {
        SchemaType::DecimalSchema(_) => true,
        SchemaType::AnyOf(a) => a.any_of.iter().any(schema_targets_decimal),
        SchemaType::AllOf(a) => a.all_of.first().is_some_and(schema_targets_decimal),
        _ => false,
    }
}

/// If `schema` is (or wraps in `anyOf:[…, null]` / single-element
/// `allOf`) a non-empty `$ref`, return the target's module path
/// (segments including the class name). Otherwise `None`.
fn extract_ref_target_module(schema: &SchemaType) -> Option<Vec<String>> {
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
        SchemaType::AllOf(a) => {
            if let Some(SchemaType::ReferenceSchema(r)) = a.all_of.first() {
                if r.r#ref.is_empty() {
                    return None;
                }
                r
            } else {
                return None;
            }
        }
        _ => return None,
    };
    let (module, _) = crate::refs::parse_ref(&r.r#ref);
    Some(module)
}

/// Check a default value attached to a `$ref` property.
///
/// - `null` default → always accepted (signals "absent" regardless of
///   the target's shape; renders as `None` / `Value::Null`).
/// - `String` default → target must resolve to a `StrEnum`, and the
///   string must be one of the declared members. `$ref` to an
///   `Object` schema with a non-null default is rejected (the
///   renderer has no way to emit one).
/// - Any other primitive kind on a `$ref` → rejected by
///   [`allowed_default_kinds`] before reaching this function.
fn check_ref_default(
    schema_name: &str,
    prop_name: &str,
    default: &PrimitiveValue,
    ref_module: &[String],
    schemas: &Schemas,
) -> Result<(), Error> {
    if matches!(default, PrimitiveValue::Null) {
        return Ok(());
    }
    let Some(target_rc) = schemas.get_by_module(ref_module) else {
        return Err(Error::InconsistentSchema {
            schema: schema_name.to_string(),
            property: prop_name.to_string(),
            reason: format!(
                "`$ref` default target `{}` not found in schemas",
                ref_module.join("/")
            ),
        });
    };
    let mut target = target_rc.borrow_mut();
    let parsed = target.schema().map_err(Error::Schema)?;
    match parsed {
        SchemaRootType::StrEnum(e) => {
            let PrimitiveValue::String(s) = default else {
                return Err(Error::InconsistentSchema {
                    schema: schema_name.to_string(),
                    property: prop_name.to_string(),
                    reason: format!(
                        "default for `$ref` to enum must be a string member name, \
                         got primitive kind `{:?}`",
                        primitive_kind(default)
                    ),
                });
            };
            if !e.str_enum.enum_values.contains(s) {
                return Err(Error::InconsistentSchema {
                    schema: schema_name.to_string(),
                    property: prop_name.to_string(),
                    reason: format!(
                        "default `{s}` is not a member of enum `{}` \
                         (declared members: {:?})",
                        ref_module.last().map(String::as_str).unwrap_or("?"),
                        e.str_enum.enum_values,
                    ),
                });
            }
            Ok(())
        }
        SchemaRootType::Object(_) => Err(Error::InconsistentSchema {
            schema: schema_name.to_string(),
            property: prop_name.to_string(),
            reason: format!(
                "`$ref` to object schema `{}` only accepts `null` as a default \
                 value, got non-null",
                ref_module.join("/")
            ),
        }),
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

fn primitive_kind(v: &PrimitiveValue) -> PrimitiveKind {
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
    use bo4e_schemas::Schema;
    use bo4e_schemas::models::json_schema::{
        AnyOfSchema, LiteralTypeObject, NullSchema, PrimitiveValue, SchemaType, StringSchema,
        TypeBase,
    };
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

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

    /// Empty `Schemas` (no target resolution available). Tests that
    /// don't exercise `$ref` defaults can use this as a stub.
    fn empty_schemas() -> Schemas {
        Schemas::new("v202401.0.0".parse().unwrap())
    }

    /// Build a `Schemas` containing one `StrEnum` so `$ref` defaults
    /// can resolve to it.
    fn schemas_with_enum(name: &str, members: &[&str]) -> Schemas {
        let mut schemas = Schemas::new("v202401.0.0".parse().unwrap());
        let members_json = serde_json::to_string(members).unwrap();
        let mut s = Schema::new(vec!["enum".into(), name.into()], None).unwrap();
        s.load_schema(format!(
            "{{\"type\":\"string\",\"title\":\"{name}\",\"enum\":{members_json}}}"
        ));
        schemas.add_schema(Rc::new(RefCell::new(s))).unwrap();
        schemas
    }

    #[test]
    fn required_without_default_is_valid() {
        let o = obj(&[("name", s_string_no_default())], &["name"]);
        assert!(object_invariants("Foo", &o, &empty_schemas()).is_ok());
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
        assert!(object_invariants("Foo", &o, &empty_schemas()).is_ok());
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
        match object_invariants("Foo", &o, &empty_schemas()) {
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
        match object_invariants("Foo", &o, &empty_schemas()) {
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
        let o = obj(
            &[(
                "name",
                s_nullable_string_with_default(Some(PrimitiveValue::Null)),
            )],
            &[],
        );
        assert!(object_invariants("Foo", &o, &empty_schemas()).is_ok());
    }

    #[test]
    fn required_name_missing_from_properties_is_rejected() {
        let o = obj(&[], &["ghost"]);
        match object_invariants("Foo", &o, &empty_schemas()) {
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
        match object_invariants("Foo", &o, &empty_schemas()) {
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
        match object_invariants("Foo", &o, &empty_schemas()) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("pure"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn property_name_with_hyphen_is_rejected() {
        let o = obj(&[("foo-bar", s_string_no_default())], &["foo-bar"]);
        match object_invariants("Foo", &o, &empty_schemas()) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("identifier"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn property_name_underscore_only_is_rejected() {
        for bad in ["_", "__", "___"] {
            let o = obj(&[(bad, s_string_no_default())], &[bad]);
            match object_invariants("Foo", &o, &empty_schemas()) {
                Err(Error::InconsistentSchema { reason, .. }) => {
                    assert!(
                        reason.contains("meaningful identifier"),
                        "got: {reason} for {bad:?}"
                    );
                }
                other => panic!("expected InconsistentSchema for {bad:?}, got {other:?}"),
            }
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
        match object_invariants("Foo", &o, &empty_schemas()) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("parseable"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn array_with_default_is_rejected() {
        use bo4e_schemas::models::json_schema::{ArraySchema, LiteralTypeArray};
        let schema = SchemaType::Array(ArraySchema {
            base: TypeBase {
                default: Some(PrimitiveValue::Null),
                ..TypeBase::default()
            },
            r#type: LiteralTypeArray::Array,
            items: Box::new(s_string_no_default()),
        });
        let o = obj(&[("xs", schema)], &[]);
        match object_invariants("Foo", &o, &empty_schemas()) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("kind"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn any_schema_only_accepts_null_default() {
        use bo4e_schemas::models::json_schema::AnySchema;
        // Valid: null default on AnySchema.
        let any_null = SchemaType::AnySchema(AnySchema {
            base: TypeBase {
                default: Some(PrimitiveValue::Null),
                ..TypeBase::default()
            },
        });
        let o = obj(&[("extras", any_null)], &[]);
        assert!(object_invariants("Foo", &o, &empty_schemas()).is_ok());

        // Invalid: string default on AnySchema.
        let any_str = SchemaType::AnySchema(AnySchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("oops".into())),
                ..TypeBase::default()
            },
        });
        let o = obj(&[("extras", any_str)], &[]);
        match object_invariants("Foo", &o, &empty_schemas()) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("kind"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn decimal_unparseable_string_default_is_rejected() {
        use bo4e_schemas::models::json_schema::{
            DecimalSchema, LiteralFormatDecimal, LiteralTypeDecimal,
        };
        let schema = SchemaType::DecimalSchema(DecimalSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("not-a-decimal".into())),
                ..TypeBase::default()
            },
            r#type: LiteralTypeDecimal::Number,
            format: LiteralFormatDecimal::Decimal,
        });
        let o = obj(&[("price", schema)], &[]);
        match object_invariants("Foo", &o, &empty_schemas()) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("decimal"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn enum_ref_default_with_known_variant_is_valid() {
        use bo4e_schemas::models::json_schema::ReferenceSchema;
        let prop = SchemaType::ReferenceSchema(ReferenceSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("RED".into())),
                ..TypeBase::default()
            },
            r#ref: "../enum/Color.json#".into(),
        });
        let o = obj(&[("color", prop)], &[]);
        let schemas = schemas_with_enum("Color", &["RED", "GREEN", "BLUE"]);
        assert!(object_invariants("Foo", &o, &schemas).is_ok());
    }

    #[test]
    fn enum_ref_default_with_unknown_variant_is_rejected() {
        use bo4e_schemas::models::json_schema::ReferenceSchema;
        let prop = SchemaType::ReferenceSchema(ReferenceSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("MAGENTA".into())),
                ..TypeBase::default()
            },
            r#ref: "../enum/Color.json#".into(),
        });
        let o = obj(&[("color", prop)], &[]);
        let schemas = schemas_with_enum("Color", &["RED", "GREEN", "BLUE"]);
        match object_invariants("Foo", &o, &schemas) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("not a member"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }

    #[test]
    fn ref_default_to_missing_target_is_rejected() {
        use bo4e_schemas::models::json_schema::ReferenceSchema;
        let prop = SchemaType::ReferenceSchema(ReferenceSchema {
            base: TypeBase {
                default: Some(PrimitiveValue::String("X".into())),
                ..TypeBase::default()
            },
            r#ref: "../enum/Nonexistent.json#".into(),
        });
        let o = obj(&[("c", prop)], &[]);
        match object_invariants("Foo", &o, &empty_schemas()) {
            Err(Error::InconsistentSchema { reason, .. }) => {
                assert!(reason.contains("not found"), "got: {reason}");
            }
            other => panic!("expected InconsistentSchema, got {other:?}"),
        }
    }
}
