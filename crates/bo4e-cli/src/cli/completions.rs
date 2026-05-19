// crates/bo4e-cli/src/cli/completions.rs
use crate::cli::base::{Cli, Executable};
use crate::completion::{install, paths, shells, show, uninstall};
use crate::cprint_normal;
use clap::{Args, CommandFactory, Subcommand};

/// Manage shell completion scripts for `bo4e`.
#[derive(Args)]
pub struct Completions {
    #[command(subcommand)]
    pub command: CompletionsAction,
}

#[derive(Subcommand)]
pub enum CompletionsAction {
    /// Install or refresh shell completion for the detected shell.
    Install {
        /// Target shell (defaults to auto-detect from $SHELL).
        #[arg(long, value_enum)]
        shell: Option<shells::Selected>,
        /// Overwrite existing script and rc-file block without checking idempotency.
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Remove the completion script and rc-file block installed by `install`.
    Uninstall {
        #[arg(long, value_enum)]
        shell: Option<shells::Selected>,
    },
    /// Print the completion script for the given shell to stdout.
    Show {
        #[arg(value_enum)]
        shell: shells::Selected,
    },
}

impl Executable for Completions {
    fn run(&self) -> Result<(), String> {
        let p = paths::RealPaths;
        match &self.command {
            CompletionsAction::Install { shell, force } => {
                let sh = resolve_shell(*shell)?;
                let mut cmd = Cli::command();
                cprint_normal!("Detected shell: {:?}", sh);
                let outcome = install::install(&mut cmd, sh, &p, *force)
                    .map_err(|e| format!("install failed: {e}"))?;
                report_install_outcome(outcome);
                if sh == shells::Selected::Nushell {
                    cprint_normal!(
                        "Note: dynamic completers (live version tags, class names, etc.) are not available on nushell — only static completion (subcommands, flags, enum values) will be provided."
                    );
                }
                Ok(())
            }
            CompletionsAction::Uninstall { shell } => {
                let sh = resolve_shell(*shell)?;
                let outcome =
                    uninstall::uninstall(sh, &p).map_err(|e| format!("uninstall failed: {e}"))?;
                report_uninstall_outcome(outcome);
                Ok(())
            }
            CompletionsAction::Show { shell } => {
                let mut cmd = Cli::command();
                let mut out = std::io::stdout().lock();
                show::show(&mut cmd, *shell, &mut out).map_err(|e| e.to_string())
            }
        }
    }
}

fn resolve_shell(explicit: Option<shells::Selected>) -> Result<shells::Selected, String> {
    if let Some(s) = explicit {
        return Ok(s);
    }
    shells::Selected::from_env().ok_or_else(|| {
        "could not auto-detect shell from $SHELL; pass --shell explicitly".to_string()
    })
}

fn report_install_outcome(o: install::Outcome) {
    match o {
        install::Outcome::Installed { script, rc } => {
            if let Some(s) = &script {
                cprint_normal!("Wrote {}", s.display());
            }
            if let Some(r) = &rc {
                cprint_normal!("Appended source line to {}", r.display());
            }
            cprint_normal!("Restart your shell to activate completion.");
        }
        install::Outcome::AlreadyInstalled { script, rc } => {
            let target = script
                .or(rc)
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            cprint_normal!("bo4e completion already installed at {target}");
            cprint_normal!("(pass --force to overwrite)");
        }
        install::Outcome::Replaced { script, rc } => {
            if let Some(s) = &script {
                cprint_normal!("Replaced {}", s.display());
            }
            if let Some(r) = &rc {
                cprint_normal!("Replaced bo4e block in {}", r.display());
            }
            cprint_normal!("Restart your shell to activate completion.");
        }
    }
}

fn report_uninstall_outcome(o: uninstall::Outcome) {
    match o {
        uninstall::Outcome::Removed {
            script_removed,
            rc_changed,
        } => {
            if let Some(s) = &script_removed {
                cprint_normal!("Removed {}", s.display());
            }
            if let Some(r) = &rc_changed {
                cprint_normal!("Removed bo4e block from {}", r.display());
            }
        }
        uninstall::Outcome::NothingToRemove { script, rc } => {
            if let Some(s) = &script {
                cprint_normal!(
                    "No completion script found at {}; nothing to remove.",
                    s.display()
                );
            }
            if let Some(r) = &rc {
                cprint_normal!("No bo4e block found in {}; nothing to remove.", r.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn cli(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }

    #[test]
    fn parses_show_for_each_shell() {
        for sh in ["bash", "zsh", "fish", "powershell", "elvish", "nushell"] {
            cli(&["bo4e", "completions", "show", sh]);
        }
    }

    #[test]
    fn parses_install_without_args() {
        cli(&["bo4e", "completions", "install"]);
    }

    #[test]
    fn parses_install_with_shell_and_force() {
        cli(&[
            "bo4e",
            "completions",
            "install",
            "--shell",
            "zsh",
            "--force",
        ]);
    }

    #[test]
    fn parses_uninstall() {
        cli(&["bo4e", "completions", "uninstall"]);
    }
}
