use std::path::Path;
use tokio::io;
use tokio::process::Command;

pub async fn clone_repo(repo_url: &str, branch_or_tag: &str, dest: &Path) -> io::Result<()> {
    let output = Command::new("git")
        .args(["clone", "--branch", branch_or_tag, "--depth", "1", repo_url])
        .arg(dest.as_os_str())
        .output()
        .await?; // get exit status

    if output.status.success() {
        Ok(())
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Failed to clone repository.\nStdout: {}\nStderr: {}",
                stdout, stderr
            ),
        ))
    }
}
