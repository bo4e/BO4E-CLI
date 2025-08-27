use crate::models::schema_meta::Schemas;
use crate::models::version::DirtyVersion;
use std::str::FromStr;

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
        let full_path = output_dir.join(&schema.as_relative_json_path());
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        schema
            .get_serialized_schema()
            .map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to serialize schema {}: {}", schema.name(), err),
                )
            })
            .and_then(|schema_text| std::fs::write(full_path, schema_text))?;
    }

    create_version_file(output_dir, &schemas.version)
}

// pub fn read_schemas(input_dir: &std::path::Path) -> Result<Schemas, String> {
//     let version = read_version_file(input_dir)?;
//
//     let mut schemas = Schemas::new(version);
//
//     for entry in walkdir::WalkDir::new(input_dir)
//         .into_iter()
//         .filter_map(|e| e.ok())
//         .filter(|e| {
//             e.path().is_file()
//                 && e.path().extension().map_or(false, |ext| ext == "json")
//                 && !e.file_name().to_string_lossy().starts_with('.')
//         })
//     {
//         let relative_path = entry
//             .path()
//             .strip_prefix(input_dir)
//             .map_err(|e| format!("Failed to get relative path: {}", e))?;
//         let schema_text = std::fs::read_to_string(entry.path())
//             .map_err(|e| format!("Failed to read schema file: {}", e))?;
//         let module: Vec<String> = relative_path
//             .with_extension("") // remove .json extension
//             .components()
//             .map(|comp| comp.as_os_str().to_string_lossy().to_string())
//             .collect();
//
//         let schema = crate::models::schema_meta::Schema {
//             module,
//             schema: None,
//             _schema_text: Some(schema_text),
//         };
//         schemas.add_schema(schema);
//     }
//
//     Ok(schemas)
// }
