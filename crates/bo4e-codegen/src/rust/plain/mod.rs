//! `rust-plain` orchestrator — generates a loose Rust module tree (no Cargo.toml).
//!
//! Path layout is fully resolved at path-build time via
//! [`crate::rust::module_paths`] and [`crate::rust::path_segments`]:
//! BO4E's `enum/` directory becomes `enums/` (a Rust keyword would
//! otherwise produce uncompilable `pub mod enum;` declarations), and
//! the rewrite applies recursively at any depth. There is no
//! post-write disk walk to rename directories.

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
    crate::validate::all_schemas(schemas)?;

    if opts.clear_output {
        crate::clear_dir_if_exists(output_dir)?;
    } else {
        std::fs::create_dir_all(output_dir)?;
    }
    let env = crate::env::make_environment(opts.templates_dir)?;

    let mut diagnostics: Vec<String> = Vec::new();
    let version_str = schemas.version.to_string();

    let path_for = |m: &[String]| module_paths(output_dir, m);
    let mut written = crate::for_each_schema_file(schemas, path_for, |ctx| {
        let leaf = ctx.file_name.trim_end_matches(".rs");
        let file_rel = if ctx.module.len() > 1 {
            let dir = rewrite_keyword_segment(&ctx.module[0]);
            format!("{dir}/{leaf}.rs")
        } else {
            format!("{leaf}.rs")
        };

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

    // Walk the tree of directories implied by the schema set and emit
    // a `mod.rs` at every level (root included). Each on-disk path is
    // computed via `path_segments` so keyword rewrites are applied
    // consistently with how the per-schema files were written above.
    let tree = ModuleTree::from_schemas(schemas);

    for (dir_path, node) in tree.iter() {
        let is_root = dir_path.is_empty();
        let rust_dir_segments = path_segments(dir_path);
        let on_disk_dir = output_dir.join(rust_dir_segments.iter().collect::<PathBuf>());
        std::fs::create_dir_all(&on_disk_dir)?;

        // Direct child sub-modules: lowercased + keyword-rewritten.
        let mut sub_modules: Vec<String> = node
            .children
            .iter()
            .map(|s| rewrite_keyword_segment(s))
            .collect();
        sub_modules.sort();
        sub_modules.dedup();

        // Leaf files in this directory: `pub mod <leaf>;` declarations
        // and `pub use <leaf>::<ClassName>;` re-exports.
        let mut leaves = node.leaves.clone();
        leaves.sort_by(|a, b| a.leaf.cmp(&b.leaf));
        leaves.dedup_by(|a, b| a.leaf == b.leaf);
        let leaf_modules: Vec<&str> = leaves.iter().map(|l| l.leaf.as_str()).collect();
        let leaf_reexports: Vec<_> = leaves
            .iter()
            .map(|l| context! { module => l.leaf, name => l.class_name })
            .collect();

        let final_body = if is_root {
            let root_tpl = env.get_template("rust/plain/RootModRs.jinja2")?;
            root_tpl.render(context! {
                top_modules => &sub_modules,
                modules => &leaf_modules,
                reexports => &leaf_reexports,
                version => &version_str,
            })?
        } else {
            // Non-root mod.rs: sub-modules and leaf files are both
            // declared via `pub mod X;` from the parent's POV. Re-exports
            // stay scoped to leaf files only.
            let mut all_mods: Vec<&str> = sub_modules.iter().map(String::as_str).collect();
            all_mods.extend(leaf_modules.iter().copied());
            all_mods.sort();
            all_mods.dedup();
            let tpl = env.get_template("rust/plain/ModRs.jinja2")?;
            tpl.render(context! {
                modules => &all_mods,
                reexports => &leaf_reexports,
            })?
        };

        let mod_path = on_disk_dir.join("mod.rs");
        std::fs::write(&mod_path, format!("{final_body}\n"))?;
        written.push(mod_path);

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

    Ok(crate::GenerateOutput {
        written,
        diagnostics,
    })
}
