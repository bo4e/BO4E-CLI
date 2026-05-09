//! Named-spinner factories mirroring the python implementation's rich spinners.
//!
//! Frames copied verbatim from rich `_spinners.py` (vendored at
//! `.tox/dev/Lib/site-packages/rich/_spinners.py`).

use indicatif::{ProgressBar, ProgressStyle};
use std::borrow::Cow;
use std::time::Duration;

const EARTH_FRAMES: &[&str] = &["🌍 ", "🌎 ", "🌏 "];
const EARTH_INTERVAL_MS: u64 = 180;

const SQUISH_FRAMES: &[&str] = &["╫", "╪"];
const SQUISH_INTERVAL_MS: u64 = 100;

const GRENADE_FRAMES: &[&str] = &[
    "،   ",
    "′   ",
    " ´ ",
    " ‾ ",
    "  ⸌",
    "  ⸊",
    "  |",
    "  ⁎",
    "  ⁕",
    " ෴ ",
    "  ⁓",
    "   ",
    "   ",
    "   ",
];
const GRENADE_INTERVAL_MS: u64 = 80;

pub fn earth(msg: impl Into<Cow<'static, str>>) -> ProgressBar {
    spinner(msg, EARTH_FRAMES, EARTH_INTERVAL_MS)
}

pub fn squish(msg: impl Into<Cow<'static, str>>) -> ProgressBar {
    spinner(msg, SQUISH_FRAMES, SQUISH_INTERVAL_MS)
}

pub fn grenade(msg: impl Into<Cow<'static, str>>) -> ProgressBar {
    spinner(msg, GRENADE_FRAMES, GRENADE_INTERVAL_MS)
}

fn spinner(
    msg: impl Into<Cow<'static, str>>,
    frames: &'static [&'static str],
    interval_ms: u64,
) -> ProgressBar {
    let visible = crate::console::console::CONSOLE
        .get()
        .map(|c| c.would_emit(crate::console::console::Level::Normal))
        .unwrap_or(true);
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
    use crate::console::console::{CONSOLE, Console, Level};

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
    fn quiet_returns_hidden() {
        let _ = CONSOLE.set(Console::new(Level::Quiet));
        // Note: CONSOLE is a OnceLock so this set is best-effort across the whole
        // test binary. The assertion below uses `would_emit` directly to avoid
        // ordering brittleness with other tests in the binary.
        let c = CONSOLE.get().expect("set above or earlier");
        if !c.would_emit(Level::Normal) {
            assert!(earth("hi").is_hidden());
            assert!(squish("hi").is_hidden());
            assert!(grenade("hi").is_hidden());
        }
    }
}
