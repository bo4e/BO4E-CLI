use crate::models::git::Reference;
use std::io;
use std::path::Path;
use std::process::{Command, Output};

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

pub fn clone_repo(repo_url: &str, branch_or_tag: &str, dest: &Path) -> io::Result<()> {
    let output = Command::new("git")
        .args(["clone", "--branch", branch_or_tag, "--depth", "1", repo_url])
        .arg(dest.as_os_str())
        .output()?; // get exit status

    check_success(&output, "Failed to clone repository.")
}

fn is_version_tag(value: &str) -> io::Result<bool> {
    Command::new("git")
        .args(["show-ref", "--quiet", &format!("refs/tags/{value}")])
        .status()
        .map(|exit_status| exit_status.success())
}

fn is_branch(value: &str) -> io::Result<bool> {
    Command::new("git")
        .args([
            "show-ref",
            "--quiet",
            &format!("refs/remotes/origin/{value}"),
        ])
        .status()
        .map(|exit_status| exit_status.success())
}

fn is_commit_hash(value: &str) -> io::Result<bool> {
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

    check_success(&output, "Failed to get branches containing commit.")?;
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

fn get_commit_sha(branch_or_tag: &str) -> io::Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", branch_or_tag])
        .output()?;

    check_success(&output, "Failed to get commit SHA.")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_commit_date(commit: &str) -> io::Result<String> {
    let output = Command::new("git")
        .args(["show", "-s", "--format=%ci", commit])
        .output()?;

    check_success(&output, "Failed to get commit date.")?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

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

// def get_last_n_tags(
//     n: int,
//     *,
//     ref: str = "main",
//     exclude_candidates: bool = True,
//     exclude_technical_bumps: bool = False,
//     token: str | None = None,
// ) -> Iterable[Version]:
//     """
//     Get the last n tags in chronological descending order starting from `ref`.
//     If `ref` is a branch, it will start from the current HEAD of the branch.
//     If `ref` is a tag, it will start from the tag itself. But the tag itself will not be included in the output.
//     If `ref` is neither nor, the main branch will be used as fallback.
//     If `exclude_candidates` is True, candidate versions will be excluded from the output.
//     If the number of found versions is less than `n`, a warning will be logged.
//     If n=0, all versions since v202401.0.0 will be taken into account.
//     If exclude_technical_bumps is True, from each functional release group,
//     the highest technical release will be returned.
//     """

fn get_last_n_tags(
    n: u8,
    ref_value: String,
    token: Option<&str>,
    exclude_candidates: bool,
    exclude_technical_bumps: bool,
) -> io::Result<Vec<Reference>> {
    let reference = parse_reference(ref_value.to_string())?;
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "This function is not implemented yet.",
    ))
}
