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
use std::path::Path;

/// Orchestrate the entire SQL model code generation: walk the plan, stage every
/// rendered file in memory, then commit atomically. Two-phase so a plan-build
/// failure or render error can never delete the user's previous output.
pub fn generate(
    schemas: &Schemas,
    output_dir: &Path,
    opts: &crate::Options,
) -> Result<crate::GenerateOutput, Error> {
    // ── Phase 1: validate + plan + render (no destructive IO) ──────────────────
    crate::validate::all_schemas(schemas)?;
    let plan = plan::build_plan(schemas)?;
    let env = crate::env::make_environment(opts.templates_dir)?;
    let version_str = schemas.version.to_string();

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

    let mut files: Vec<crate::PreparedFile> = Vec::new();

    for table in plan.tables.values() {
        let (out_dir, file_name, depth) =
            crate::layout::module_paths(output_dir, &table.module, "py");
        let body = renderer::render_table(&env, table, depth, &class_to_module)?;
        files.push((out_dir.join(&file_name), body));
    }

    if !plan.junctions.is_empty() {
        let many = renderer::render_many(&env, &plan.junctions)?;
        files.push((output_dir.join("many.py"), many));
    }

    let init_body = renderer::render_init(&env, &plan)?;
    files.push((
        output_dir.join("__init__.py"),
        format!(
            "{}\n{init_body}",
            crate::python::root_init_module_docstring(&version_str)
        ),
    ));

    files.push((
        output_dir.join("__version__.py"),
        renderer::render_version(&version_str),
    ));

    let tree = crate::layout::ModuleTree::from_schemas(schemas);
    files.extend(crate::python::prepare_empty_subdir_inits_recursive(
        output_dir, &tree,
    ));

    // ── Phase 2: commit (clear + write) ────────────────────────────────────────
    crate::write_prepared(output_dir, opts.clear_output, files, vec![])
}
