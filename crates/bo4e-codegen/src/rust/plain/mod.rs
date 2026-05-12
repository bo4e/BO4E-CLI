//! `rust-plain` orchestrator — generates a loose Rust module tree (no Cargo.toml).

use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::SchemaRootType;
use minijinja::context;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::layout::{first_level_subdirs, module_paths};
use crate::rust::render::{render_object, render_str_enum};

pub fn generate(
    schemas: &Schemas,
    output_dir: &Path,
    opts: &crate::Options,
) -> Result<Vec<PathBuf>, Error> {
    if opts.clear_output {
        crate::clear_dir_if_exists(output_dir)?;
    } else {
        std::fs::create_dir_all(output_dir)?;
    }
    let env = crate::env::make_environment(opts.templates_dir)?;

    let mut written: Vec<PathBuf> = Vec::new();
    let version_str = schemas.version.to_string();

    // Track per top-level dir the leaf module names so we can write `mod.rs`s.
    let mut by_top: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for schema_rc in schemas {
        let mut schema = schema_rc.borrow_mut();
        let module = schema.module().to_vec();
        let class_name = schema.name().to_string();

        let (out_dir, file_name, depth) = module_paths(output_dir, &module, "rs");
        std::fs::create_dir_all(&out_dir)?;
        let out_path = out_dir.join(&file_name);

        let parsed = schema.schema().map_err(Error::Schema)?.clone();
        drop(schema);

        let body = match &parsed {
            SchemaRootType::StrEnum(e) => render_str_enum(
                &class_name,
                &e.str_enum.enum_values,
                e.str_enum.base.description.as_deref(),
            ),
            SchemaRootType::Object(o) => {
                render_object(
                    &env,
                    &class_name,
                    &module[..module.len().saturating_sub(1)],
                    &o.object,
                    depth,
                )?
                .body
            }
        };

        std::fs::write(&out_path, &body)?;
        written.push(out_path);

        if module.len() > 1 {
            let top = module[0].to_ascii_lowercase();
            let leaf = file_name.trim_end_matches(".rs").to_string();
            by_top.entry(top).or_default().push(leaf);
        }
    }

    // Per-subdir mod.rs files. BO4E's `enum/` directory is renamed to `enums/`
    // because `enum` is a Rust keyword.
    let modules: Vec<Vec<String>> = schemas
        .iter()
        .map(|s| s.borrow().module().to_vec())
        .collect();
    let raw_subdirs = first_level_subdirs(modules.iter().map(Vec::as_slice));
    for raw_sub in &raw_subdirs {
        let leaves: Vec<String> = by_top.get(raw_sub).cloned().unwrap_or_default();
        let mut leaves_sorted = leaves;
        leaves_sorted.sort();
        leaves_sorted.dedup();

        let mod_tpl = env.get_template("rust/plain/ModRs.jinja2")?;
        let reexports: Vec<_> = leaves_sorted
            .iter()
            .map(|leaf| {
                let mut chars = leaf.chars();
                let class = match chars.next() {
                    Some(c) => format!("{}{}", c.to_ascii_uppercase(), chars.as_str()),
                    None => String::new(),
                };
                context! { module => leaf, name => class }
            })
            .collect();
        let mod_body = mod_tpl.render(context! {
            modules => &leaves_sorted,
            reexports => &reexports,
        })?;
        let mod_dir = output_dir.join(raw_sub);
        std::fs::create_dir_all(&mod_dir)?;
        let mod_path = mod_dir.join("mod.rs");
        std::fs::write(&mod_path, format!("{mod_body}\n"))?;
        written.push(mod_path);
    }

    // Rename `<out>/enum/` → `<out>/enums/` (since `enum` is a keyword).
    let enum_dir = output_dir.join("enum");
    let enums_dir = output_dir.join("enums");
    if enum_dir.exists() && !enums_dir.exists() {
        std::fs::rename(&enum_dir, &enums_dir)?;
        for p in &mut written {
            if let Ok(rel) = p.strip_prefix(&enum_dir) {
                *p = enums_dir.join(rel);
            }
        }
    }

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

    Ok(written)
}
