use crate::models::json_schema::SchemaRootType;
use crate::models::version::DirtyVersion;
use itertools::chain;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    module: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    schema: Option<SchemaRootType>,
    #[serde(skip, default)]
    _schema_text: Option<String>,
}

impl Schema {
    pub fn new(module: Vec<String>, schema: Option<SchemaRootType>) -> Result<Self, String> {
        if module.is_empty() {
            return Err("Module name cannot be empty".to_string());
        }
        Ok(Self {
            module,
            schema,
            _schema_text: None,
        })
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
    pub fn get_serialized_schema(&self) -> Result<String, String> {
        if let Some(schema) = &self.schema {
            serde_json::to_string_pretty(schema)
                .map_err(|e| format!("Failed to serialize schema: {}", e))
        } else if let Some(schema_text) = &self._schema_text {
            Ok(schema_text.clone())
        } else {
            Err("Schema has neither parsed schema nor schema text.".to_string())
        }
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
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Schemas {
    #[serde(default)]
    schemas: Vec<Rc<Schema>>,
    pub version: DirtyVersion,
    #[serde(skip, default)]
    _schemas_by_name: HashMap<String, Rc<Schema>>,
    #[serde(skip, default)]
    _schemas_by_module: HashMap<Vec<String>, Rc<Schema>>,
}

impl Schemas {
    pub fn new(version: DirtyVersion) -> Self {
        Self {
            schemas: Vec::new(),
            version,
            _schemas_by_name: HashMap::new(),
            _schemas_by_module: HashMap::new(),
        }
    }
    pub fn with_capacity(capacity: usize, version: DirtyVersion) -> Self {
        Self {
            schemas: Vec::with_capacity(capacity),
            version,
            _schemas_by_name: HashMap::with_capacity(capacity),
            _schemas_by_module: HashMap::with_capacity(capacity),
        }
    }
    pub fn schemas(&self) -> &[Rc<Schema>] {
        &self.schemas
    }
    pub fn names(&self) -> HashSet<&String> {
        self._schemas_by_name.keys().collect()
    }
    pub fn modules(&self) -> HashSet<&Vec<String>> {
        self._schemas_by_module.keys().collect()
    }
    pub fn add_schema(&mut self, schema: Rc<Schema>) -> Result<(), String> {
        let name = schema.name().to_string();
        let module = schema.module().to_vec();

        if self._schemas_by_name.contains_key(&name) {
            return Err(format!("Schema with name '{}' already exists.", &name));
        }
        // We don't need to check for module uniqueness here,
        // as the schema's name is part of the module.
        self._schemas_by_name.insert(name, schema.clone());
        self._schemas_by_module.insert(module, schema.clone());
        self.schemas.push(schema);
        Ok(())
    }
    pub fn get_by_name(&self, name: &str) -> Option<Rc<Schema>> {
        self._schemas_by_name.get(name).cloned()
    }
    pub fn get_by_module(&self, module: &[String]) -> Option<Rc<Schema>> {
        self._schemas_by_module.get(module).cloned()
    }
    pub fn remove(&mut self, schema: &Schema) -> Option<Rc<Schema>> {
        let name = schema.name();
        let module = schema.module();

        if let Some(schema) = self._schemas_by_name.remove(name) {
            self._schemas_by_module.remove(module).unwrap();
            self.schemas.retain(|s| s.name() != name);
            Some(schema)
        } else {
            None
        }
    }
    pub fn remove_by_name(&mut self, name: &str) -> Option<Rc<Schema>> {
        if let Some(schema) = self._schemas_by_name.remove(name) {
            self._schemas_by_module.remove(schema.module()).unwrap();
            self.schemas.retain(|s| s.name() != name);
            Some(schema)
        } else {
            None
        }
    }
    pub fn remove_by_module(&mut self, module: &[String]) -> Option<Rc<Schema>> {
        if let Some(schema) = self._schemas_by_module.remove(module) {
            self._schemas_by_name.remove(schema.name());
            self.schemas.retain(|s| s.module() != module);
            Some(schema)
        } else {
            None
        }
    }

    fn try_from_iter<T: IntoIterator<Item = Schema>>(
        iter: T,
        version: DirtyVersion,
    ) -> Result<Self, String> {
        let mut schemas = Schemas::new(version);
        for schema in iter {
            schemas.add_schema(Rc::new(schema))?;
        }
        Ok(schemas)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Rc<Schema>> {
        self.schemas.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Rc<Schema>> {
        self.schemas.iter_mut()
    }
}

impl<'a> IntoIterator for &'a Schemas {
    type Item = &'a Rc<Schema>;
    type IntoIter = std::slice::Iter<'a, Rc<Schema>>;

    fn into_iter(self) -> Self::IntoIter {
        self.schemas.iter()
    }
}

impl<'a> IntoIterator for &'a mut Schemas {
    type Item = &'a mut Rc<Schema>;
    type IntoIter = std::slice::IterMut<'a, Rc<Schema>>;

    fn into_iter(self) -> Self::IntoIter {
        self.schemas.iter_mut()
    }
}

impl TryFrom<(Vec<Rc<Schema>>, DirtyVersion)> for Schemas {
    type Error = String;

    fn try_from((schemas, version): (Vec<Rc<Schema>>, DirtyVersion)) -> Result<Self, Self::Error> {
        let mut schemas_coll = Self::with_capacity(schemas.len(), version);
        for schema in schemas {
            schemas_coll.add_schema(schema)?;
        }
        Ok(schemas_coll)
    }
}

impl TryFrom<(Vec<Schema>, DirtyVersion)> for Schemas {
    type Error = String;

    fn try_from((schemas, version): (Vec<Schema>, DirtyVersion)) -> Result<Self, Self::Error> {
        let mut schemas_coll = Self::with_capacity(schemas.len(), version);
        for schema in schemas {
            schemas_coll.add_schema(Rc::new(schema))?;
        }
        Ok(schemas_coll)
    }
}
