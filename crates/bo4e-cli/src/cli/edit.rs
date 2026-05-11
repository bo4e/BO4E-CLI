use crate::cli::base::Executable;
use crate::console::console::CONSOLE;
use crate::console::highlighter::SchemaModule;
use crate::edit::add::{transform_all_additional_enum_items, transform_all_additional_fields};
use crate::edit::non_nullable::transform_all_non_nullable_fields;
use crate::io::cleanse::clear_dir_if_needed;
use crate::io::config::{get_additional_schemas, load_config};
use bo4e_schemas::io::schemas::{read_schemas, write_schemas};
use bo4e_schemas::models::json_schema::{PrimitiveValue, SchemaRootType};
use crate::{cprint_normal, cprint_verbose, cwarn};
use clap::Args;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

fn classify_schema_module(module: &[String]) -> SchemaModule {
    match module.first().map(String::as_str) {
        Some("bo") => SchemaModule::Bo,
        Some("com") => SchemaModule::Com,
        Some("enum") => SchemaModule::Enum,
        _ => SchemaModule::Other,
    }
}

fn classify_all(schemas: &bo4e_schemas::models::schema_meta::Schemas) -> Vec<(SchemaModule, String)> {
    schemas
        .schemas()
        .iter()
        .map(|s| {
            let s = s.borrow();
            (classify_schema_module(s.module()), s.name().to_string())
        })
        .collect()
}

/// Edit JSON-schemas in the input directory and save the results to the output directory.
///
/// The schemas in the input directory won't be changed. If no configuration file is provided,
/// schemas are copied unchanged.
#[derive(Args)]
pub struct Edit {
    /// The directory to read the JSON-schemas from.
    #[arg(short = 'i', long = "input", required = true, value_name = "INPUT_DIRECTORY")]
    pub input_dir: PathBuf,

    /// The directory to save the edited JSON-schemas to.
    #[arg(short = 'o', long = "output", required = true, value_name = "OUTPUT_DIRECTORY")]
    pub output_dir: PathBuf,

    /// The configuration file to use for editing the JSON-schemas.
    #[arg(short = 'c', long = "config", value_name = "CONFIG_FILE")]
    pub config_file: Option<PathBuf>,

    /// Don't set the default value of the `_version` field of all schemas to the BO4E version.
    #[arg(long)]
    pub no_default_version: bool,

    /// Don't clear the output directory before saving the schemas.
    #[arg(long)]
    pub no_clear_output: bool,

    /// Don't append a `.d<YYYYMMDD>` dirty-workdir suffix to the output version. By default the
    /// edited output is branded with today's date so it is distinguishable from the upstream
    /// (unmodified) BO4E release.
    #[arg(long)]
    pub no_dirty_version: bool,
}

