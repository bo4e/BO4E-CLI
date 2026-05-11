use crate::console::highlighter::{Highlighter, SchemaModule};
use crate::console::mark::{MarkStyle, SENTINEL_END, SENTINEL_SEP, SENTINEL_START};
use crate::console::palette;
use console::Style;
use std::sync::{OnceLock, RwLock};

pub static CONSOLE: OnceLock<Console> = OnceLock::new();

/// Importance of a console message. Lower discriminants are more important —
/// a message is emitted iff its level is `<=` the console's level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Level {
    /// Must always be surfaced, including under `--quiet`.
    Quiet = 0,
    /// Default informational output.
    Normal = 1,
    /// Detail emitted only under `--verbose`.
    Verbose = 2,
}

pub struct Console {
    level: Level,
    highlighter: RwLock<Highlighter>,
}

impl Console {
    pub fn new(level: Level) -> Self {
        Self {
            level,
            highlighter: RwLock::new(Highlighter::default()),
        }
    }

    /// Returns `true` iff a message of the given level would be emitted by this console.
    pub fn would_emit(&self, message_level: Level) -> bool {
        message_level <= self.level
    }

    /// Emit an informational message to stdout iff `message_level <= self.level`,
    /// after applying the highlighter.
    pub fn print_info(&self, message_level: Level, msg: &str) {
        if !self.would_emit(message_level) {
            return;
        }
        println!("{}", self.render(msg));
    }

    /// Emit a warning to stderr. Never suppressed (warnings are always shown).
    /// Mirrors Python's `Console.print_warn`: text wrapped in the ERROR colour.
    pub fn print_warn(&self, msg: &str) {
        eprintln!("{}", wrap_with_outer_color(&self.render(msg), &warning_open_code()));
    }

    /// Emit an error to stderr. Never suppressed. Same styling as `print_warn`.
    #[allow(dead_code)]
    pub fn print_error(&self, msg: &str) {
        eprintln!("{}", wrap_with_outer_color(&self.render(msg), &warning_open_code()));
    }

    /// Register schema names with per-module classification (call once after read_schemas).
    pub fn add_schema_names(&self, classified: &[(SchemaModule, String)]) {
        self.highlighter.write().unwrap().add_schema_names(classified);
    }

    /// Render a message: split on `Mark` sentinels, run the highlighter on plain
    /// segments, apply each mark's explicit style verbatim (no inner highlighting).
    /// Fast path: no sentinels → straight-through highlighter call.
    fn render(&self, msg: &str) -> String {
        if !msg.contains(SENTINEL_START) {
            return self.highlighter.read().unwrap().apply(msg);
        }

        let mut out = String::with_capacity(msg.len());
        let mut rest = msg;
        loop {
            match rest.find(SENTINEL_START) {
                None => {
                    out.push_str(&self.highlighter.read().unwrap().apply(rest));
                    return out;
                }
                Some(start_off) => {
                    // Plain prefix → highlighter.
                    let (plain, after_start) = rest.split_at(start_off);
                    out.push_str(&self.highlighter.read().unwrap().apply(plain));

                    // Strip START sentinel.
                    let after_start = &after_start[SENTINEL_START.len_utf8()..];

                    // Style code is the next char; SEP must follow; then content; then END.
                    let code = after_start.chars().next();
                    let after_code = code
                        .map(|c| &after_start[c.len_utf8()..])
                        .unwrap_or(after_start);
                    let after_sep = after_code
                        .strip_prefix(SENTINEL_SEP)
                        .unwrap_or(after_code);
                    let end_off = after_sep.find(SENTINEL_END).unwrap_or(after_sep.len());
                    let (inner, tail) = after_sep.split_at(end_off);
                    let advance = tail.strip_prefix(SENTINEL_END).unwrap_or(tail);

                    match code.and_then(MarkStyle::from_code) {
                        Some(ms) => out.push_str(&ms.style().apply_to(inner).to_string()),
                        // Unknown / missing code: emit content unstyled, no panic.
                        None => out.push_str(inner),
                    }
                    rest = advance;
                }
            }
        }
    }
}

fn warning_style() -> Style {
    Style::new().fg(parse_hex_color(palette::ERROR)).force_styling(true)
}

/// SGR open sequence for the warning style, with the trailing reset stripped —
/// usable as a "re-open" code to re-apply the warning colour after an inner reset.
fn warning_open_code() -> String {
    let s = warning_style().apply_to("").to_string();
    s.trim_end_matches("\x1b[0m").to_string()
}

/// Wrap `rendered` in `open_code`, and re-apply `open_code` after every inner
/// `\x1b[0m` reset so the outer colour persists across inner styled spans
/// (highlighter spans, `pat()` marks, …) instead of being cleared by their resets.
fn wrap_with_outer_color(rendered: &str, open_code: &str) -> String {
    let body = rendered.replace("\x1b[0m", &format!("\x1b[0m{open_code}"));
    format!("{open_code}{body}\x1b[0m")
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
    fn test_level_ordering() {
        assert!(Level::Quiet < Level::Normal);
        assert!(Level::Normal < Level::Verbose);
    }

    #[test]
    fn test_wrap_with_outer_color_reapplies_after_inner_reset() {
        // Inner span's `\x1b[0m` would normally clear the outer warning colour;
        // wrap_with_outer_color must re-emit the open code after every inner reset
        // so the outer colour persists across the rest of the message.
        let open = "\x1b[38;2;227;91;58m";
        let inner = "\x1b[1minner\x1b[0m";
        let rendered = format!("before {inner} after");
        let wrapped = super::wrap_with_outer_color(&rendered, open);
        // Expected: open + before + inner_open + inner + reset + open + after + reset
        let expected = format!("{open}before \x1b[1minner\x1b[0m{open} after\x1b[0m");
        assert_eq!(wrapped, expected);
    }

    #[test]
    fn test_wrap_with_outer_color_no_inner_resets() {
        // No inner styled spans → just open + body + reset, no extra reopens.
        let open = "\x1b[38;2;227;91;58m";
        let wrapped = super::wrap_with_outer_color("plain only", open);
        assert_eq!(wrapped, format!("{open}plain only\x1b[0m"));
    }

    /// Full 3×3 emission table from the design doc.
    #[test]
    fn test_emission_table() {
        let cases: &[(Level, Level, bool)] = &[
            (Level::Quiet,   Level::Quiet,   true),
            (Level::Quiet,   Level::Normal,  false),
            (Level::Quiet,   Level::Verbose, false),
            (Level::Normal,  Level::Quiet,   true),
            (Level::Normal,  Level::Normal,  true),
            (Level::Normal,  Level::Verbose, false),
            (Level::Verbose, Level::Quiet,   true),
            (Level::Verbose, Level::Normal,  true),
            (Level::Verbose, Level::Verbose, true),
        ];
        for (cl, ml, expected) in cases {
            let c = Console::new(*cl);
            assert_eq!(
                c.would_emit(*ml),
                *expected,
                "console={:?} message={:?}",
                cl, ml
            );
        }
    }
}
