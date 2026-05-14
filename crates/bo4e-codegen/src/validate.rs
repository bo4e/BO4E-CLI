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

    let required: BTreeSet<&str> = obj.required.iter().map(String::as_str).collect();
    for (prop_name, prop_schema) in &obj.properties {
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
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::models::json_schema::{
        LiteralTypeObject, PrimitiveValue, SchemaType, StringSchema, TypeBase,
    };
    use std::collections::BTreeMap;

    fn s_string_nullable_with_default(default: Option<PrimitiveValue>) -> SchemaType {
        SchemaType::StringSchema(StringSchema {
            base: TypeBase {
                default,
                ..TypeBase::default()
            },
            r#type: bo4e_schemas::models::json_schema::LiteralTypeString::String,
            format: None,
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
                s_string_nullable_with_default(Some(PrimitiveValue::Null)),
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
                s_string_nullable_with_default(Some(PrimitiveValue::String("X".into()))),
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
                s_string_nullable_with_default(Some(PrimitiveValue::Null)),
            )],
            &[],
        );
        assert!(object_invariants("Foo", &o).is_ok());
    }
}
