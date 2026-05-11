use crate::cli::base::Executable;
use crate::cprint_normal;
use crate::edit::update_refs::update_references_all;
use crate::io::cleanse::clear_dir_if_needed;
use crate::io::github::{get_schemas_from_github, resolve_latest_version};
use crate::models::cli::{Token, get_token_as_string};
use crate::utils::tokio::get_runtime;
use bo4e_schemas::io::schemas::write_schemas;
use bo4e_schemas::models::version::Version;
use clap::{Args, value_parser};
use std::path::PathBuf;
use std::str::FromStr;

/// Pull all BO4E-JSON-schemas of a specific version.
///
/// Beside the json-files a .version file will be created in utf-8 format at root of the output
/// directory. This file is needed for other commands.
#[derive(Args)]
pub struct Pull {
    /// The BO4E-version tag to pull the data for. If none is provided, the latest version will
    /// be queried from GitHub. They will be pulled from https://github.com/bo4e/BO4E-Schemas.
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

    /// Don't automatically update the references in the schemas. By default, online references to
    /// BO4E-schemas will be replaced by relative paths.
    #[arg(short = 'u', long, default_value_t = false)]
    pub no_update_refs: bool,

    /// Don't clear the output directory before saving the schemas.
    #[arg(short = 'c', long, default_value_t = false)]
    pub no_clear_output: bool,

    /// A GitHub Access token to authenticate with the GitHub API. Use this if you have rate
    /// limiting problems with the GitHub API. It is encouraged to set the environment variable
    /// GITHUB_ACCESS_TOKEN instead to prevent accidentally storing your token into the shell
    /// history. Alternatively, if you have the GitHub CLI installed and the token can't be found
    /// in the environment variables, the token will be fetched from the GitHub CLI (if you are
    /// logged in). Uses `gh auth token`.
    #[arg(long, env = "GITHUB_ACCESS_TOKEN", value_parser = value_parser!(Token), default_value = None)]
    pub token: Option<Token>,
}

impl Executable for Pull {
    fn run(&self) -> Result<(), String> {
        let token = get_token_as_string(&self.token);
        let token = token.as_deref();
        let runtime = get_runtime();
        let version = {
            if self.version_tag == "latest" {
                runtime.block_on(resolve_latest_version(token))?
            } else {
                let v = Version::from_str(&self.version_tag)?;
                cprint_normal!("Using version {}", v);
                v
            }
        };
        clear_dir_if_needed(&self.output_dir, !self.no_clear_output)
            .map_err(|err| err.to_string())?;
        let mut schemas = runtime.block_on(get_schemas_from_github(&version, token))?;
        if !self.no_update_refs {
            update_references_all(&mut schemas)?;
        }
        write_schemas(&schemas, self.output_dir.as_path())
            .map_err(|err| format!("Failed to write schemas to output directory: {}", err))
    }
}
