use crate::cli::base::Executable;
use crate::console::console::{Console, CONSOLE};

mod cli;
mod console;
mod edit;
mod io;
mod models;
mod utils;

use clap::Parser;

fn main() -> Result<(), String> {
    let cli = cli::base::Cli::parse();
    CONSOLE
        .set(Console::new(cli.verbose))
        .map_err(|_| "CONSOLE already initialized".to_string())?;
    cli.run()
}
