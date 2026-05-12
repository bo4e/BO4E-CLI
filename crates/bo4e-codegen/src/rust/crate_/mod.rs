//! `rust-crate` orchestrator — wraps `rust-plain` output as a self-contained Cargo crate.

use bo4e_schemas::Schemas;
use std::path::{Path, PathBuf};

use crate::Error;

pub fn generate(
    schemas: &Schemas,
    output_dir: &Path,
    opts: &crate::Options,
    crate_opts: &crate::RustCrateOptions,
) -> Result<Vec<PathBuf>, Error> {
    if opts.clear_output {
        crate::clear_dir_if_exists(output_dir)?;
    } else {
        std::fs::create_dir_all(output_dir)?;
    }

    // Plain output goes under `<output_dir>/src/`.
    let src_dir = output_dir.join("src");
    let inner_opts = crate::Options {
        clear_output: false, // we already cleared
        templates_dir: opts.templates_dir,
    };
    let mut written = crate::rust::plain::generate(schemas, &src_dir, &inner_opts)?;

    // Rename `<src>/mod.rs` → `<src>/lib.rs`.
    let mod_rs = src_dir.join("mod.rs");
    let lib_rs = src_dir.join("lib.rs");
    if mod_rs.exists() {
        std::fs::rename(&mod_rs, &lib_rs)?;
        for p in &mut written {
            if *p == mod_rs {
                *p = lib_rs.clone();
            }
        }
    }

    // Emit Cargo.toml.
    let version_str = schemas.version.to_string();
    let semver = version_str.strip_prefix('v').unwrap_or(&version_str);
    let cargo_toml = render_cargo_toml(&crate_opts.crate_name, semver, &version_str);
    let cargo_path = output_dir.join("Cargo.toml");
    std::fs::write(&cargo_path, cargo_toml)?;
    written.push(cargo_path);

    Ok(written)
}

fn render_cargo_toml(crate_name: &str, semver: &str, bo4e_version: &str) -> String {
    format!(
        "[package]\n\
         name = \"{crate_name}\"\n\
         version = \"{semver}\"\n\
         edition = \"2024\"\n\
         description = \"Generated Rust types for the BO4E energy data model, version {bo4e_version}\"\n\
         license = \"Apache-2.0\"\n\
         \n\
         [dependencies]\n\
         serde = {{ version = \"1\", features = [\"derive\"] }}\n\
         serde_json = \"1\"\n\
         chrono = {{ version = \"0.4\", features = [\"serde\"] }}\n\
         uuid = {{ version = \"1\", features = [\"serde\", \"v4\"] }}\n\
         rust_decimal = {{ version = \"1\", features = [\"serde\"] }}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_toml_strips_leading_v_for_semver() {
        let s = render_cargo_toml("bo4e", "202401.4.0", "v202401.4.0");
        assert!(s.contains("name = \"bo4e\""));
        assert!(s.contains("version = \"202401.4.0\""));
        assert!(s.contains(
            "description = \"Generated Rust types for the BO4E energy data model, version v202401.4.0\""
        ));
    }
}
