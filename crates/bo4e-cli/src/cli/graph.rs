use crate::cli::base::Executable;
use crate::graph::extract::extract;
use crate::io::graph::{write_graph_graphml, write_graph_json};
use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Generate diagrams and machine-readable graphs from BO4E schemas.
#[derive(Args)]
pub struct Graph {
    #[command(subcommand)]
    pub command: GraphSubcommand,
}

#[derive(Subcommand)]
pub enum GraphSubcommand {
    Extract(ExtractArgs),
    // Overview / Single subcommands added in Tasks 14 and 15.
}

/// Build a directed graph of BO4E class references and write it as JSON or GraphML.
#[derive(Args)]
pub struct ExtractArgs {
    /// BO4E schemas directory (the kind written by `bo4e pull`).
    #[arg(short = 'i', long = "input", required = true)]
    pub input_dir: PathBuf,
    /// Output file.
    #[arg(short = 'o', long = "output", required = true)]
    pub output_file: PathBuf,
    /// Output format.
    #[arg(long = "format", default_value = "json")]
    pub format: GraphFormat,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum GraphFormat {
    Json,
    Graphml,
}

impl Executable for Graph {
    fn run(&self) -> Result<(), String> {
        match &self.command {
            GraphSubcommand::Extract(a) => run_extract(a),
        }
    }
}

fn run_extract(a: &ExtractArgs) -> Result<(), String> {
    let out = bo4e_schemas::io::schemas::read_schemas(&a.input_dir)?;
    for w in &out.warnings {
        crate::cwarn!("{w}");
    }
    let schemas = out.schemas;
    let g = extract(&schemas)?;
    match a.format {
        GraphFormat::Json => write_graph_json(&g, &a.output_file)?,
        GraphFormat::Graphml => write_graph_graphml(&g, &a.output_file)?,
    }
    crate::cprint_normal!("Wrote graph to {}", a.output_file.display());
    Ok(())
}
