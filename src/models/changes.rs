use crate::models::json_schema::SchemaType;
use crate::models::schema_meta::Schemas;
use crate::models::version::DirtyVersion;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::PathBuf;

/// This enum class lists the different types of changes of a single change between two
/// BO4E versions.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChangeType {
    FieldAdded,
    FieldRemoved,
    FieldDefaultChanged,
    FieldDescriptionChanged,
    FieldTitleChanged,
    FieldCardinalityChanged,
    FieldReferenceChanged,
    FieldStringFormatChanged,
    FieldAnyOfTypeAdded,
    FieldAnyOfTypeRemoved,
    FieldAllOfTypeAdded,
    FieldAllOfTypeRemoved,
    FieldTypeChanged,
    ClassAdded,
    ClassRemoved,
    ClassDescriptionChanged,
    EnumValueAdded,
    EnumValueRemoved,
}

impl Display for ChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeType::FieldAdded => write!(f, "Field Added"),
            ChangeType::FieldRemoved => write!(f, "Field Removed"),
            ChangeType::FieldDefaultChanged => write!(f, "Field Default Changed"),
            ChangeType::FieldDescriptionChanged => write!(f, "Field Description Changed"),
            ChangeType::FieldTitleChanged => write!(f, "Field Title Changed"),
            ChangeType::FieldCardinalityChanged => write!(f, "Field Cardinality Changed"),
            ChangeType::FieldReferenceChanged => write!(f, "Field Reference Changed"),
            ChangeType::FieldStringFormatChanged => write!(f, "Field String Format Changed"),
            ChangeType::FieldAnyOfTypeAdded => write!(f, "Field AnyOf Type Added"),
            ChangeType::FieldAnyOfTypeRemoved => write!(f, "Field AnyOf Type Removed"),
            ChangeType::FieldAllOfTypeAdded => write!(f, "Field AllOf Type Added"),
            ChangeType::FieldAllOfTypeRemoved => write!(f, "Field AllOf Type Removed"),
            ChangeType::FieldTypeChanged => write!(f, "Field Type Changed"),
            ChangeType::ClassAdded => write!(f, "Class Added"),
            ChangeType::ClassRemoved => write!(f, "Class Removed"),
            ChangeType::ClassDescriptionChanged => write!(f, "Class Description Changed"),
            ChangeType::EnumValueAdded => write!(f, "Enum Value Added"),
            ChangeType::EnumValueRemoved => write!(f, "Enum Value Removed"),
        }
    }
}

/// The old or new value can be a SchemaType, PathBuf, or String
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ChangeValue {
    Schema(SchemaType),
    Path(PathBuf),
    String(String),
}

impl Display for ChangeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeValue::Schema(schema) => write!(f, "{:?}", schema),
            ChangeValue::Path(path) => write!(f, "{}", path.display()),
            ChangeValue::String(s) => write!(f, "{}", s),
        }
    }
}

/// This pydantic class models a single change between two BO4E versions.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub r#type: ChangeType,
    pub old: Option<ChangeValue>,
    pub new: Option<ChangeValue>,
    pub old_trace: String,
    pub new_trace: String,
}

impl Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {} -> {}",
            self.r#type,
            self.old
                .as_ref()
                .map_or("None".to_string(), |v| v.to_string()),
            self.new
                .as_ref()
                .map_or("None".to_string(), |v| v.to_string())
        )
    }
}

/// This pydantic class models the changes between two BO4E versions.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Changes {
    pub old_schemas: Schemas,
    pub new_schemas: Schemas,
    pub changes: Vec<Change>,
}

impl Changes {
    /// Returns the old version of the changes.
    pub fn old_version(&self) -> &DirtyVersion {
        &self.old_schemas.version
    }

    /// Returns the new version of the changes.
    pub fn new_version(&self) -> &DirtyVersion {
        &self.new_schemas.version
    }
}
