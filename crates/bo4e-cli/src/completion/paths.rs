use clap_complete::Shell;
use std::path::PathBuf;

pub trait Paths {
    fn home(&self) -> PathBuf;
    fn config(&self) -> PathBuf {
        self.home().join(".config")
    }
    /// Where the PowerShell profile lives. Default impl shells out to
    /// `pwsh -NoProfile -Command '$PROFILE'`, then falls back to PS7's
    /// hard-coded default. Tests can override this to avoid the subprocess
    /// (which on macOS/Windows runners returns the real user's profile,
    /// outside any test sandbox).
    fn powershell_profile(&self) -> PathBuf {
        detect_powershell_profile(self)
    }
}

pub struct RealPaths;

impl Paths for RealPaths {
    fn home(&self) -> PathBuf {
        dirs::home_dir().expect("home directory not resolvable")
    }
    fn config(&self) -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| self.home().join(".config"))
    }
}

#[derive(Debug, Clone)]
pub struct ShellPaths {
    /// Where the completion script is written (None for shells that embed
    /// the entire script into the rc file directly, e.g. PowerShell).
    pub script: Option<PathBuf>,
    /// Where to edit/add the source line (None for fish, which auto-loads
    /// scripts from `~/.config/fish/completions/`).
    pub rc: Option<PathBuf>,
}

pub fn paths_for(shell: Shell, p: &dyn Paths) -> ShellPaths {
    match shell {
        Shell::Bash => ShellPaths {
            script: Some(p.home().join(".bash_completions/bo4e.sh")),
            rc: Some(p.home().join(".bashrc")),
        },
        Shell::Zsh => ShellPaths {
            script: Some(p.home().join(".zfunc/_bo4e")),
            rc: Some(p.home().join(".zshrc")),
        },
        Shell::Fish => ShellPaths {
            script: Some(p.config().join("fish/completions/bo4e.fish")),
            rc: None,
        },
        Shell::PowerShell => ShellPaths {
            script: None,
            rc: Some(p.powershell_profile()),
        },
        Shell::Elvish => ShellPaths {
            script: Some(p.config().join("elvish/lib/bo4e.elv")),
            rc: Some(p.config().join("elvish/rc.elv")),
        },
        // clap_complete::Shell is #[non_exhaustive]; we only call paths_for for the
        // five variants above. A future new variant would need to be handled here.
        _ => unreachable!("paths_for called with unsupported shell variant"),
    }
}

fn detect_powershell_profile(p: &(impl Paths + ?Sized)) -> PathBuf {
    use std::process::Command;
    if let Ok(out) = Command::new("pwsh")
        .args(["-NoProfile", "-Command", "$PROFILE"])
        .output()
        && out.status.success()
    {
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    if let Ok(out) = Command::new("powershell")
        .args(["-NoProfile", "-Command", "$PROFILE"])
        .output()
        && out.status.success()
    {
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    // Fallback: PS7's default profile location
    p.home()
        .join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakePaths {
        home: PathBuf,
    }
    impl Paths for FakePaths {
        fn home(&self) -> PathBuf {
            self.home.clone()
        }
        fn powershell_profile(&self) -> PathBuf {
            // Force deterministic, in-tempdir path. Matches the fallback the
            // production code uses when pwsh/powershell aren't available.
            self.home()
                .join("Documents/PowerShell/Microsoft.PowerShell_profile.ps1")
        }
    }

    #[test]
    fn bash_paths_under_home() {
        let p = FakePaths {
            home: PathBuf::from("/tmp/home"),
        };
        let sp = paths_for(Shell::Bash, &p);
        assert_eq!(
            sp.script.as_deref(),
            Some(std::path::Path::new("/tmp/home/.bash_completions/bo4e.sh"))
        );
        assert_eq!(
            sp.rc.as_deref(),
            Some(std::path::Path::new("/tmp/home/.bashrc"))
        );
    }

    #[test]
    fn zsh_paths_under_home() {
        let p = FakePaths {
            home: PathBuf::from("/tmp/home"),
        };
        let sp = paths_for(Shell::Zsh, &p);
        assert!(sp.script.unwrap().ends_with(".zfunc/_bo4e"));
        assert!(sp.rc.unwrap().ends_with(".zshrc"));
    }

    #[test]
    fn fish_has_no_rc() {
        let p = FakePaths {
            home: PathBuf::from("/tmp/home"),
        };
        let sp = paths_for(Shell::Fish, &p);
        assert!(sp.script.is_some());
        assert!(sp.rc.is_none());
    }

    #[test]
    fn powershell_has_rc_only() {
        let p = FakePaths {
            home: PathBuf::from("/tmp/home"),
        };
        let sp = paths_for(Shell::PowerShell, &p);
        assert!(sp.script.is_none());
        assert!(sp.rc.is_some());
    }

    #[test]
    fn elvish_has_both() {
        let p = FakePaths {
            home: PathBuf::from("/tmp/home"),
        };
        let sp = paths_for(Shell::Elvish, &p);
        assert!(sp.script.is_some());
        assert!(sp.rc.is_some());
    }
}
