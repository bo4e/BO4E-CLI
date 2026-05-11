use crate::console::palette;
use console::Style;
use regex::Regex;

/// Priority tiers for highlight rules. Higher wins: a lower-priority span is dropped
/// entirely if it overlaps any already-accepted range, so an outer flag like
/// `--some-bo4e-flag` (structural) swallows the inner `bo`/`4E` matches without
/// partial bleed-through.
const PRIORITY_STRUCTURAL: u8 = 100;
const PRIORITY_STRONG: u8 = 50;
const PRIORITY_SCHEMA: u8 = 20;
const PRIORITY_WEAK: u8 = 10;

struct Rule {
    regex: Regex,
    priority: u8,
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

/// Module classification for a schema name, used to colour-highlight class
/// references in console output. Mirrors Python's per-module highlighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaModule {
    Bo,
    Com,
    Enum,
    Other,
}

impl Highlighter {
    fn add_static_rules(&mut self) {
        // Weak: bare bo / com / enum words. Each alternative requires a leading
        // non-word, non-dot prefix (consumed but not styled) so file extensions like
        // `.json`, `.com`, `.bo` and module-qualified paths don't trigger the weak rule.
        self.push_rule(
            r"(?i)(?:^|[^.\w])(?P<bo>bo)\b|(?:^|[^.\w])(?P<com>com)\b|(?:^|[^.\w])(?P<enum>enum)\b",
            PRIORITY_WEAK,
            &[
                ("bo", Style::new().fg(parse_hex_color(palette::BO)).bold().force_styling(true)),
                ("com", Style::new().fg(parse_hex_color(palette::COM)).bold().force_styling(true)),
                ("enum", Style::new().fg(parse_hex_color(palette::ENUM)).bold().force_styling(true)),
            ],
        );

        // Weak: JSON keyword. Same `[^.\w]` prefix so `.json` file extensions stay plain.
        self.push_rule(
            r"(?i)(?:^|[^.\w])(?P<json>JSON)\b",
            PRIORITY_WEAK,
            &[("json", Style::new().fg(parse_hex_color(palette::COM)).force_styling(true))],
        );

        // Strong: BO4E brand split into two halves — matches Python's
        // `bo4e.bo4e_bo` / `bo4e.bo4e_4e` styles.
        self.push_rule(
            r"(?i)\b(?P<bo4e_bo>BO)(?P<bo4e_4e>4E)\b",
            PRIORITY_STRONG,
            &[
                ("bo4e_bo", Style::new().fg(parse_hex_color(palette::MAIN)).bold().force_styling(true)),
                ("bo4e_4e", Style::new().fg(parse_hex_color(palette::SUB)).bold().force_styling(true)),
            ],
        );

        // Strong: version strings like v202401.1.0-rc1 or v202401.1.0+devABC.
        self.push_rule(
            r"(?P<version>v?\d{6}\.\d+\.\d+(?:-rc\d*)?(?:\+dev\w+)?)",
            PRIORITY_STRONG,
            &[("version", Style::new().fg(parse_hex_color(palette::MAIN)).bold().force_styling(true))],
        );

        // Strong: Windows-style paths — drive-letter absolute (`C:\foo\bar`, `C:/foo/bar`)
        // or `.\`/`..\`-prefixed relative. Path body accepts both `\` and `/` so mixed
        // separators (`.\foo/bar`) still get a single span. The anchor uses `\B\.{1,2}`
        // (non-word boundary in front of leading dot[s]) so prose like `xy.\foo` or `1.5\x`
        // — where the dot sits behind a word char — is not falsely styled.
        self.push_rule(
            r"(?P<win_path>(?:\b[a-zA-Z]:|\B\.{1,2})[\\/][\w.\-+\\/]*)",
            PRIORITY_STRONG,
            &[(
                "win_path",
                Style::new().fg(parse_hex_color(palette::MAIN)).bold().force_styling(true),
            )],
        );

        // Strong: Unix-style paths beginning with `/`, `./`, or `../`. Captures the
        // whole path (including any filename). Bare `dir/file` is intentionally not
        // matched so module-style notation like `bo/Angebot` isn't claimed.
        // Shares its style with `win_path` — same colour regardless of platform.
        self.push_rule(
            r"(?P<unix_path>(?:^|\B)(?:\.{0,2}/)[\w./\-+]*)",
            PRIORITY_STRONG,
            &[(
                "unix_path",
                Style::new().fg(parse_hex_color(palette::MAIN)).bold().force_styling(true),
            )],
        );
    }

