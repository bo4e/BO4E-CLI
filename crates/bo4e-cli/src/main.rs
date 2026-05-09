use crate::cli::base::Executable;
use crate::console::console::{Console, Level, CONSOLE};

mod cli;
mod console;
mod diff;
mod edit;
mod io;
mod models;
mod repo;
mod utils;

use clap::Parser;

/// A process-global mutex used by tests in multiple modules that call
/// `std::env::set_current_dir`. Cargo runs tests in parallel by default;
/// any test that mutates the process cwd must hold this lock for the
/// duration of the test.
#[cfg(test)]
pub(crate) mod test_lock {
    use std::sync::Mutex;
    pub(crate) static CWD_LOCK: Mutex<()> = Mutex::new(());
}

fn main() -> Result<(), String> {
    let cli = cli::base::Cli::parse();
    let level = match (cli.verbose, cli.quiet) {
        (true, _) => Level::Verbose,
        (_, true) => Level::Quiet,
        _         => Level::Normal,
    };
    CONSOLE
        .set(Console::new(level))
        .map_err(|_| "CONSOLE already initialized".to_string())?;
    cli.run()
}
