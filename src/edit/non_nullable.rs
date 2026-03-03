use crate::models::json_schema::{PrimitiveValue, SchemaRootObject, SchemaRootType, SchemaType, TypeBase};
use crate::models::schema_meta::Schemas;
use crate::{cprint, cprint_verbose};

/// Remove the `null` variant from a nullable `AnyOf` property.
///
/// Returns `Err` if:
/// - The property does not exist.
/// - The property is not `SchemaType::AnyOf`.
/// - The `AnyOf` contains no `SchemaType::NullSchema` variant.
pub fn field_to_non_nullable(
    schema: &mut SchemaRootObject,
    field_name: &str,
) -> Result<(), String> {
    let prop = schema
        .object
        .properties
        .get_mut(field_name)
        .ok_or_else(|| format!("Field '{}' not found", field_name))?;

    let any_of_schema = match prop {
        SchemaType::AnyOf(a) => a,
        other => {
            return Err(format!(
                "Expected AnyOf for field '{}', got {:?}",
                field_name, other
            ))
        }
    };

    // Remove the first NullSchema variant.
    let null_pos = any_of_schema
        .any_of
        .iter()
        .position(|v| matches!(v, SchemaType::NullSchema(_)))
        .ok_or_else(|| format!("Field '{}' AnyOf contains no NullSchema", field_name))?;
    any_of_schema.any_of.remove(null_pos);

    // If the default was explicitly null, remove it and add field to required.
    let had_null_default = any_of_schema.base.default == Some(PrimitiveValue::Null);
    if had_null_default {
        any_of_schema.base.default = None;
        if !schema.object.required.contains(&field_name.to_string()) {
            schema.object.required.push(field_name.to_string());
        }
    }

    // Flatten to single type when only one variant remains.
    if any_of_schema.any_of.len() == 1 {
        let inherited_base = any_of_schema.base.clone();
        // We need to get the inner SchemaType out — re-borrow the property
        let prop_mut = schema.object.properties.get_mut(field_name).expect("field was proven to exist above");
        if let SchemaType::AnyOf(a) = prop_mut {
            let inner = a.any_of.remove(0);
            let new_prop = apply_base_to_schema_type(inner, inherited_base);
            *prop_mut = new_prop;
        }
    }

    Ok(())
}

fn apply_base_to_schema_type(mut schema_type: SchemaType, base: TypeBase) -> SchemaType {
    let inner_base = match &mut schema_type {
        SchemaType::StringSchema(s) => &mut s.base,
        SchemaType::IntegerSchema(s) => &mut s.base,
        SchemaType::NumberSchema(s) => &mut s.base,
        SchemaType::BooleanSchema(s) => &mut s.base,
        SchemaType::AnySchema(s) => &mut s.base,
        SchemaType::NullSchema(s) => &mut s.base,
        SchemaType::DecimalSchema(s) => &mut s.base,
        SchemaType::ConstantSchema(s) => &mut s.base,
        SchemaType::ReferenceSchema(s) => &mut s.base,
        SchemaType::Array(s) => &mut s.base,
        SchemaType::AnyOf(s) => &mut s.base,
        SchemaType::AllOf(s) => &mut s.base,
        SchemaType::Object(s) => &mut s.base,
        SchemaType::StrEnum(s) => &mut s.base,
    };
    if inner_base.title.is_none() {
        inner_base.title = base.title;
    }
    if inner_base.description.is_none() {
        inner_base.description = base.description;
    }
    if inner_base.default.is_none() {
        inner_base.default = base.default;
    }
    schema_type
}

