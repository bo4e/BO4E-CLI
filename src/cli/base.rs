use crate::cli::pull::Pull;
use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
//#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<SubcommandsLevel1>,
}

#[derive(Subcommand)]
pub enum SubcommandsLevel1 {
    Pull(Pull),
}
