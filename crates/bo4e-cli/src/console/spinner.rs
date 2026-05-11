//! Named-spinner factories mirroring the python implementation's rich spinners.
//!
//! Frames copied verbatim from rich `_spinners.py` (vendored at
//! `.tox/dev/Lib/site-packages/rich/_spinners.py`).

use indicatif::{ProgressBar, ProgressStyle};
use std::borrow::Cow;
use std::time::Duration;

/// RAII wrapper: stops the spinner and clears the rendered line on drop.
/// `indicatif::ProgressBar` does not call `finish_and_clear` on drop by default,
/// so the last spinner frame would otherwise stay painted on the terminal.
pub struct Spinner {
    pb: ProgressBar,
}

impl Spinner {
    /// Test-only inspector: was this spinner created in hidden mode?
    #[cfg(test)]
    fn is_hidden(&self) -> bool {
        self.pb.is_hidden()
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.pb.finish_and_clear();
    }
}

const EARTH_FRAMES: &[&str] = &["🌍 ", "🌎 ", "🌏 "];
const EARTH_INTERVAL_MS: u64 = 180;

const SQUISH_FRAMES: &[&str] = &["╫", "╪"];
const SQUISH_INTERVAL_MS: u64 = 100;

const GRENADE_FRAMES: &[&str] = &[
    "،   ", "′   ", " ´ ", " ‾ ", "  ⸌", "  ⸊", "  |", "  ⁎", "  ⁕", " ෴ ", "  ⁓", "   ", "   ",
    "   ",
];
const GRENADE_INTERVAL_MS: u64 = 80;

pub fn earth(msg: impl Into<Cow<'static, str>>) -> Spinner {
    Spinner {
        pb: make_spinner(msg, EARTH_FRAMES, EARTH_INTERVAL_MS, would_show()),
    }
}

pub fn squish(msg: impl Into<Cow<'static, str>>) -> Spinner {
    Spinner {
        pb: make_spinner(msg, SQUISH_FRAMES, SQUISH_INTERVAL_MS, would_show()),
    }
}

pub fn grenade(msg: impl Into<Cow<'static, str>>) -> Spinner {
    Spinner {
        pb: make_spinner(msg, GRENADE_FRAMES, GRENADE_INTERVAL_MS, would_show()),
    }
}

fn would_show() -> bool {
    crate::console::console::CONSOLE
        .get()
        .map(|c| c.would_emit(crate::console::console::Level::Normal))
        .unwrap_or(true)
}

fn make_spinner(
    msg: impl Into<Cow<'static, str>>,
    frames: &'static [&'static str],
    interval_ms: u64,
    visible: bool,
) -> ProgressBar {
    if !visible {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .expect("static template parses")
            .tick_strings(frames),
    );
    pb.set_message(msg.into());
    pb.enable_steady_tick(Duration::from_millis(interval_ms));
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_match_rich_v14_earth() {
        assert_eq!(EARTH_FRAMES, &["🌍 ", "🌎 ", "🌏 "]);
        assert_eq!(EARTH_INTERVAL_MS, 180);
    }

    #[test]
    fn frames_match_rich_v14_squish() {
        assert_eq!(SQUISH_FRAMES, &["╫", "╪"]);
        assert_eq!(SQUISH_INTERVAL_MS, 100);
    }

    #[test]
    fn frames_match_rich_v14_grenade() {
        // Length and a few key frames; full equality is enforced by the const itself.
        assert_eq!(GRENADE_FRAMES.len(), 14);
        assert_eq!(GRENADE_FRAMES[0], "،   ");
        assert_eq!(GRENADE_FRAMES[6], "  |");
        assert_eq!(GRENADE_FRAMES[13], "   ");
        assert_eq!(GRENADE_INTERVAL_MS, 80);
    }

    #[test]
    fn invisible_returns_hidden() {
        let spinner = Spinner {
            pb: make_spinner("hi", EARTH_FRAMES, EARTH_INTERVAL_MS, false),
        };
        assert!(spinner.is_hidden());
    }
}
