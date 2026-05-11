use bo4e_schemas::models::json_schema::{SchemaRootObject, SchemaRootStrEnum, SchemaType};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// A `{ "$ref": "path" }` pointer used inside config fields.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SchemaRef {
    #[serde(rename = "$ref")]
    pub path: String,
}

/// An entry in `additionalFields` — either a concrete definition or a file reference.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum AdditionalFieldOrRef {
    Field(AdditionalField),
    Reference(SchemaRef),
}

/// A field that is added to the schema.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalField {
    #[serde(with = "serde_regex")]
    pub pattern: Regex,
    pub field_name: String,
    pub field_def: SchemaType,
}

impl PartialEq for AdditionalField {
    fn eq(&self, other: &Self) -> bool {
        self.pattern.as_str() == other.pattern.as_str()
            && self.field_name == other.field_name
            && self.field_def == other.field_def
    }
}

/// An enum item that is added to the schema.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalEnumItem {
    #[serde(with = "serde_regex")]
    pub pattern: Regex,
    pub items: Vec<String>,
}

impl PartialEq for AdditionalEnumItem {
    fn eq(&self, other: &Self) -> bool {
        self.pattern.as_str() == other.pattern.as_str() && self.items == other.items
    }
}

/// The inline schema or a file reference for an additional model.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum SchemaRootTypeOrReference {
    SchemaRootObject(SchemaRootObject),
    SchemaRootStrEnum(SchemaRootStrEnum),
    Reference(SchemaRef),
}

/// A model that is added to the schema.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalModel {
    pub module: String, // "bo", "com", or "enum"
    pub schema: SchemaRootTypeOrReference,
}

/// The config file model.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default, with = "serde_regex")]
    pub non_nullable_fields: Vec<Regex>,
    #[serde(default)]
    pub additional_fields: Vec<AdditionalFieldOrRef>,
    #[serde(default)]
    pub additional_enum_items: Vec<AdditionalEnumItem>,
    #[serde(default)]
    pub additional_models: Vec<AdditionalModel>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_deserializes_additional_field_ref() {
        let json = r#"{
            "additionalFields": [
                { "$ref": "./some_fields.json" }
            ]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.additional_fields.len(), 1);
        assert!(matches!(
            &config.additional_fields[0],
            AdditionalFieldOrRef::Reference(r) if r.path == "./some_fields.json"
        ));
    }

    #[test]
    fn test_config_deserializes_concrete_additional_field() {
        let json = r#"{
            "additionalFields": [
                {
                    "pattern": "bo\\..*",
                    "fieldName": "foo",
                    "fieldDef": { "type": "string" }
                }
            ]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(matches!(
            &config.additional_fields[0],
            AdditionalFieldOrRef::Field(_)
        ));
    }

    #[test]
    fn test_config_empty_deserializes() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert!(config.additional_fields.is_empty());
        assert!(config.additional_enum_items.is_empty());
        assert!(config.additional_models.is_empty());
        assert!(config.non_nullable_fields.is_empty());
    }
}
