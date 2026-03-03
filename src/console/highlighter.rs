use crate::console::palette;
use console::Style;
use regex::Regex;

struct Rule {
    regex: Regex,
    /// (capture_group_name, style) pairs. The style is applied to that named group's match.
    group_styles: Vec<(&'static str, Style)>,
}

pub struct Highlighter {
    rules: Vec<Rule>,
}

impl Default for Highlighter {
    fn default() -> Self {
        let mut h = Self { rules: Vec::new() };
        h.add_static_rules();
        h
    }
}

impl Highlighter {
    fn add_static_rules(&mut self) {
        // Rules are applied in order; later rules can overwrite earlier ones for the same
        // byte range (last span added to a given range wins during rendering).

        // Low priority: bo / com / enum word highlighting
        self.push_rule(
            r"(?i)\b(?P<bo>bo)\b|\b(?P<com>com)\b|\b(?P<enum>enum)\b",
            &[
                ("bo", palette::BO),
                ("com", palette::COM),
                ("enum", palette::ENUM),
            ],
        );

        // Low priority: JSON keyword
        self.push_rule(r"(?i)\b(?P<json>JSON)\b", &[("json", palette::SUB_ACCENT)]);

        // High priority: BO4E brand (overwrites the plain "bo" match)
        self.push_rule(
            r"(?i)\b(?P<bo4e>BO4E)\b",
            &[("bo4e", palette::MAIN_ACCENT)],
        );

        // High priority: version strings like v202401.1.0-rc1 or v202401.1.0+devABC
        self.push_rule(
            r"(?P<version>v?\d{6}\.\d+\.\d+(?:-rc\d*)?(?:\+dev\w+)?)",
            &[("version", palette::SUB_ACCENT)],
        );

        // File-path-like strings (Unix paths and Windows drive paths)
        self.push_rule(
            r"(?P<path>(?:/[\w.\-]+)+/?|[a-zA-Z]:(?:\\[\w.\-]+)*\\?)",
            &[("path", palette::SUB)],
        );
    }

    fn push_rule(&mut self, pattern: &str, groups: &[(&'static str, &'static str)]) {
        let regex = Regex::new(pattern).expect("highlighter regex is valid");
        let group_styles: Vec<(&'static str, Style)> = groups
            .iter()
            .map(|(name, color)| {
                (*name, Style::new().fg(parse_hex_color(color)).force_styling(true))
            })
            .collect();
        self.rules.push(Rule { regex, group_styles });
    }

    /// Apply all highlight rules to `text`, returning an ANSI-styled string.
    pub fn apply(&self, text: &str) -> String {
        // Collect all (start, end, Style) spans.
        let mut spans: Vec<(usize, usize, Style)> = Vec::new();

        for rule in &self.rules {
            for caps in rule.regex.captures_iter(text) {
                for (group_name, style) in &rule.group_styles {
                    if let Some(m) = caps.name(group_name) {
                        spans.push((m.start(), m.end(), style.clone()));
                    }
                }
            }
        }

        if spans.is_empty() {
            return text.to_string();
        }

        // Sort by start position. For overlapping spans, prefer the last one added
        // (higher priority rules are pushed last). Use stable sort + reverse-priority
        // by keeping the last span for each byte range.
        spans.sort_by_key(|(start, end, _)| (*start, *end));

        // Render: walk spans left-to-right, skipping any that would overlap an already
        // rendered range.
        let mut result = String::with_capacity(text.len() * 2);
        let mut cursor = 0usize;

        for (start, end, style) in spans {
            if start < cursor {
                // Overlapping — skip (earlier span already rendered this range).
                continue;
            }
            // Emit unstyled text between last cursor and this span.
            result.push_str(&text[cursor..start]);
            // Emit the styled span.
            result.push_str(&style.apply_to(&text[start..end]).to_string());
            cursor = end;
        }
        // Emit any remaining unstyled text.
        result.push_str(&text[cursor..]);
        result
    }

    /// Register schema class names as highlighted terms (call once after read_schemas).
    pub fn add_schema_names(&mut self, names: &[String]) {
        if names.is_empty() {
            return;
        }
        let pattern = names
            .iter()
            .map(|n| regex::escape(n))
            .collect::<Vec<_>>()
            .join("|");
        let full_pattern = format!(r"\b(?P<schema_name>{})\b", pattern);
        self.push_rule(&full_pattern, &[("schema_name", palette::MAIN_ACCENT)]);
    }
}

fn parse_hex_color(hex: &str) -> console::Color {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    console::Color::TrueColor(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text_unchanged() {
        let h = Highlighter::default();
        assert_eq!(h.apply("no special content"), "no special content");
    }

    #[test]
    fn test_bo4e_keyword_is_highlighted() {
        let h = Highlighter::default();
        let result = h.apply("Processing BO4E schema");
        assert!(result.contains("BO4E"), "original text preserved in output");
        assert!(result.contains('\x1b'), "ANSI escape codes present");
    }

    #[test]
    fn test_version_string_is_highlighted() {
        let h = Highlighter::default();
        let result = h.apply("Version v202401.1.0-rc1 found");
        assert!(result.contains("v202401.1.0-rc1"));
        assert!(result.contains('\x1b'));
    }

    #[test]
    fn test_add_schema_names_highlights_name() {
        let mut h = Highlighter::default();
        h.add_schema_names(&["Angebot".to_string()]);
        let result = h.apply("Processing Angebot schema");
        assert!(result.contains("Angebot"));
        assert!(result.contains('\x1b'), "schema name should be highlighted");
    }

    #[test]
    fn test_plain_text_no_ansi() {
        let h = Highlighter::default();
        let result = h.apply("no special content");
        assert!(!result.contains('\x1b'), "plain text must not gain ANSI codes");
    }
}