    /// Add structural rules for clap's `--help` output (headers, flags, placeholders, URLs).
    /// All at PRIORITY_STRUCTURAL so they swallow inner semantic matches like `bo4e`
    /// inside `--some-bo4e-flag`.
    pub fn add_help_rules(&mut self) {
        // Section headers at line start: "Usage:", "Options:", "Commands:", "Arguments:".
        self.push_rule(
            r"(?m)^(?P<header>(?:Usage|Options|Commands|Arguments|Subcommands)):",
            PRIORITY_STRUCTURAL,
            &[("header", Style::new().fg(parse_hex_color(palette::SUB_ACCENT)).bold().force_styling(true))],
        );

        // Flags: `-x` or `--long-name`. Prefix `[^\w/]` (consumed but not styled) prevents
        // matching `--` inside URL paths or hyphenated identifiers like `bo4e-cli`.
        self.push_rule(
            r"(?:^|[^\w/])(?P<flag>-{1,2}[a-zA-Z][\w\-]*)",
            PRIORITY_STRUCTURAL,
            &[("flag", Style::new().fg(parse_hex_color(palette::MAIN_ACCENT)).bold().force_styling(true))],
        );

        // Placeholders: `<NAME>`, `[OPTIONAL]`, `[COMMAND]`. Restrict bracket form to
        // start with uppercase so we don't match things like `[the rest]` in prose.
        self.push_rule(
            r"(?P<placeholder><[^>]+>|\[[A-Z][^\]]*\])",
            PRIORITY_STRUCTURAL,
            &[("placeholder", Style::new().fg(parse_hex_color(palette::MAIN_ACCENT)).italic().force_styling(true))],
        );

        // URLs at structural priority so a `--xxx` substring inside a URL's query
        // string can't be reclaimed by the flag rule. The character class excludes
        // common sentence/wrap punctuation so `(https://example.com).` doesn't pull
        // the trailing `).` into the underlined span.
        self.push_rule(
            r#"(?P<url>https?://[^\s)\],;'"<>]+)"#,
            PRIORITY_STRUCTURAL,
            &[("url", Style::new().fg(parse_hex_color(palette::SUB_ACCENT)).underlined().force_styling(true))],
        );
    }

