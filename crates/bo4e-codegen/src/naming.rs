//! Pure naming conversions used by all output types.

/// Lower-case the schema's last module segment to form its Python module file name.
/// `module_file_name(&["bo", "Angebot"])` → `"angebot"`.
pub fn module_file_name(module: &[String]) -> String {
    module
        .last()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_file_name_lowercases_last_segment() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        assert_eq!(module_file_name(&m), "angebot");
    }

    #[test]
    fn module_file_name_handles_single_segment() {
        let m = vec!["Typ".to_string()];
        assert_eq!(module_file_name(&m), "typ");
    }

    #[test]
    fn module_file_name_handles_already_lowercase() {
        let m = vec!["enum".to_string(), "marktrolle".to_string()];
        assert_eq!(module_file_name(&m), "marktrolle");
    }

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
}
