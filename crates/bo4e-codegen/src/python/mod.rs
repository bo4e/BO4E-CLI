use crate::error::Error;
use crate::naming::module_file_name;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(crate) mod imports;
pub(crate) mod types;

#[cfg(feature = "python-pydantic")]
pub(crate) mod pydantic;

#[cfg(feature = "python-sql-model")]
pub(crate) mod sql_model;

/// Compute the output directory, file name, and import depth for a schema with the
/// given module path (e.g. `["bo", "Angebot"]`). Pure — does not touch the filesystem.
///
/// Returns `(out_dir, file_name, depth)` where `depth` is the relative-import depth
/// suitable for both `ImportBlock::render(depth)` (pydantic) and the `..`-prefix
/// repetition used by the sql-model renderer (1 = root-level module, 2 = one subdir, …).
pub(crate) fn module_paths(output_dir: &Path, module: &[String]) -> (PathBuf, String, usize) {
    let path_segments: Vec<String> = module
        .iter()
        .take(module.len().saturating_sub(1))
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let mut out_dir = output_dir.to_path_buf();
    for seg in &path_segments {
        out_dir.push(seg);
    }
    let file_name = format!("{}.py", module_file_name(module));
    let depth = path_segments.len() + 1;
    (out_dir, file_name, depth)
}

/// Collect the set of first-level subpackage directory names from an iterator of module paths.
/// A module of length 1 (e.g. `["__version__"]`) is at the root and contributes nothing.
pub(crate) fn first_level_subdirs<'a, I>(modules: I) -> BTreeSet<String>
where
    I: IntoIterator<Item = &'a [String]>,
{
    modules
        .into_iter()
        .filter(|m| m.len() > 1)
        .map(|m| m[0].to_ascii_lowercase())
        .collect()
}

/// Write an empty `__init__.py` to each first-level subdirectory listed in `subdirs`,
/// skipping any that already exist. Pushes resulting paths onto `written`.
pub(crate) fn write_empty_subdir_inits(
    output_dir: &Path,
    subdirs: &BTreeSet<String>,
    written: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    for sub in subdirs {
        let p = output_dir.join(sub).join("__init__.py");
        if !p.exists() {
            std::fs::write(&p, "")?;
            written.push(p);
        }
    }
    Ok(())
}
