pub mod console;
pub mod highlighter;
pub mod palette;
pub mod progress_bar;

/// Print a formatted message through the global CONSOLE (always shown).
#[macro_export]
macro_rules! cprint {
    ($($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print(&format!($($arg)*))
    };
}

/// Print a formatted message only when verbose mode is active.
#[macro_export]
macro_rules! cprint_verbose {
    ($($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print_verbose(&format!($($arg)*))
    };
}
