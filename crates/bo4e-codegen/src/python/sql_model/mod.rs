//! python-sql-model generator orchestration.
//!
//! Two-phase: a pure pre-pass walks `Schemas` and produces an immutable
//! `SqlPlan` (in the private `plan` submodule); a render pass consumes the
//! plan and writes Python files via vendored MiniJinja templates.

pub(crate) mod plan;
mod renderer;

use crate::error::Error;
use bo4e_schemas::Schemas;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Orchestrate the entire SQL model code generation: walk the plan, render each table,
/// write per-class files at the right paths, and write root-level __init__, __version__,
/// and per-subpackage __init__ files.
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
    let mut written: Vec<PathBuf> = Vec::new();
    let plan = plan::build_plan(schemas)?;

    // Build a class_name → parent-directory-segments (lowercased) lookup.
    let class_to_module: BTreeMap<String, Vec<String>> = plan
        .tables
        .values()
        .map(|t| {
            let parents: Vec<String> = t
                .module
                .iter()
                .take(t.module.len().saturating_sub(1))
                .map(|s| s.to_ascii_lowercase())
                .collect();
            (t.class_name.clone(), parents)
        })
        .collect();

    // ── Per-class files ────────────────────────────────────────────────────────
    for table in plan.tables.values() {
        let (out_dir, file_name, depth) =
            crate::layout::module_paths(output_dir, &table.module, "py");
        std::fs::create_dir_all(&out_dir)?;
        let body = renderer::render_table(&env, table, depth, &class_to_module)?;
        let out_path = out_dir.join(&file_name);
        std::fs::write(&out_path, body)?;
        written.push(out_path);
    }

    // ── many.py at the root (only if there are junctions) ──────────────────────
    if !plan.junctions.is_empty() {
        let many = renderer::render_many(&env, &plan.junctions)?;
        let many_path = output_dir.join("many.py");
        std::fs::write(&many_path, many)?;
        written.push(many_path);
    }

    // ── __init__.py + __version__.py at the root ───────────────────────────────
    let version_str = schemas.version.to_string();
    let init_body = renderer::render_init(&env, &plan)?;
    let init_path = output_dir.join("__init__.py");
    std::fs::write(
        &init_path,
        format!(
            "{}\n{init_body}",
            crate::python::root_init_module_docstring(&version_str)
        ),
    )?;
    written.push(init_path);

    let version_path = output_dir.join("__version__.py");
    std::fs::write(&version_path, renderer::render_version(&version_str))?;
    written.push(version_path);

    // ── Empty __init__.py at every nested subdirectory ─────────────────────────
    let tree = crate::layout::ModuleTree::from_schemas(schemas);
    crate::python::write_empty_subdir_inits_recursive(output_dir, &tree, &mut written)?;

    Ok(crate::GenerateOutput {
        written,
        diagnostics: vec![],
    })
}
