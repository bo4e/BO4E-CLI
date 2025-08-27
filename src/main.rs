use crate::cli::base::Executable;
mod cli;
mod io;
mod models;
mod utils;

use clap::{CommandFactory, Parser};

fn main() -> Result<(), String> {
    let cli = cli::base::Cli::parse();
    cli.run()
}
