use crate::models::macros::{
    literal_enum, visitable_forwarded, visitable_forwarded_iter, visitable_leaf,
};
use crate::utils::visitable::Visitable;
use color_eyre::owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::iter;
use std::iter::empty;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeBase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

visitable_leaf!(TypeBase);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct SchemaRootTypeBase {
    #[serde(
        rename = "$defs",
        alias = "$definitions",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub defs: BTreeMap<String, SchemaClassType>,
}

visitable_leaf!(SchemaRootTypeBase);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ObjectSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeObject,

    pub additional_properties: bool,
    pub properties: BTreeMap<String, SchemaType>,
    pub required: Vec<String>,
}

literal_enum!(LiteralTypeObject, Object);
impl Visitable for ObjectSchema {
    fn sub_nodes(&self) -> Box<dyn Iterator<Item = &dyn Visitable> + '_> {
        Box::new(
            self.properties
                .values()
                .map(|schema_type| schema_type as &dyn Visitable),
        )
    }
    fn sub_nodes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Visitable> + '_> {
        Box::new(
            self.properties
                .values_mut()
                .map(|schema_type| schema_type as &mut dyn Visitable),
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct StrEnumSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeString,

    #[serde(rename = "enum")]
    pub enum_values: Vec<String>,
}

literal_enum!(LiteralTypeString, String);
visitable_leaf!(StrEnumSchema);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SchemaRootObject {
    #[serde(flatten)]
    pub base: SchemaRootTypeBase,
    #[serde(flatten)]
    pub object: ObjectSchema,
}

visitable_forwarded!(SchemaRootObject, object);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SchemaRootStrEnum {
    #[serde(flatten)]
    pub base: SchemaRootTypeBase,
    #[serde(flatten)]
    pub str_enum: StrEnumSchema,
}

visitable_forwarded!(SchemaRootStrEnum, str_enum);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ArraySchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeArray,

    pub items: Box<SchemaType>,
}

literal_enum!(LiteralTypeArray, Array);
visitable_forwarded!(ArraySchema, items);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AnyOfSchema {
    #[serde(flatten)]
    pub base: TypeBase,

    #[serde(rename = "anyOf")]
    pub any_of: Vec<SchemaType>,
}

visitable_forwarded_iter!(AnyOfSchema, any_of);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AllOfSchema {
    #[serde(flatten)]
    pub base: TypeBase,

    #[serde(rename = "allOf")]
    pub all_of: Vec<SchemaType>,
}

visitable_forwarded_iter!(AllOfSchema, all_of);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct StringSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeString,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<StringSchemaFormat>,
}

visitable_leaf!(StringSchema);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StringSchemaFormat {
    DateTime,
    Date,
    Time,
    Email,
    Hostname,
    Ipv4,
    Ipv6,
    Uri,
    UriReference,
    Iri,
    IriReference,
    Uuid,
    JsonPointer,
    RelativeJsonPointer,
    Regex,
    IdnEmail,
    IdnHostname,
    Binary,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ConstantSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeString,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<StringSchemaFormat>,
    #[serde(rename = "const")]
    pub constant: String,
}

visitable_leaf!(ConstantSchema);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct NumberSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeNumber,
}

literal_enum!(LiteralTypeNumber, Number);
visitable_leaf!(NumberSchema);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct DecimalSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeDecimal,

    pub format: LiteralFormatDecimal,
}

literal_enum!(LiteralFormatDecimal, Decimal);
visitable_leaf!(DecimalSchema);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LiteralTypeDecimal {
    Number,
    String,
}

impl Default for LiteralTypeDecimal {
    fn default() -> Self {
        LiteralTypeDecimal::Number
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct IntegerSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeInteger,
}

literal_enum!(LiteralTypeInteger, Integer);
visitable_leaf!(IntegerSchema);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct BooleanSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeBoolean,
}

literal_enum!(LiteralTypeBoolean, Boolean);
visitable_leaf!(BooleanSchema);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct NullSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeNull,
}

literal_enum!(LiteralTypeNull, Null);
visitable_leaf!(NullSchema);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct AnySchema {
    #[serde(flatten)]
    pub base: TypeBase,
}

visitable_leaf!(AnySchema);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct ReferenceSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    #[serde(rename = "$ref", default)]
    pub r#ref: String,
}

