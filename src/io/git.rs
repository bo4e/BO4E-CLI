use crate::models::git::{RefKind, Reference};
use crate::models::version::Version;
use crate::repo::filter::{FilterOptions, filter_tags};
use std::io;
use std::path::Path;
use std::process::{Command, Output};
use std::str::FromStr;

fn check_success(output: &Output, error_message: &str) -> io::Result<()> {
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{error_message}\nStdout: {stdout}\nStderr: {stderr}"),
        ))
    } else {
        Ok(())
    }
}

#[allow(dead_code)]
pub fn clone_repo(repo_url: &str, branch_or_tag: &str, dest: &Path) -> io::Result<()> {
    let output = Command::new("git")
        .args(["clone", "--branch", branch_or_tag, "--depth", "1", repo_url])
        .arg(dest.as_os_str())
        .output()?; // get exit status

    check_success(&output, "Failed to clone repository.")
}

pub fn is_version_tag(value: &str) -> io::Result<bool> {
    Command::new("git")
        .args(["show-ref", "--quiet", &format!("refs/tags/{value}")])
        .status()
        .map(|exit_status| exit_status.success())
}

pub fn is_branch(value: &str) -> io::Result<bool> {
    // Check local branches first, then remote-tracking branches.
    let local = Command::new("git")
        .args(["show-ref", "--quiet", &format!("refs/heads/{value}")])
        .status()
        .map(|s| s.success())?;
    if local {
        return Ok(true);
    }
    Command::new("git")
        .args([
            "show-ref",
            "--quiet",
            &format!("refs/remotes/origin/{value}"),
        ])
        .status()
        .map(|exit_status| exit_status.success())
}

/// Returns true if `value` resolves to a commit (any ref `git branch --contains` accepts).
///
/// Note: this returns true for tag and branch names too. Call this after
/// `is_version_tag` and `is_branch` if you want strict "is this a raw commit hash" classification.
pub fn is_commit_hash(value: &str) -> io::Result<bool> {
    match get_branches_containing_commit(value) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}

fn get_branches_containing_commit(commit: &str) -> io::Result<Vec<String>> {
    let output = Command::new("git")
        .args(["branch", "-a", "--contains", commit])
        .output()?;

    // `git branch --contains` may print "error: …" to stderr and exit non-zero for
    // unknown/malformed refs — treat these as "not a commit" rather than a hard error.
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No branches found containing the specified commit.",
        ));
    }
    let output = String::from_utf8_lossy(&output.stdout);
    let output = output.trim();
    if output.starts_with("error: no such commit") {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No branches found containing the specified commit.",
        ));
    }
    Ok(output
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim_start().to_string())
        .collect())
}

pub fn get_commit_sha(branch_or_tag: &str) -> io::Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", branch_or_tag])
        .output()?;

    check_success(&output, "Failed to get commit SHA.")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn get_commit_date(commit: &str) -> io::Result<String> {
    let output = Command::new("git")
        .args(["show", "-s", "--format=%ci", commit])
        .output()?;

    check_success(&output, "Failed to get commit date.")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[allow(dead_code)]
fn parse_reference(value: String) -> io::Result<Reference> {
    if is_version_tag(&value)? {
        Ok(Reference::Tag(value))
    } else if is_branch(&value)? {
        Ok(Reference::Branch(value))
    } else if is_commit_hash(&value)? {
        Ok(Reference::Commit(value))
    } else if value == "HEAD" {
        Ok(Reference::Head)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Supplied value is not a valid branch, tag, commit hash or the string literal 'HEAD'.",
        ))
    }
}

pub struct GetLastNTagsOpts<'a, F>
where
    F: FnMut(&Version) -> Result<bool, String>,
{
    pub n: u32,
    pub reference: &'a str,
    pub exclude_candidates: bool,
    pub exclude_technical_bumps: bool,
    pub skip_first: bool,
    pub is_release: F,
}

pub fn get_last_n_tags<F>(opts: GetLastNTagsOpts<'_, F>) -> Result<Vec<Version>, String>
where
    F: FnMut(&Version) -> Result<bool, String>,
{
    let raw = tags_merged(opts.reference).map_err(|e| e.to_string())?;

    let mut candidates: Vec<Version> = Vec::with_capacity(raw.len());
    for tag in raw {
        match Version::from_str(&tag) {
            Ok(v) => candidates.push(v),
            Err(_) => crate::cwarn!("skipping unparseable tag '{tag}'"),
        }
    }

    let filter_opts = FilterOptions {
        n: opts.n,
        exclude_candidates: opts.exclude_candidates,
        exclude_technical_bumps: opts.exclude_technical_bumps,
        skip_first: opts.skip_first,
        threshold: Version::from_str("v202401.0.0").expect("hardcoded threshold parses"),
    };

    filter_tags(&candidates, &filter_opts, opts.is_release)
}

pub fn tags_merged(reference: &str) -> io::Result<Vec<String>> {
    let output = Command::new("git")
        .args([
            "tag",
            "--merged",
            reference,
            "--sort=-version:refname",
            "--sort=-creatordate",
        ])
        .output()?;
    check_success(&output, "Failed to list merged tags.")?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect())
}

