use crate::cli::base::Executable;
use clap::{Args, Subcommand};

/// Command group for interacting with the BO4E-python repository
/// (<https://github.com/bo4e/BO4E-python>). See 'repo --help' for more information.
#[derive(Args)]
pub struct Repo {
    #[command(subcommand)]
    pub command: RepoSubcommand,
}

#[derive(Subcommand)]
pub enum RepoSubcommand {
    Versions(VersionsArgs),
}

/// Get the last n versions of the BO4E-python repository starting from the given reference.
///
/// This command must be executed from the root of the BO4E-python repository. Technically, it
/// should also work on other repositories following the same versioning scheme, but it is
/// primarily intended for BO4E-python. Note that the command will not explicitly check if the
/// current directory is the root of the BO4E-python repository.
///
/// The output will contain the version tags in chronological descending order, i.e. the newest
/// version first. If executed without any arguments, it will return all versions on the main
/// branch since v202401.0.0.
#[derive(Args)]
pub struct VersionsArgs {
    /// Number of last versions to retrieve. If the number is set to 0, all versions will be
    /// retrieved up until v202401.0.0.
    #[arg(short = 'n', default_value_t = 0)]
    pub n: u32,

    /// The git reference object to start from. The reference can be a tag, branch or commit.
    /// From this point the last n versions will be retrieved. If the reference is a tag, the tag
    /// itself won't be included in the output. If the reference is neither a tag, branch nor a
    /// commit, all versions prior to the current checkout commit (i.e. "HEAD") will be retrieved.
    #[arg(short = 'r', long = "ref", default_value = "main")]
    #[cfg_attr(
        feature = "dynamic-completion",
        arg(add = clap_complete::engine::ArgValueCompleter::new(
            crate::completion::completers::git_refs::complete
        ))
    )]
    pub reference: String,

    /// Exclude release candidates from the output. If set to False, release candidates will be
    /// included in the output. Excluded elements don't count towards the number of versions to
    /// retrieve.
    #[arg(short = 'c', long, default_value_t = false)]
    pub exclude_candidates: bool,

    /// Exclude technical version bumps from the output. If set to False, technical bumps will be
    /// included in the output. Excluded elements don't count towards the number of versions to
    /// retrieve. From versions differing only in the technical version, the newest technical
    /// release will be returned.
    #[arg(short = 't', long, default_value_t = false)]
    pub exclude_technical_bumps: bool,

    /// If set, the full commit SHA will be shown in the output. Otherwise, only the first 6
    /// characters of the commit SHA will be shown. This option has no effect if the output is
    /// quiet (i.e. --quiet is set).
    #[arg(short = 's', long, default_value_t = false)]
    pub show_full_commit_sha: bool,

    /// Skip GitHub release validation (faster, fully offline).
    #[arg(long = "no-validate-releases", action = clap::ArgAction::SetFalse, default_value_t = true)]
    pub validate_releases: bool,

    /// GitHub token for the BO4E-Schemas release validation. Falls back to `gh auth token`.
    #[arg(long, env = "GITHUB_ACCESS_TOKEN")]
    pub token: Option<String>,
}

impl Executable for Repo {
    fn run(&self) -> Result<(), String> {
        match &self.command {
            RepoSubcommand::Versions(a) => run_versions(a),
        }
    }
}

/// Hand-rolled 3-column table writer. Prints to stdout.
fn render_table(title: &str, rows: &[(String, String, String)]) {
    println!("{title}");

    if rows.is_empty() {
        println!("{}", ::console::style("(no versions found)").italic());
        return;
    }

    let headers = ("Version", "Commit SHA", "Commit date");
    let widths = (
        rows.iter()
            .map(|r| r.0.len())
            .max()
            .unwrap_or(0)
            .max(headers.0.len()),
        rows.iter()
            .map(|r| r.1.len())
            .max()
            .unwrap_or(0)
            .max(headers.1.len()),
        rows.iter()
            .map(|r| r.2.len())
            .max()
            .unwrap_or(0)
            .max(headers.2.len()),
    );

    let bold = ::console::Style::new().bold();
    let dim = ::console::Style::new().dim();

    println!(
        "{}  {}  {}",
        bold.apply_to(format!("{:<w$}", headers.0, w = widths.0)),
        bold.apply_to(format!("{:<w$}", headers.1, w = widths.1)),
        bold.apply_to(format!("{:<w$}", headers.2, w = widths.2)),
    );

    for (i, (a, b, c)) in rows.iter().enumerate() {
        let s = if i % 2 == 1 {
            dim.clone()
        } else {
            ::console::Style::new()
        };
        println!(
            "{}  {}  {}",
            s.apply_to(format!("{:<w$}", a, w = widths.0)),
            s.apply_to(format!("{:<w$}", b, w = widths.1)),
            s.apply_to(format!("{:<w$}", c, w = widths.2)),
        );
    }
}

