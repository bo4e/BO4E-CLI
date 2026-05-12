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

use std::path::{Path, PathBuf};

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

/// Per-schema rendering context passed to the closure that
/// [`for_each_schema_file`] invokes for every schema in the input.
///
/// Carries everything the closure needs to render a single file without
/// touching the underlying `Rc<RefCell<Schema>>` borrow — the iterator
/// already cloned the parsed shape and released the cell.
#[cfg(any(
    feature = "python-pydantic",
    feature = "python-sql-model",
    feature = "rust-plain",
))]
pub(crate) struct SchemaCtx {
    pub class_name: String,
    pub module: Vec<String>,
    pub parsed: bo4e_schemas::models::json_schema::SchemaRootType,
    pub depth: usize,
    pub file_name: String,
}

/// Drive the per-schema file write loop shared by every per-flavour
/// generator. For each schema:
///
/// 1. Borrow the cell, snapshot `module` / `name` / parsed schema, drop the
///    borrow (so the closure can call back into anything without aliasing).
/// 2. Compute the output path via [`crate::layout::module_paths`] with the
///    flavour's file extension and create the parent directory.
/// 3. Call `render` for the file body.
/// 4. Write the body and record the path.
///
/// Returns every path written, in the same order as iteration. Closures
/// that need extra per-file state (diagnostics, mod.rs reexport maps, …)
/// capture it themselves.
#[cfg(any(
    feature = "python-pydantic",
    feature = "python-sql-model",
    feature = "rust-plain",
))]
pub(crate) fn for_each_schema_file<F>(
    schemas: &bo4e_schemas::Schemas,
    output_dir: &Path,
    ext: &str,
    mut render: F,
) -> Result<Vec<PathBuf>, Error>
where
    F: FnMut(&SchemaCtx) -> Result<String, Error>,
{
    let mut written = Vec::new();
    for schema_rc in schemas {
        let mut schema = schema_rc.borrow_mut();
        let module = schema.module().to_vec();
        let class_name = schema.name().to_string();
        let (out_dir, file_name, depth) = layout::module_paths(output_dir, &module, ext);
        let parsed = schema.schema().map_err(Error::Schema)?.clone();
        drop(schema);

        std::fs::create_dir_all(&out_dir)?;
        let ctx = SchemaCtx {
            class_name,
            module,
            parsed,
            depth,
            file_name: file_name.clone(),
        };
        let body = render(&ctx)?;
        let out_path = out_dir.join(&file_name);
        std::fs::write(&out_path, &body)?;
        written.push(out_path);
    }
    Ok(written)
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

/// Rename `from` → `to` on disk and update any matching entries in `written` to
/// point at the new path. Works for both a single file (exact-match) and a
/// directory (any descendant path is relocated). No-op when `from` doesn't
/// exist or `to` already does (the latter keeps repeat `--no-clear-output`
/// runs idempotent rather than failing).
///
/// Used by:
/// - `rust::plain::generate` to rename `<out>/enum/` → `<out>/enums/`
///   (the JSON-schema dir name is a Rust keyword).
/// - `rust::crate_::generate` to rename `<out>/src/mod.rs` → `<out>/src/lib.rs`.
pub(crate) fn rename_in_written(
    from: &Path,
    to: &Path,
    written: &mut [PathBuf],
) -> std::io::Result<()> {
    if !from.exists() || to.exists() {
        return Ok(());
    }
    std::fs::rename(from, to)?;
    for p in written.iter_mut() {
        if *p == from {
            *p = to.to_path_buf();
        } else if let Ok(rel) = p.strip_prefix(from) {
            *p = to.join(rel);
        }
    }
    Ok(())
}
