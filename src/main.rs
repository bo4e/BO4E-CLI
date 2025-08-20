mod cli;
mod io;
mod models;

use crate::cli::{Cli, SubcommandsLevel1};
use clap::{CommandFactory, Parser};

fn main() {
    let cli = Cli::parse();

    // You can check for the existence of subcommands, and if found use their
    // matches just as you would the top level cmd
    match &cli.command {
        Some(SubcommandsLevel1::Pull(pull)) => {
            println!("'myapp add' was used, name is: {:?}", &pull.version_tag)
        }
        None => {
            // print help page if no subcommand is provided
            Cli::command().print_help().unwrap();
        }
    }
}
