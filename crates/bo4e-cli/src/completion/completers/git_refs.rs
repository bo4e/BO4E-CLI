use clap_complete::CompletionCandidate;
use std::process::Command;

pub fn complete(prefix: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
    let prefix = prefix.to_string_lossy().to_string();
    let mut candidates: Vec<String> = vec!["HEAD".to_string()];
    if let Ok(out) = Command::new("git")
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads", "refs/tags"])
        .output()
        && out.status.success()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            candidates.push(line.trim().to_string());
        }
    }
    candidates
        .into_iter()
        .filter(|c| c.starts_with(&prefix))
        .map(CompletionCandidate::new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::process::Command;
    use tempfile::TempDir;

    fn names(cs: Vec<CompletionCandidate>) -> Vec<String> {
        cs.iter().map(|c| c.get_value().to_string_lossy().to_string()).collect()
    }

    /// Run a closure with cwd set to `dir`, restoring the original cwd after.
    /// Uses a `Mutex` to serialise cwd-mutating tests across the file.
    fn with_cwd<F: FnOnce()>(dir: &std::path::Path, f: F) {
        use std::sync::Mutex;
        static CWD_LOCK: Mutex<()> = Mutex::new(());
        let _guard = CWD_LOCK.lock().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        std::env::set_current_dir(original).unwrap();
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    #[test]
    fn returns_head_at_minimum_when_in_git_repo() {
        let td = TempDir::new().unwrap();
        let _ = Command::new("git").arg("init").current_dir(td.path()).output();
        with_cwd(td.path(), || {
            let names = names(complete(&OsString::from("")));
            assert!(names.contains(&"HEAD".to_string()));
        });
    }

    #[test]
    fn prefix_filter_works() {
        let td = TempDir::new().unwrap();
        let _ = Command::new("git").arg("init").current_dir(td.path()).output();
        with_cwd(td.path(), || {
            let names = names(complete(&OsString::from("HEA")));
            assert!(names.iter().all(|n| n.starts_with("HEA")));
        });
    }

    #[test]
    fn returns_only_head_in_non_git_dir() {
        // A fresh tempdir with no `git init` — `git for-each-ref` will error,
        // so only the literal "HEAD" is returned.
        let td = TempDir::new().unwrap();
        with_cwd(td.path(), || {
            let names = names(complete(&OsString::from("")));
            assert_eq!(names, vec!["HEAD".to_string()]);
        });
    }
}
