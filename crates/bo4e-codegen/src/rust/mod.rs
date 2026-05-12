//! Rust output generators.

#[cfg(feature = "rust-plain")]
pub mod plain;

#[cfg(feature = "rust-crate")]
pub mod crate_;

/// Reserved Rust keywords (current + reserved-for-future) that a field name
/// must not equal. Drives [`rust_field_name`]'s keyword-escape branch.
#[allow(dead_code)] // used by plain/crate_ renderers added in later tasks
pub(crate) const RUST_RESERVED: &[&str] = &[
    // Keywords
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", // Reserved
    "abstract", "become", "box", "do", "final", "macro", "override", "priv", "try", "typeof",
    "unsized", "virtual", "yield",
];

/// Translate a JSON property name into a valid Rust struct field identifier,
/// returning `(rust_name, needs_serde_rename)`.
///
/// Rules:
/// 1. Strip a single leading `_` (BO4E uses `_id`, `_typ`, `_version`).
/// 2. snake_case the result (via [`crate::naming::to_snake_case`]).
/// 3. If the snake_case result is a Rust keyword/reserved word, append `_`.
///
/// `needs_serde_rename` is `true` whenever the JSON original cannot be
/// recovered from the Rust name via `#[serde(rename_all = "camelCase")]`
/// alone — i.e. when:
///   - the original had a leading underscore, OR
///   - the name was keyword-escaped, OR
///   - the original wasn't pure camelCase (had digits, hyphens, etc. that survived).
#[allow(dead_code)] // used by plain/crate_ renderers added in later tasks
pub(crate) fn rust_field_name(json_name: &str) -> (String, bool) {
    let leading_underscore = json_name.starts_with('_');
    let stripped = json_name.strip_prefix('_').unwrap_or(json_name);
    let snake = crate::naming::to_snake_case(stripped);
    let (final_name, was_escaped) = if RUST_RESERVED.contains(&snake.as_str()) {
        (format!("{snake}_"), true)
    } else {
        (snake.clone(), false)
    };

    // Detect whether `final_name` round-trips to `json_name` via camelCase.
    // The cheap, correct test: rebuild camelCase from `snake` and compare.
    let camel_back = snake_to_camel(&snake);
    let camel_matches = camel_back == json_name;

    let needs_rename = leading_underscore || was_escaped || !camel_matches;
    (final_name, needs_rename)
}

#[allow(dead_code)] // used by rust_field_name which is used in later tasks
fn snake_to_camel(snake: &str) -> String {
    let mut out = String::with_capacity(snake.len());
    let mut next_upper = false;
    for c in snake.chars() {
        if c == '_' {
            next_upper = true;
            continue;
        }
        if next_upper {
            out.extend(c.to_uppercase());
            next_upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_name_simple_camel_case_no_rename() {
        let (n, r) = rust_field_name("angebotsdatum");
        assert_eq!(n, "angebotsdatum");
        assert!(!r);
    }

    #[test]
    fn field_name_mixed_case_round_trips_via_camel() {
        let (n, r) = rust_field_name("coErgaenzung");
        assert_eq!(n, "co_ergaenzung");
        assert!(
            !r,
            "snake-camel roundtrip should not need an explicit rename"
        );
    }

    #[test]
    fn field_name_leading_underscore_needs_rename() {
        let (n, r) = rust_field_name("_id");
        assert_eq!(n, "id");
        assert!(r);

        let (n, r) = rust_field_name("_typ");
        assert_eq!(n, "typ");
        assert!(r);

        let (n, r) = rust_field_name("_version");
        assert_eq!(n, "version");
        assert!(r);
    }

    #[test]
    fn field_name_keyword_clash_appends_underscore_and_renames() {
        let (n, r) = rust_field_name("type");
        assert_eq!(n, "type_");
        assert!(r);

        let (n, r) = rust_field_name("loop");
        assert_eq!(n, "loop_");
        assert!(r);
    }

    #[test]
    fn snake_to_camel_basic() {
        assert_eq!(snake_to_camel("co_ergaenzung"), "coErgaenzung");
        assert_eq!(snake_to_camel("api_version"), "apiVersion");
        assert_eq!(snake_to_camel("plain"), "plain");
    }
}
