use crate::models::macros::literal_enum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TypeBase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SchemaRootTypeBase {
    #[serde(
        rename = "$defs",
        alias = "$definitions",
        default,
        skip_serializing_if = "HashMap::is_empty"
    )]
    pub defs: HashMap<String, SchemaClassType>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ObjectSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeObject,

    pub additional_properties: bool,
    pub properties: HashMap<String, SchemaType>,
    pub required: Vec<String>,
}

literal_enum!(LiteralTypeObject, Object);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct StrEnumSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeString,

    #[serde(rename = "enum")]
    pub enum_values: Vec<String>,
}

literal_enum!(LiteralTypeString, String);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SchemaRootObject {
    #[serde(flatten)]
    pub base: SchemaRootTypeBase,
    #[serde(flatten)]
    pub object: ObjectSchema,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SchemaRootStrEnum {
    #[serde(flatten)]
    pub base: SchemaRootTypeBase,
    #[serde(flatten)]
    pub str_enum: StrEnumSchema,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ArraySchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeArray,

    pub items: Box<SchemaType>,
}

literal_enum!(LiteralTypeArray, Array);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AnyOfSchema {
    #[serde(flatten)]
    pub base: TypeBase,

    #[serde(rename = "anyOf")]
    pub any_of: Vec<SchemaType>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AllOfSchema {
    #[serde(flatten)]
    pub base: TypeBase,

    #[serde(rename = "allOf")]
    pub all_of: Vec<SchemaType>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct StringSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeString,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<StringSchemaFormat>,
}

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NumberSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeNumber,
}

literal_enum!(LiteralTypeNumber, Number);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DecimalSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeDecimal,

    pub format: LiteralFormatDecimal,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LiteralTypeDecimal {
    Number,
    String,
}

literal_enum!(LiteralFormatDecimal, Decimal);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct IntegerSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeInteger,
}

literal_enum!(LiteralTypeInteger, Integer);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct BooleanSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeBoolean,
}

literal_enum!(LiteralTypeBoolean, Boolean);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NullSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeNull,
}

literal_enum!(LiteralTypeNull, Null);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AnySchema {
    #[serde(flatten)]
    pub base: TypeBase,
}

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
    AnySchema(AnySchema),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum SchemaClassType {
    Object(ObjectSchema),
    StrEnum(StrEnumSchema),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum SchemaRootType {
    Object(SchemaRootObject),
    StrEnum(SchemaRootStrEnum),
}

// Unittests for the JSON Schema models
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_complex_root_object_schema_serialization_roundtrip() {
        let schema = SchemaRootObject {
            base: SchemaRootTypeBase {
                defs: HashMap::new(),
            },
            object: ObjectSchema {
                base: TypeBase {
                    description: Some("A complex object schema".to_string()),
                    title: Some("ComplexObject".to_string()),
                },
                r#type: LiteralTypeObject::Object,
                additional_properties: true,
                properties: HashMap::from([
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
                        SchemaType::IntegerSchema(IntegerSchema {
                            base: TypeBase {
                                description: Some("Second property".to_string()),
                                title: Some("Property2".to_string()),
                            },
                            r#type: LiteralTypeInteger::Integer,
                        }),
                    ),
                ]),
                required: vec!["property1".to_string()],
            },
        };

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
}
