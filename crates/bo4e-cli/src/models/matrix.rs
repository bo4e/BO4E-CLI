use bimap::BiMap;
use bo4e_schemas::models::version::DirtyVersion;
use indexmap::IndexMap;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

lazy_static! {
    pub static ref COMPATIBILITY_SYMBOLS: BiMap<CompatibilitySymbol, String> = BiMap::from_iter([
        (CompatibilitySymbol::ChangeNone,        "\u{1F7E2}".to_string()), // 🟢
        (CompatibilitySymbol::ChangeNonCritical, "\u{1F7E1}".to_string()), // 🟡
        (CompatibilitySymbol::ChangeCritical,    "\u{1F534}".to_string()), // 🔴
        (CompatibilitySymbol::NonExistent,       "-".to_string()),
        (CompatibilitySymbol::Added,             "\u{2795}".to_string()),  // ➕
        (CompatibilitySymbol::Removed,           "\u{2796}".to_string()),  // ➖
    ]);

    pub static ref COMPATIBILITY_TEXTS: BiMap<CompatibilityText, String> = BiMap::from_iter([
        (CompatibilityText::ChangeNone,        "none".to_string()),
        (CompatibilityText::ChangeNonCritical, "non-critical".to_string()),
        (CompatibilityText::ChangeCritical,    "critical".to_string()),
        (CompatibilityText::NonExistent,       "non-existent".to_string()),
        (CompatibilityText::Added,             "added".to_string()),
        (CompatibilityText::Removed,           "removed".to_string()),
    ]);
}

/// Emoji rendering of a compatibility cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompatibilitySymbol {
    ChangeNone,
    ChangeNonCritical,
    ChangeCritical,
    NonExistent,
    Added,
    Removed,
}

impl Display for CompatibilitySymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            COMPATIBILITY_SYMBOLS
                .get_by_left(self)
                .ok_or(std::fmt::Error)?
        )
    }
}

impl Serialize for CompatibilitySymbol {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(COMPATIBILITY_SYMBOLS.get_by_left(self).ok_or_else(|| {
            serde::ser::Error::custom(format!("Unknown compatibility symbol: {:?}", self))
        })?)
    }
}

impl<'de> Deserialize<'de> for CompatibilitySymbol {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        COMPATIBILITY_SYMBOLS
            .get_by_right(&s)
            .copied()
            .ok_or_else(|| serde::de::Error::custom(format!("Unknown compatibility symbol: {}", s)))
    }
}

/// Plain-text rendering of a compatibility cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompatibilityText {
    ChangeNone,
    ChangeNonCritical,
    ChangeCritical,
    NonExistent,
    Added,
    Removed,
}

impl Display for CompatibilityText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            COMPATIBILITY_TEXTS
                .get_by_left(self)
                .ok_or(std::fmt::Error)?
        )
    }
}

impl Serialize for CompatibilityText {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(COMPATIBILITY_TEXTS.get_by_left(self).ok_or_else(|| {
            serde::ser::Error::custom(format!("Unknown compatibility text: {:?}", self))
        })?)
    }
}

impl<'de> Deserialize<'de> for CompatibilityText {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        COMPATIBILITY_TEXTS
            .get_by_right(&s)
            .copied()
            .ok_or_else(|| serde::de::Error::custom(format!("Unknown compatibility text: {}", s)))
    }
}

/// Either an emoji or a textual rendering. (De)serializes untagged: emoji first, text second.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Compatibility {
    Symbol(CompatibilitySymbol),
    Text(CompatibilityText),
}

impl Display for Compatibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Compatibility::Symbol(s) => Display::fmt(s, f),
            Compatibility::Text(t) => Display::fmt(t, f),
        }
    }
}

/// A single entry of the compatibility matrix: one (prev → next) cell for one module.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityMatrixEntry {
    pub previous_version: DirtyVersion,
    pub next_version: DirtyVersion,
    pub compatibility: Compatibility,
}

/// Module name → row of (prev, next, compatibility) entries. `IndexMap` preserves insertion order.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityMatrix {
    #[serde(flatten, default)]
    pub root: IndexMap<String, Vec<CompatibilityMatrixEntry>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::models::version::DirtyVersion;

    #[test]
    fn test_compatibility_symbol_roundtrip_emoji() {
        for sym in [
            CompatibilitySymbol::ChangeNone,
            CompatibilitySymbol::ChangeNonCritical,
            CompatibilitySymbol::ChangeCritical,
            CompatibilitySymbol::NonExistent,
            CompatibilitySymbol::Added,
            CompatibilitySymbol::Removed,
        ] {
            let s = serde_json::to_string(&sym).unwrap();
            let back: CompatibilitySymbol = serde_json::from_str(&s).unwrap();
            assert_eq!(sym, back);
        }
    }

    #[test]
    fn test_compatibility_text_roundtrip() {
        for t in [
            CompatibilityText::ChangeNone,
            CompatibilityText::ChangeNonCritical,
            CompatibilityText::ChangeCritical,
            CompatibilityText::NonExistent,
            CompatibilityText::Added,
            CompatibilityText::Removed,
        ] {
            let s = serde_json::to_string(&t).unwrap();
            let back: CompatibilityText = serde_json::from_str(&s).unwrap();
            assert_eq!(t, back);
        }
    }

    #[test]
    fn test_compatibility_serializes_emoji_then_text() {
        let c_emoji = Compatibility::Symbol(CompatibilitySymbol::Added);
        assert_eq!(serde_json::to_string(&c_emoji).unwrap(), "\"\u{2795}\"");

        let c_text = Compatibility::Text(CompatibilityText::Added);
        assert_eq!(serde_json::to_string(&c_text).unwrap(), "\"added\"");
    }

    #[test]
    fn test_compatibility_deserialize_tries_emoji_first() {
        let parsed: Compatibility = serde_json::from_str("\"\u{2795}\"").unwrap();
        assert!(matches!(
            parsed,
            Compatibility::Symbol(CompatibilitySymbol::Added)
        ));

        let parsed_text: Compatibility = serde_json::from_str("\"added\"").unwrap();
        assert!(matches!(
            parsed_text,
            Compatibility::Text(CompatibilityText::Added)
        ));
    }

    #[test]
    fn test_compatibility_matrix_preserves_module_order() {
        let v: DirtyVersion = "v202401.0.1".parse().unwrap();
        let entry = CompatibilityMatrixEntry {
            previous_version: v.clone(),
            next_version: v.clone(),
            compatibility: Compatibility::Symbol(CompatibilitySymbol::ChangeNone),
        };
        let mut m = CompatibilityMatrix {
            root: IndexMap::new(),
        };
        m.root.insert("bo.Angebot".to_string(), vec![entry.clone()]);
        m.root.insert("com.Adresse".to_string(), vec![entry]);

        let json = serde_json::to_string(&m).unwrap();
        let pos_bo = json.find("bo.Angebot").unwrap();
        let pos_com = json.find("com.Adresse").unwrap();
        assert!(
            pos_bo < pos_com,
            "module insertion order must survive serialization"
        );
    }
}
