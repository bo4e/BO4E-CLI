use clap::{Args, Parser, Subcommand};

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
    pub name: Option<String>,
}
