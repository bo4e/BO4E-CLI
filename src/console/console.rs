use crate::console::highlighter::Highlighter;
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
        let highlighted = self.highlighter.read().unwrap().apply(msg);
        println!("{highlighted}");
    }

    /// Emit a warning to stderr. Never suppressed (warnings are always shown).
    pub fn print_warn(&self, msg: &str) {
        let highlighted = self.highlighter.read().unwrap().apply(msg);
        eprintln!("{highlighted}");
    }

    /// Emit an error to stderr. Never suppressed.
    pub fn print_error(&self, msg: &str) {
        let highlighted = self.highlighter.read().unwrap().apply(msg);
        eprintln!("{highlighted}");
    }

    /// Register schema names for dynamic highlighting (call once after read_schemas).
    pub fn add_schema_names(&self, names: &[String]) {
        self.highlighter.write().unwrap().add_schema_names(names);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_ordering() {
        assert!(Level::Quiet < Level::Normal);
        assert!(Level::Normal < Level::Verbose);
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
