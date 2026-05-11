use crate::cli::base::Executable;
use clap::Args;
use std::path::PathBuf;

/// Generate the BO4E models from the JSON-schemas in the input directory and save them in the
/// output directory.
///
/// Several output types are available, see --output-type.
#[derive(Args)]
pub struct Generate {
    /// The directory to read the JSON-schemas from.
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,

    /// The directory to save the generated code to.
    #[arg(short = 'o', long = "output")]
    pub output: PathBuf,

    /// The type of code to generate.
    #[arg(short = 't', long = "output-type", value_enum)]
    pub output_type: bo4e_codegen::OutputType,

    /// Don't clear the output directory before saving the generated code.
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

        let written = {
            let _spin = crate::console::spinner::squish(format!(
                "Generating {} output",
                self.output_type.as_str()
            ));

            bo4e_codegen::generate(
                &out.schemas,
                self.output_type,
                &self.output,
                &bo4e_codegen::Options {
                    clear_output: self.clear_output,
                    templates_dir: self.templates_dir.as_deref(),
                },
            )
            .map_err(|e| e.to_string())?
        };

        crate::cprint_normal!(
            "Wrote {} files to {}",
            written.len(),
            self.output.display()
        );
        Ok(())
    }
}
