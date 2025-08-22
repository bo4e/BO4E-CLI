use crate::cli::base::Executable;
mod cli;
mod io;
mod models;

use clap::{CommandFactory, Parser};

fn main() -> std::io::Result<()> {
    let cli = cli::base::Cli::parse();
    cli.run()
}
