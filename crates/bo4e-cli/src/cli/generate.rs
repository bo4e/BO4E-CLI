use crate::cli::base::Executable;
use clap::{Args, Subcommand};
use std::path::PathBuf;

/// Generate BO4E models from the JSON-schemas in the input directory and save them in the
/// output directory.
///
/// Pick a flavour as the subcommand; `--help` per subcommand for flavour-specific options.
#[derive(Args)]
pub struct Generate {
    #[command(flatten)]
    pub common: GenerateCommon,

    #[command(subcommand)]
    pub flavour: GenerateFlavour,
}

#[derive(Args)]
pub struct GenerateCommon {
    /// The directory to read the JSON-schemas from.
    #[arg(short = 'i', long = "input", global = true)]
    pub input: Option<PathBuf>,

    /// The directory to save the generated code to.
    #[arg(short = 'o', long = "output", global = true)]
    pub output: Option<PathBuf>,

    /// Don't clear the output directory before saving the generated code.
    #[arg(long = "no-clear-output", action = clap::ArgAction::SetFalse, default_value_t = true, global = true)]
    pub clear_output: bool,

    /// Override embedded templates with a directory of Jinja templates.
    #[arg(long = "templates-dir", global = true)]
    pub templates_dir: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum GenerateFlavour {
    /// Pydantic-v2 Python models.
    #[cfg(feature = "python-pydantic")]
    PythonPydantic,
    /// SQLModel Python models (Pydantic + SQLAlchemy).
    #[cfg(feature = "python-sql-model")]
    PythonSqlModel,
    /// Rust types as loose files (mod.rs-rooted module tree). Consumer mounts it into their crate.
    #[cfg(feature = "rust-plain")]
    RustPlain,
    /// Rust types as a full self-contained Cargo crate.
    #[cfg(feature = "rust-crate")]
    RustCrate(RustCrateArgs),
}

#[derive(Args)]
pub struct RustCrateArgs {
    /// Cargo package name written into the generated Cargo.toml.
    #[arg(long = "crate-name", default_value = "bo4e")]
    pub crate_name: String,
}

impl Executable for Generate {
    fn run(&self) -> Result<(), String> {
        let input = self
            .common
            .input
            .as_ref()
            .ok_or_else(|| "missing required --input/-i".to_string())?;
        let output = self
            .common
            .output
            .as_ref()
            .ok_or_else(|| "missing required --output/-o".to_string())?;

        let out = bo4e_schemas::io::schemas::read_schemas(input)
            .map_err(|e| format!("failed to read schemas: {e}"))?;
        for w in &out.warnings {
            crate::cwarn!("{w}");
        }

        let opts = bo4e_codegen::Options {
            clear_output: self.common.clear_output,
            templates_dir: self.common.templates_dir.as_deref(),
        };

        let label = flavour_label(&self.flavour);
        let written = {
            let _spin = crate::console::spinner::squish(format!("Generating {label} output"));
            dispatch(&self.flavour, &out.schemas, output, &opts).map_err(|e| e.to_string())?
        };

        crate::cprint_normal!("Wrote {} files to {}", written.len(), output.display());
        Ok(())
    }
}

fn flavour_label(f: &GenerateFlavour) -> &'static str {
    #[allow(unreachable_patterns)]
    match f {
        #[cfg(feature = "python-pydantic")]
        GenerateFlavour::PythonPydantic => "python-pydantic",
        #[cfg(feature = "python-sql-model")]
        GenerateFlavour::PythonSqlModel => "python-sql-model",
        #[cfg(feature = "rust-plain")]
        GenerateFlavour::RustPlain => "rust-plain",
        #[cfg(feature = "rust-crate")]
        GenerateFlavour::RustCrate(_) => "rust-crate",
        #[allow(unreachable_patterns)]
        _ => unreachable!("GenerateFlavour variant not handled"),
    }
}

fn dispatch(
    flavour: &GenerateFlavour,
    schemas: &bo4e_schemas::Schemas,
    output: &std::path::Path,
    opts: &bo4e_codegen::Options<'_>,
) -> Result<Vec<PathBuf>, bo4e_codegen::Error> {
    #[allow(unreachable_patterns)]
    match flavour {
        #[cfg(feature = "python-pydantic")]
        GenerateFlavour::PythonPydantic => {
            bo4e_codegen::python::pydantic::generate(schemas, output, opts)
        }
        #[cfg(feature = "python-sql-model")]
        GenerateFlavour::PythonSqlModel => {
            bo4e_codegen::python::sql_model::generate(schemas, output, opts)
        }
        #[cfg(feature = "rust-plain")]
        GenerateFlavour::RustPlain => bo4e_codegen::rust::plain::generate(schemas, output, opts),
        #[cfg(feature = "rust-crate")]
        GenerateFlavour::RustCrate(_args) => {
            // Wired in Task 23.
            unimplemented!("rust-crate dispatch is wired in Task 23")
        }
        #[allow(unreachable_patterns)]
        _ => unreachable!("GenerateFlavour variant not handled"),
    }
}
