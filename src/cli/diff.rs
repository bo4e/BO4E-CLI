use crate::cli::base::Executable;
use crate::cprint_normal;
use crate::diff::diff::diff_schemas;
use crate::diff::matrix::{build_chain, create_compatibility_matrix};
use crate::diff::version::check_version_bump;
use crate::io::changes::{read_changes_from_diff_files, write_changes};
use crate::io::matrix::{write_compatibility_matrix_csv, write_compatibility_matrix_json};
use crate::io::schemas::read_schemas;
use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

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

#[derive(Args)]
pub struct DiffSchemasArgs {
    /// Baseline directory of JSON schemas (the "old" side).
    pub input_dir_base: PathBuf,
    /// Directory of JSON schemas to compare against the baseline (the "new" side).
    pub input_dir_comp: PathBuf,
    /// Output diff JSON file.
    #[arg(short = 'o', long = "output", required = true)]
    pub output_file: PathBuf,
}

#[derive(Args)]
pub struct DiffMatrixArgs {
    /// One or more diff JSON files. Order does not matter.
    #[arg(required = true)]
    pub input_diff_files: Vec<PathBuf>,
    /// Output file path (CSV or JSON).
    #[arg(short = 'o', long = "output", required = true)]
    pub output_file: PathBuf,
    /// Output format.
    #[arg(short = 't', long = "output-type", default_value = "csv")]
    pub output_type: MatrixOutputType,
    /// Use emoji symbols instead of plain-text labels.
    #[arg(long = "use-emotes", default_value_t = false)]
    pub use_emotes: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum MatrixOutputType {
    Json,
    Csv,
}

#[derive(Args)]
pub struct VersionBumpArgs {
    /// Diff JSON file to validate.
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
    let old = read_schemas(&a.input_dir_base)?;
    let new = read_schemas(&a.input_dir_comp)?;
    cprint_normal!("Comparing JSON-schemas...");
    let changes = diff_schemas(&old, &new);
    cprint_normal!("Compared JSON-schemas.");
    write_changes(&changes, &a.output_file)?;
    cprint_normal!("Saved Diff to file: {}", a.output_file.display());
    Ok(())
}

fn run_matrix(a: &DiffMatrixArgs) -> Result<(), String> {
    let diffs = read_changes_from_diff_files(&a.input_diff_files)?;
    let chain = build_chain(diffs)?;
    let matrix = create_compatibility_matrix(&chain, a.use_emotes);
    let path: Vec<String> = chain.nodes.iter().map(|n| n.version_key.clone()).collect();
    match a.output_type {
        MatrixOutputType::Csv => write_compatibility_matrix_csv(&a.output_file, &matrix, &path)?,
        MatrixOutputType::Json => write_compatibility_matrix_json(&a.output_file, &matrix)?,
    }
    cprint_normal!("Saved compatibility matrix to: {}", a.output_file.display());
    Ok(())
}

fn run_version_bump(a: &VersionBumpArgs) -> Result<(), String> {
    let mut diffs = read_changes_from_diff_files(std::slice::from_ref(&a.diff_file))?;
    let changes = diffs.pop().ok_or("Empty diff file list")?;
    let kind = check_version_bump(&changes, a.major_bump_allowed)?;
    cprint_normal!("Valid {:?} version bump.", kind);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{CONSOLE, Console, Level};
    use crate::models::changes::Changes;
    use crate::models::schema_meta::Schemas;
    use crate::models::version::DirtyVersion;
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
    fn test_run_version_bump_errors_on_dirty_baseline() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("d.json");
        write_diff(&p, "v202401.0.1+gabc", "v202401.0.2");
        let args = VersionBumpArgs {
            diff_file: p,
            major_bump_allowed: true,
        };
        assert!(run_version_bump(&args).is_err());
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
