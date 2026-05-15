//! `rust-plain` orchestrator — generates a loose Rust module tree (no Cargo.toml).

use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::SchemaRootType;
use minijinja::context;
use std::path::Path;

use crate::Error;
use crate::layout::ModuleTree;
use crate::rust::render::{render_object, render_str_enum};

pub fn generate(
    schemas: &Schemas,
    output_dir: &Path,
    opts: &crate::Options,
) -> Result<crate::GenerateOutput, Error> {
    if opts.clear_output {
        crate::clear_dir_if_exists(output_dir)?;
    } else {
        std::fs::create_dir_all(output_dir)?;
    }
    let env = crate::env::make_environment(opts.templates_dir)?;

    let mut diagnostics: Vec<String> = Vec::new();
    let version_str = schemas.version.to_string();

    let mut written = crate::for_each_schema_file(schemas, output_dir, "rs", |ctx| {
        let leaf = ctx.file_name.trim_end_matches(".rs");
        let file_rel = if ctx.module.len() > 1 {
            format!("{}/{leaf}.rs", ctx.module[0].to_ascii_lowercase())
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

    // Build a tree view of every directory in the output so we can emit
    // `mod.rs` files at every level, including the root and arbitrary
    // depth subdirectories.
    let tree = ModuleTree::from_schemas(schemas);

    for (dir_path, node) in tree.iter() {
        let is_root = dir_path.is_empty();
        let on_disk_dir = on_disk_dir_for(output_dir, dir_path);
        std::fs::create_dir_all(&on_disk_dir)?;

        // Direct child sub-modules (sorted, deduped, with `enum` → `enums`
        // applied at every level — `pub mod enum;` would not compile).
        let mut sub_modules: Vec<String> = node
            .children
            .iter()
            .map(|s| rewrite_enum_segment(s).to_string())
            .collect();
        sub_modules.sort();
        sub_modules.dedup();

        // Leaf files in this directory: `pub mod <leaf>;` declarations and
        // `pub use <leaf>::<ClassName>;` re-exports.
        let mut leaves = node.leaves.clone();
        leaves.sort_by(|a, b| a.leaf.cmp(&b.leaf));
        leaves.dedup_by(|a, b| a.leaf == b.leaf);
        let leaf_modules: Vec<&str> = leaves.iter().map(|l| l.leaf.as_str()).collect();
        let leaf_reexports: Vec<_> = leaves
            .iter()
            .map(|l| context! { module => l.leaf, name => l.class_name })
            .collect();

        let mod_body = if is_root {
            let root_tpl = env.get_template("rust/plain/RootModRs.jinja2")?;
            root_tpl.render(context! {
                top_modules => &sub_modules,
                modules => &leaf_modules,
                reexports => &leaf_reexports,
                version => &version_str,
            })?
        } else {
            let tpl = env.get_template("rust/plain/ModRs.jinja2")?;
            tpl.render(context! {
                modules => &leaf_modules,
                reexports => &leaf_reexports,
                // Children of non-root dirs are emitted via `modules` (they
                // appear alongside the leaves; from the parent's POV both
                // are `pub mod X;` declarations).
            })?
        };

        // Non-root mod.rs files don't currently distinguish sub-modules
        // from leaf files in the template — they all live in `modules`.
        // We expand the non-root rendering to include children too.
        let final_body = if is_root {
            mod_body
        } else {
            render_subdir_mod_rs(&env, &leaf_modules, &leaf_reexports, &sub_modules)?
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
                dir_path_display(dir_path),
                leaves.len(),
                sub_modules.len()
            )
        });
    }

    // Rename any `enum/` directory (any depth) to `enums/` since `enum` is a
    // Rust keyword. Walk top-down so parents are renamed before children
    // (`rename_in_written` updates all matching paths in `written`).
    rename_enum_dirs(output_dir, &mut written)?;

    Ok(crate::GenerateOutput {
        written,
        diagnostics,
    })
}

fn on_disk_dir_for(output_dir: &Path, dir_path: &[String]) -> std::path::PathBuf {
    let mut p = output_dir.to_path_buf();
    for seg in dir_path {
        p.push(seg);
    }
    p
}

/// Rewrites the segment `"enum"` to `"enums"` since `enum` is a Rust keyword.
/// Used when emitting `pub mod <X>;` declarations and re-exports.
fn rewrite_enum_segment(seg: &str) -> &str {
    if seg == "enum" { "enums" } else { seg }
}

/// Recursively rename any directory whose final segment is `enum` to `enums`.
/// Operates on `output_dir` and applies the rename to entries in `written`.
fn rename_enum_dirs(output_dir: &Path, written: &mut [std::path::PathBuf]) -> Result<(), Error> {
    // Collect candidate paths (any directory named `enum` under output_dir)
    // before renaming, to avoid iterator invalidation during the walk.
    let mut to_rename: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    collect_enum_dirs(output_dir, &mut to_rename)?;
    for (from, to) in to_rename {
        crate::rename_in_written(&from, &to, written)?;
    }
    Ok(())
}

fn collect_enum_dirs(
    dir: &Path,
    out: &mut Vec<(std::path::PathBuf, std::path::PathBuf)>,
) -> Result<(), Error> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if path.file_name().is_some_and(|n| n == "enum") {
                let renamed = path.with_file_name("enums");
                out.push((path.clone(), renamed));
            }
            collect_enum_dirs(&path, out)?;
        }
    }
    Ok(())
}

fn dir_path_display(dir_path: &[String]) -> String {
    if dir_path.is_empty() {
        ".".to_string()
    } else {
        dir_path.join("/")
    }
}

fn render_subdir_mod_rs(
    env: &minijinja::Environment<'static>,
    leaf_modules: &[&str],
    leaf_reexports: &[minijinja::Value],
    sub_modules: &[String],
) -> Result<String, Error> {
    // The existing ModRs.jinja2 expects `modules` + `reexports`. Sub-modules
    // (child directories) ALSO need `pub mod X;` declarations. Merge them
    // into `modules`. Re-exports stay scoped to leaf files only (we don't
    // pull every nested class into every level).
    let mut all_mods: Vec<&str> = sub_modules.iter().map(String::as_str).collect();
    all_mods.extend(leaf_modules.iter().copied());
    all_mods.sort();
    all_mods.dedup();
    let tpl = env.get_template("rust/plain/ModRs.jinja2")?;
    Ok(tpl.render(context! {
        modules => &all_mods,
        reexports => leaf_reexports,
    })?)
}
