#[allow(clippy::module_inception)]
pub mod console;
pub mod highlighter;
pub mod mark;
pub mod palette;
pub mod progress_bar;
pub mod spinner;

/// Print a formatted info message at an explicit `Level`. Goes to stdout.
/// Emitted only if `level <= CONSOLE.level`.
#[macro_export]
macro_rules! cprint {
    ($level:expr, $($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print_info($level, &format!($($arg)*))
    };
}

/// Print a `Level::Quiet` info message. Emitted under every console level (including `--quiet`).
/// Goes to stdout.
#[macro_export]
macro_rules! cprint_quiet {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Quiet, $($arg)*)
    };
}

/// Print a `Level::Normal` info message. Default informational output. Goes to stdout.
#[macro_export]
macro_rules! cprint_normal {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Normal, $($arg)*)
    };
}

/// Print a `Level::Verbose` info message. Emitted only under `--verbose`. Goes to stdout.
#[macro_export]
macro_rules! cprint_verbose {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Verbose, $($arg)*)
    };
}

/// Print a warning to stderr. Always shown, regardless of `--quiet`.
#[macro_export]
macro_rules! cwarn {
    ($($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print_warn(&format!($($arg)*))
    };
}

/// Print an error to stderr. Always shown, regardless of `--quiet`.
#[macro_export]
macro_rules! cerror {
    ($($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print_error(&format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use crate::console::console::{CONSOLE, Console, Level};

    fn ensure_console_initialized() {
        let _ = CONSOLE.set(Console::new(Level::Verbose));
    }

    #[test]
    fn test_cprint_macros_compile_and_run() {
        ensure_console_initialized();
        crate::cprint!(Level::Normal, "hello {}", "world");
        crate::cprint_quiet!("forced");
        crate::cprint_normal!("default");
        crate::cprint_verbose!("detail {}", 42);
        crate::cwarn!("warn {}", "msg");
        crate::cerror!("error {}", "msg");
    }
}
