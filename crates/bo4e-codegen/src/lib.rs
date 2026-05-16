mod env;
mod error;
pub mod imports;
pub mod layout;
pub mod naming;
pub mod refs;
pub mod validate;

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
    // `module` is only consumed by the rust-plain orchestrator (for
    // diagnostics + file-relative path building); cfg-gated here so
    // python-only builds don't get a dead-code warning.
    #[cfg(feature = "rust-plain")]
    pub module: Vec<String>,
    pub parsed: bo4e_schemas::models::json_schema::SchemaRootType,
    pub depth: usize,
    pub file_name: String,
}

/// One file the generator wants on disk: an absolute output path plus the
/// rendered body. The orchestration pattern is **render-everything-then-commit**:
/// each `generate()` accumulates `PreparedFile`s in memory through a
/// pure pre-render phase, then a single [`write_prepared`] call clears the
/// output and writes the buffered files. A failure during render therefore
/// leaves the user's prior output intact (no half-clobbered crate or
/// partially-regenerated package).
#[cfg(any(
    feature = "python-pydantic",
    feature = "python-sql-model",
    feature = "rust-plain",
    feature = "rust-crate",
))]
pub(crate) type PreparedFile = (PathBuf, String);

/// Iterate every schema, call `render` to produce a body string, and
/// return the resulting `(path, body)` pairs *without touching the
/// filesystem*. The caller is responsible for any subsequent IO (see
/// [`write_prepared`]).
///
/// 1. Borrow the cell, snapshot `module` / `name` / parsed schema, drop the
///    borrow (so the closure can call back into anything without aliasing).
/// 2. Compute the output path via `path_for` (caller-supplied — Python
///    uses [`crate::layout::module_paths`], Rust uses
///    [`crate::rust::module_paths`] so the `enum/`→`enums/` rewrite
///    happens at path-build time rather than as a post-write rename).
/// 3. Call `render` for the file body.
/// 4. Stash `(out_path, body)` into the returned buffer.
///
/// Schema validation must have run before this iterator — see
/// `validate::all_schemas`. Closures that need extra per-file state
/// (diagnostics, mod.rs reexport maps, …) capture it themselves.
///
/// `sql_model` deliberately doesn't use this helper: it iterates a pre-built
/// `SqlPlan` rather than the raw `Schemas`, so the contract here doesn't fit.
#[cfg(any(
    feature = "python-pydantic",
    feature = "python-sql-model",
    feature = "rust-plain",
))]
pub(crate) fn for_each_schema_file<F, P>(
    schemas: &bo4e_schemas::Schemas,
    path_for: P,
    mut render: F,
) -> Result<Vec<PreparedFile>, Error>
where
    P: Fn(&[String]) -> (PathBuf, String, usize),
    F: FnMut(&SchemaCtx) -> Result<String, Error>,
{
    let mut prepared: Vec<PreparedFile> = Vec::new();
    for schema_rc in schemas {
        let mut schema = schema_rc.borrow_mut();
        let module = schema.module().to_vec();
        let class_name = schema.name().to_string();
        let (out_dir, file_name, depth) = path_for(&module);
        let parsed = schema.schema().map_err(Error::Schema)?.clone();
        drop(schema);

        let ctx = SchemaCtx {
            class_name,
            #[cfg(feature = "rust-plain")]
            module,
            parsed,
            depth,
            file_name,
        };
        // `module` is read only by rust-plain; pacify the unused-binding
        // warning under other feature sets.
        #[cfg(not(feature = "rust-plain"))]
        let _ = module;
        let body = render(&ctx)?;
        let out_path = out_dir.join(&ctx.file_name);
        prepared.push((out_path, body));
    }
    Ok(prepared)
}

/// Commit a fully-prepared file set to disk. Runs *only* after every
/// pre-render check (schema validation, plan build, per-property
/// rendering) has succeeded — so a render-time failure can never leave
/// the user with a half-clobbered output tree.
///
/// Behaviour:
/// - If `clear_output` is true, wipe `output_dir` first (created if missing).
/// - Otherwise just ensure `output_dir` exists.
/// - Create parent directories for each prepared file on demand.
/// - Write every body, in order, returning the paths in the same order.
#[cfg(any(
    feature = "python-pydantic",
    feature = "python-sql-model",
    feature = "rust-plain",
    feature = "rust-crate",
))]
pub(crate) fn write_prepared(
    output_dir: &Path,
    clear_output: bool,
    files: Vec<PreparedFile>,
    diagnostics: Vec<String>,
) -> Result<GenerateOutput, Error> {
    if clear_output {
        clear_dir_if_exists(output_dir)?;
    } else {
        std::fs::create_dir_all(output_dir)?;
    }
    let mut written: Vec<PathBuf> = Vec::with_capacity(files.len());
    for (path, body) in files {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, body)?;
        written.push(path);
    }
    Ok(GenerateOutput {
        written,
        diagnostics,
    })
}