/// Classify a user-supplied ref string as a tag, branch, or commit, and resolve it.
///
/// Precedence (when a value matches multiple): tag → branch → commit. If the value
/// resolves to none of these, falls back to the current HEAD's SHA and emits an
/// info message via `cprint_normal!`.
///
/// For `RefKind::Commit`, the returned string is always a concrete 40-char SHA —
/// shorthand like `HEAD` or `HEAD~3` is resolved via `git rev-parse`.
pub fn get_ref(value: &str) -> io::Result<(RefKind, String)> {
    if is_version_tag(value)? {
        return Ok((RefKind::Tag, value.to_string()));
    }
    if is_branch(value)? {
        return Ok((RefKind::Branch, value.to_string()));
    }
    if is_commit_hash(value)? {
        return Ok((RefKind::Commit, get_commit_sha(value)?));
    }
    let cur = get_commit_sha("HEAD")?;
    let short: String = cur.chars().take(7).collect();
    crate::cprint_normal!("'{value}' is not a tag, branch, or commit; falling back to HEAD ({short}).");
    Ok((RefKind::Commit, cur))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::Mutex;

    // Serializes tests that mutate process cwd. Cargo runs tests in parallel by default;
    // any test in this module that calls set_current_dir must hold this lock.
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    /// Initialize a git repo with 3 tagged commits, acquire the cwd lock, and set
    /// the process cwd to the tempdir. Returns both so the caller holds them for the
    /// test's full lifetime — dropping either ends the exclusive window.
    fn make_git_repo() -> (tempfile::TempDir, std::sync::MutexGuard<'static, ()>) {
        let guard = CWD_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let run = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(p)
                .output()
                .expect("git invocation failed");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@t.t"]);
        run(&["config", "user.name", "t"]);
        run(&["commit", "--allow-empty", "-m", "c1", "-q"]);
        run(&["tag", "v202401.0.1"]);
        run(&["commit", "--allow-empty", "-m", "c2", "-q"]);
        run(&["tag", "v202401.0.2"]);
        run(&["commit", "--allow-empty", "-m", "c3", "-q"]);
        run(&["tag", "v202401.1.0"]);
        run(&["tag", "not-a-version"]);
        std::env::set_current_dir(p).unwrap();
        (dir, guard)
    }

    #[test]
    fn test_tags_merged_returns_descending_version_order() {
        let (_dir, _guard) = make_git_repo();
        let tags = tags_merged("HEAD").unwrap();
        // Descending: 1.0 first, then 0.2, 0.1, then non-version tag.
        assert_eq!(tags[0], "v202401.1.0");
        assert!(tags.contains(&"v202401.0.2".to_string()));
        assert!(tags.contains(&"v202401.0.1".to_string()));
        assert!(tags.contains(&"not-a-version".to_string()));
    }

    #[test]
    fn test_get_ref_classifies_tag_branch_and_falls_back_to_head() {
        let (_dir, _guard) = make_git_repo();

        // Existing console for the fallback's info message.
        use crate::console::console::{Console, Level, CONSOLE};
        let _ = CONSOLE.set(Console::new(Level::Quiet));

        let (kind, value) = get_ref("v202401.0.1").unwrap();
        assert_eq!(kind, crate::models::git::RefKind::Tag);
        assert_eq!(value, "v202401.0.1");

        let (kind, value) = get_ref("main").unwrap();
        assert_eq!(kind, crate::models::git::RefKind::Branch);
        assert_eq!(value, "main");

        // Unknown value → fallback to HEAD's commit SHA.
        let (kind, value) = get_ref("definitely-not-a-ref").unwrap();
        assert_eq!(kind, crate::models::git::RefKind::Commit);
        assert_eq!(value.len(), 40); // full SHA from `git rev-parse HEAD`

        // "HEAD" should resolve to a concrete SHA, not pass through as a literal.
        let (kind, value) = get_ref("HEAD").unwrap();
        assert_eq!(kind, crate::models::git::RefKind::Commit);
        assert_eq!(value.len(), 40);
        assert!(value.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_get_last_n_tags_returns_valid_versions_in_order() {
        let (_dir, _guard) = make_git_repo();

        use crate::console::console::{Console, Level, CONSOLE};
        let _ = CONSOLE.set(Console::new(Level::Quiet));

        let opts = GetLastNTagsOpts {
            n: 0,
            reference: "HEAD",
            exclude_candidates: false,
            exclude_technical_bumps: false,
            skip_first: false,
            is_release: |_| Ok(true),
        };
        let out = get_last_n_tags(opts).unwrap();
        // The 'not-a-version' tag is dropped; 3 valid versions remain in descending order.
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].to_string(), "v202401.1.0");
        assert_eq!(out[1].to_string(), "v202401.0.2");
        assert_eq!(out[2].to_string(), "v202401.0.1");
    }
}
