// crates/bo4e-cli/src/completion/install.rs
use crate::completion::marker;
use crate::completion::paths::{Paths, paths_for};
use crate::completion::shells::Selected;
use clap::Command;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Outcome reported back to the caller for printing.
#[derive(Debug)]
pub enum Outcome {
    Installed { script: Option<PathBuf>, rc: Option<PathBuf> },
    AlreadyInstalled { script: Option<PathBuf>, rc: Option<PathBuf> },
    Replaced { script: Option<PathBuf>, rc: Option<PathBuf> },
}

pub fn install(
    cmd: &mut Command,
    shell: Selected,
    paths: &dyn Paths,
    force: bool,
) -> io::Result<Outcome> {
    let sp = paths_for_selected(shell, paths);

    // PowerShell branch: no separate script file — embed full content in
    // the marker block written to $PROFILE.
    if shell == Selected::Powershell {
        let rc = sp.rc.as_ref().expect("powershell always has an rc");
        let original = read_or_empty(rc)?;
        let rc_present = marker::is_installed(&original, shell.comment_leader());
        if !force && rc_present {
            return Ok(Outcome::AlreadyInstalled { script: None, rc: Some(rc.clone()) });
        }
        let body = shell.script(cmd);
        let new = marker::splice(&original, &body, shell.comment_leader());
        write_with_parents(rc, &new)?;
        return Ok(if force && rc_present {
            Outcome::Replaced { script: None, rc: Some(rc.clone()) }
        } else {
            Outcome::Installed { script: None, rc: Some(rc.clone()) }
        });
    }

    // Other shells: write script file, then (optionally) edit rc.
    let script_path = sp.script.as_ref().expect("non-powershell has script path");
    let script_body = shell.script(cmd);
    let script_existed = script_path.exists();

    let rc_present = if let Some(rc) = &sp.rc {
        let r = read_or_empty(rc)?;
        marker::is_installed(&r, shell.comment_leader())
    } else {
        false
    };

    if !force && script_existed && (sp.rc.is_none() || rc_present) {
        return Ok(Outcome::AlreadyInstalled {
            script: Some(script_path.clone()),
            rc: sp.rc.clone(),
        });
    }

    write_with_parents(script_path, &script_body)?;

    if let Some(rc) = &sp.rc {
        let original = read_or_empty(rc)?;
        let body = shell.rc_body(script_path).unwrap_or_default();
        let new = marker::splice(&original, &body, shell.comment_leader());
        write_with_parents(rc, &new)?;
    }

    Ok(if force && (script_existed || rc_present) {
        Outcome::Replaced { script: Some(script_path.clone()), rc: sp.rc.clone() }
    } else {
        Outcome::Installed { script: Some(script_path.clone()), rc: sp.rc.clone() }
    })
}

pub(crate) fn paths_for_selected(s: Selected, p: &dyn Paths) -> crate::completion::paths::ShellPaths {
    let cs = match s {
        Selected::Bash => clap_complete::Shell::Bash,
        Selected::Zsh => clap_complete::Shell::Zsh,
        Selected::Fish => clap_complete::Shell::Fish,
        Selected::Powershell => clap_complete::Shell::PowerShell,
        Selected::Elvish => clap_complete::Shell::Elvish,
        Selected::Nushell => {
            // Nushell isn't in clap_complete::Shell; we handle its paths
            // here directly.
            return crate::completion::paths::ShellPaths {
                script: Some(p.config().join("nushell/completions/bo4e.nu")),
                rc: Some(p.config().join("nushell/config.nu")),
            };
        }
    };
    paths_for(cs, p)
}

