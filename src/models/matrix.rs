use crate::models::version::DirtyVersion;
use bimap::BiMap;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Debug, Display};

lazy_static! {
    pub static ref COMPATIBILITY_SYMBOLS: BiMap<CompatibilitySymbol, String> = BiMap::from_iter([
        (CompatibilitySymbol::Unchanged, "🟢".to_string()),
        (CompatibilitySymbol::NonCriticalChange, "🟡".to_string()),
        (CompatibilitySymbol::CriticalChange, "🔴".to_string()),
        (CompatibilitySymbol::NonExistentModel, "-".to_string()),
        (CompatibilitySymbol::AddedModel, "➕".to_string()),
        (CompatibilitySymbol::RemovedModel, "➖".to_string()),
        (CompatibilitySymbol::ReprUnchanged, "none".to_string()),
        (
            CompatibilitySymbol::ReprNonCriticalChange,
            "non-critical".to_string()
        ),
        (
            CompatibilitySymbol::ReprCriticalChange,
            "critical".to_string()
        ),
        (
            CompatibilitySymbol::ReprNonExistentModel,
            "non-existent".to_string()
        ),
        (CompatibilitySymbol::ReprAddedModel, "added".to_string()),
        (CompatibilitySymbol::ReprRemovedModel, "removed".to_string()),
    ]);
}

/// This enum class lists the different symbols of changes in the compatibility matrix.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompatibilitySymbol {
    Unchanged,
    NonCriticalChange,
    CriticalChange,
    NonExistentModel,
    AddedModel,
    RemovedModel,
    ReprUnchanged,
    ReprNonCriticalChange,
    ReprCriticalChange,
    ReprNonExistentModel,
    ReprAddedModel,
    ReprRemovedModel,
}

impl Display for CompatibilitySymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            COMPATIBILITY_SYMBOLS
                .get_by_left(self)
                .ok_or_else(|| std::fmt::Error {})?
        )
    }
}

impl Serialize for CompatibilitySymbol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(COMPATIBILITY_SYMBOLS.get_by_left(self).ok_or_else(|| {
            serde::ser::Error::custom(format!("Unknown compatibility symbol: {:?}", self))
        })?)
    }
}

impl<'de> Deserialize<'de> for CompatibilitySymbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        COMPATIBILITY_SYMBOLS
            .get_by_right(&s)
            .cloned()
            .ok_or_else(|| serde::de::Error::custom(format!("Unknown compatibility symbol: {}", s)))
    }
}

/// This class models a single entry in the compatibility matrix.
/// It contains the compatibility status and the versions related to this change entry.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityMatrixEntry {
    pub previous_version: DirtyVersion,
    pub next_version: DirtyVersion,
    pub compatibility: CompatibilitySymbol,
}

/// This class models the compatibility matrix of BO4E versions.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityMatrix {
    #[serde(flatten, default)]
    pub root: HashMap<String, Vec<CompatibilityMatrixEntry>>,
}
