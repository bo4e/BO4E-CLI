use crate::models::changes::{Change, ChangeType};
use regex::Regex;

/// Drop every change whose `old_trace` matches any `old` pattern or whose
/// `new_trace` matches any `new` pattern. Patterns are partial (`is_match`)
/// regexes over the `/`-separated trace path. Retains the rest in place.
pub fn filter_ignored_traces(changes: &mut Vec<Change>, old: &[&Regex], new: &[&Regex]) {
    changes.retain(|c| {
        !(old.iter().any(|r| r.is_match(&c.old_trace))
            || new.iter().any(|r| r.is_match(&c.new_trace)))
    });
}

/// Set of change types that are considered breaking. Mirrors Python `_is_critical_change`.
pub fn is_change_critical(change: &Change) -> bool {
    matches!(
        change.r#type,
        ChangeType::FieldRemoved
            | ChangeType::FieldTypeChanged
            | ChangeType::FieldConstantChanged
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
            (ChangeType::FieldConstantChanged, true),
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

    fn ch_trace(old_trace: &str, new_trace: &str) -> Change {
        Change {
            r#type: ChangeType::FieldDefaultChanged,
            old: None,
            new: None,
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        }
    }

    #[test]
    fn test_filter_ignored_traces_matches_either_side() {
        let ver = Regex::new(r"/_version$").unwrap();
        let mut changes = vec![
            ch_trace("/bo/Angebot/_version", "/bo/Angebot/_version"),
            ch_trace("/bo/Angebot/preis", "/bo/Angebot/preis"),
            // matched only on the new side
            ch_trace("/bo/Angebot", "/bo/Angebot/_version"),
        ];
        filter_ignored_traces(&mut changes, &[&ver], &[&ver]);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].new_trace, "/bo/Angebot/preis");
    }

    #[test]
    fn test_filter_ignored_traces_anchor_excludes_partial_segment() {
        // `/_version$` must not match a field like `bar_version`.
        let ver = Regex::new(r"/_version$").unwrap();
        let mut changes = vec![ch_trace(
            "/bo/Angebot/bar_version",
            "/bo/Angebot/bar_version",
        )];
        filter_ignored_traces(&mut changes, &[&ver], &[&ver]);
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_filter_ignored_traces_empty_patterns_keep_everything() {
        let mut changes = vec![ch_trace("/a", "/a"), ch_trace("/b", "/b")];
        filter_ignored_traces(&mut changes, &[], &[]);
        assert_eq!(changes.len(), 2);
    }
}
