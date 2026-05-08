use crate::models::changes::{Change, ChangeType};

/// Set of change types that are considered breaking. Mirrors Python `_is_critical_change`.
pub fn is_change_critical(change: &Change) -> bool {
    matches!(
        change.r#type,
        ChangeType::FieldRemoved
            | ChangeType::FieldTypeChanged
            | ChangeType::FieldCardinalityChanged
            | ChangeType::FieldReferenceChanged
            | ChangeType::FieldStringFormatChanged
            | ChangeType::FieldAnyOfTypeAdded
            | ChangeType::FieldAnyOfTypeRemoved
            | ChangeType::FieldAllOfTypeAdded
            | ChangeType::FieldAllOfTypeRemoved
            | ChangeType::ClassRemoved
            | ChangeType::EnumValueRemoved
    )
}

/// Returns true iff any change in the iterator is critical.
pub fn has_critical<'a, I: IntoIterator<Item = &'a Change>>(changes: I) -> bool {
    changes.into_iter().any(is_change_critical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::changes::{Change, ChangeType};

    fn ch(t: ChangeType) -> Change {
        Change {
            r#type: t,
            old: None,
            new: None,
            old_trace: String::new(),
            new_trace: String::new(),
        }
    }

    #[test]
    fn test_is_change_critical_full_table() {
        let cases: &[(ChangeType, bool)] = &[
            (ChangeType::FieldAdded, false),
            (ChangeType::FieldRemoved, true),
            (ChangeType::FieldDefaultChanged, false),
            (ChangeType::FieldDescriptionChanged, false),
            (ChangeType::FieldTitleChanged, false),
            (ChangeType::FieldCardinalityChanged, true),
            (ChangeType::FieldReferenceChanged, true),
            (ChangeType::FieldStringFormatChanged, true),
            (ChangeType::FieldAnyOfTypeAdded, true),
            (ChangeType::FieldAnyOfTypeRemoved, true),
            (ChangeType::FieldAllOfTypeAdded, true),
            (ChangeType::FieldAllOfTypeRemoved, true),
            (ChangeType::FieldTypeChanged, true),
            (ChangeType::ClassAdded, false),
            (ChangeType::ClassRemoved, true),
            (ChangeType::ClassDescriptionChanged, false),
            (ChangeType::EnumValueAdded, false),
            (ChangeType::EnumValueRemoved, true),
        ];
        for (t, expected) in cases {
            assert_eq!(is_change_critical(&ch(t.clone())), *expected, "{:?}", t);
        }
    }

    #[test]
    fn test_has_critical_finds_one() {
        let v = vec![
            ch(ChangeType::FieldAdded),
            ch(ChangeType::FieldRemoved),
            ch(ChangeType::FieldDescriptionChanged),
        ];
        assert!(has_critical(&v));
    }

    #[test]
    fn test_has_critical_returns_false_for_only_non_critical() {
        let v = vec![
            ch(ChangeType::FieldAdded),
            ch(ChangeType::FieldDescriptionChanged),
        ];
        assert!(!has_critical(&v));
    }

    #[test]
    fn test_has_critical_empty_is_false() {
        let v: Vec<Change> = vec![];
        assert!(!has_critical(&v));
    }
}