pub(crate) fn run_versions(args: &VersionsArgs) -> Result<(), String> {
    use crate::io::git::{
        GetLastNTagsOpts, get_commit_date, get_commit_sha, get_last_n_tags, parse_reference,
    };
    use crate::io::github::{get_token_from_github_cli, release_exists};
    use crate::models::git::Reference;
    use crate::utils::tokio::get_runtime;

    let parsed = parse_reference(args.reference.clone()).map_err(|e| e.to_string())?;

    let (resolved_ref, ref_display, skip_first) = match parsed {
        Reference::Tag(s) => {
            let display = s.clone();
            (s, display, true)
        }
        Reference::Branch(s) => {
            let display = format!("latest commit on branch {s}");
            (s, display, false)
        }
        Reference::Commit(s) => {
            let short: String = s.chars().take(6).collect();
            let display = format!("commit {short}");
            (s, display, false)
        }
        Reference::Head => ("HEAD".to_string(), "HEAD".to_string(), false),
    };

    let title = if args.n == 0 {
        format!("All versions between v202401.0.0 and {ref_display}")
    } else {
        format!("Last {} versions before {ref_display}", args.n)
    };

    // Resolve the token: prefer explicit --token / GITHUB_TOKEN, then fall back to gh CLI.
    let token: Option<String> = if args.validate_releases {
        match &args.token {
            Some(t) => Some(t.clone()),
            None => get_token_from_github_cli(),
        }
    } else {
        None
    };

    let versions = if args.validate_releases {
        let runtime = get_runtime();
        let token_ref = token.as_deref();
        get_last_n_tags(GetLastNTagsOpts {
            n: args.n,
            reference: &resolved_ref,
            exclude_candidates: args.exclude_candidates,
            exclude_technical_bumps: args.exclude_technical_bumps,
            skip_first,
            is_release: |v| runtime.block_on(release_exists(v, token_ref)),
        })?
    } else {
        get_last_n_tags(GetLastNTagsOpts {
            n: args.n,
            reference: &resolved_ref,
            exclude_candidates: args.exclude_candidates,
            exclude_technical_bumps: args.exclude_technical_bumps,
            skip_first,
            is_release: |_| Ok(true),
        })?
    };

    if args.n > 0 && (versions.len() as u32) < args.n {
        crate::cwarn!(
            "fewer than {} tags found from this reference; got {}",
            args.n,
            versions.len()
        );
    }

    // Quiet mode: plain stdout, no metadata fetches.
    if !crate::console::console::CONSOLE
        .get()
        .expect("CONSOLE")
        .would_emit(crate::console::console::Level::Normal)
    {
        for v in &versions {
            println!("{v}");
        }
        return Ok(());
    }

    // Non-quiet: fetch commit metadata and render the table.
    let mut rows: Vec<(String, String, String)> = Vec::with_capacity(versions.len());
    for v in &versions {
        let sha = get_commit_sha(&v.to_string()).map_err(|e| e.to_string())?;
        let displayed_sha = if args.show_full_commit_sha {
            sha.clone()
        } else {
            sha.chars().take(6).collect()
        };
        let date = get_commit_date(&sha).map_err(|e| e.to_string())?;
        rows.push((v.to_string(), displayed_sha, date));
    }

    crate::cprint_normal!(""); // blank line before the table for spacing
    render_table(&title, &rows);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_lock::CWD_LOCK;
    use std::process::Command;

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
            assert!(out.status.success(), "git {args:?} failed");
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
        std::env::set_current_dir(p).unwrap();
        (dir, guard)
    }

    fn ensure_console(level: crate::console::console::Level) {
        let _ = crate::console::console::CONSOLE.set(crate::console::console::Console::new(level));
    }

    #[test]
    fn test_run_versions_quiet_returns_versions_only() {
        let (_dir, _guard) = make_git_repo();
        ensure_console(crate::console::console::Level::Quiet);

        let args = VersionsArgs {
            n: 0,
            reference: "HEAD".into(),
            exclude_candidates: false,
            exclude_technical_bumps: false,
            show_full_commit_sha: false,
            validate_releases: false,
            token: None,
        };
        run_versions(&args).expect("run_versions failed");
        // We can't capture stdout from inside the test runner without extra plumbing.
        // The assertion is implicit: no panic, no error.
    }

    #[test]
    fn test_run_versions_non_quiet_renders_table() {
        let (_dir, _guard) = make_git_repo();
        ensure_console(crate::console::console::Level::Normal);

        let args = VersionsArgs {
            n: 0,
            reference: "HEAD".into(),
            exclude_candidates: false,
            exclude_technical_bumps: false,
            show_full_commit_sha: false,
            validate_releases: false,
            token: None,
        };
        run_versions(&args).expect("run_versions failed");
    }
}