/// Rename `from` → `to` inside a prepared file buffer (in-memory; no
/// disk IO). Mirrors the on-disk semantics of the old `rename_in_written`
/// but operates on the pre-commit buffer so the rename happens atomically
/// with the rest of the write phase. Used by `rust::crate_::generate` to
/// rewrite the inner `<src>/mod.rs` path to `<src>/lib.rs` after
/// `rust::plain::prepare` has staged it.
///
/// Works for both single-file (exact match) and directory (prefix-match)
/// renames.
#[cfg(feature = "rust-crate")]
pub(crate) fn rename_in_prepared(from: &Path, to: &Path, files: &mut [PreparedFile]) {
    for (path, _body) in files.iter_mut() {
        if *path == from {
            *path = to.to_path_buf();
        } else if let Ok(rel) = path.strip_prefix(from) {
            *path = to.join(rel);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(
        feature = "python-pydantic",
        feature = "python-sql-model",
        feature = "rust-plain",
    ))]
    #[test]
    fn for_each_schema_file_buffers_per_schema_without_writing() {
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
        let path_for = |m: &[String]| layout::module_paths(tmp.path(), m, "txt");
        let prepared = for_each_schema_file(&schemas, path_for, |ctx| {
            seen.push(ctx.class_name.clone());
            Ok(format!("// {}\n", ctx.class_name))
        })
        .unwrap();

        assert_eq!(seen, vec!["Angebot".to_string(), "Typ".to_string()]);
        assert_eq!(prepared.len(), 2);
        // Buffered, not on disk yet.
        assert!(!prepared[0].0.exists());
        assert!(!prepared[1].0.exists());
        // Body matches what the closure produced.
        assert_eq!(prepared[0].1, "// Angebot\n");
        // Path is `<out>/<top>/<lowercased-leaf>.<ext>`.
        assert_eq!(prepared[0].0, tmp.path().join("bo").join("angebot.txt"));
        assert_eq!(prepared[1].0, tmp.path().join("enum").join("typ.txt"));
    }

    /// `write_prepared` is the single commit point. If the buffer is
    /// empty we still create the output dir; otherwise we write every
    /// file in order, creating parent dirs on demand.
    #[cfg(any(
        feature = "python-pydantic",
        feature = "python-sql-model",
        feature = "rust-plain",
        feature = "rust-crate",
    ))]
    #[test]
    fn write_prepared_writes_buffer_and_creates_parents() {
        let tmp = tempfile::tempdir().unwrap();
        let files = vec![
            (tmp.path().join("bo/angebot.txt"), "a".into()),
            (tmp.path().join("com/adresse.txt"), "b".into()),
        ];
        let out = write_prepared(tmp.path(), true, files, vec![]).unwrap();
        assert_eq!(out.written.len(), 2);
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("bo/angebot.txt")).unwrap(),
            "a"
        );
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("com/adresse.txt")).unwrap(),
            "b"
        );
    }

    /// `write_prepared(..., clear_output=true, ...)` wipes the output
    /// dir before writing. Files that exist outside the buffer are
    /// removed.
    #[cfg(any(
        feature = "python-pydantic",
        feature = "python-sql-model",
        feature = "rust-plain",
        feature = "rust-crate",
    ))]
    #[test]
    fn write_prepared_clears_first_when_requested() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("stale.txt"), "stale").unwrap();
        let files = vec![(tmp.path().join("fresh.txt"), "fresh".into())];
        write_prepared(tmp.path(), true, files, vec![]).unwrap();
        assert!(!tmp.path().join("stale.txt").exists());
        assert_eq!(
            std::fs::read_to_string(tmp.path().join("fresh.txt")).unwrap(),
            "fresh"
        );
    }

    /// Buffered rename of `from` → `to` operates on the prepared file
    /// list, never on disk. Used by `rust::crate_::generate` to rewrite
    /// the inner `src/mod.rs` → `src/lib.rs` before commit.
    #[cfg(feature = "rust-crate")]
    #[test]
    fn rename_in_prepared_renames_files_in_buffer() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("src").join("mod.rs");
        let to = tmp.path().join("src").join("lib.rs");
        let mut files = vec![
            (from.clone(), "// root".into()),
            (tmp.path().join("src/bo/foo.rs"), "// bo".into()),
        ];
        rename_in_prepared(&from, &to, &mut files);
        assert_eq!(files[0].0, to);
        // Unrelated entries untouched.
        assert_eq!(files[1].0, tmp.path().join("src/bo/foo.rs"));
        // No disk effects.
        assert!(!from.exists());
        assert!(!to.exists());
    }

    /// Directory-prefix rename: every entry whose path starts with
    /// `from` is relocated under `to`.
    #[cfg(feature = "rust-crate")]
    #[test]
    fn rename_in_prepared_relocates_directory_prefixes() {
        let tmp = tempfile::tempdir().unwrap();
        let from = tmp.path().join("enum");
        let to = tmp.path().join("enums");
        let mut files = vec![
            (from.join("a.rs"), "".into()),
            (from.join("nested/b.rs"), "".into()),
            (tmp.path().join("other.rs"), "".into()),
        ];
        rename_in_prepared(&from, &to, &mut files);
        assert_eq!(files[0].0, to.join("a.rs"));
        assert_eq!(files[1].0, to.join("nested/b.rs"));
        assert_eq!(files[2].0, tmp.path().join("other.rs"));
    }
}
