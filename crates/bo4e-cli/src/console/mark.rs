//! Explicit per-message highlighting via `Mark` / `pat()`.
//!
//! Some console messages embed values that should be rendered as a single styled
//! span — typically regex patterns whose contents (`bo\.Angebot`, `enum\.Sparte`)
//! would otherwise be partially highlighted by the regex-rule highlighter. Wrapping
//! the value with `pat(&value)` is opaque: the marked region is styled with one
//! explicit colour, and the rule-based highlighter never sees the inside, so no
//! inner match can leak through.
//!
//! Implementation: `Mark` Display-formats the value sandwiched between three
//! private-use Unicode codepoints (`U+E000`/`U+E001`/`U+E002`). The console layer
//! finds these sentinels in the final formatted message before highlighting,
//! splits the message into plain and marked segments, and renders each segment
//! independently — plain through the highlighter, marked through the explicit
//! style. Choice of private-use codepoints means the sentinels can never collide
//! with legitimate message content.

use crate::console::palette;
use console::Style;
use std::fmt;

pub(crate) const SENTINEL_START: char = '\u{E000}';
pub(crate) const SENTINEL_SEP: char = '\u{E001}';
pub(crate) const SENTINEL_END: char = '\u{E002}';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkStyle {
    /// Regex pattern: rendered bold + italic in the SUB_ACCENT colour.
    Pattern,
}

impl MarkStyle {
    pub(crate) fn code(self) -> char {
        match self {
            MarkStyle::Pattern => 'p',
        }
    }

    pub(crate) fn from_code(c: char) -> Option<Self> {
        match c {
            'p' => Some(MarkStyle::Pattern),
            _ => None,
        }
    }

    pub(crate) fn style(self) -> Style {
        match self {
            MarkStyle::Pattern => Style::new()
                .fg(parse_hex_color(palette::SUB_ACCENT))
                .bold()
                .italic()
                .force_styling(true),
        }
    }
}

pub struct Mark<'a, T: fmt::Display + ?Sized> {
    inner: &'a T,
    style: MarkStyle,
}

impl<T: fmt::Display + ?Sized> fmt::Display for Mark<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}{}{}",
            SENTINEL_START,
            self.style.code(),
            SENTINEL_SEP,
            self.inner,
            SENTINEL_END,
        )
    }
}

/// Wrap a value as a regex pattern in console output.
///
/// The wrapped text is rendered as a single styled span; the rule-based
/// highlighter is bypassed for its contents, so e.g. `bo\.Angebot` appears as one
/// uniform span instead of being split into separately-coloured `bo` and
/// `Angebot` halves.
pub fn pat<T: fmt::Display + ?Sized>(t: &T) -> Mark<'_, T> {
    Mark {
        inner: t,
        style: MarkStyle::Pattern,
    }
}

fn parse_hex_color(hex: &str) -> console::Color {
    let h = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(255);
    console::Color::TrueColor(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mark_display_emits_sentinels() {
        let m = pat(&"bo\\.Angebot");
        let s = format!("{m}");
        assert!(s.starts_with(SENTINEL_START));
        assert!(s.ends_with(SENTINEL_END));
        assert!(s.contains("bo\\.Angebot"));
    }
}
