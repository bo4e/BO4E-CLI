//! Pure naming conversions used by all output types.

/// Convert a JSON property name (typically camelCase) into snake_case for use as a
/// Python field name. Acronyms are treated as case-preserving runs.
/// `to_snake_case("marktlokationsId")` → `"marktlokations_id"`.
/// `to_snake_case("URL")` → `"url"`.
/// `to_snake_case("APIVersion")` → `"api_version"`.
pub fn to_snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_is_lower_or_digit =
                i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let next_is_lower = i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase();
            let prev_is_upper = i > 0 && chars[i - 1].is_ascii_uppercase();
            // Insert underscore before an uppercase that begins a new word:
            // either after a lower/digit, or when an acronym ends and a new
            // capitalised word begins (UPPER followed by Upper+lower).
            if i > 0 && (prev_is_lower_or_digit || (prev_is_upper && next_is_lower)) {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Convert an identifier-shaped string (typically UPPER_SNAKE_CASE or sanitised
/// member-name shape) into PascalCase. Words are split on `_`; each word's
/// first character is uppercased and the rest are lowercased. A leading `_`
/// is preserved (the sanitiser uses it to escape digit-starters).
/// `to_pascal_case("ANGEBOT")` → `"Angebot"`.
/// `to_pascal_case("Z88_VERGLEICHSMESSUNG_GEEICHT_")` → `"Z88VergleichsmessungGeeicht"`.
/// `to_pascal_case("_2_01_7_001")` → `"_2_01_7_001"` (digit-starter prefix preserved).
pub fn to_pascal_case(name: &str) -> String {
    if name.is_empty() {
        return String::new();
    }
    let leading_underscore = name.starts_with('_')
        && name
            .chars()
            .nth(1)
            .is_some_and(|c| c.is_ascii_digit() || c == '_');
    if leading_underscore {
        return name.to_string();
    }
    let body = name;
    let mut out = String::with_capacity(name.len());
    for word in body.split('_').filter(|w| !w.is_empty()) {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            for c in chars {
                out.push(c.to_ascii_lowercase());
            }
        }
    }
    out
}

/// Make a string a valid C-family-style identifier.
///
/// Replaces any non-`[A-Za-z0-9_]` character with `_`. If the result starts
/// with a digit, a leading `_` is prepended. Used by enum-member naming for
/// both Python and Rust generators (sanitised member becomes a Python
/// identifier directly; for Rust it then feeds into `to_pascal_case`).
pub fn sanitize_member_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("_{cleaned}")
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_basic_camel_case() {
        assert_eq!(to_snake_case("marktlokationsId"), "marktlokations_id");
    }

    #[test]
    fn snake_case_pascal_case() {
        assert_eq!(to_snake_case("MarktLokation"), "markt_lokation");
    }

    #[test]
    fn snake_case_acronym_at_start() {
        assert_eq!(to_snake_case("APIVersion"), "api_version");
    }

    #[test]
    fn snake_case_all_caps_acronym_alone() {
        assert_eq!(to_snake_case("URL"), "url");
    }

    #[test]
    fn snake_case_already_snake_case_passthrough() {
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn snake_case_with_digits() {
        assert_eq!(to_snake_case("v2Version"), "v2_version");
    }

    #[test]
    fn pascal_case_from_upper_snake() {
        assert_eq!(to_pascal_case("ANGEBOT"), "Angebot");
        assert_eq!(to_pascal_case("BUENDELVERTRAG"), "Buendelvertrag");
    }

    #[test]
    fn pascal_case_from_mixed_underscore() {
        assert_eq!(
            to_pascal_case("Z88_VERGLEICHSMESSUNG_GEEICHT_"),
            "Z88VergleichsmessungGeeicht"
        );
    }

    #[test]
    fn pascal_case_with_leading_underscore() {
        assert_eq!(to_pascal_case("_2_01_7_001"), "_2_01_7_001");
    }

    #[test]
    fn pascal_case_already_pascal_pass_through() {
        assert_eq!(to_pascal_case("Angebot"), "Angebot");
    }

    #[test]
    fn pascal_case_empty_string() {
        assert_eq!(to_pascal_case(""), "");
    }

    #[test]
    fn sanitize_member_keeps_valid_identifiers() {
        assert_eq!(sanitize_member_name("STROM"), "STROM");
        assert_eq!(sanitize_member_name("Z85_REALER"), "Z85_REALER");
    }

    #[test]
    fn sanitize_member_replaces_hyphens_and_prefixes_digit_starts() {
        assert_eq!(sanitize_member_name("2-01-7-001"), "_2_01_7_001");
    }

    #[test]
    fn sanitize_member_replaces_parens() {
        assert_eq!(
            sanitize_member_name("Z88_VERGLEICHSMESSUNG(GEEICHT)"),
            "Z88_VERGLEICHSMESSUNG_GEEICHT_"
        );
    }
}
