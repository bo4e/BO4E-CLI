use crate::console::highlighter::Highlighter;
use std::sync::{OnceLock, RwLock};

pub static CONSOLE: OnceLock<Console> = OnceLock::new();

pub struct Console {
    pub(crate) verbose: bool,
    highlighter: RwLock<Highlighter>,
}

impl Console {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            highlighter: RwLock::new(Highlighter::default()),
        }
    }

    /// Print a message (always shown), applying the highlighter.
    pub fn print(&self, msg: &str) {
        let highlighted = self.highlighter.read().unwrap().apply(msg);
        eprintln!("{}", highlighted); // NOTE: output to stderr keeps stdout clean for piping
    }

    /// Print a message only when verbose mode is enabled.
    pub fn print_verbose(&self, msg: &str) {
        if self.verbose {
            self.print(msg);
        }
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
    fn test_console_new_stores_verbose_flag() {
        let c = Console::new(true);
        assert!(c.verbose);
        let c2 = Console::new(false);
        assert!(!c2.verbose);
    }

    #[test]
    fn test_console_add_schema_names_does_not_panic() {
        let c = Console::new(false);
        c.add_schema_names(&["Angebot".to_string(), "Typ".to_string()]);
        // Just verify it doesn't panic — the stub highlighter ignores the names
    }
}
