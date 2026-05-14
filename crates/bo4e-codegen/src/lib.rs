mod env;
mod error;
pub mod imports;
pub mod layout;
pub mod naming;
pub mod refs;
mod validate;

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

/// Drive the per-schema file write loop shared by the per-schema-file
/// flavours (pydantic, rust-plain). For each schema:
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
///
/// `sql_model` deliberately doesn't use this helper: it iterates a pre-built
/// `SqlPlan` rather than the raw `Schemas`, so the contract here doesn't fit.
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

        // Gate: object schemas must satisfy the strict required/default
        // invariant before any renderer sees them. StrEnum top-levels carry
        // no properties so they're trivially consistent.
        if let bo4e_schemas::models::json_schema::SchemaRootType::Object(o) = &parsed {
            validate::object_invariants(&class_name, &o.object)?;
        }

        std::fs::create_dir_all(&out_dir)?;
        let ctx = SchemaCtx {
            class_name,
            module,
            parsed,
            depth,
            file_name,
        };
        let body = render(&ctx)?;
        let out_path = out_dir.join(&ctx.file_name);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_in_written_noop_when_source_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("missing");
        let to = tmp.path().join("renamed");
        let mut written = vec![tmp.path().join("unrelated.txt")];
        rename_in_written(&from, &to, &mut written).unwrap();
        assert!(!to.exists());
        assert_eq!(written[0], tmp.path().join("unrelated.txt"));
    }

    #[test]
    fn rename_in_written_noop_when_target_already_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("src");
        let to = tmp.path().join("dst");
        std::fs::write(&from, "a").unwrap();
        std::fs::write(&to, "b").unwrap();
        let mut written = vec![from.clone()];
        rename_in_written(&from, &to, &mut written).unwrap();
        // Both still exist; written unchanged because we skipped.
        assert!(from.exists());
        assert!(to.exists());
        assert_eq!(written[0], from);
    }

    #[test]
    fn rename_in_written_relocates_exact_file_path() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("a.rs");
        let to = tmp.path().join("b.rs");
        std::fs::write(&from, "data").unwrap();
        let mut written = vec![from.clone(), tmp.path().join("other.rs")];
        rename_in_written(&from, &to, &mut written).unwrap();
        assert!(!from.exists());
        assert!(to.exists());
        assert_eq!(written[0], to);
        assert_eq!(written[1], tmp.path().join("other.rs"));
    }

    #[test]
    fn rename_in_written_relocates_directory_descendants() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("enum");
        let to = tmp.path().join("enums");
        std::fs::create_dir_all(from.join("nested")).unwrap();
        std::fs::write(from.join("a.rs"), "").unwrap();
        std::fs::write(from.join("nested/b.rs"), "").unwrap();
        let mut written = vec![from.join("a.rs"), from.join("nested/b.rs")];
        rename_in_written(&from, &to, &mut written).unwrap();
        assert!(!from.exists());
        assert!(to.join("a.rs").exists());
        assert!(to.join("nested/b.rs").exists());
        assert_eq!(written[0], to.join("a.rs"));
        assert_eq!(written[1], to.join("nested/b.rs"));
    }

    #[cfg(any(
        feature = "python-pydantic",
        feature = "python-sql-model",
        feature = "rust-plain",
    ))]
    #[test]
    fn for_each_schema_file_writes_per_schema_and_returns_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let mut schemas = bo4e_schemas::Schemas::new("v202401.0.0".parse().unwrap());
        let mut s1 = bo4e_schemas::Schema::new(vec!["bo".into(), "Angebot".into()], None).unwrap();
        s1.load_schema(
            r#"{"type":"object","title":"Angebot","properties":{},"required":[]}"#.into(),
        );
        schemas
            .add_schema(std::rc::Rc::new(std::cell::RefCell::new(s1)))
            .unwrap();
        let mut s2 = bo4e_schemas::Schema::new(vec!["enum".into(), "Typ".into()], None).unwrap();
        s2.load_schema(r#"{"type":"string","title":"Typ","enum":["A","B"]}"#.into());
        schemas
            .add_schema(std::rc::Rc::new(std::cell::RefCell::new(s2)))
            .unwrap();

        let mut seen: Vec<String> = Vec::new();
        let written = for_each_schema_file(&schemas, tmp.path(), "txt", |ctx| {
            seen.push(ctx.class_name.clone());
            Ok(format!("// {}\n", ctx.class_name))
        })
        .unwrap();

        assert_eq!(seen, vec!["Angebot".to_string(), "Typ".to_string()]);
        assert_eq!(written.len(), 2);
        // Files were actually written with the closure's body.
        let body = std::fs::read_to_string(&written[0]).unwrap();
        assert_eq!(body, "// Angebot\n");
        // Path is `<out>/<top>/<lowercased-leaf>.<ext>`.
        assert_eq!(written[0], tmp.path().join("bo").join("angebot.txt"));
        assert_eq!(written[1], tmp.path().join("enum").join("typ.txt"));
    }
}
