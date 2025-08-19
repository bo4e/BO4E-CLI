mod cli;
mod io;

use crate::cli::{Cli, Commands};
use clap::{CommandFactory, Parser};

fn main() {
    let cli = Cli::parse();

    // You can check for the existence of subcommands, and if found use their
    // matches just as you would the top level cmd
    match &cli.command {
        Some(Commands::Pull(pull)) => {
            println!("'myapp add' was used, name is: {:?}", &pull.version_tag)
        }
        None => {
            // print help page if no subcommand is provided
            Cli::command().print_help().unwrap();
        }
    }
}
