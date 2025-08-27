use crate::cli::pull::Pull;
use clap::{Args, CommandFactory, Parser, Subcommand};
use std::io;

pub trait Executable {
    fn run(&self) -> Result<(), String>;
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
//#[command(propagate_version = true)]
pub struct Cli {
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
}

impl Executable for SubcommandsLevel1 {
    fn run(&self) -> Result<(), String> {
        match self {
            SubcommandsLevel1::Pull(pull) => pull.run(),
        }
    }
}
