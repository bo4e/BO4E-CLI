use crate::cli::base::Executable;
use crate::console::console::{CONSOLE, Level};
use crate::console::spinner;
use crate::diff::diff::{DiffOptions, diff_schemas_with};
use crate::diff::matrix::{build_chain, create_compatibility_matrix};
use crate::diff::version::check_version_bump;
use crate::io::changes::{read_changes_from_diff_files, write_changes};
use crate::io::matrix::{write_compatibility_matrix_csv, write_compatibility_matrix_json};
use crate::{cerror, cprint_normal, cprint_verbose};
use bo4e_schemas::io::schemas::read_schemas;
use clap::{Args, Subcommand, ValueEnum, ValueHint};
use std::path::PathBuf;

/// Command group for comparing JSON-schemas of different BO4E versions.
/// See 'diff --help' for more information.
#[derive(Args)]
pub struct Diff {
    #[command(subcommand)]
    pub command: DiffSubcommand,
}

#[derive(Subcommand)]
pub enum DiffSubcommand {
    Schemas(DiffSchemasArgs),
    Matrix(DiffMatrixArgs),
    VersionBump(VersionBumpArgs),
}

/// Compare the JSON-schemas in the two input directories and save the differences to the output
/// file (JSON).
///
/// The output file will contain the differences in JSON-format. It will also contain information
/// about the compared versions.
#[derive(Args)]
pub struct DiffSchemasArgs {
    #[arg(value_hint = ValueHint::DirPath)]
    pub input_dir_base: PathBuf,
    #[arg(value_hint = ValueHint::DirPath)]
    pub input_dir_comp: PathBuf,
    /// The JSON-file to save the differences to.
    #[arg(short = 'o', long = "output", required = true, value_hint = ValueHint::FilePath)]
    pub output_file: PathBuf,
    /// Include version-related field differences in the diff output.
    ///
    /// By default, two kinds of differences are suppressed:
    ///   - description text that only differs in an inlined version string
    ///     (e.g. a documentation URL pointing at a different release tag);
    ///   - the default value of the `_version` field (which always carries
    ///     the schema's own version).
    ///
    /// Pass this flag to surface those differences as
    /// `FieldDescriptionChanged` / `FieldDefaultChanged` entries in the diff
    /// JSON — useful for a truly verbatim schema-to-schema diff. Note that
    /// `diff version-bump` consumes the diff JSON as-is, so opting in here
    /// will cause those changes to influence the inferred bump type too.
    #[arg(long = "include-version-changes", default_value_t = false)]
    pub include_version_changes: bool,
}