/// Apply non-nullable patterns to all matching fields across all schemas.
pub fn transform_all_non_nullable_fields(
    patterns: &[regex::Regex],
    schemas: &mut Schemas,
) -> Result<(), String> {
    // Collect (field_path, field_name, module) triples up-front to avoid re-iteration.
    let mut triples: Vec<(String, String, Vec<String>)> = Vec::new();
    for schema_rc in schemas.iter() {
        let mut schema = schema_rc.borrow_mut();
        let module = schema.module().to_vec();
        let module_path = module.join(".");
        let root = match schema.schema_mut() {
            Ok(r) => r,
            Err(e) => {
                cprint!("Warning: could not parse schema '{}': {}", module_path, e);
                continue;
            }
        };
        if let SchemaRootType::Object(obj) = root {
            let prefix = module.join(".");
            for field_name in obj.object.properties.keys() {
                let field_path = format!("{}.{}", prefix, field_name);
                triples.push((field_path, field_name.clone(), module.clone()));
            }
        }
    }

    for pattern in patterns {
        let mut match_count = 0usize;
        for (field_path, field_name, module) in &triples {
            if super::is_fullmatch(pattern, field_path) {
                let schema_rc = schemas
                    .get_by_module(module)
                    .ok_or_else(|| format!("Schema not found for module {:?}", module))?;
                let mut schema = schema_rc.borrow_mut();
                let root = schema.schema_mut()?;
                if let SchemaRootType::Object(obj) = root {
                    let is_anyof_with_null = obj
                        .object
                        .properties
                        .get(field_name)
                        .is_some_and(|p| {
                            matches!(p, SchemaType::AnyOf(a) if
                                a.any_of.iter().any(|v| matches!(v, SchemaType::NullSchema(_)))
                            )
                        });
                    if is_anyof_with_null {
                        field_to_non_nullable(obj, field_name)?;
                        match_count += 1;
                        cprint_verbose!(
                            "Applied non-nullable pattern '{}' to field {}",
                            pattern,
                            field_path
                        );
                    }
                }
            }
        }
        if match_count == 0 {
            cprint!("Warning: non-nullable pattern '{}' did not match any fields", pattern);
        } else {
            cprint!("Applied non-nullable pattern '{}' to {} field(s)", pattern, match_count);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{Console, CONSOLE};
    use crate::models::json_schema::*;
    use std::collections::BTreeMap;

    fn init_console() {
        let _ = CONSOLE.set(Console::new(false));
    }

    fn make_nullable_object(field_name: &str) -> SchemaRootObject {
        SchemaRootObject {
            base: Default::default(),
            object: ObjectSchema {
                base: Default::default(),
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: BTreeMap::from([(
                    field_name.to_string(),
                    SchemaType::AnyOf(AnyOfSchema {
                        base: TypeBase {
                            default: Some(PrimitiveValue::Null),
                            ..Default::default()
                        },
                        any_of: vec![
                            SchemaType::StringSchema(Default::default()),
                            SchemaType::NullSchema(Default::default()),
                        ],
                    }),
                )]),
                required: vec![],
            },
        }
    }

    #[test]
    fn test_field_to_non_nullable_removes_null_variant() {
        init_console();
        let mut schema = make_nullable_object("name");
        field_to_non_nullable(&mut schema, "name").unwrap();
        let prop = schema.object.properties.get("name").unwrap();
        // Should be a bare StringSchema now (flattened)
        assert!(matches!(prop, SchemaType::StringSchema(_)));
    }

    #[test]
    fn test_field_to_non_nullable_removes_null_default() {
        init_console();
        let mut schema = make_nullable_object("name");
        field_to_non_nullable(&mut schema, "name").unwrap();
        // field moved to required since null default was removed
        assert!(schema.object.required.contains(&"name".to_string()));
    }

    #[test]
    fn test_field_to_non_nullable_keeps_non_null_default() {
        init_console();
        let field_name = "count";
        let mut schema = SchemaRootObject {
            base: Default::default(),
            object: ObjectSchema {
                base: Default::default(),
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: BTreeMap::from([(
                    field_name.to_string(),
                    SchemaType::AnyOf(AnyOfSchema {
                        base: TypeBase {
                            default: Some(PrimitiveValue::Integer(0)),
                            ..Default::default()
                        },
                        any_of: vec![
                            SchemaType::IntegerSchema(Default::default()),
                            SchemaType::NullSchema(Default::default()),
                        ],
                    }),
                )]),
                required: vec![],
            },
        };
        field_to_non_nullable(&mut schema, field_name).unwrap();
        let prop = schema.object.properties.get(field_name).unwrap();
        // AnyOf flattened to IntegerSchema; default 0 preserved
        if let SchemaType::IntegerSchema(s) = prop {
            assert_eq!(s.base.default, Some(PrimitiveValue::Integer(0)));
        } else {
            panic!("Expected IntegerSchema, got {:?}", prop);
        }
        // Not added to required because default is still present
        assert!(!schema.object.required.contains(&field_name.to_string()));
    }

    #[test]
    fn test_field_to_non_nullable_preserves_multi_variant() {
        init_console();
        // AnyOf with 3 variants — removing null leaves 2, should not flatten.
        let field_name = "value";
        let mut schema = SchemaRootObject {
            base: Default::default(),
            object: ObjectSchema {
                base: Default::default(),
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: BTreeMap::from([(
                    field_name.to_string(),
                    SchemaType::AnyOf(AnyOfSchema {
                        base: TypeBase {
                            default: Some(PrimitiveValue::Null),
                            ..Default::default()
                        },
                        any_of: vec![
                            SchemaType::StringSchema(Default::default()),
                            SchemaType::IntegerSchema(Default::default()),
                            SchemaType::NullSchema(Default::default()),
                        ],
                    }),
                )]),
                required: vec![],
            },
        };
        field_to_non_nullable(&mut schema, field_name).unwrap();
        let prop = schema.object.properties.get(field_name).unwrap();
        // Still AnyOf (two remaining variants), not flattened
        assert!(matches!(prop, SchemaType::AnyOf(_)));
    }
}
