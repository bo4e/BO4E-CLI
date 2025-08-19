use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
//#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    Pull(Pull),
}

/// Pull all BO4E-JSON-schemas of a specific version.
///
/// Besides the json-files, a .version file will be created in utf-8 format at the root of
/// the output directory. This file is needed for other commands.
#[derive(Args)]
pub struct Pull {
    /// The BO4E-version tag to pull the data for.
    /// If none or "latest" is provided, the latest version will be queried from GitHub.
    /// They will be pulled from https://github.com/bo4e/BO4E-Schemas.
    #[arg(short = 't', long, default_value = "latest")]
    pub version_tag: String,

    /// The directory to save the JSON-schemas to.
    #[arg(
        short = 'o',
        long = "output",
        required = true,
        value_name = "OUTPUT_DIRECTORY"
    )]
    pub output_dir: PathBuf,

    /// Don't automatically update the references in the schemas.
    /// On default, online references to BO4E-schemas will be replaced by relative paths.
    /// To keep the online references, set this flag.
    #[arg(short = 'u', long)]
    pub no_update_refs: bool,

    /// Don't automatically clear the output directory before saving the schemas.
    #[arg(short = 'c', long)]
    pub no_clear_output: bool,

    /// A GitHub Access token to authenticate with the GitHub API.
    /// Use this if you have rate limiting problems with the GitHub API.
    /// It is encouraged to set the environment variable GITHUB_ACCESS_TOKEN instead to prevent
    /// accidentally storing your token into the shell history.
    /// Alternatively, if you have the GitHub CLI installed and
    /// the token can't be found in the environment variables,
    /// the token will be fetched from the GitHub CLI (if you are logged in). Uses `gh auth token`.
    #[arg(long, env = "GITHUB_ACCESS_TOKEN")]
    pub token: Option<String>,
}
