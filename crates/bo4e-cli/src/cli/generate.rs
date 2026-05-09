use crate::cli::base::Executable;
use clap::Args;
use std::path::PathBuf;

/// Generate code from BO4E JSON schemas. Same flag set as the Python CLI plus
/// an optional `--templates-dir` override for the embedded MiniJinja templates.
#[derive(Args)]
pub struct Generate {
    /// Directory containing input JSON schemas.
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,

    /// Directory to write generated code to.
    #[arg(short = 'o', long = "output")]
    pub output: PathBuf,

    /// Output type. Variants are gated by Cargo features.
    #[arg(short = 't', long = "output-type", value_enum)]
    pub output_type: bo4e_codegen::OutputType,

    /// Skip clearing the output directory before writing.
    #[arg(long = "no-clear-output", action = clap::ArgAction::SetFalse, default_value_t = true)]
    pub clear_output: bool,

    /// Override embedded templates with a directory of Jinja templates.
    #[arg(long = "templates-dir")]
    pub templates_dir: Option<PathBuf>,
}

impl Executable for Generate {
    fn run(&self) -> Result<(), String> {
        let out = bo4e_schemas::io::schemas::read_schemas(&self.input)
            .map_err(|e| format!("failed to read schemas: {e}"))?;
        for w in &out.warnings {
            crate::cwarn!("{w}");
        }

        bo4e_codegen::generate(
            &out.schemas,
            self.output_type,
            &self.output,
            &bo4e_codegen::Options {
                clear_output: self.clear_output,
                templates_dir: self.templates_dir.as_deref(),
            },
        )
        .map_err(|e| e.to_string())
    }
}
