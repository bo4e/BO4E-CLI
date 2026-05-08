use crate::cli::base::Executable;
use crate::console::console::{Console, Level, CONSOLE};

mod cli;
mod console;
mod edit;
mod io;
mod models;
mod utils;

use clap::Parser;

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
