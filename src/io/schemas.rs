use crate::models::schema_meta::Schema;
use crate::models::schema_meta::Schemas;
use crate::models::version::DirtyVersion;
use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use walkdir::WalkDir;

const VERSION_FILE_NAME: &str = ".version";

fn create_version_file(
    output_dir: &std::path::Path,
    version: &DirtyVersion,
) -> Result<(), std::io::Error> {
    let version_file_path = output_dir.join(VERSION_FILE_NAME);
    std::fs::write(version_file_path, version.to_string())
}

fn read_version_file(input_dir: &std::path::Path) -> Result<DirtyVersion, String> {
    let version_file_path = input_dir.join(VERSION_FILE_NAME);
    let version_str = std::fs::read_to_string(&version_file_path)
        .map_err(|e| format!("Failed to read version file: {}", e))?;
    DirtyVersion::from_str(&version_str)
}

pub fn write_schemas(
    schemas: &Schemas,
    output_dir: &std::path::Path,
) -> Result<(), std::io::Error> {
    for schema in schemas {
        let full_path = output_dir.join(&schema.borrow().as_relative_json_path());
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let name = schema.borrow().name().to_string();
        schema
            .borrow()
            .get_serialized_schema()
            .map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to serialize schema {}: {}", name, err),
                )
            })
            .and_then(|schema_text| std::fs::write(full_path, schema_text))?;
    }

    create_version_file(output_dir, &schemas.version)
}

pub fn read_schemas(input_dir: &std::path::Path) -> Result<Schemas, String> {
    let version = read_version_file(input_dir)?;
    let mut schemas = Schemas::new(version);

    for entry in WalkDir::new(input_dir)
        .into_iter()
        .filter_map(|e| {
            e.map_err(|err| eprintln!("Warning: skipping unreadable entry: {}", err))
                .ok()
        })
        .filter(|e| {
            e.path().is_file()
                && e.path().extension().is_some_and(|ext| ext == "json")
                && !e.file_name().to_string_lossy().starts_with('.')
        })
    {
        let relative_path = entry
            .path()
            .strip_prefix(input_dir)
            .map_err(|e| format!("Failed to strip prefix: {}", e))?
            .with_extension(""); // remove .json

        let module: Vec<String> = relative_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();

        let schema_text = std::fs::read_to_string(entry.path())
            .map_err(|e| format!("Failed to read {}: {}", entry.path().display(), e))?;

        let mut schema = Schema::new(module, None)?;
        schema.load_schema(schema_text);
        schemas.add_schema(Rc::new(RefCell::new(schema)))?;
    }

    Ok(schemas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_test_dir() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".version"), "v202401.1.0").unwrap();

        let bo_dir = dir.path().join("bo");
        fs::create_dir_all(&bo_dir).unwrap();
        fs::write(
            bo_dir.join("Angebot.json"),
            r#"{"type":"object","title":"Angebot","properties":{},"required":[],"additionalProperties":false}"#,
        ).unwrap();

        let enum_dir = dir.path().join("enum");
        fs::create_dir_all(&enum_dir).unwrap();
        fs::write(
            enum_dir.join("Typ.json"),
            r#"{"type":"string","title":"Typ","enum":["A","B"]}"#,
        ).unwrap();

        dir
    }

    #[test]
    fn test_read_schemas_finds_all_json_files() {
        let dir = make_test_dir();
        let schemas = read_schemas(dir.path()).unwrap();
        assert_eq!(schemas.schemas().len(), 2);
        assert!(schemas.get_by_name("Angebot").is_some());
        assert!(schemas.get_by_name("Typ").is_some());
    }

    #[test]
    fn test_read_schemas_derives_module_path() {
        let dir = make_test_dir();
        let schemas = read_schemas(dir.path()).unwrap();
        let angebot = schemas.get_by_name("Angebot").unwrap();
        assert_eq!(angebot.borrow().module(), &["bo", "Angebot"]);
    }

    #[test]
    fn test_read_schemas_skips_dotfiles() {
        let dir = make_test_dir();
        let schemas = read_schemas(dir.path()).unwrap();
        // .version is not a .json file so it must not appear
        assert!(schemas.get_by_name(".version").is_none());
        // No schema with a dot-prefixed name should exist
        assert_eq!(schemas.schemas().len(), 2);
    }
}
