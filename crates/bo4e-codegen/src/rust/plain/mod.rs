//! `rust-plain` orchestrator — generates a loose Rust module tree (no Cargo.toml).

use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::SchemaRootType;
use minijinja::context;
use std::collections::BTreeMap;
use std::path::Path;

use crate::Error;
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

    // Track per top-level dir the `(leaf module name, class name)` pairs so we can
    // emit accurate `pub use <leaf>::<ClassName>;` reexports in `mod.rs`. The class
    // name must come from the schema itself — reconstructing it by uppercasing the
    // first char of the lowercased file stem would lose internal CamelCase
    // (e.g. `PreisblattDienstleistung` would become `Preisblattdienstleistung`).
    let mut by_top: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();

    let mut written = crate::for_each_schema_file(schemas, output_dir, "rs", |ctx| {
        let top_dir = ctx.module.first().map(|s| s.as_str()).unwrap_or("");
        let leaf = ctx.file_name.trim_end_matches(".rs");
        let file_rel = format!("{top_dir}/{leaf}.rs");

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

        if ctx.module.len() > 1 {
            by_top
                .entry(ctx.module[0].to_ascii_lowercase())
                .or_default()
                .push((leaf.to_string(), ctx.class_name.clone()));
        }

        Ok(body)
    })?;

    // Per-subdir mod.rs files. BO4E's `enum/` directory is renamed to `enums/`
    // because `enum` is a Rust keyword.
    let raw_subdirs = crate::layout::first_level_subdirs_from_schemas(schemas);
    for raw_sub in &raw_subdirs {
        let mut entries: Vec<(String, String)> = by_top.get(raw_sub).cloned().unwrap_or_default();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries.dedup_by(|a, b| a.0 == b.0);

        let mod_tpl = env.get_template("rust/plain/ModRs.jinja2")?;
        let modules: Vec<&str> = entries.iter().map(|(leaf, _)| leaf.as_str()).collect();
        let reexports: Vec<_> = entries
            .iter()
            .map(|(leaf, class)| context! { module => leaf, name => class })
            .collect();
        let mod_body = mod_tpl.render(context! {
            modules => &modules,
            reexports => &reexports,
        })?;
        let mod_dir = output_dir.join(raw_sub);
        std::fs::create_dir_all(&mod_dir)?;
        let mod_path = mod_dir.join("mod.rs");
        std::fs::write(&mod_path, format!("{mod_body}\n"))?;
        written.push(mod_path);
    }

    // Rename `<out>/enum/` → `<out>/enums/` (since `enum` is a keyword).
    crate::rename_in_written(
        &output_dir.join("enum"),
        &output_dir.join("enums"),
        &mut written,
    )?;

    // Root mod.rs
    let mut top: Vec<String> = raw_subdirs
        .iter()
        .map(|s| {
            if s == "enum" {
                "enums".to_string()
            } else {
                s.clone()
            }
        })
        .collect();
    top.sort();
    top.dedup();
    let root_tpl = env.get_template("rust/plain/RootModRs.jinja2")?;
    let root_body = root_tpl.render(context! {
        top_modules => &top,
        version => &version_str,
    })?;
    let root_path = output_dir.join("mod.rs");
    std::fs::write(&root_path, format!("{root_body}\n"))?;
    written.push(root_path);
    diagnostics.push(format!(
        "mod.rs: VERSION = {version_str}, top-level modules: {}",
        top.join(", ")
    ));

    Ok(crate::GenerateOutput {
        written,
        diagnostics,
    })
}
