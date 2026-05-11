pub(crate) mod add;
pub(crate) mod non_nullable;
pub(crate) mod update_refs;

/// Full-match equivalent of Python's `re.fullmatch`: returns `true` only if
/// the entire string `s` is consumed by the pattern.
pub(crate) fn is_fullmatch(pattern: &regex::Regex, s: &str) -> bool {
    pattern
        .find(s)
        .is_some_and(|m| m.start() == 0 && m.end() == s.len())
}