/// Create a difference matrix from the diff-files created by the 'diff schemas' command.
///
/// The data structure models a table where the columns are a list of ascending versions where
/// each column is a comparison to the version before. This means that the very first version
/// will not appear in the matrix as text. The rows will represent each model such that each
/// cell indicates how the model has changed between the two versions.
#[derive(Args)]
pub struct DiffMatrixArgs {
    /// An unordered list of Diff-files created by the 'diff schemas' command. At least one file
    /// must be provided.
    ///
    /// The versions inside these diff files must be consecutive and ascending. I.e. you have to
    /// be able to create an ascending series of versions from the versions in the diff files.
    /// E.g.:
    ///
    /// |      file 3      | -> |      file 1      | -> |      file 2      |
    ///
    /// | v1.0.0 -> v1.0.2 |    | v1.0.2 -> v1.3.0 |    | v1.3.0 -> v2.0.0 |
    #[arg(required = true, value_hint = ValueHint::FilePath)]
    pub input_diff_files: Vec<PathBuf>,
    /// The file to save the difference matrix to.
    #[arg(short = 'o', long = "output", required = true, value_hint = ValueHint::FilePath)]
    pub output_file: PathBuf,
    /// The type of the output file.
    #[arg(short = 't', long = "output-type", default_value = "csv")]
    pub output_type: MatrixOutputType,
    /// Whether to use emojis in the output file. If disabled, text will be used instead to
    /// indicate the type of change.
    #[arg(long = "use-emotes", default_value_t = false)]
    pub use_emotes: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum MatrixOutputType {
    Json,
    Csv,
}

/// Determine the release bump type according to a diff file created by 'diff schemas'.
///
/// Prints 'valid' to stdout if the version bump is valid. Otherwise, a descriptive error
/// message is printed. The bump type will be determined using the list of changes and compared
/// to the corresponding versions inside the diff file.
#[derive(Args)]
pub struct VersionBumpArgs {
    #[arg(value_hint = ValueHint::FilePath)]
    pub diff_file: PathBuf,
    /// Reject major version bumps.
    #[arg(long = "no-major", action = clap::ArgAction::SetFalse, default_value_t = true)]
    pub major_bump_allowed: bool,
}

impl Executable for Diff {
    fn run(&self) -> Result<(), String> {
        match &self.command {
            DiffSubcommand::Schemas(a) => run_schemas(a),
            DiffSubcommand::Matrix(a) => run_matrix(a),
            DiffSubcommand::VersionBump(a) => run_version_bump(a),
        }
    }
}

fn run_schemas(a: &DiffSchemasArgs) -> Result<(), String> {
    let out_old = read_schemas(&a.input_dir_base)?;
    for w in &out_old.warnings {
        crate::cwarn!("{w}");
    }
    let old = out_old.schemas;
    let out_new = read_schemas(&a.input_dir_comp)?;
    for w in &out_new.warnings {
        crate::cwarn!("{w}");
    }
    let new = out_new.schemas;
    let changes = {
        let _spin = spinner::squish("Comparing JSON-schemas...");
        diff_schemas_with(
            &old,
            &new,
            &DiffOptions {
                include_version_changes: a.include_version_changes,
            },
        )
    };
    cprint_normal!("Compared JSON-schemas.");
    write_changes(&changes, &a.output_file)?;
    cprint_normal!("Saved Diff to file: {}", a.output_file.display());
    Ok(())
}

fn run_matrix(a: &DiffMatrixArgs) -> Result<(), String> {
    cprint_verbose!(
        "Received {} diff file(s) in input order:",
        a.input_diff_files.len()
    );
    for (idx, p) in a.input_diff_files.iter().enumerate() {
        cprint_verbose!("  [{}] {}", idx, p.display());
    }
    let (chain, diff_versions) = {
        let _spin = spinner::squish("Reading changes from diff files...");
        let diffs = read_changes_from_diff_files(&a.input_diff_files)?;
        let parsed: Vec<(String, String)> = diffs
            .iter()
            .map(|d| (d.old_version().to_string(), d.new_version().to_string()))
            .collect();
        let chain = build_chain(diffs)?;
        (chain, parsed)
    };
    cprint_normal!("Read changes from diff files.");
    cprint_verbose!("Parsed diff files (input order, before chaining):");
    for (idx, (old, new)) in diff_versions.iter().enumerate() {
        cprint_verbose!("  [{}] {} -> {}", idx, old, new);
    }
    cprint_verbose!(
        "Detected version chain ({} version(s), {} edge(s)):",
        chain.nodes.len(),
        chain.edges.len()
    );
    for (idx, node) in chain.nodes.iter().enumerate() {
        cprint_verbose!("  [{}] {}", idx, node.version_key);
    }
    let matrix = {
        let _spin = spinner::squish("Creating compatibility matrix...");
        create_compatibility_matrix(&chain, a.use_emotes)
    };
    cprint_normal!("Created compatibility matrix.");

    let path: Vec<String> = chain.nodes.iter().map(|n| n.version_key.clone()).collect();
    {
        let _spin = spinner::squish(format!(
            "Saving compatibility matrix to file {} ...",
            a.output_file.display()
        ));
        match a.output_type {
            MatrixOutputType::Csv => {
                write_compatibility_matrix_csv(&a.output_file, &matrix, &path)?
            }
            MatrixOutputType::Json => write_compatibility_matrix_json(&a.output_file, &matrix)?,
        }
    }
    cprint_normal!(
        "Saved compatibility matrix to file {}.",
        a.output_file.display()
    );
    Ok(())
}

fn run_version_bump(a: &VersionBumpArgs) -> Result<(), String> {
    let mut diffs = read_changes_from_diff_files(std::slice::from_ref(&a.diff_file))?;
    let changes = diffs.pop().ok_or("Empty diff file list")?;
    match check_version_bump(&changes, a.major_bump_allowed) {
        Ok(kind) => {
            cprint_normal!("The version bump is valid ({} bump).", kind);
            Ok(())
        }
        Err(e) => {
            // Exit code is only nonzero in --quiet mode: non-quiet runs surface the failure on
            // stderr (always shown) but still exit 0 so interactive callers and shell pipelines
            // that don't care about the bump outcome are not poisoned. Quiet mode is the
            // scripted/CI path: there the exit code is the signal, so bubble the error up.
            let quiet = !CONSOLE
                .get()
                .expect("CONSOLE not initialized")
                .would_emit(Level::Normal);
            if quiet {
                Err(e)
            } else {
                cerror!("{e}");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{CONSOLE, Console, Level};
    use crate::models::changes::Changes;
    use bo4e_schemas::models::schema_meta::Schemas;
    use bo4e_schemas::models::version::DirtyVersion;
    use std::fs;

    fn ensure_console() {
        let _ = CONSOLE.set(Console::new(Level::Normal));
    }

    fn write_diff(path: &std::path::Path, old_v: &str, new_v: &str) -> Changes {
        let v_old: DirtyVersion = old_v.parse().unwrap();
        let v_new: DirtyVersion = new_v.parse().unwrap();
        let c = Changes {
            old_schemas: Schemas::new(v_old),
            new_schemas: Schemas::new(v_new),
            changes: vec![],
        };
        write_changes(&c, path).unwrap();
        c
    }

    #[test]
    fn test_run_version_bump_succeeds_on_valid_technical_bump() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("d.json");
        write_diff(&p, "v202401.0.1", "v202401.0.2");
        let args = VersionBumpArgs {
            diff_file: p,
            major_bump_allowed: true,
        };
        run_version_bump(&args).unwrap();
    }

    #[test]
    fn test_run_version_bump_swallows_error_in_non_quiet_mode() {
        // In non-quiet mode the failure goes to stderr (via cerror!) but the process exits 0
        // — see `run_version_bump`. Tests share a single CONSOLE (OnceLock) initialised to
        // Normal, so the quiet-path Err branch is exercised via the lower-level
        // `check_version_bump` tests instead.
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("d.json");
        write_diff(&p, "v202401.0.1+gabc", "v202401.0.2");
        let args = VersionBumpArgs {
            diff_file: p,
            major_bump_allowed: true,
        };
        assert!(run_version_bump(&args).is_ok());
    }

    #[test]
    fn test_run_matrix_writes_csv() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let in_path = dir.path().join("d.json");
        write_diff(&in_path, "v202401.0.1", "v202401.0.2");
        let out_path = dir.path().join("m.csv");
        let args = DiffMatrixArgs {
            input_diff_files: vec![in_path],
            output_file: out_path.clone(),
            output_type: MatrixOutputType::Csv,
            use_emotes: false,
        };
        run_matrix(&args).unwrap();
        assert!(out_path.exists());
        let text = fs::read_to_string(&out_path).unwrap();
        assert!(text.contains("v202401.0.1"));
    }

    fn write_minimal_schema_dir(dir: &std::path::Path, version: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(".version"), version).unwrap();
        let bo = dir.join("bo");
        std::fs::create_dir_all(&bo).unwrap();
        std::fs::write(
            bo.join("Angebot.json"),
            r#"{"type":"object","title":"Angebot","properties":{},"required":[],"additionalProperties":false}"#,
        )
        .unwrap();
    }

    #[test]
    fn test_end_to_end_schemas_then_matrix_then_version_bump() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base");
        let comp = dir.path().join("comp");
        write_minimal_schema_dir(&base, "v202401.0.1");
        write_minimal_schema_dir(&comp, "v202401.0.2");

        let diff_file = dir.path().join("diff.json");
        run_schemas(&DiffSchemasArgs {
            input_dir_base: base,
            input_dir_comp: comp,
            output_file: diff_file.clone(),
            include_version_changes: false,
        })
        .unwrap();
        assert!(diff_file.exists());

        let matrix_file = dir.path().join("m.csv");
        run_matrix(&DiffMatrixArgs {
            input_diff_files: vec![diff_file.clone()],
            output_file: matrix_file.clone(),
            output_type: MatrixOutputType::Csv,
            use_emotes: false,
        })
        .unwrap();
        assert!(matrix_file.exists());

        run_version_bump(&VersionBumpArgs {
            diff_file,
            major_bump_allowed: true,
        })
        .unwrap();
    }
}