visitable_leaf!(ReferenceSchema);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum SchemaType {
    Object(ObjectSchema),
    StrEnum(StrEnumSchema),
    Array(ArraySchema),
    AnyOf(AnyOfSchema),
    AllOf(AllOfSchema),
    StringSchema(StringSchema),
    ConstantSchema(ConstantSchema),
    NumberSchema(NumberSchema),
    DecimalSchema(DecimalSchema),
    IntegerSchema(IntegerSchema),
    BooleanSchema(BooleanSchema),
    NullSchema(NullSchema),
    ReferenceSchema(ReferenceSchema),
    AnySchema(AnySchema),
}

impl Visitable for SchemaType {
    fn sub_nodes(&self) -> Box<dyn Iterator<Item = &dyn Visitable> + '_> {
        Box::new(iter::once(match self {
            SchemaType::Object(value) => value as &dyn Visitable,
            SchemaType::StrEnum(value) => value as &dyn Visitable,
            SchemaType::Array(value) => value as &dyn Visitable,
            SchemaType::AnyOf(value) => value as &dyn Visitable,
            SchemaType::AllOf(value) => value as &dyn Visitable,
            SchemaType::StringSchema(value) => value as &dyn Visitable,
            SchemaType::ConstantSchema(value) => value as &dyn Visitable,
            SchemaType::NumberSchema(value) => value as &dyn Visitable,
            SchemaType::DecimalSchema(value) => value as &dyn Visitable,
            SchemaType::IntegerSchema(value) => value as &dyn Visitable,
            SchemaType::BooleanSchema(value) => value as &dyn Visitable,
            SchemaType::NullSchema(value) => value as &dyn Visitable,
            SchemaType::ReferenceSchema(value) => value as &dyn Visitable,
            SchemaType::AnySchema(value) => value as &dyn Visitable,
        }))
    }
    fn sub_nodes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Visitable> + '_> {
        Box::new(iter::once(match self {
            SchemaType::Object(value) => value as &mut dyn Visitable,
            SchemaType::StrEnum(value) => value as &mut dyn Visitable,
            SchemaType::Array(value) => value as &mut dyn Visitable,
            SchemaType::AnyOf(value) => value as &mut dyn Visitable,
            SchemaType::AllOf(value) => value as &mut dyn Visitable,
            SchemaType::StringSchema(value) => value as &mut dyn Visitable,
            SchemaType::ConstantSchema(value) => value as &mut dyn Visitable,
            SchemaType::NumberSchema(value) => value as &mut dyn Visitable,
            SchemaType::DecimalSchema(value) => value as &mut dyn Visitable,
            SchemaType::IntegerSchema(value) => value as &mut dyn Visitable,
            SchemaType::BooleanSchema(value) => value as &mut dyn Visitable,
            SchemaType::NullSchema(value) => value as &mut dyn Visitable,
            SchemaType::ReferenceSchema(value) => value as &mut dyn Visitable,
            SchemaType::AnySchema(value) => value as &mut dyn Visitable,
        }))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum SchemaClassType {
    Object(ObjectSchema),
    StrEnum(StrEnumSchema),
}

impl Visitable for SchemaClassType {
    fn sub_nodes(&self) -> Box<dyn Iterator<Item = &dyn Visitable> + '_> {
        match self {
            SchemaClassType::Object(value) => value.sub_nodes(),
            SchemaClassType::StrEnum(value) => value.sub_nodes(),
        }
    }
    fn sub_nodes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Visitable> + '_> {
        match self {
            SchemaClassType::Object(value) => value.sub_nodes_mut(),
            SchemaClassType::StrEnum(value) => value.sub_nodes_mut(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum SchemaRootType {
    Object(SchemaRootObject),
    StrEnum(SchemaRootStrEnum),
}

impl Visitable for SchemaRootType {
    fn sub_nodes(&self) -> Box<dyn Iterator<Item = &dyn Visitable> + '_> {
        match self {
            SchemaRootType::Object(value) => value.sub_nodes(),
            SchemaRootType::StrEnum(value) => value.sub_nodes(),
        }
    }
    fn sub_nodes_mut(&mut self) -> Box<dyn Iterator<Item = &mut dyn Visitable> + '_> {
        match self {
            SchemaRootType::Object(value) => value.sub_nodes_mut(),
            SchemaRootType::StrEnum(value) => value.sub_nodes_mut(),
        }
    }
}

