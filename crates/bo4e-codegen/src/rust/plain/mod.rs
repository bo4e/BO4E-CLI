//! `rust-plain` orchestrator — generates a loose Rust module tree (no Cargo.toml).
//!
//! Path layout is fully resolved at path-build time via
//! [`crate::rust::module_paths`] and [`crate::rust::path_segments`]:
//! BO4E's `enum/` directory becomes `enums/` (a Rust keyword would
//! otherwise produce uncompilable `pub mod enum;` declarations), and
//! the rewrite applies recursively at any depth. There is no
//! post-write disk walk to rename directories.
//!
//! Two-phase: `prepare` renders every file into an in-memory buffer
//! without touching the filesystem; [`generate`] calls `prepare` and
//! then commits via `crate::write_prepared`. The `rust-crate`
//! flavour reuses `prepare` and adds its own `Cargo.toml` to the
//! buffer before committing, so destructive IO only happens after
//! every render has succeeded.

use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::SchemaRootType;
use minijinja::context;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::layout::ModuleTree;
use crate::rust::render::{render_object, render_str_enum};
use crate::rust::{module_paths, path_segments, rewrite_keyword_segment};

pub fn generate(
    schemas: &Schemas,
    output_dir: &Path,
    opts: &crate::Options,
) -> Result<crate::GenerateOutput, Error> {
    let (files, diagnostics) = prepare(schemas, output_dir, opts)?;
    crate::write_prepared(output_dir, opts.clear_output, files, diagnostics)
}

/// Render every file `rust-plain` would emit, into an in-memory buffer.
/// No filesystem mutation. Used by both [`generate`] and the
/// `rust-crate` orchestrator (which appends `Cargo.toml` and patches
/// the inner `mod.rs` → `lib.rs` path before committing).
pub(crate) fn prepare(
    schemas: &Schemas,
    output_dir: &Path,
    opts: &crate::Options,
) -> Result<(Vec<crate::PreparedFile>, Vec<String>), Error> {
    crate::validate::all_schemas(schemas)?;

    let env = crate::env::make_environment(opts.templates_dir)?;
    let mut diagnostics: Vec<String> = Vec::new();
    let version_str = schemas.version.to_string();

    let path_for = |m: &[String]| module_paths(output_dir, m);
    let mut files: Vec<crate::PreparedFile> =
        crate::for_each_schema_file(schemas, path_for, |ctx| {
            let file_rel = relative_rust_path(&ctx.module);
            let (body, diag) = match &ctx.parsed {
                SchemaRootType::StrEnum(e) => {
                    let n = e.str_enum.enum_values.len();
                    let body = render_str_enum(
                        &env,
                        &ctx.class_name,
                        &e.str_enum.enum_values,
                        e.str_enum.base.description.as_deref(),
                    )?;
                    (
                        body,
                        format!("{file_rel}: enum {} ({n} variants)", ctx.class_name),
                    )
                }
                SchemaRootType::Object(o) => {
                    let rendered = render_object(&env, &ctx.class_name, &o.object, ctx.depth)?;
                    (
                        rendered.body,
                        format!("{file_rel}: {}", rendered.diagnostic),
                    )
                }
            };
            diagnostics.push(diag);
            Ok(body)
        })?;

    // Emit a `mod.rs` at every directory level implied by the schemas
    // (root included). Path segments are rewritten via `path_segments`
    // so on-disk paths match the per-schema files staged above.
    let tree = ModuleTree::from_schemas(schemas);
    for (dir_path, node) in tree.iter() {
        let is_root = dir_path.is_empty();
        let rust_dir_segments = path_segments(dir_path);
        let on_disk_dir = output_dir.join(rust_dir_segments.iter().collect::<PathBuf>());

        // Direct child sub-modules: lowercased + keyword-rewritten.
        let mut sub_modules: Vec<String> = node
            .children
            .iter()
            .map(|s| rewrite_keyword_segment(s))
            .collect();
        sub_modules.sort();
        sub_modules.dedup();

        // Leaf files in this directory. The leaf identifier used in
        // `pub mod X;` and `pub use X::Class;` must go through the
        // same keyword rewrite as the on-disk path (`for_each_schema_file`
        // wrote `enums.rs` via `rust::module_paths`, so the declaration
        // must read `pub mod enums;`, not `pub mod enum;`).
        let mut leaves = node.leaves.clone();
        leaves.sort_by(|a, b| a.leaf.cmp(&b.leaf));
        leaves.dedup_by(|a, b| a.leaf == b.leaf);
        let leaf_idents: Vec<String> = leaves
            .iter()
            .map(|l| rewrite_keyword_segment(&l.leaf))
            .collect();
        let leaf_reexports: Vec<_> = leaves
            .iter()
            .zip(leaf_idents.iter())
            .map(|(l, ident)| context! { module => ident, name => l.class_name })
            .collect();

        let final_body = if is_root {
            let root_tpl = env.get_template("rust/plain/RootModRs.jinja2")?;
            root_tpl.render(context! {
                top_modules => &sub_modules,
                modules => &leaf_idents,
                reexports => &leaf_reexports,
                version => &version_str,
            })?
        } else {
            // Non-root mod.rs: sub-modules and leaf files are both
            // declared via `pub mod X;` from the parent's POV. Re-exports
            // stay scoped to leaf files only.
            let mut all_mods: Vec<&str> = sub_modules.iter().map(String::as_str).collect();
            all_mods.extend(leaf_idents.iter().map(String::as_str));
            all_mods.sort();
            all_mods.dedup();
            let tpl = env.get_template("rust/plain/ModRs.jinja2")?;
            tpl.render(context! {
                modules => &all_mods,
                reexports => &leaf_reexports,
            })?
        };

        files.push((on_disk_dir.join("mod.rs"), format!("{final_body}\n")));

        diagnostics.push(if is_root {
            format!(
                "mod.rs: VERSION = {version_str}, top-level modules: {}",
                sub_modules.join(", ")
            )
        } else {
            format!(
                "{}/mod.rs: {} leaves, {} children",
                rust_dir_segments.join("/"),
                leaves.len(),
                sub_modules.len()
            )
        });
    }

    Ok((files, diagnostics))
}

/// Build the diagnostic-friendly relative path for a schema's output
/// file, using the **Rust segments** (keyword-rewritten) so the path
/// shown by `--verbose` matches what's actually on disk. Used by the
/// `prepare` loop above.
fn relative_rust_path(module: &[String]) -> String {
    let segments = path_segments(module);
    if segments.is_empty() {
        return String::new();
    }
    // Last segment is the leaf file stem; preceding segments are the
    // directory path. Both have already been lowercased + rewritten.
    let (leaf, dirs) = segments.split_last().unwrap();
    let leaf_rs = format!("{leaf}.rs");
    if dirs.is_empty() {
        leaf_rs
    } else {
        format!("{}/{leaf_rs}", dirs.join("/"))
    }
}