fn read_or_empty(p: &Path) -> io::Result<String> {
    match fs::read_to_string(p) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

fn write_with_parents(p: &Path, body: &str) -> io::Result<()> {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(p, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::base::Cli;
    use clap::CommandFactory;
    use tempfile::TempDir;

    struct FakePaths { home: PathBuf }
    impl Paths for FakePaths {
        fn home(&self) -> PathBuf { self.home.clone() }
    }

    fn fake(home: &TempDir) -> FakePaths {
        FakePaths { home: home.path().to_path_buf() }
    }

    #[test]
    fn bash_install_writes_script_and_edits_rcfile() {
        let home = TempDir::new().unwrap();
        let p = fake(&home);
        let mut cmd = Cli::command();
        let outcome = install(&mut cmd, Selected::Bash, &p, false).unwrap();
        assert!(matches!(outcome, Outcome::Installed { .. }));
        let script = home.path().join(".bash_completions/bo4e.sh");
        let rc = home.path().join(".bashrc");
        assert!(script.exists(), "script not written");
        let rc_body = fs::read_to_string(&rc).unwrap();
        assert!(rc_body.contains("# >>> bo4e completion >>>"));
        assert!(rc_body.contains("source '"));
    }

    #[test]
    fn bash_install_is_idempotent() {
        let home = TempDir::new().unwrap();
        let p = fake(&home);
        let mut cmd = Cli::command();
        install(&mut cmd, Selected::Bash, &p, false).unwrap();
        let outcome = install(&mut cmd, Selected::Bash, &p, false).unwrap();
        assert!(matches!(outcome, Outcome::AlreadyInstalled { .. }));
    }

    #[test]
    fn bash_install_force_replaces() {
        let home = TempDir::new().unwrap();
        let p = fake(&home);
        let mut cmd = Cli::command();
        install(&mut cmd, Selected::Bash, &p, false).unwrap();
        let outcome = install(&mut cmd, Selected::Bash, &p, true).unwrap();
        assert!(matches!(outcome, Outcome::Replaced { .. }));
    }

    #[test]
    fn fish_install_writes_script_no_rc_edit() {
        let home = TempDir::new().unwrap();
        let p = fake(&home);
        let mut cmd = Cli::command();
        install(&mut cmd, Selected::Fish, &p, false).unwrap();
        let script = home.path().join(".config/fish/completions/bo4e.fish");
        assert!(script.exists());
    }

    #[test]
    fn powershell_install_writes_to_profile_only() {
        let home = TempDir::new().unwrap();
        let p = fake(&home);
        let mut cmd = Cli::command();
        let outcome = install(&mut cmd, Selected::Powershell, &p, false).unwrap();
        // We don't assert the exact $PROFILE path here because
        // detect_powershell_profile may shell out; in CI with no pwsh
        // installed it falls back to the documented default.
        if let Outcome::Installed { rc: Some(rc), .. } = outcome {
            let body = fs::read_to_string(&rc).unwrap();
            assert!(body.contains("# >>> bo4e completion >>>"));
            assert!(body.contains("Register-ArgumentCompleter"));
        } else {
            panic!("expected Installed outcome");
        }
    }

    #[test]
    fn nushell_install_writes_script_and_edits_rc() {
        let home = TempDir::new().unwrap();
        let p = fake(&home);
        let mut cmd = Cli::command();
        install(&mut cmd, Selected::Nushell, &p, false).unwrap();
        let script = home.path().join(".config/nushell/completions/bo4e.nu");
        let rc = home.path().join(".config/nushell/config.nu");
        assert!(script.exists());
        let rc_body = fs::read_to_string(&rc).unwrap();
        assert!(rc_body.contains("source"));
    }

    #[test]
    fn powershell_install_force_on_fresh_returns_installed() {
        let home = TempDir::new().unwrap();
        let p = fake(&home);
        let mut cmd = Cli::command();
        let outcome = install(&mut cmd, Selected::Powershell, &p, true).unwrap();
        assert!(matches!(outcome, Outcome::Installed { .. }),
            "force=true on fresh install must return Installed, got: {:?}", outcome);
    }

    #[test]
    fn powershell_install_force_after_existing_returns_replaced() {
        let home = TempDir::new().unwrap();
        let p = fake(&home);
        let mut cmd = Cli::command();
        install(&mut cmd, Selected::Powershell, &p, false).unwrap();
        let outcome = install(&mut cmd, Selected::Powershell, &p, true).unwrap();
        assert!(matches!(outcome, Outcome::Replaced { .. }),
            "force=true after prior install must return Replaced, got: {:?}", outcome);
    }
}
