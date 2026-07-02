use crate::graph::link_template::{accessor_names_for, placeholder_names};
use clap_complete::CompletionCandidate;
use std::ffi::OsStr;

pub fn complete(prefix: &OsStr) -> Vec<CompletionCandidate> {
    let s = prefix.to_string_lossy().to_string();
    // Emit candidates only when the cursor is inside an unclosed `{`.
    let open = s.rfind('{');
    let close = s.rfind('}');
    let inside_open_brace = match (open, close) {
        (Some(o), Some(c)) => o > c,
        (Some(_), None) => true,
        _ => false,
    };
    if !inside_open_brace {
        return Vec::new();
    }
    let open_idx = open.unwrap();
    // `inner` is the text after the `{`. Once the user types a `.`, switch from
    // suggesting base placeholders to suggesting that placeholder's accessors —
    // both derived from the link-template spec, so completion and substitution
    // stay in lockstep.
    let inner = &s[open_idx + 1..];
    let tokens: Vec<String> = match inner.split_once('.') {
        None => placeholder_names()
            .filter(|name| name.starts_with(inner))
            .map(|name| format!("{{{name}}}"))
            .collect(),
        Some((base, acc_prefix)) => accessor_names_for(base)
            .into_iter()
            .filter(|acc| acc.starts_with(acc_prefix))
            .map(|acc| format!("{{{base}.{acc}}}"))
            .collect(),
    };
    tokens
        .into_iter()
        .map(|token| {
            // Replace from the `{` onward with the token so the shell
            // substitutes the whole placeholder.
            let mut replaced = s[..open_idx].to_string();
            replaced.push_str(&token);
            CompletionCandidate::new(replaced)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn names(cs: Vec<CompletionCandidate>) -> Vec<String> {
        cs.iter()
            .map(|c| c.get_value().to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn no_candidates_when_outside_brace() {
        assert!(complete(&OsString::from("https://example.com/")).is_empty());
        assert!(complete(&OsString::from("https://x.com/{pkg}/")).is_empty());
    }

    #[test]
    fn all_placeholders_for_bare_opening_brace() {
        let n = names(complete(&OsString::from("https://x.com/{")));
        assert!(n.iter().any(|s| s.ends_with("{pkg}")));
        assert!(n.iter().any(|s| s.ends_with("{class}")));
        assert!(n.iter().any(|s| s.ends_with("{namespace}")));
        assert!(n.iter().any(|s| s.ends_with("{output_dir}")));
        // No accessor candidates until a `.` is typed (check the token, not the
        // URL prefix which legitimately contains dots).
        assert!(
            !n.iter()
                .any(|s| s.rsplit_once('{').unwrap().1.contains('.'))
        );
    }

    #[test]
    fn prefix_filter_inside_brace() {
        let n = names(complete(&OsString::from("https://x.com/{cl")));
        assert!(n.iter().any(|s| s.ends_with("{class}")));
        assert!(!n.iter().any(|s| s.ends_with("{pkg}")));
    }

    #[test]
    fn dot_switches_to_accessor_candidates() {
        let n = names(complete(&OsString::from("https://x.com/{class.")));
        assert!(n.iter().any(|s| s.ends_with("{class.lower}")));
        assert!(n.iter().any(|s| s.ends_with("{class.upper}")));
        assert!(!n.iter().any(|s| s.ends_with("{class}")));
    }

    #[test]
    fn dot_accessor_prefix_filters() {
        let n = names(complete(&OsString::from("api/{cwd.r")));
        assert!(n.iter().any(|s| s.ends_with("{cwd.rel}")));
        assert!(!n.iter().any(|s| s.ends_with("{cwd.abs}")));
    }

    #[test]
    fn version_offers_no_accessors() {
        assert!(complete(&OsString::from("{version.")).is_empty());
    }
}