// Unittests for the JSON Schema models
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use std::cell::RefCell;
    use std::collections::HashSet;

    fn get_example_schema() -> SchemaRootObject {
        SchemaRootObject {
            base: Default::default(),
            object: ObjectSchema {
                base: TypeBase {
                    description: Some("A complex object schema".to_string()),
                    title: Some("ComplexObject".to_string()),
                },
                r#type: LiteralTypeObject::Object,
                additional_properties: true,
                properties: BTreeMap::from([
                    (
                        "property1".to_string(),
                        SchemaType::StringSchema(StringSchema {
                            base: TypeBase {
                                description: Some("First property".to_string()),
                                title: Some("Property1".to_string()),
                            },
                            r#type: LiteralTypeString::String,
                            format: None,
                        }),
                    ),
                    (
                        "property2".to_string(),
                        SchemaType::AnyOf(AnyOfSchema {
                            base: TypeBase {
                                description: Some("Second property".to_string()),
                                title: Some("Property2".to_string()),
                            },
                            any_of: vec![
                                SchemaType::IntegerSchema(Default::default()),
                                SchemaType::NullSchema(Default::default()),
                            ],
                        }),
                    ),
                    (
                        "property3".to_string(),
                        SchemaType::ReferenceSchema(ReferenceSchema {
                            base: TypeBase {
                                description: Some("Reference to something".to_string()),
                                title: Some("Property3".to_string()),
                            },
                            r#ref: "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/\
                            v202501.1.0-rc1/src/bo4e_schemas/bo/Geschaeftspartner.json"
                                .to_string(),
                        }),
                    ),
                    (
                        "property4".to_string(),
                        SchemaType::ReferenceSchema(ReferenceSchema {
                            base: TypeBase {
                                description: Some("Second property".to_string()),
                                title: Some("Property2".to_string()),
                            },
                            r#ref: "../bo/Geschaeftspartner.json".to_string(),
                        }),
                    ),
                ]),
                required: vec!["property1".to_string()],
            },
        }
    }

    #[test]
    fn test_complex_root_object_schema_serialization_roundtrip() {
        let schema = get_example_schema();

        let serialized = serde_json::to_string_pretty(&schema).unwrap();
        println!("{}", serialized);
        assert!(!serialized.contains("\"$defs\": {}"));
        assert!(serialized.contains("\"type\": \"object\""));
        assert!(serialized.contains("\"description\": \"A complex object schema\""));

        let deserialized: SchemaRootType = serde_json::from_str(&serialized).unwrap();
        let mut deserialized = match deserialized {
            SchemaRootType::Object(obj) => obj,
            _ => panic!("Deserialized type is not an ObjectSchema"),
        };
        assert_eq!(deserialized == schema, true);
        deserialized
            .object
            .properties
            .remove(&"property1".to_string());
        assert_eq!(deserialized != schema, true);
    }

    fn get_ref_strings(schema: &SchemaRootObject) -> HashSet<String> {
        let visitable_schema: &dyn Visitable = schema;

        let refs = RefCell::new(HashSet::new());
        let track_refs = |ref_schema: &ReferenceSchema| {
            refs.borrow_mut().insert(ref_schema.r#ref.clone());
        };
        visitable_schema.visit_by_type(&track_refs);
        refs.into_inner()
    }

    #[test]
    fn test_complex_root_object_visit_trait() {
        let schema = get_example_schema();
        let refs = get_ref_strings(&schema);

        println!("{}", serde_json::to_string_pretty(&refs).unwrap());
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_complex_root_object_visit_and_mutate() {
        let mut schema = get_example_schema();
        let visitable_schema: &mut dyn Visitable = &mut schema;

        let ref_online_regex = regex::Regex::new(
            "^https://raw\\.githubusercontent\\.com/BO4E/BO4E-Schemas/\
            (?P<version>[^/]+)/\
            src/bo4e_schemas/(?P<sub_path>(?:\\w+/)*)(?P<model>\\w+)\\.json#?$",
        )
        .unwrap();
        let mut transform_http_refs = |ref_schema: &mut ReferenceSchema| {
            ref_schema.r#ref = ref_online_regex
                .replace(&ref_schema.r#ref, "../${sub_path}${model}.json")
                .to_string();
        };
        visitable_schema.visit_by_type_mut(&mut transform_http_refs);
        let refs = get_ref_strings(&schema);

        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        assert_eq!(
            refs,
            HashSet::from([
                "../bo/Geschaeftspartner.json".to_string(),
                "../bo/Geschaeftspartner.json".to_string()
            ])
        );
    }
}
