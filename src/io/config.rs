use crate::models::config::{
    AdditionalEnumItem, AdditionalField, AdditionalFieldOrRef, AdditionalModel, Config,
    SchemaRootTypeOrReference,
};
use crate::models::json_schema::SchemaRootType;
use crate::models::schema_meta::Schema;
use std::path::Path;

/// Resolved config — `additional_fields` is always a flat list of concrete `AdditionalField`
/// values (any `$ref` entries from the raw config have been resolved and inlined).
pub struct ResolvedConfig {
    pub non_nullable_fields: Vec<regex::Regex>,
    pub additional_fields: Vec<AdditionalField>,
    pub additional_enum_items: Vec<AdditionalEnumItem>,
    pub additional_models: Vec<AdditionalModel>,
}

/// Load and fully resolve the config file at `path`.
///
/// Any `{ "$ref": "..." }` entries in `additionalFields` are replaced with the referenced
/// `AdditionalField` or `Vec<AdditionalField>` loaded from the referenced path.
/// Relative `$ref` paths are resolved relative to the config file's parent directory.
pub fn load_config(path: &Path) -> Result<ResolvedConfig, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read config '{}': {}", path.display(), e))?;
    let raw: Config = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse config '{}': {}", path.display(), e))?;

    let config_dir = path.parent().unwrap_or(Path::new("."));
    let mut additional_fields: Vec<AdditionalField> = Vec::new();

    for entry in raw.additional_fields {
        match entry {
            AdditionalFieldOrRef::Field(f) => additional_fields.push(f),
            AdditionalFieldOrRef::Reference(r) => {
                let ref_path = resolve_path(config_dir, &r.path);
                let ref_text = std::fs::read_to_string(&ref_path).map_err(|e| {
                    format!("Failed to read field ref '{}': {}", ref_path.display(), e)
                })?;
                // Detect array vs object by the first non-whitespace character.
                if ref_text.trim_start().starts_with('[') {
                    let list: Vec<AdditionalField> = serde_json::from_str(&ref_text)
                        .map_err(|e| format!(
                            "Failed to parse field ref list '{}': {}",
                            ref_path.display(), e
                        ))?;
                    additional_fields.extend(list);
                } else {
                    let single: AdditionalField = serde_json::from_str(&ref_text).map_err(|e| {
                        format!(
                            "Failed to parse field ref '{}': {}",
                            ref_path.display(),
                            e
                        )
                    })?;
                    additional_fields.push(single);
                }
            }
        }
    }

    Ok(ResolvedConfig {
        non_nullable_fields: raw.non_nullable_fields,
        additional_fields,
        additional_enum_items: raw.additional_enum_items,
        additional_models: raw.additional_models,
    })
}

/// Load additional schemas declared in the config's `additionalModels`.
///
/// Returns a `Vec<Schema>` ready to be added to `Schemas`.
/// `config_path` is used to resolve relative `$ref` paths.
pub fn get_additional_schemas(
    models: &[AdditionalModel],
    config_path: &Path,
) -> Result<Vec<Schema>, String> {
    let config_dir = config_path.parent().unwrap_or(Path::new("."));
    let mut result = Vec::new();

    for model in models {
        let schema_root: SchemaRootType = match &model.schema {
            SchemaRootTypeOrReference::SchemaRootObject(o) => SchemaRootType::Object(o.clone()),
            SchemaRootTypeOrReference::SchemaRootStrEnum(e) => SchemaRootType::StrEnum(e.clone()),
            SchemaRootTypeOrReference::Reference(r) => {
                let ref_path = resolve_path(config_dir, &r.path);
                let text = std::fs::read_to_string(&ref_path).map_err(|e| {
                    format!("Failed to read schema ref '{}': {}", ref_path.display(), e)
                })?;
                serde_json::from_str(&text).map_err(|e| {
                    format!("Failed to parse schema ref '{}': {}", ref_path.display(), e)
                })?
            }
        };

        let title = match &schema_root {
            SchemaRootType::Object(o) => o.object.base.title.clone(),
            SchemaRootType::StrEnum(e) => e.str_enum.base.title.clone(),
        }
        .ok_or_else(|| "Config error: title is required for additional models".to_string())?;

        if title.is_empty() {
            return Err(
                "Config error: title must be non-empty for additional models".to_string(),
            );
        }

        let module = vec![model.module.clone(), title];
        let schema_text = serde_json::to_string(&schema_root)
            .map_err(|e| format!("Failed to serialize additional model: {}", e))?;
        let mut schema = Schema::new(module, None)?;
        schema.load_schema(schema_text);
        result.push(schema);
    }

    Ok(result)
}

fn resolve_path(base_dir: &Path, relative_or_absolute: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(relative_or_absolute);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base_dir.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_load_config_empty() {
        let dir = tempfile::tempdir().unwrap();
        let cfg_path = dir.path().join("config.json");
        fs::write(&cfg_path, "{}").unwrap();
        let config = load_config(&cfg_path).unwrap();
        assert!(config.additional_fields.is_empty());
        assert!(config.non_nullable_fields.is_empty());
        assert!(config.additional_models.is_empty());
    }

    #[test]
    fn test_load_config_resolves_ref_to_single_field() {
        let dir = tempfile::tempdir().unwrap();
        let field_json =
            r#"{"pattern":"bo\\..*","fieldName":"myField","fieldDef":{"type":"string"}}"#;
        fs::write(dir.path().join("field.json"), field_json).unwrap();

        let config_json = r#"{"additionalFields":[{"$ref":"./field.json"}]}"#;
        let cfg_path = dir.path().join("config.json");
        fs::write(&cfg_path, config_json).unwrap();

        let config = load_config(&cfg_path).unwrap();
        assert_eq!(config.additional_fields.len(), 1);
        assert_eq!(config.additional_fields[0].field_name, "myField");
    }

    #[test]
    fn test_load_config_resolves_ref_to_list() {
        let dir = tempfile::tempdir().unwrap();
        let fields_json = r#"[
            {"pattern":"bo\\..*","fieldName":"f1","fieldDef":{"type":"string"}},
            {"pattern":"com\\..*","fieldName":"f2","fieldDef":{"type":"integer"}}
        ]"#;
        fs::write(dir.path().join("fields.json"), fields_json).unwrap();

        let config_json = r#"{"additionalFields":[{"$ref":"./fields.json"}]}"#;
        let cfg_path = dir.path().join("config.json");
        fs::write(&cfg_path, config_json).unwrap();

        let config = load_config(&cfg_path).unwrap();
        assert_eq!(config.additional_fields.len(), 2);
    }

    #[test]
    fn test_get_additional_schemas_from_inline_model() {
        let dir = tempfile::tempdir().unwrap();
        let config_json = r#"{
            "additionalModels": [
                {
                    "module": "bo",
                    "schema": {
                        "type": "object",
                        "title": "MyModel",
                        "properties": {},
                        "required": [],
                        "additionalProperties": false
                    }
                }
            ]
        }"#;
        let cfg_path = dir.path().join("config.json");
        fs::write(&cfg_path, config_json).unwrap();

        let config = load_config(&cfg_path).unwrap();
        let schemas = get_additional_schemas(&config.additional_models, &cfg_path).unwrap();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0].module(), &["bo", "MyModel"]);
    }
}
