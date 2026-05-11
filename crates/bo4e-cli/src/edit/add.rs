use crate::console::mark::pat;
use crate::models::config::{AdditionalEnumItem, AdditionalField};
use crate::{cprint_normal, cprint_verbose, cwarn};
use bo4e_schemas::models::json_schema::{SchemaRootType, SchemaType};
use bo4e_schemas::models::schema_meta::Schemas;

/// Insert additional fields into matching `SchemaRootObject` schemas.
pub fn transform_all_additional_fields(fields: &[AdditionalField], schemas: &mut Schemas) {
    for field in fields {
        let mut match_count = 0usize;
        for schema_rc in schemas.iter() {
            let module_path = schema_rc.borrow().module().join(".");
            if !super::is_fullmatch(&field.pattern, &module_path) {
                continue;
            }
            let mut schema = schema_rc.borrow_mut();
            let root = match schema.schema_mut() {
                Ok(r) => r,
                Err(e) => {
                    cwarn!("could not parse schema '{}': {}", module_path, e);
                    continue;
                }
            };
            if let SchemaRootType::Object(obj) = root {
                obj.object
                    .properties
                    .insert(field.field_name.clone(), field.field_def.clone());

                let has_default = field_def_has_default(&field.field_def);
                if !has_default && !obj.object.required.contains(&field.field_name) {
                    obj.object.required.push(field.field_name.clone());
                }

                match_count += 1;
                cprint_verbose!(
                    "Applied pattern '{}' to schema {}. Added field '{}'",
                    pat(&field.pattern),
                    module_path,
                    field.field_name
                );
            }
        }
        if match_count == 0 {
            cwarn!(
                "pattern '{}' did not match any schemas",
                pat(&field.pattern)
            );
        } else {
            cprint_normal!(
                "Pattern '{}' matched {} schema(s)",
                pat(&field.pattern),
                match_count
            );
        }
    }
}

fn field_def_has_default(schema_type: &SchemaType) -> bool {
    let base = match schema_type {
        SchemaType::StringSchema(s) => &s.base,
        SchemaType::IntegerSchema(s) => &s.base,
        SchemaType::NumberSchema(s) => &s.base,
        SchemaType::BooleanSchema(s) => &s.base,
        SchemaType::AnySchema(s) => &s.base,
        SchemaType::NullSchema(s) => &s.base,
        SchemaType::DecimalSchema(s) => &s.base,
        SchemaType::ConstantSchema(s) => &s.base,
        SchemaType::ReferenceSchema(s) => &s.base,
        SchemaType::Array(s) => &s.base,
        SchemaType::AnyOf(s) => &s.base,
        SchemaType::AllOf(s) => &s.base,
        SchemaType::Object(s) => &s.base,
        SchemaType::StrEnum(s) => &s.base,
    };
    base.default.is_some()
}

