mod env;
mod error;
pub mod imports;
pub mod layout;
pub mod naming;
pub mod refs;

#[cfg(any(feature = "python-pydantic", feature = "python-sql-model",))]
pub mod python;

#[cfg(any(feature = "rust-plain", feature = "rust-crate"))]
pub mod rust;

pub use error::Error;

use std::path::Path;

#[derive(Debug, Default)]
pub struct Options<'a> {
    pub clear_output: bool,
    pub templates_dir: Option<&'a Path>,
}

#[cfg(feature = "rust-crate")]
#[derive(Debug)]
pub struct RustCrateOptions {
    pub crate_name: String,
}

#[cfg(feature = "rust-crate")]
impl Default for RustCrateOptions {
    fn default() -> Self {
        Self {
            crate_name: "bo4e".to_string(),
        }
    }
}

/// Result of a `generate` call: list of files written plus optional diagnostics
/// (info-level messages — per-file decisions and similar — that callers can surface
/// via verbose output).
#[derive(Debug, Default)]
pub struct GenerateOutput {
    pub written: Vec<std::path::PathBuf>,
    pub diagnostics: Vec<String>,
}

pub(crate) fn clear_dir_if_exists(dir: &Path) -> Result<(), Error> {
    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
    } else {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
