use crate::models::json_schema::SchemaRootType;
use itertools::chain;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use url::Url;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SchemaMeta {
    module: Box<[String]>,
    #[serde(default)]
    pub src: Option<Source>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Local(PathBuf),
    Online(Url),
}

impl SchemaMeta {
    pub fn new(module: Box<[String]>, src: Option<Source>) -> Result<Self, String> {
        if module.is_empty() {
            return Err("Module name cannot be empty".to_string());
        }
        Ok(Self { module, src })
    }
    pub fn module(&self) -> &[String] {
        &self.module
    }
    pub fn name(&self) -> &str {
        &self.module.last().unwrap()
    }

    pub fn as_relative_json_path(&self) -> PathBuf {
        let last_index = self.module.len() - 1;
        chain(
            &self.module[..last_index],
            [&(self.module[last_index].clone() + ".json")],
        )
        .collect()
    }
    pub fn src_url(&self) -> Option<&Url> {
        if let Some(Source::Online(url)) = &self.src {
            Some(url)
        } else {
            None
        }
    }
    pub fn src_path(&self) -> Option<&PathBuf> {
        if let Some(Source::Local(path)) = &self.src {
            Some(path)
        } else {
            None
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub meta: SchemaMeta,
    #[serde(default)]
    schema: Option<SchemaRootType>,
    #[serde(skip, default)]
    _schema_text: Option<String>,
}

impl Schema {
    pub fn new(meta: SchemaMeta) -> Self {
        Self {
            meta,
            schema: None,
            _schema_text: None,
        }
    }

    pub fn load_schema(&mut self, schema_text: String) {
        self._schema_text = Some(schema_text);
    }
    pub fn schema(&mut self) -> Result<&SchemaRootType, String> {
        if self.schema.is_some() {
            Ok(self.schema.as_ref().unwrap())
        } else {
            self.schema = serde_json::from_str(self._schema_text.as_ref().ok_or_else(|| {
                "Schema text was not loaded before through 'load_schema'.".to_string()
            })?)
            .map_err(|e| format!("Failed to parse schema: {}", e))?;
            Ok(self.schema.as_ref().unwrap())
        }
    }

    pub fn module(&self) -> &[String] {
        self.meta.module()
    }

    pub fn name(&self) -> &str {
        self.meta.name()
    }

    pub fn as_relative_json_path(&self) -> PathBuf {
        self.meta.as_relative_json_path()
    }

    pub fn src_url(&self) -> Option<&Url> {
        self.meta.src_url()
    }

    pub fn src_path(&self) -> Option<&PathBuf> {
        self.meta.src_path()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Schemas {
    schemas: Vec<Schema>,
}

impl Schemas {
    pub fn schemas(&self) -> &[Schema] {
        &self.schemas
    }
}

pub struct IndexedSchemas<'a> {
    schemas_by_name: HashMap<&'a str, &'a Schema>,
    schemas_by_module: HashMap<&'a [String], &'a Schema>,
    _schemas: &'a mut Schemas,
}

impl<'a> From<&'a mut Schemas> for IndexedSchemas<'a> {
    fn from(schemas: &'a mut Schemas) -> Self {
        let mut schemas_by_name = HashMap::new();
        let mut schemas_by_module = HashMap::new();

        for schema in &schemas.schemas {
            schemas_by_name.insert(schema.name(), schema);
            schemas_by_module.insert(schema.module(), schema);
        }

        Self {
            schemas_by_name,
            schemas_by_module,
            _schemas: schemas,
        }
    }
}

impl<'a> IndexedSchemas<'a> {
    pub fn get_by_name(&self, name: &str) -> Option<&'a Schema> {
        self.schemas_by_name.get(name).copied()
    }

    pub fn get_by_module(&self, module: &[String]) -> Option<&'a Schema> {
        self.schemas_by_module.get(module).copied()
    }

    pub fn add_schema(&mut self, schema: &'a Schema) {
        self.schemas_by_name.insert(schema.name(), schema);
        self.schemas_by_module.insert(schema.module(), schema);
        self._schemas.schemas.push(schema);
    }
}
