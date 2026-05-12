//! Per-language filesystem layout helpers: turn a schema module path
//! (e.g. `["bo", "Angebot"]`) into an output directory + file name + depth.
//!
//! These helpers are language-neutral; the caller passes the file extension
//! (`"py"` for Python, `"rs"` for Rust) and the resulting `file_name` includes
//! that extension. The leaf module name is lowercased (matches BO4E PascalCase
//! → snake-style file naming convention for both languages).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Lower-case the schema's last module segment to form its module file stem.
/// `module_file_name(&["bo", "Angebot"])` → `"angebot"`.
pub fn module_file_name(module: &[String]) -> String {
    module
        .last()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Compute the output directory, file name, and import depth for a schema
/// with the given module path. Pure — does not touch the filesystem.
///
/// Returns `(out_dir, file_name, depth)` where:
/// - `out_dir` = `output_dir` joined with the lowercased preceding path segments
/// - `file_name` = `module_file_name(module) + "." + extension`
/// - `depth` = `path_segments.len() + 1` (1 = root-level module, 2 = one subdir, …)
pub fn module_paths(
    output_dir: &Path,
    module: &[String],
    extension: &str,
) -> (PathBuf, String, usize) {
    let path_segments: Vec<String> = module
        .iter()
        .take(module.len().saturating_sub(1))
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let mut out_dir = output_dir.to_path_buf();
    for seg in &path_segments {
        out_dir.push(seg);
    }
    let file_name = format!("{}.{extension}", module_file_name(module));
    let depth = path_segments.len() + 1;
    (out_dir, file_name, depth)
}

/// Collect the set of first-level subpackage directory names from an iterator of module paths.
/// A module of length 1 (e.g. `["__version__"]`) is at the root and contributes nothing.
pub fn first_level_subdirs<'a, I>(modules: I) -> BTreeSet<String>
where
    I: IntoIterator<Item = &'a [String]>,
{
    modules
        .into_iter()
        .filter(|m| m.len() > 1)
        .map(|m| m[0].to_ascii_lowercase())
        .collect()
}

/// Convenience wrapper for the common pydantic/rust-plain pattern of collecting
/// the module path of every schema in `schemas` into a side `Vec`, just to feed
/// [`first_level_subdirs`]. The intermediate `Vec<Vec<String>>` exists purely
/// because each `s.borrow()` is a short-lived `Ref` that can't be slice-borrowed
/// across the `first_level_subdirs` call.
#[cfg(any(
    feature = "python-pydantic",
    feature = "python-sql-model",
    feature = "rust-plain",
))]
pub fn first_level_subdirs_from_schemas(schemas: &bo4e_schemas::Schemas) -> BTreeSet<String> {
    let modules: Vec<Vec<String>> = schemas
        .iter()
        .map(|s| s.borrow().module().to_vec())
        .collect();
    first_level_subdirs(modules.iter().map(Vec::as_slice))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn module_file_name_lowercases_last_segment() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        assert_eq!(module_file_name(&m), "angebot");
    }

    #[test]
    fn module_file_name_handles_single_segment() {
        let m = vec!["Typ".to_string()];
        assert_eq!(module_file_name(&m), "typ");
    }

    #[test]
    fn module_paths_python_extension() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        let (dir, file, depth) = module_paths(Path::new("/tmp/out"), &m, "py");
        assert_eq!(dir, Path::new("/tmp/out/bo"));
        assert_eq!(file, "angebot.py");
        assert_eq!(depth, 2);
    }

    #[test]
    fn module_paths_rust_extension() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        let (dir, file, depth) = module_paths(Path::new("/tmp/out"), &m, "rs");
        assert_eq!(dir, Path::new("/tmp/out/bo"));
        assert_eq!(file, "angebot.rs");
        assert_eq!(depth, 2);
    }

    #[test]
    fn module_paths_root_level_has_depth_1() {
        let m = vec!["Standalone".to_string()];
        let (dir, file, depth) = module_paths(Path::new("/tmp/out"), &m, "rs");
        assert_eq!(dir, Path::new("/tmp/out"));
        assert_eq!(file, "standalone.rs");
        assert_eq!(depth, 1);
    }

    #[test]
    fn first_level_subdirs_collects_unique_lowercased() {
        let a = vec!["bo".to_string(), "Angebot".to_string()];
        let b = vec!["BO".to_string(), "Andere".to_string()];
        let c = vec!["com".to_string(), "Adresse".to_string()];
        let d = vec!["__version__".to_string()];
        let set = first_level_subdirs([a.as_slice(), b.as_slice(), c.as_slice(), d.as_slice()]);
        assert_eq!(set.len(), 2);
        assert!(set.contains("bo"));
        assert!(set.contains("com"));
    }
}
