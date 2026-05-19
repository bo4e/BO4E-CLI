use crate::cli::completions::Completions;
use crate::cli::diff::Diff;
use crate::cli::edit::Edit;
use crate::cli::generate::Generate;
use crate::cli::graph::Graph;
use crate::cli::pull::Pull;
use crate::cli::repo::Repo;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{CommandFactory, Parser, Subcommand};

pub trait Executable {
    fn run(&self) -> Result<(), String>;
}

// Matches palette::MAIN/SUB/ENUM/ERROR by tone; uses 16-colour AnsiColor for
// const-friendliness — help renders before CONSOLE is initialised.
const HELP_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Magenta.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Yellow.on_default().effects(Effects::ITALIC))
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .valid(AnsiColor::Cyan.on_default())
    .invalid(AnsiColor::Red.on_default().effects(Effects::BOLD));

#[derive(Parser)]
#[command(
    author,
    about = "BO4E - Business Objects for Energy",
    long_about = "BO4E - Business Objects for Energy\n\n\
        This CLI is intended for developers working with BO4E. \
        For more information see '--help' or visit \
        https://github.com/bo4e/BO4E-CLI?tab=readme-ov-file#bo4e-cli",
    styles = HELP_STYLES,
    disable_version_flag = true,
)]
pub struct Cli {
    /// Print programs current version number.
    ///
    /// Handled manually in `main` so the output is `v{version}` (matches Python).
    #[arg(long = "version", action = clap::ArgAction::SetTrue)]
    pub show_version: bool,

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
    Diff(Diff),
    Repo(Repo),
    Generate(Generate),
    Graph(Graph),
    Completions(Completions),
}

impl Executable for SubcommandsLevel1 {
    fn run(&self) -> Result<(), String> {
        match self {
            SubcommandsLevel1::Pull(pull) => pull.run(),
            SubcommandsLevel1::Edit(edit) => edit.run(),
            SubcommandsLevel1::Diff(diff) => diff.run(),
            SubcommandsLevel1::Repo(repo) => repo.run(),
            SubcommandsLevel1::Generate(generate) => generate.run(),
            SubcommandsLevel1::Graph(graph) => graph.run(),
            SubcommandsLevel1::Completions(c) => c.run(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_quiet_and_verbose_are_mutually_exclusive() {
        let result = Cli::try_parse_from([
            "bo4e",
            "--quiet",
            "--verbose",
            "edit",
            "-i",
            "in",
            "-o",
            "out",
        ]);
        assert!(result.is_err(), "--quiet and --verbose must conflict");
    }

    #[test]
    fn test_quiet_flag_parses() {
        let cli =
            Cli::try_parse_from(["bo4e", "--quiet", "edit", "-i", "in", "-o", "out"]).unwrap();
        assert!(cli.quiet);
        assert!(!cli.verbose);
    }

    #[test]
    fn help_contains_ansi_when_styled() {
        let mut cmd = Cli::command();
        let rendered = cmd.render_help().ansi().to_string();
        assert!(
            rendered.contains("\x1b["),
            "expected ANSI escape sequences in --help output, got:\n{}",
            rendered
        );
    }

    #[test]
    fn each_subcommand_help_contains_ansi() {
        let mut cmd = Cli::command();
        for name in [
            "pull",
            "edit",
            "diff",
            "repo",
            "generate",
            "graph",
            "completions",
        ] {
            let sub = cmd
                .find_subcommand_mut(name)
                .unwrap_or_else(|| panic!("subcommand {} missing", name));
            let rendered = sub.clone().render_help().ansi().to_string();
            assert!(
                rendered.contains("\x1b["),
                "subcommand {} help has no ANSI: {}",
                name,
                rendered
            );
        }
    }
}
