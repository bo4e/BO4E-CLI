// crates/bo4e-cli/src/completion/uninstall.rs
use crate::completion::marker;
use crate::completion::paths::Paths;
use crate::completion::shells::Selected;
use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Outcome {
    /// Both script + rc-block were absent.
    NothingToRemove { script: Option<PathBuf>, rc: Option<PathBuf> },
    /// Removed at least one of script / rc-block; lists what was removed.
    Removed { script_removed: Option<PathBuf>, rc_changed: Option<PathBuf> },
}

pub fn uninstall(shell: Selected, paths: &dyn Paths) -> io::Result<Outcome> {
    let sp = super::install::paths_for_selected(shell, paths);
    let mut script_removed = None;
    let mut rc_changed = None;

    if let Some(script) = &sp.script
        && script.exists()
    {
        fs::remove_file(script)?;
        script_removed = Some(script.clone());
    }

    if let Some(rc) = &sp.rc
        && let Ok(body) = fs::read_to_string(rc)
    {
        let (new, present) = marker::strip(&body, shell.comment_leader());
        if present {
            fs::write(rc, new)?;
            rc_changed = Some(rc.clone());
        }
    }

    if script_removed.is_none() && rc_changed.is_none() {
        Ok(Outcome::NothingToRemove { script: sp.script, rc: sp.rc })
    } else {
        Ok(Outcome::Removed { script_removed, rc_changed })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::base::Cli;
    use crate::completion::install::install;
    use clap::CommandFactory;
    use tempfile::TempDir;

    struct FakePaths { home: PathBuf }
    impl Paths for FakePaths {
        fn home(&self) -> PathBuf { self.home.clone() }
    }

    #[test]
    fn uninstall_removes_script_and_rc_block() {
        let home = TempDir::new().unwrap();
        let p = FakePaths { home: home.path().to_path_buf() };
        let mut cmd = Cli::command();
        install(&mut cmd, Selected::Bash, &p, false).unwrap();
        let outcome = uninstall(Selected::Bash, &p).unwrap();
        assert!(matches!(outcome, Outcome::Removed { .. }));
        assert!(!home.path().join(".bash_completions/bo4e.sh").exists());
        let rc = fs::read_to_string(home.path().join(".bashrc")).unwrap();
        assert!(!rc.contains("# >>> bo4e completion >>>"));
    }

    #[test]
    fn uninstall_reports_nothing_when_absent() {
        let home = TempDir::new().unwrap();
        let p = FakePaths { home: home.path().to_path_buf() };
        let outcome = uninstall(Selected::Bash, &p).unwrap();
        assert!(matches!(outcome, Outcome::NothingToRemove { .. }));
    }
}
