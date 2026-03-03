use crate::cli::edit::Edit;
use crate::cli::pull::Pull;
use clap::{CommandFactory, Parser, Subcommand};

pub trait Executable {
    fn run(&self) -> Result<(), String>;
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
//#[command(propagate_version = true)]
pub struct Cli {
    /// Enable verbose output for all commands.
    #[arg(global = true, short = 'v', long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<SubcommandsLevel1>,
}

impl Executable for Cli {
    fn run(&self) -> Result<(), String> {
        if let Some(command) = &self.command {
            command.run()
        } else {
            Cli::command().print_help().map_err(|err| err.to_string())
        }
    }
}

#[derive(Subcommand)]
pub enum SubcommandsLevel1 {
    Pull(Pull),
    Edit(Edit),
}

impl Executable for SubcommandsLevel1 {
    fn run(&self) -> Result<(), String> {
        match self {
            SubcommandsLevel1::Pull(pull) => pull.run(),
            SubcommandsLevel1::Edit(edit) => edit.run(),
        }
    }
}
