use crate::models::json_schema::SchemaRootType;
use crate::models::version::DirtyVersion;
use itertools::chain;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
        if self.schema.is_none() {
            self.schema = serde_json::from_str(self._schema_text.as_ref().ok_or_else(|| {
                "Schema text was not loaded before through 'load_schema'.".to_string()
            })?)
            .map_err(|e| format!("Failed to parse schema: {}", e))?;
        }
        Ok(self.schema.as_ref().unwrap())
    }
    pub fn schema_mut(&mut self) -> Result<&mut SchemaRootType, String> {
        if self.schema.is_none() {
            self.schema = serde_json::from_str(self._schema_text.as_ref().ok_or_else(|| {
                "Schema text was not loaded before through 'load_schema'.".to_string()
            })?)
            .map_err(|e| format!("Failed to parse schema: {}", e))?;
        }
        Ok(self.schema.as_mut().unwrap())
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
        self.module.last().unwrap()
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(from = "SchemasOnDisk")]
pub struct Schemas {
    #[serde(default)]
    schemas: Vec<Rc<RefCell<Schema>>>,
    pub version: DirtyVersion,
    #[serde(skip, default)]
    _schemas_by_name: HashMap<String, Rc<RefCell<Schema>>>,
    #[serde(skip, default)]
    _schemas_by_module: HashMap<Vec<String>, Rc<RefCell<Schema>>>,
}

/// Deserialization shape for `Schemas`. The persisted JSON only carries `schemas` and
/// `version`; the lookup indexes are derived. Going through this helper guarantees the
/// indexes are rebuilt via `add_schema`, otherwise `modules()`/`get_by_module()` etc.
/// return empty results on a freshly deserialized value.
#[derive(Deserialize)]
struct SchemasOnDisk {
    #[serde(default)]
    schemas: Vec<Rc<RefCell<Schema>>>,
    version: DirtyVersion,
}

