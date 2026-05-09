use crate::models::macros::{
    literal_enum, visitable_dispatch_enum, visitable_forwarded_iter, visitable_leaf,
};
use crate::visitable::Visitable;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A primitive JSON value used for schema `default` fields.
/// Only null, bool, integer, float, and string are permitted.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum PrimitiveValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct TypeBase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<PrimitiveValue>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct SchemaRootTypeBase {
    #[serde(
        rename = "$defs",
        alias = "$definitions",
        default,
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub defs: BTreeMap<String, SchemaClassType>,
}

/// Default value for JSON-Schema `additionalProperties`, which is `true`
/// when the keyword is omitted (per the JSON-Schema specification).
fn default_additional_properties() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ObjectSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeObject,

    #[serde(
        rename = "additionalProperties",
        default = "default_additional_properties"
    )]
    pub additional_properties: bool,
    #[serde(default)]
    pub properties: BTreeMap<String, SchemaType>,
    #[serde(default)]
    pub required: Vec<String>,
}

literal_enum!(LiteralTypeObject, Object);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StrEnumSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeString,

    #[serde(rename = "enum")]
    pub enum_values: Vec<String>,
}

literal_enum!(LiteralTypeString, String);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SchemaRootObject {
    #[serde(flatten)]
    pub base: SchemaRootTypeBase,
    #[serde(flatten)]
    pub object: ObjectSchema,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SchemaRootStrEnum {
    #[serde(flatten)]
    pub base: SchemaRootTypeBase,
    #[serde(flatten)]
    pub str_enum: StrEnumSchema,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ArraySchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeArray,

    pub items: Box<SchemaType>,
}

literal_enum!(LiteralTypeArray, Array);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AnyOfSchema {
    #[serde(flatten)]
    pub base: TypeBase,

    #[serde(rename = "anyOf")]
    pub any_of: Vec<SchemaType>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AllOfSchema {
    #[serde(flatten)]
    pub base: TypeBase,

    #[serde(rename = "allOf")]
    pub all_of: Vec<SchemaType>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ConstantSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeString,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<StringSchemaFormat>,
    #[serde(rename = "const")]
    pub constant: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct NumberSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeNumber,
}

literal_enum!(LiteralTypeNumber, Number);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct DecimalSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeDecimal,

    pub format: LiteralFormatDecimal,
}

literal_enum!(LiteralFormatDecimal, Decimal);

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct IntegerSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeInteger,
}

literal_enum!(LiteralTypeInteger, Integer);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct BooleanSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeBoolean,
}

literal_enum!(LiteralTypeBoolean, Boolean);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct NullSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    pub r#type: LiteralTypeNull,
}

literal_enum!(LiteralTypeNull, Null);

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct AnySchema {
    #[serde(flatten)]
    pub base: TypeBase,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ReferenceSchema {
    #[serde(flatten)]
    pub base: TypeBase,
    #[serde(rename = "$ref", default)]
    pub r#ref: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum SchemaClassType {
    Object(ObjectSchema),
    StrEnum(StrEnumSchema),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum SchemaRootType {
    Object(SchemaRootObject),
    StrEnum(SchemaRootStrEnum),
}

// ── Leaf types (no schema children) ──────────────────────────────────────────
visitable_leaf!(TypeBase);
visitable_leaf!(SchemaRootTypeBase);
visitable_leaf!(StrEnumSchema);
visitable_leaf!(StringSchema);
visitable_leaf!(ConstantSchema);
visitable_leaf!(NumberSchema);
visitable_leaf!(DecimalSchema);
visitable_leaf!(IntegerSchema);
visitable_leaf!(BooleanSchema);
visitable_leaf!(NullSchema);
visitable_leaf!(AnySchema);
visitable_leaf!(ReferenceSchema);

// ── Collection types ──────────────────────────────────────────────────────────
visitable_forwarded_iter!(AnyOfSchema, any_of);
visitable_forwarded_iter!(AllOfSchema, all_of);

// ── ObjectSchema: children are its property values ────────────────────────────
impl Visitable for ObjectSchema {
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
        for value in self.properties.values() {
            f(value);
        }
    }
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
        for value in self.properties.values_mut() {
            f(value);
        }
    }
}

// ── ArraySchema: single child is the boxed item type ─────────────────────────
impl Visitable for ArraySchema {
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
        f(&*self.items);
    }
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
        f(&mut *self.items);
    }
}

// ── Root types: inner schema + any inline $defs ───────────────────────────────
impl Visitable for SchemaRootObject {
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
        f(&self.object);
        for value in self.base.defs.values() {
            f(value);
        }
    }
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
        f(&mut self.object);
        for value in self.base.defs.values_mut() {
            f(value);
        }
    }
}

impl Visitable for SchemaRootStrEnum {
    fn for_each_child(&self, f: &mut dyn FnMut(&dyn Visitable)) {
        f(&self.str_enum);
        for value in self.base.defs.values() {
            f(value);
        }
    }
    fn for_each_child_mut(&mut self, f: &mut dyn FnMut(&mut dyn Visitable)) {
        f(&mut self.str_enum);
        for value in self.base.defs.values_mut() {
            f(value);
        }
    }
}

