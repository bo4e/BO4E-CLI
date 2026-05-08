use crate::cli::base::Executable;
use clap::{Args, Subcommand};

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
/// This command must be executed from the root of a BO4E-python checkout.
#[derive(Args)]
pub struct VersionsArgs {
    /// Number of last versions to retrieve. 0 = all versions since v202401.0.0.
    #[arg(short = 'n', default_value_t = 0)]
    pub n: u32,

    /// Git reference to start from (tag, branch, commit, or "HEAD").
    /// Falls back to current HEAD if the value is none of those.
    #[arg(short = 'r', long = "ref", default_value = "main")]
    pub reference: String,

    /// Exclude release candidates from the output.
    #[arg(short = 'c', long, default_value_t = false)]
    pub exclude_candidates: bool,

    /// Exclude technical bumps; from each functional group, keep only the newest technical.
    #[arg(short = 't', long, default_value_t = false)]
    pub exclude_technical_bumps: bool,

    /// Show the full commit SHA. By default the SHA is truncated to 6 chars.
    #[arg(short = 's', long, default_value_t = false)]
    pub show_full_commit_sha: bool,

    /// Skip GitHub release validation (faster, fully offline).
    #[arg(long = "no-validate-releases", action = clap::ArgAction::SetFalse, default_value_t = true)]
    pub validate_releases: bool,

    /// GitHub token for the BO4E-Schemas release validation. Falls back to `gh auth token`.
    #[arg(long, env = "GITHUB_TOKEN")]
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
        rows.iter().map(|r| r.0.len()).max().unwrap_or(0).max(headers.0.len()),
        rows.iter().map(|r| r.1.len()).max().unwrap_or(0).max(headers.1.len()),
        rows.iter().map(|r| r.2.len()).max().unwrap_or(0).max(headers.2.len()),
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
        let s = if i % 2 == 1 { dim.clone() } else { ::console::Style::new() };
        println!(
            "{}  {}  {}",
            s.apply_to(format!("{:<w$}", a, w = widths.0)),
            s.apply_to(format!("{:<w$}", b, w = widths.1)),
            s.apply_to(format!("{:<w$}", c, w = widths.2)),
        );
    }
}

fn run_versions(args: &VersionsArgs) -> Result<(), String> {
    use crate::io::git::{GetLastNTagsOpts, get_commit_date, get_commit_sha, get_last_n_tags, get_ref};
    use crate::io::github::{get_token_from_github_cli, release_exists};
    use crate::models::git::RefKind;
    use crate::utils::tokio::get_runtime;

    let (ref_kind, resolved_ref) = get_ref(&args.reference).map_err(|e| e.to_string())?;

    let ref_display = match ref_kind {
        RefKind::Tag => resolved_ref.clone(),
        RefKind::Branch => format!("latest commit on branch {resolved_ref}"),
        RefKind::Commit => {
            let short: String = resolved_ref.chars().take(6).collect();
            format!("commit {short}")
        }
    };

    let title = if args.n == 0 {
        format!("All versions between v202401.0.0 and {ref_display}")
    } else {
        format!("Last {} versions before {ref_display}", args.n)
    };

    let skip_first = matches!(ref_kind, RefKind::Tag);

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
