use clap_complete::CompletionCandidate;
use std::ffi::OsStr;

const PLACEHOLDERS: &[&str] = &[
    "{pkg}", "{module}", "{class}", "{version}", "{cwd}", "{output_dir}",
];

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
    let partial = &s[open_idx..];
    PLACEHOLDERS
        .iter()
        .filter(|p| p.starts_with(partial))
        .map(|p| {
            // Replace from the `{` onward with the placeholder so the
            // shell substitutes the whole token.
            let mut replaced = s[..open_idx].to_string();
            replaced.push_str(p);
            CompletionCandidate::new(replaced)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn names(cs: Vec<CompletionCandidate>) -> Vec<String> {
        cs.iter().map(|c| c.get_value().to_string_lossy().to_string()).collect()
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
        assert!(n.iter().any(|s| s.ends_with("{output_dir}")));
    }

    #[test]
    fn prefix_filter_inside_brace() {
        let n = names(complete(&OsString::from("https://x.com/{cl")));
        assert!(n.iter().any(|s| s.ends_with("{class}")));
        assert!(!n.iter().any(|s| s.ends_with("{pkg}")));
    }
}
