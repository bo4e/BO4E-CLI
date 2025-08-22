use crate::models::json_schema::{SchemaRootObject, SchemaRootStrEnum, SchemaType};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// A field that is added to the schema
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
impl Eq for AdditionalField {}

/// An enum item that is added to the schema
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
impl Eq for AdditionalEnumItem {}

/// A model that is added to the schema
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalModel {
    pub module: String,                    // "bo", "com", or "enum"
    pub schema: SchemaRootTypeOrReference, // This can be a SchemaRootObject, SchemaRootStrEnum, or Reference
}

/// Represents the root type or reference for a schema.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum SchemaRootTypeOrReference {
    SchemaRootObject(SchemaRootObject),
    SchemaRootStrEnum(SchemaRootStrEnum),
    Reference(String), // Assuming Reference is a String for simplicity
}

/// The config file model
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default, with = "serde_regex")]
    pub non_nullable_fields: Vec<Regex>,
    #[serde(default)]
    pub additional_fields: Vec<AdditionalField>,
    #[serde(default)]
    pub additional_enum_items: Vec<AdditionalEnumItem>,
    #[serde(default)]
    pub additional_models: Vec<AdditionalModel>,
}

impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.non_nullable_fields.len() == other.non_nullable_fields.len()
            && Iterator::zip(
                self.non_nullable_fields.iter(),
                other.non_nullable_fields.iter(),
            )
            .all(|(a, b)| a.as_str() == b.as_str())
            && self.additional_fields == other.additional_fields
            && self.additional_enum_items == other.additional_enum_items
            && self.additional_models == other.additional_models
    }
}
impl Eq for Config {}
