pub mod bash;
pub mod elvish;
pub mod fish;
pub mod nushell;
pub mod powershell;
pub mod zsh;

use clap::Command;
use std::path::Path;

/// One enum that covers all six shells (clap_complete::Shell has only five
/// — nushell is via clap_complete_nushell).
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Selected {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Elvish,
    Nushell,
}

impl Selected {
    pub fn script(self, cmd: &mut Command) -> String {
        match self {
            Selected::Bash => bash::script(cmd),
            Selected::Zsh => zsh::script(cmd),
            Selected::Fish => fish::script(cmd),
            Selected::Powershell => powershell::script(cmd),
            Selected::Elvish => elvish::script(cmd),
            Selected::Nushell => nushell::script(cmd),
        }
    }

    pub fn rc_body(self, script: &Path) -> Option<String> {
        match self {
            Selected::Bash => Some(bash::rc_body(script)),
            Selected::Zsh => Some(zsh::rc_body(script)),
            Selected::Fish => None, // fish auto-loads
            Selected::Powershell => Some(powershell::rc_body(script)),
            Selected::Elvish => Some(elvish::rc_body(script)),
            Selected::Nushell => Some(nushell::rc_body(script)),
        }
    }

    pub fn comment_leader(self) -> &'static str {
        "#"
    }

    pub fn from_env() -> Option<Self> {
        clap_complete::Shell::from_env().and_then(|s| match s {
            clap_complete::Shell::Bash => Some(Self::Bash),
            clap_complete::Shell::Zsh => Some(Self::Zsh),
            clap_complete::Shell::Fish => Some(Self::Fish),
            clap_complete::Shell::PowerShell => Some(Self::Powershell),
            clap_complete::Shell::Elvish => Some(Self::Elvish),
            _ => None,
        })
    }
}