impl Executable for Edit {
    fn run(&self) -> Result<(), String> {
        clear_dir_if_needed(&self.output_dir, !self.no_clear_output)
            .map_err(|e| e.to_string())?;

        let out = read_schemas(&self.input_dir)?;
        for w in &out.warnings {
            crate::cwarn!("{w}");
        }
        let mut schemas = out.schemas;

        if let Some(config_path) = &self.config_file {
            cprint_normal!("Loading config from {}", config_path.display());
            let config = load_config(config_path)?;

            let extra = get_additional_schemas(&config.additional_models, config_path)?;
            for schema in extra {
                let name = schema.name().to_string();
                schemas.add_schema(Rc::new(RefCell::new(schema)))?;
                cprint_verbose!("Loaded additional model {}", name);
            }

            CONSOLE
                .get()
                .expect("CONSOLE not initialized")
                .add_schema_names(&classify_all(&schemas));
            cprint_normal!("Added all additional models");

            transform_all_additional_fields(&config.additional_fields, &mut schemas);
            cprint_normal!("Added all additional fields");

            transform_all_non_nullable_fields(&config.non_nullable_fields, &mut schemas)?;
            cprint_normal!("Transformed all non nullable fields");

            transform_all_additional_enum_items(&config.additional_enum_items, &mut schemas);
            cprint_normal!("Added all additional enum items");
        } else {
            // No config: still register schema names so the highlighter colours them in any
            // subsequent output (mirrors Python's `add_schemas_to_highlighter` call site).
            CONSOLE
                .get()
                .expect("CONSOLE not initialized")
                .add_schema_names(&classify_all(&schemas));
        }

        if !self.no_dirty_version {
            let today = chrono::Local::now().date_naive();
            schemas.version.set_dirty_worktree_date(today);
            cprint_normal!("Marked output version as locally edited: {}", schemas.version);
        }

        if !self.no_default_version {
            let version_str = schemas.version.to_string();
            for schema_rc in schemas.iter() {
                let mut schema = schema_rc.borrow_mut();
                let root = match schema.schema_mut() {
                    Ok(r) => r,
                    Err(e) => {
                        cwarn!("could not parse schema for version stamping: {}", e);
                        continue;
                    }
                };
                if let SchemaRootType::Object(obj) = root
                    && let Some(version_prop) = obj.object.properties.get_mut("_version")
                {
                    let base = match version_prop {
                        bo4e_schemas::models::json_schema::SchemaType::StringSchema(s) => &mut s.base,
                        // AnyOf: set default on the wrapper — correct JSON Schema placement for nullable fields.
                        bo4e_schemas::models::json_schema::SchemaType::AnyOf(s) => &mut s.base,
                        _ => {
                            cprint_verbose!("_version field has unexpected schema type, skipping default assignment");
                            continue;
                        }
                    };
                    base.default = Some(PrimitiveValue::String(version_str.clone()));
                }
            }
            cprint_normal!("Set default versions to {}", schemas.version);
        }

        write_schemas(&schemas, &self.output_dir).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{Console, Level, CONSOLE};
    use std::fs;
    use tempfile::TempDir;

    fn init_console() {
        let _ = CONSOLE.set(Console::new(Level::Normal));
    }

    fn make_input_dir() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".version"), "v202401.1.0").unwrap();

        let bo = dir.path().join("bo");
        fs::create_dir_all(&bo).unwrap();
        fs::write(
            bo.join("Angebot.json"),
            r#"{"type":"object","title":"Angebot","properties":{},"required":[],"additionalProperties":false}"#,
        ).unwrap();

        let enums = dir.path().join("enum");
        fs::create_dir_all(&enums).unwrap();
        fs::write(
            enums.join("Typ.json"),
            r#"{"type":"string","title":"Typ","enum":["A"]}"#,
        ).unwrap();
        dir
    }

    fn make_config(dir: &TempDir) -> std::path::PathBuf {
        let cfg = r#"{
            "additionalFields": [
                { "pattern": "bo\\.Angebot", "fieldName": "foo", "fieldDef": { "type": "string" } }
            ],
            "additionalEnumItems": [
                { "pattern": "enum\\.Typ", "items": ["B", "C"] }
            ]
        }"#;
        let path = dir.path().join("config.json");
        fs::write(&path, cfg).unwrap();
        path
    }

    #[test]
    fn test_edit_without_config_copies_schemas() {
        init_console();
        let input = make_input_dir();
        let output = tempfile::tempdir().unwrap();

        let cmd = Edit {
            input_dir: input.path().to_path_buf(),
            output_dir: output.path().to_path_buf(),
            config_file: None,
            no_default_version: true,
            no_clear_output: false,
            no_dirty_version: true,
        };
        cmd.run().unwrap();

        assert!(output.path().join("bo/Angebot.json").exists());
        assert!(output.path().join("enum/Typ.json").exists());
        assert!(output.path().join(".version").exists());
    }

    #[test]
    fn test_edit_with_config_applies_transformations() {
        init_console();
        let input = make_input_dir();
        let output = tempfile::tempdir().unwrap();
        let cfg_dir = tempfile::tempdir().unwrap();
        let cfg_path = make_config(&cfg_dir);

        let cmd = Edit {
            input_dir: input.path().to_path_buf(),
            output_dir: output.path().to_path_buf(),
            config_file: Some(cfg_path),
            no_default_version: true,
            no_clear_output: false,
            no_dirty_version: true,
        };
        cmd.run().unwrap();

        let angebot_text = fs::read_to_string(output.path().join("bo/Angebot.json")).unwrap();
        assert!(angebot_text.contains("\"foo\""));

        let typ_text = fs::read_to_string(output.path().join("enum/Typ.json")).unwrap();
        assert!(typ_text.contains("\"B\""));
        assert!(typ_text.contains("\"C\""));
    }

    #[test]
    fn test_edit_sets_default_version() {
        init_console();
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".version"), "v202401.1.0").unwrap();

        let bo = dir.path().join("bo");
        fs::create_dir_all(&bo).unwrap();
        // Schema with a _version field as anyOf: [string, null]
        fs::write(
            bo.join("WithVersion.json"),
            r#"{"type":"object","title":"WithVersion","properties":{"_version":{"anyOf":[{"type":"string"},{"type":"null"}]}},"required":[],"additionalProperties":false}"#,
        ).unwrap();

        let output = tempfile::tempdir().unwrap();
        let cmd = Edit {
            input_dir: dir.path().to_path_buf(),
            output_dir: output.path().to_path_buf(),
            config_file: None,
            no_default_version: false,  // enable version stamping
            no_clear_output: false,
            no_dirty_version: true,
        };
        cmd.run().unwrap();

        let text = fs::read_to_string(output.path().join("bo/WithVersion.json")).unwrap();
        assert!(text.contains("\"default\""), "output should contain a default value");
        assert!(text.contains("v202401.1.0"), "default should be the schema version");
    }

    #[test]
    fn test_edit_marks_dirty_version_by_default() {
        init_console();
        let input = make_input_dir();
        let output = tempfile::tempdir().unwrap();

        let cmd = Edit {
            input_dir: input.path().to_path_buf(),
            output_dir: output.path().to_path_buf(),
            config_file: None,
            no_default_version: true,
            no_clear_output: false,
            no_dirty_version: false, // default: mark dirty
        };
        cmd.run().unwrap();

        // The persisted .version file should now carry today's `.dYYYYMMDD` suffix.
        let written = fs::read_to_string(output.path().join(".version")).unwrap();
        let today = chrono::Local::now().date_naive().format("%Y%m%d").to_string();
        assert!(
            written.contains(&format!(".d{}", today)),
            "expected dirty workdir suffix in {written}"
        );
    }

    #[test]
    fn test_edit_keeps_clean_version_when_disabled() {
        init_console();
        let input = make_input_dir();
        let output = tempfile::tempdir().unwrap();

        let cmd = Edit {
            input_dir: input.path().to_path_buf(),
            output_dir: output.path().to_path_buf(),
            config_file: None,
            no_default_version: true,
            no_clear_output: false,
            no_dirty_version: true,
        };
        cmd.run().unwrap();

        let written = fs::read_to_string(output.path().join(".version")).unwrap();
        assert!(!written.contains(".d"), "expected no dirty suffix in {written}");
    }
}