// ── Enum wrappers: dispatch to the single inner value ────────────────────────
visitable_dispatch_enum!(
    SchemaType,
    Object,
    StrEnum,
    Array,
    AnyOf,
    AllOf,
    StringSchema,
    ConstantSchema,
    NumberSchema,
    DecimalSchema,
    IntegerSchema,
    BooleanSchema,
    NullSchema,
    ReferenceSchema,
    AnySchema,
);
visitable_dispatch_enum!(SchemaClassType, Object, StrEnum);
visitable_dispatch_enum!(SchemaRootType, Object, StrEnum);

// Unittests for the JSON Schema models
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use std::collections::HashSet;

    fn get_example_schema() -> SchemaRootObject {
        SchemaRootObject {
            base: Default::default(),
            object: ObjectSchema {
                base: TypeBase {
                    description: Some("A complex object schema".to_string()),
                    title: Some("ComplexObject".to_string()),
                    default: None,
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
                                default: None,
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
                                default: None,
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
                                default: None,
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
                                default: None,
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
        let mut refs = HashSet::new();
        (schema as &dyn Visitable).visit_all::<ReferenceSchema>(&mut |r| {
            refs.insert(r.r#ref.clone());
        });
        refs
    }

    #[test]
    fn test_complex_root_object_visit_trait() {
        let schema = get_example_schema();
        let refs = get_ref_strings(&schema);
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_primitive_value_roundtrip() {
        let cases: &[(&str, PrimitiveValue)] = &[
            ("null", PrimitiveValue::Null),
            ("true", PrimitiveValue::Bool(true)),
            ("42", PrimitiveValue::Integer(42)),
            ("3.14", PrimitiveValue::Float(3.14)),
            ("\"hello\"", PrimitiveValue::String("hello".into())),
        ];
        for (json, expected) in cases {
            let v: PrimitiveValue = serde_json::from_str(json).unwrap();
            assert_eq!(v, *expected);
            let back = serde_json::to_string(expected).unwrap();
            assert_eq!(back, *json);
        }
    }

    #[test]
    fn test_typebase_default_absent_not_emitted() {
        let base = TypeBase { description: None, title: None, default: None };
        let json = serde_json::to_string(&base).unwrap();
        assert!(!json.contains("default"), "absent default must not appear in JSON");
    }

    #[test]
    fn test_typebase_default_null_emitted() {
        let base = TypeBase { description: None, title: None, default: Some(PrimitiveValue::Null) };
        let json = serde_json::to_string(&base).unwrap();
        assert!(json.contains("\"default\":null"));
    }

    #[test]
    fn test_typebase_default_string_roundtrip() {
        let base = TypeBase {
            description: None,
            title: None,
            default: Some(PrimitiveValue::String("v202401.1.0".into())),
        };
        let json = serde_json::to_string(&base).unwrap();
        let back: TypeBase = serde_json::from_str(&json).unwrap();
        assert_eq!(back.default, base.default);
    }

    #[test]
    fn parses_object_schema_without_required() {
        let raw = r#"{"type":"object","properties":{},"additionalProperties":true}"#;
        let parsed: SchemaRootType = serde_json::from_str(raw).unwrap();
        let SchemaRootType::Object(root) = parsed else {
            panic!("expected SchemaRootType::Object");
        };
        assert!(root.object.required.is_empty());
    }

    #[test]
    fn parses_object_schema_without_required_or_additional_properties() {
        let raw = r#"{"type":"object","properties":{}}"#;
        let parsed: SchemaRootType = serde_json::from_str(raw).unwrap();
        let SchemaRootType::Object(root) = parsed else {
            panic!("expected SchemaRootType::Object");
        };
        // JSON-Schema spec: additionalProperties defaults to true when omitted.
        assert!(
            root.object.additional_properties,
            "additionalProperties default must be true per JSON-Schema spec"
        );
        assert!(root.object.required.is_empty());
    }

    #[test]
    fn parses_object_schema_without_properties() {
        // Neither `properties` nor `required` nor `additionalProperties` set.
        let raw = r#"{"type":"object"}"#;
        let parsed: SchemaRootType = serde_json::from_str(raw).unwrap();
        let SchemaRootType::Object(root) = parsed else {
            panic!("expected SchemaRootType::Object");
        };
        assert!(root.object.properties.is_empty());
        assert!(root.object.required.is_empty());
        assert!(root.object.additional_properties);
    }

    #[test]
    fn test_complex_root_object_visit_and_mutate() {
        let mut schema = get_example_schema();

        let ref_online_regex = regex::Regex::new(
            "^https://raw\\.githubusercontent\\.com/BO4E/BO4E-Schemas/\
            (?P<version>[^/]+)/\
            src/bo4e_schemas/(?P<sub_path>(?:\\w+/)*)(?P<model>\\w+)\\.json#?$",
        )
        .unwrap();

        ((&mut schema) as &mut dyn Visitable).visit_all_mut::<ReferenceSchema>(&mut |r| {
            r.r#ref = ref_online_regex
                .replace(&r.r#ref, "../${sub_path}${model}.json")
                .to_string();
        });

        let refs = get_ref_strings(&schema);
        assert_eq!(
            refs,
            HashSet::from(["../bo/Geschaeftspartner.json".to_string()])
        );
    }
}