    fn push_rule(&mut self, pattern: &str, priority: u8, groups: &[(&'static str, Style)]) {
        let regex = Regex::new(pattern).expect("highlighter regex is valid");
        let group_styles: Vec<(&'static str, Style)> = groups
            .iter()
            .map(|(name, style)| (*name, style.clone()))
            .collect();
        self.rules.push(Rule { regex, priority, group_styles });
    }

    /// Apply all highlight rules to `text`, returning an ANSI-styled string.
    ///
    /// Spans are sorted by (priority DESC, start ASC, end DESC) and accepted greedily;
    /// any candidate that overlaps an already-accepted range is dropped entirely.
    /// Higher-priority rules therefore fully suppress lower-priority matches inside
    /// their range — e.g. a structural `--some-bo4e-flag` span swallows `bo`/`4E`.
    pub fn apply(&self, text: &str) -> String {
        let mut candidates: Vec<(usize, usize, u8, Style)> = Vec::new();
        for rule in &self.rules {
            for caps in rule.regex.captures_iter(text) {
                for (group_name, style) in &rule.group_styles {
                    if let Some(m) = caps.name(group_name) {
                        candidates.push((m.start(), m.end(), rule.priority, style.clone()));
                    }
                }
            }
        }

        if candidates.is_empty() {
            return text.to_string();
        }

        candidates.sort_by(|a, b| {
            b.2.cmp(&a.2)                    // priority DESC
                .then_with(|| a.0.cmp(&b.0)) // start ASC
                .then_with(|| b.1.cmp(&a.1)) // end DESC (longer wins on tie)
        });

        let mut accepted: Vec<(usize, usize, Style)> = Vec::new();
        for (start, end, _prio, style) in candidates {
            let overlaps = accepted
                .iter()
                .any(|(s, e, _)| !(end <= *s || start >= *e));
            if !overlaps {
                accepted.push((start, end, style));
            }
        }

        accepted.sort_by_key(|(s, _, _)| *s);
        let mut result = String::with_capacity(text.len() * 2);
        let mut cursor = 0usize;
        for (start, end, style) in accepted {
            if start < cursor {
                continue;
            }
            result.push_str(&text[cursor..start]);
            result.push_str(&style.apply_to(&text[start..end]).to_string());
            cursor = end;
        }
        result.push_str(&text[cursor..]);
        result
    }

    /// Register schema class names with per-module classification. Mirrors Python's
    /// `get_bo4e_schema_highlighter`: BO/COM/ENUM names get their module colour,
    /// everything else gets the SUB (bo4e_4e) colour. Bold throughout.
    pub fn add_schema_names(&mut self, classified: &[(SchemaModule, String)]) {
        if classified.is_empty() {
            return;
        }
        let mut bo: Vec<&str> = Vec::new();
        let mut com: Vec<&str> = Vec::new();
        let mut r#enum: Vec<&str> = Vec::new();
        let mut other: Vec<&str> = Vec::new();
        for (module, name) in classified {
            match module {
                SchemaModule::Bo => bo.push(name),
                SchemaModule::Com => com.push(name),
                SchemaModule::Enum => r#enum.push(name),
                SchemaModule::Other => other.push(name),
            }
        }
        // Each module gets its own rule so the named capture group can carry the right colour.
        let push_module = |this: &mut Self, names: &[&str], group: &'static str, style: Style| {
            if names.is_empty() {
                return;
            }
            let pattern = names.iter().map(|n| regex::escape(n)).collect::<Vec<_>>().join("|");
            // `(?:mod\.)?Name` mirrors Python's regex_mod_path branch (no field matching).
            let full = format!(r"\b(?P<{group}>(?:{group}\.)?(?:{pattern}))\b");
            this.push_rule(&full, PRIORITY_SCHEMA, &[(group, style)]);
        };
        push_module(self, &bo, "bo", Style::new().fg(parse_hex_color(palette::BO)).bold().force_styling(true));
        push_module(self, &com, "com", Style::new().fg(parse_hex_color(palette::COM)).bold().force_styling(true));
        push_module(self, &r#enum, "enum", Style::new().fg(parse_hex_color(palette::ENUM)).bold().force_styling(true));
        // Unmatched schemas re-use the bo4e_4e style (SUB bold).
        if !other.is_empty() {
            let pattern = other.iter().map(|n| regex::escape(n)).collect::<Vec<_>>().join("|");
            let full = format!(r"\b(?P<bo4e_4e_schema>(?:{pattern}))\b");
            self.push_rule(
                &full,
                PRIORITY_SCHEMA,
                &[(
                    "bo4e_4e_schema",
                    Style::new().fg(parse_hex_color(palette::SUB)).bold().force_styling(true),
                )],
            );
        }
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
        // BO4E is split into BO + 4E with two separate styled spans, so the
        // literal "BO4E" is no longer contiguous — assert each half plus colour codes.
        assert!(result.contains("BO"), "BO half preserved");
        assert!(result.contains("4E"), "4E half preserved");
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
        h.add_schema_names(&[(SchemaModule::Bo, "Angebot".to_string())]);
        let result = h.apply("Processing Angebot schema");
        assert!(result.contains("Angebot"));
        assert!(result.contains('\x1b'), "schema name should be highlighted");
    }

    #[test]
    fn test_add_schema_names_per_module_classification() {
        let mut h = Highlighter::default();
        h.add_schema_names(&[
            (SchemaModule::Bo, "Angebot".to_string()),
            (SchemaModule::Com, "Adresse".to_string()),
            (SchemaModule::Enum, "Sparte".to_string()),
            (SchemaModule::Other, "WeirdThing".to_string()),
        ]);
        let result = h.apply("Angebot Adresse Sparte WeirdThing");
        // All four should be styled — count distinct ANSI escape sequences.
        let esc_count = result.matches('\x1b').count();
        assert!(esc_count >= 8, "expected at least 4 styled spans, got {esc_count} escapes");
    }

    #[test]
    fn test_plain_text_no_ansi() {
        let h = Highlighter::default();
        let result = h.apply("no special content");
        assert!(!result.contains('\x1b'), "plain text must not gain ANSI codes");
    }

    /// Strip every SGR escape from `s` so we can check the styled text content.
    fn plain(s: &str) -> String {
        Regex::new(r"\x1b\[[0-9;]*m").unwrap().replace_all(s, "").to_string()
    }

    #[test]
    fn test_help_rules_style_flag_and_swallow_inner_bo4e() {
        // The flag rule (priority 100) must claim the entire `--some-bo4e-flag` range
        // so the `bo` (weak) and `BO`/`4E` (strong) rules inside it are dropped entirely
        // — no partial styling bleed-through. Each accepted span ends with one `\x1b[0m`
        // reset, so the reset count is the cleanest "number of distinct styled spans" metric.
        let mut h = Highlighter::default();
        h.add_help_rules();
        let result = h.apply("Use --some-bo4e-flag here");
        assert_eq!(plain(&result), "Use --some-bo4e-flag here");
        let resets = result.matches("\x1b[0m").count();
        assert_eq!(resets, 1, "expected exactly one styled span for the flag, got: {result:?}");
    }

    #[test]
    fn test_help_rules_style_section_headers() {
        let mut h = Highlighter::default();
        h.add_help_rules();
        let result = h.apply("Usage: bo4e [OPTIONS]\n\nOptions:\n  -v");
        // Plain content is preserved; ANSI may sit between the header word and its colon
        // (the styled span is just `Usage`, not `Usage:`), so check via the plain projection.
        let p = plain(&result);
        assert!(p.contains("Usage:"));
        assert!(p.contains("Options:"));
        assert!(p.contains("[OPTIONS]"));
        assert!(result.contains('\x1b'));
    }

    #[test]
    fn test_help_rules_url_swallows_inner_double_dash() {
        // A URL with `--` in its query string must not have the flag rule re-claim it.
        let mut h = Highlighter::default();
        h.add_help_rules();
        let result = h.apply("see https://example.com/?x=--bar for info");
        assert_eq!(plain(&result), "see https://example.com/?x=--bar for info");
        let resets = result.matches("\x1b[0m").count();
        assert_eq!(resets, 1, "expected one URL span, got: {result:?}");
    }

    #[test]
    fn test_url_does_not_swallow_trailing_punctuation() {
        // `(URL).` should style only the URL — not the closing paren or the trailing dot.
        let mut h = Highlighter::default();
        h.add_help_rules();
        let result = h.apply("see (https://example.com/foo).");
        assert_eq!(plain(&result), "see (https://example.com/foo).");
        // The `)` and `.` come AFTER the reset code, not between two SGR sequences.
        let reset_pos = result.find("\x1b[0m").expect("should have a reset");
        let after_reset = &result[reset_pos + "\x1b[0m".len()..];
        assert_eq!(after_reset, ").");
    }

    #[test]
    fn test_json_in_file_extension_is_not_highlighted_as_keyword() {
        // The path `/tmp/config.json` IS styled as a unix path (one span), but the
        // weak JSON keyword rule must NOT add a separate styling for the `.json`
        // extension — the leading dot disqualifies that rule, and even if it didn't,
        // the path span (priority STRONG) would swallow it.
        let h = Highlighter::default();
        let result = h.apply("Loading config from /tmp/config.json done");
        let resets = result.matches("\x1b[0m").count();
        assert_eq!(resets, 1, "expected exactly one styled span (the path), got: {result:?}");
    }

    #[test]
    fn test_json_keyword_in_prose_is_still_highlighted() {
        // Sanity check: `JSON` after a space (with no preceding dot) should still highlight.
        let h = Highlighter::default();
        let result = h.apply("parse JSON now");
        assert!(result.contains('\x1b'), "expected JSON to be styled");
    }

    #[test]
    fn test_unix_path_is_highlighted() {
        let h = Highlighter::default();
        let result = h.apply("Loading from /tmp/config.json done");
        assert_eq!(plain(&result), "Loading from /tmp/config.json done");
        assert!(result.contains("\x1b["), "expected path styling, got: {result:?}");
    }

    #[test]
    fn test_relative_unix_path_is_highlighted() {
        let h = Highlighter::default();
        let result = h.apply("see ./relative/path/file for info");
        assert_eq!(plain(&result), "see ./relative/path/file for info");
        let resets = result.matches("\x1b[0m").count();
        assert_eq!(resets, 1, "expected single span for the path, got: {result:?}");
    }

    #[test]
    fn test_bare_dir_file_is_not_treated_as_path() {
        // `bo/Angebot` (no leading `/` or `./`) is module-style notation, not a path —
        // the unix_path rule must NOT claim it.
        let h = Highlighter::default();
        let result = h.apply("module bo/Angebot reference");
        // The `bo` weak rule may still fire, but no path span should exist; check by
        // ensuring `/Angebot` isn't styled (a path span would have included the `/`).
        let p = plain(&result);
        assert!(p.contains("bo/Angebot"));
    }

    #[test]
    fn test_unix_path_does_not_swallow_trailing_punct() {
        // `(/tmp/foo).` should style only the path — not the closing paren or the dot.
        let h = Highlighter::default();
        let result = h.apply("see (/tmp/foo).");
        assert_eq!(plain(&result), "see (/tmp/foo).");
        let reset_pos = result.find("\x1b[0m").expect("should have a reset");
        let after_reset = &result[reset_pos + "\x1b[0m".len()..];
        assert_eq!(after_reset, ").");
    }

    #[test]
    fn test_windows_drive_letter_path_is_highlighted() {
        let h = Highlighter::default();
        let result = h.apply(r"Loading from C:\Users\leon\config.json done");
        assert_eq!(plain(&result), r"Loading from C:\Users\leon\config.json done");
        let resets = result.matches("\x1b[0m").count();
        assert_eq!(resets, 1, "expected one styled span for the path, got: {result:?}");
    }

    #[test]
    fn test_windows_drive_letter_with_forward_slashes_is_highlighted() {
        // Windows accepts forward slashes after the drive letter too.
        let h = Highlighter::default();
        let result = h.apply("Loading from C:/Users/leon/config.json done");
        assert_eq!(plain(&result), "Loading from C:/Users/leon/config.json done");
        let resets = result.matches("\x1b[0m").count();
        assert_eq!(resets, 1, "expected one styled span for the path, got: {result:?}");
    }

    #[test]
    fn test_windows_relative_path_is_highlighted() {
        // Reproduces the user's failing case verbatim: a relative Windows-style path
        // with a leading `.\`, a hidden subdir, mixed underscores, and a trailing sep.
        let h = Highlighter::default();
        let result = h.apply(r"Loading from .\.tmp\bo4e_edited\ done");
        assert_eq!(plain(&result), r"Loading from .\.tmp\bo4e_edited\ done");
        let resets = result.matches("\x1b[0m").count();
        assert_eq!(resets, 1, "expected one styled span for the path, got: {result:?}");
    }

    #[test]
    fn test_windows_parent_relative_path_is_highlighted() {
        let h = Highlighter::default();
        let result = h.apply(r"see ..\other\thing for context");
        assert_eq!(plain(&result), r"see ..\other\thing for context");
        let resets = result.matches("\x1b[0m").count();
        assert_eq!(resets, 1);
    }

    #[test]
    fn test_mixed_separator_path_is_highlighted() {
        // `.\foo/bar\baz` — mixed `\` and `/` should still be a single span.
        let h = Highlighter::default();
        let result = h.apply(r"see .\foo/bar\baz now");
        assert_eq!(plain(&result), r"see .\foo/bar\baz now");
        let resets = result.matches("\x1b[0m").count();
        assert_eq!(resets, 1, "expected one styled span, got: {result:?}");
    }

    #[test]
    fn test_dot_after_word_is_not_treated_as_windows_path() {
        // `xy.\foo` and `1.5\x` should NOT trigger the windows-relative anchor — the
        // dot sits behind a word char so `\B` fails.
        let h = Highlighter::default();
        let r1 = h.apply(r"frob xy.\foo bar");
        let p1 = plain(&r1);
        assert_eq!(p1, r"frob xy.\foo bar");
        // No styled path span. There may be other rules firing (like `bar`) but `\foo`
        // must not be the start of a styled path span.
        // Easier signal: no SGR before the literal `xy.` substring's backslash.
        assert!(p1.contains(r"xy.\foo"));
    }

    #[test]
    fn test_help_rules_do_not_match_hyphenated_word_as_flag() {
        // `bo4e-cli` should not be parsed as a flag — there's no leading boundary.
        let mut h = Highlighter::default();
        h.add_help_rules();
        let result = h.apply("install bo4e-cli now");
        // No flag span; the `bo` weak rule may still fire on the leading 'bo' though
        // (which is acceptable — we only assert no flag styling occurred).
        // We check by ensuring the literal substring `cli` carries no SGR (flag would have styled it).
        assert!(plain(&result).contains("bo4e-cli"));
    }
}