impl From<SchemasOnDisk> for Schemas {
    fn from(disk: SchemasOnDisk) -> Self {
        let mut out = Schemas::with_capacity(disk.schemas.len(), disk.version);
        for schema in disk.schemas {
            // Ignore add_schema errors here; serialized data should not contain
            // duplicates, and silently dropping is preferable to failing deserialization
            // on a Vec-backed structure that has no other validation gate.
            let _ = out.add_schema(schema);
        }
        out
    }
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
    pub fn schemas(&self) -> &[Rc<RefCell<Schema>>] {
        &self.schemas
    }
    pub fn names(&self) -> HashSet<&String> {
        self._schemas_by_name.keys().collect()
    }
    pub fn modules(&self) -> HashSet<&Vec<String>> {
        self._schemas_by_module.keys().collect()
    }
    pub fn modules_by_name(&self) -> HashMap<String, Vec<String>> {
        HashMap::from_iter(self._schemas_by_name.iter().map(|(name, schema)| {
            (
                name.clone(),
                schema.borrow().module().iter().map(String::from).collect(),
            )
        }))
    }
    pub fn add_schema(&mut self, schema: Rc<RefCell<Schema>>) -> Result<(), String> {
        let name = schema.borrow().name().to_string();
        let module = schema.borrow().module().to_vec();

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
    pub fn get_by_name(&self, name: &str) -> Option<Rc<RefCell<Schema>>> {
        self._schemas_by_name.get(name).cloned()
    }
    pub fn get_by_module(&self, module: &[String]) -> Option<Rc<RefCell<Schema>>> {
        self._schemas_by_module.get(module).cloned()
    }
    pub fn remove(&mut self, schema: &Schema) -> Option<Rc<RefCell<Schema>>> {
        let name = schema.name();
        let module = schema.module();

        if let Some(schema) = self._schemas_by_name.remove(name) {
            self._schemas_by_module.remove(module).unwrap();
            self.schemas.retain(|s| s.borrow().name() != name);
            Some(schema)
        } else {
            None
        }
    }
    pub fn remove_by_name(&mut self, name: &str) -> Option<Rc<RefCell<Schema>>> {
        if let Some(schema) = self._schemas_by_name.remove(name) {
            self._schemas_by_module
                .remove(schema.borrow().module())
                .unwrap();
            self.schemas.retain(|s| s.borrow().name() != name);
            Some(schema)
        } else {
            None
        }
    }
    pub fn remove_by_module(&mut self, module: &[String]) -> Option<Rc<RefCell<Schema>>> {
        if let Some(schema) = self._schemas_by_module.remove(module) {
            self._schemas_by_name.remove(schema.borrow().name());
            self.schemas.retain(|s| s.borrow().module() != module);
            Some(schema)
        } else {
            None
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Rc<RefCell<Schema>>> {
        self.schemas.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, Rc<RefCell<Schema>>> {
        self.schemas.iter_mut()
    }

    /// Schemas in `self` whose module is not present in `other`.
    pub fn module_difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a Rc<RefCell<Schema>>> {
        let other_modules = other.modules();
        self.schemas.iter().filter(move |s| {
            !other_modules
                .iter()
                .any(|m| m.as_slice() == s.borrow().module())
        })
    }

    /// Schemas whose module is present in both `self` and `other` (returns self's value).
    pub fn module_intersection<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a Rc<RefCell<Schema>>> {
        let other_modules = other.modules();
        self.schemas.iter().filter(move |s| {
            other_modules
                .iter()
                .any(|m| m.as_slice() == s.borrow().module())
        })
    }
}

impl<'a> IntoIterator for &'a Schemas {
    type Item = &'a Rc<RefCell<Schema>>;
    type IntoIter = std::slice::Iter<'a, Rc<RefCell<Schema>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.schemas.iter()
    }
}

impl<'a> IntoIterator for &'a mut Schemas {
    type Item = &'a mut Rc<RefCell<Schema>>;
    type IntoIter = std::slice::IterMut<'a, Rc<RefCell<Schema>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.schemas.iter_mut()
    }
}

impl TryFrom<(Vec<Rc<RefCell<Schema>>>, DirtyVersion)> for Schemas {
    type Error = String;

    fn try_from(
        (schemas, version): (Vec<Rc<RefCell<Schema>>>, DirtyVersion),
    ) -> Result<Self, Self::Error> {
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
            schemas_coll.add_schema(Rc::new(RefCell::new(schema)))?;
        }
        Ok(schemas_coll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::version::DirtyVersion;

    fn schema(module: &[&str]) -> Rc<RefCell<Schema>> {
        let m: Vec<String> = module.iter().map(|s| s.to_string()).collect();
        Rc::new(RefCell::new(Schema::new(m, None).unwrap()))
    }

    fn collection(modules: &[&[&str]]) -> Schemas {
        let v: DirtyVersion = "v202401.0.1".parse().unwrap();
        let mut s = Schemas::new(v);
        for m in modules {
            s.add_schema(schema(m)).unwrap();
        }
        s
    }

    #[test]
    fn test_module_difference_returns_only_unique_to_self() {
        let a = collection(&[&["bo", "Angebot"], &["com", "Adresse"]]);
        let b = collection(&[&["com", "Adresse"], &["enum", "Typ"]]);
        let only_a: Vec<Vec<String>> = a
            .module_difference(&b)
            .map(|s| s.borrow().module().to_vec())
            .collect();
        assert_eq!(only_a, vec![vec!["bo".to_string(), "Angebot".to_string()]]);
    }

    #[test]
    fn test_deserialize_rebuilds_module_index() {
        // Regression: a freshly deserialized `Schemas` must repopulate the
        // `_schemas_by_module`/`_schemas_by_name` indexes; otherwise downstream
        // consumers (matrix, edit::update_refs::update_references_all) see no
        // modules and silently produce empty output.
        let original = collection(&[&["bo", "Angebot"], &["com", "Adresse"]]);
        let json = serde_json::to_string(&original).unwrap();
        let restored: Schemas = serde_json::from_str(&json).unwrap();
        let mut mods: Vec<Vec<String>> =
            restored.modules().into_iter().cloned().collect();
        mods.sort();
        assert_eq!(
            mods,
            vec![
                vec!["bo".to_string(), "Angebot".to_string()],
                vec!["com".to_string(), "Adresse".to_string()],
            ]
        );
        // get_by_module must also resolve.
        assert!(
            restored
                .get_by_module(&["bo".to_string(), "Angebot".to_string()])
                .is_some()
        );
    }

    #[test]
    fn test_module_intersection_returns_self_values_in_both() {
        let a = collection(&[&["bo", "Angebot"], &["com", "Adresse"]]);
        let b = collection(&[&["com", "Adresse"], &["enum", "Typ"]]);
        let common: Vec<Vec<String>> = a
            .module_intersection(&b)
            .map(|s| s.borrow().module().to_vec())
            .collect();
        assert_eq!(common, vec![vec!["com".to_string(), "Adresse".to_string()]]);
    }
}
