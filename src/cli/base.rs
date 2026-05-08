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
    #[arg(global = true, short = 'v', long, conflicts_with = "quiet")]
    pub verbose: bool,

    /// Suppress all non-essential output.
    #[arg(global = true, short = 'q', long)]
    pub quiet: bool,

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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_quiet_and_verbose_are_mutually_exclusive() {
        let result = Cli::try_parse_from(["bo4e", "--quiet", "--verbose", "edit",
            "-i", "in", "-o", "out"]);
        assert!(result.is_err(), "--quiet and --verbose must conflict");
    }

    #[test]
    fn test_quiet_flag_parses() {
        let cli = Cli::try_parse_from(["bo4e", "--quiet", "edit",
            "-i", "in", "-o", "out"]).unwrap();
        assert!(cli.quiet);
        assert!(!cli.verbose);
    }
}