/// Extend enum values in matching `SchemaRootStrEnum` schemas.
pub fn transform_all_additional_enum_items(items: &[AdditionalEnumItem], schemas: &mut Schemas) {
    for item in items {
        let mut match_count = 0usize;
        for schema_rc in schemas.iter() {
            let module_path = schema_rc.borrow().module().join(".");
            if !super::is_fullmatch(&item.pattern, &module_path) {
                continue;
            }
            let mut schema = schema_rc.borrow_mut();
            let root = match schema.schema_mut() {
                Ok(r) => r,
                Err(e) => {
                    cwarn!("could not parse schema '{}': {}", module_path, e);
                    continue;
                }
            };
            if let SchemaRootType::StrEnum(e) = root {
                e.str_enum.enum_values.extend(item.items.iter().cloned());
                match_count += 1;
                cprint_verbose!(
                    "Applied pattern '{}' to schema {}. Added enum items: {:?}",
                    pat(&item.pattern),
                    module_path,
                    item.items
                );
            }
        }
        if match_count == 0 {
            cwarn!("pattern '{}' did not match any schemas", pat(&item.pattern));
        } else {
            cprint_normal!(
                "Pattern '{}' matched {} schema(s)",
                pat(&item.pattern),
                match_count
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{CONSOLE, Console, Level};
    use crate::models::config::{AdditionalEnumItem, AdditionalField};
    use bo4e_schemas::models::json_schema::*;
    use bo4e_schemas::models::schema_meta::{Schema, Schemas};
    use bo4e_schemas::models::version::DirtyVersion;
    use regex::Regex;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;
    use std::str::FromStr;

    fn init_console() {
        let _ = CONSOLE.set(Console::new(Level::Normal));
    }

    fn make_schemas() -> Schemas {
        let version = DirtyVersion::from_str("v202401.1.0").unwrap();
        let mut schemas = Schemas::new(version);

        let obj = SchemaRootType::Object(SchemaRootObject {
            base: Default::default(),
            object: ObjectSchema {
                base: TypeBase {
                    title: Some("Foo".into()),
                    ..Default::default()
                },
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: BTreeMap::new(),
                required: vec![],
            },
        });
        let schema_text = serde_json::to_string(&obj).unwrap();
        let mut schema = Schema::new(vec!["bo".into(), "Foo".into()], None).unwrap();
        schema.load_schema(schema_text);
        schemas.add_schema(Rc::new(RefCell::new(schema))).unwrap();

        let str_enum = SchemaRootType::StrEnum(SchemaRootStrEnum {
            base: Default::default(),
            str_enum: StrEnumSchema {
                base: TypeBase {
                    title: Some("Bar".into()),
                    ..Default::default()
                },
                r#type: LiteralTypeString::String,
                enum_values: vec!["X".into()],
            },
        });
        let enum_text = serde_json::to_string(&str_enum).unwrap();
        let mut schema2 = Schema::new(vec!["enum".into(), "Bar".into()], None).unwrap();
        schema2.load_schema(enum_text);
        schemas.add_schema(Rc::new(RefCell::new(schema2))).unwrap();

        schemas
    }

    #[test]
    fn test_transform_additional_fields_inserts_field() {
        init_console();
        let mut schemas = make_schemas();
        let field = AdditionalField {
            pattern: Regex::new(r"bo\.Foo").unwrap(),
            field_name: "myField".into(),
            field_def: SchemaType::StringSchema(Default::default()),
        };
        transform_all_additional_fields(&[field], &mut schemas);
        let schema_rc = schemas.get_by_name("Foo").unwrap();
        let mut schema = schema_rc.borrow_mut();
        let root = schema.schema_mut().unwrap();
        if let SchemaRootType::Object(obj) = root {
            assert!(obj.object.properties.contains_key("myField"));
        } else {
            panic!("Expected Object schema");
        }
    }

    #[test]
    fn test_transform_additional_fields_adds_to_required_when_no_default() {
        init_console();
        let mut schemas = make_schemas();
        let field = AdditionalField {
            pattern: Regex::new(r"bo\.Foo").unwrap(),
            field_name: "req".into(),
            field_def: SchemaType::StringSchema(Default::default()),
        };
        transform_all_additional_fields(&[field], &mut schemas);
        let schema_rc = schemas.get_by_name("Foo").unwrap();
        let mut schema = schema_rc.borrow_mut();
        let root = schema.schema_mut().unwrap();
        if let SchemaRootType::Object(obj) = root {
            assert!(obj.object.required.contains(&"req".to_string()));
        } else {
            panic!("Expected Object schema");
        }
    }

    #[test]
    fn test_transform_additional_fields_no_required_when_has_default() {
        init_console();
        let mut schemas = make_schemas();
        let mut string_schema = StringSchema::default();
        string_schema.base.default = Some(PrimitiveValue::String("x".into()));
        let field = AdditionalField {
            pattern: Regex::new(r"bo\.Foo").unwrap(),
            field_name: "opt".into(),
            field_def: SchemaType::StringSchema(string_schema),
        };
        transform_all_additional_fields(&[field], &mut schemas);
        let schema_rc = schemas.get_by_name("Foo").unwrap();
        let mut schema = schema_rc.borrow_mut();
        let root = schema.schema_mut().unwrap();
        if let SchemaRootType::Object(obj) = root {
            assert!(!obj.object.required.contains(&"opt".to_string()));
        } else {
            panic!("Expected Object schema");
        }
    }

    #[test]
    fn test_transform_additional_enum_items_extends_enum() {
        init_console();
        let mut schemas = make_schemas();
        let item = AdditionalEnumItem {
            pattern: Regex::new(r"enum\.Bar").unwrap(),
            items: vec!["Y".into(), "Z".into()],
        };
        transform_all_additional_enum_items(&[item], &mut schemas);
        let schema_rc = schemas.get_by_name("Bar").unwrap();
        let mut schema = schema_rc.borrow_mut();
        let root = schema.schema_mut().unwrap();
        if let SchemaRootType::StrEnum(e) = root {
            assert!(e.str_enum.enum_values.contains(&"Y".to_string()));
            assert!(e.str_enum.enum_values.contains(&"Z".to_string()));
        } else {
            panic!("Expected StrEnum schema");
        }
    }
}
