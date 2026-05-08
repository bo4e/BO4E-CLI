pub mod console;
pub mod highlighter;
pub mod palette;
pub mod progress_bar;

/// Print a formatted message at an explicit `Level`. Emitted only if
/// `level <= CONSOLE.level`.
#[macro_export]
macro_rules! cprint {
    ($level:expr, $($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print($level, &format!($($arg)*))
    };
}

/// Print a `Level::Quiet` message. Emitted under every console level (including `--quiet`).
#[macro_export]
macro_rules! cprint_quiet {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Quiet, $($arg)*)
    };
}

/// Print a `Level::Normal` message. Default informational output.
#[macro_export]
macro_rules! cprint_normal {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Normal, $($arg)*)
    };
}

/// Print a `Level::Verbose` message. Emitted only under `--verbose`.
#[macro_export]
macro_rules! cprint_verbose {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Verbose, $($arg)*)
    };
}

#[cfg(test)]
mod tests {
    use crate::console::console::{Console, Level, CONSOLE};

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
    }
}
