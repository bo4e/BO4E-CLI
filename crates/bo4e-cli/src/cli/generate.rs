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
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,

    /// The directory to save the generated code to.
    #[arg(short = 'o', long = "output")]
    pub output: PathBuf,

    /// Don't clear the output directory before saving the generated code.
    #[arg(long = "no-clear-output", action = clap::ArgAction::SetFalse, default_value_t = true)]
    pub clear_output: bool,

    /// Override embedded templates with a directory of Jinja templates.
    #[arg(long = "templates-dir")]
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
    #[arg(long = "crate-name", default_value = "bo4e", value_parser = parse_crate_name)]
    pub crate_name: String,
}

/// clap value-parser for `--crate-name`. Accepts the same shape that Cargo
/// itself accepts as a `[package].name`: starts with an ASCII letter or
/// underscore, followed by ASCII alphanumerics / underscores / hyphens.
/// Length capped at 64 to keep generated `Cargo.toml` lines reasonable and
/// to block pathological inputs. Rejecting at parse time prevents TOML
/// injection through the Cargo.toml template (quotes, newlines, etc.).
fn parse_crate_name(s: &str) -> Result<String, String> {
    const MAX_LEN: usize = 64;
    if s.is_empty() {
        return Err("crate name cannot be empty".into());
    }
    if s.len() > MAX_LEN {
        return Err(format!(
            "crate name too long ({} chars); cap is {MAX_LEN}",
            s.len()
        ));
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "crate name `{s}` must start with an ASCII letter or `_`",
        ));
    }
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(format!(
                "crate name `{s}` contains invalid character `{c}`; \
                 only ASCII alphanumerics, `_`, and `-` are allowed",
            ));
        }
    }
    Ok(s.to_string())
}

impl Executable for Generate {
    fn run(&self) -> Result<(), String> {
        let input = &self.common.input;
        let output = &self.common.output;

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
        let bo4e_codegen::GenerateOutput {
            written,
            diagnostics,
        } = {
            let _spin = crate::console::spinner::squish(format!("Generating {label} output"));
            dispatch(&self.flavour, &out.schemas, output, &opts).map_err(|e| e.to_string())?
        };

        for d in &diagnostics {
            crate::cprint_verbose!("{}", d);
        }
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
) -> Result<bo4e_codegen::GenerateOutput, bo4e_codegen::Error> {
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
        GenerateFlavour::RustCrate(args) => bo4e_codegen::rust::crate_::generate(
            schemas,
            output,
            opts,
            &bo4e_codegen::RustCrateOptions {
                crate_name: args.crate_name.clone(),
            },
        ),
        #[allow(unreachable_patterns)]
        _ => unreachable!("GenerateFlavour variant not handled"),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_crate_name;

    #[test]
    fn parse_crate_name_accepts_typical_names() {
        for ok in ["bo4e", "my_crate", "my-crate", "_leading", "a1", "A_B-c"] {
            assert!(parse_crate_name(ok).is_ok(), "expected {ok:?} to be valid");
        }
    }

    #[test]
    fn parse_crate_name_rejects_empty_and_too_long() {
        assert!(parse_crate_name("").is_err());
        let too_long = "a".repeat(65);
        assert!(parse_crate_name(&too_long).is_err());
    }

    #[test]
    fn parse_crate_name_rejects_bad_first_char() {
        for bad in ["1abc", "-abc", " abc"] {
            assert!(parse_crate_name(bad).is_err(), "expected {bad:?} rejected");
        }
    }

    #[test]
    fn parse_crate_name_rejects_toml_injection_payloads() {
        for evil in ["evil\"name", "a\nb", "a b", "a$b", "a.b", "a/b", "a\\b"] {
            assert!(
                parse_crate_name(evil).is_err(),
                "expected {evil:?} rejected"
            );
        }
    }
}
