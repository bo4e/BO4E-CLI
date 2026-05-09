//! Import collector for the per-module Python renderer.
//!
//! Collects [`Import`] values produced by [`crate::python::types`] mapping, deduplicates
//! them, and renders a deterministic import block.

use crate::python::types::Import;
use std::collections::BTreeSet;

/// A registry of imports collected while rendering a single module file.
/// `render()` produces the deterministic import block.
#[derive(Debug, Default)]
#[allow(dead_code)] // Consumed by Task 8 (template renderer).
pub struct ImportBlock {
    items: BTreeSet<Import>,
}

#[allow(dead_code)] // Consumed by Task 8 (template renderer).
impl ImportBlock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn extend<I: IntoIterator<Item = Import>>(&mut self, items: I) {
        self.items.extend(items);
    }

    pub fn render(&self, module_path_depth: usize) -> String {
        use std::collections::BTreeMap;

        let mut stdlib: BTreeMap<&String, BTreeSet<&String>> = BTreeMap::new();
        let mut third_party: BTreeMap<&String, BTreeSet<&String>> = BTreeMap::new();
        let mut relative: BTreeMap<String, BTreeSet<&String>> = BTreeMap::new();

        let stdlib_modules = &[
            "decimal",
            "datetime",
            "uuid",
            "typing",
            "enum",
            "collections",
        ];

        for item in &self.items {
            match item {
                Import::Named { module, name } => {
                    let bucket = if stdlib_modules
                        .iter()
                        .any(|m| *m == module || module.starts_with(&format!("{m}.")))
                    {
                        &mut stdlib
                    } else {
                        &mut third_party
                    };
                    bucket.entry(module).or_default().insert(name);
                }
                Import::Sibling { module, name } => {
                    let dots: String = ".".repeat(module_path_depth);
                    let last_idx = module.len() - 1;
                    let dotted: String = module[..last_idx]
                        .iter()
                        .chain(std::iter::once(&module[last_idx].to_ascii_lowercase()))
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(".");
                    let key = format!("{dots}{dotted}");
                    relative.entry(key).or_default().insert(name);
                }
            }
        }

        fn fmt_block(block: &BTreeMap<&String, BTreeSet<&String>>) -> String {
            block
                .iter()
                .map(|(module, names)| {
                    let names_csv = names
                        .iter()
                        .cloned()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("from {module} import {names_csv}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        fn fmt_relative(block: &BTreeMap<String, BTreeSet<&String>>) -> String {
            block
                .iter()
                .map(|(module, names)| {
                    let names_csv = names
                        .iter()
                        .cloned()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("from {module} import {names_csv}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        [
            fmt_block(&stdlib),
            fmt_block(&third_party),
            fmt_relative(&relative),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(module: &str, name: &str) -> Import {
        Import::Named {
            module: module.into(),
            name: name.into(),
        }
    }

    fn sibling(module: &[&str], name: &str) -> Import {
        Import::Sibling {
            module: module.iter().map(|s| s.to_string()).collect(),
            name: name.to_string(),
        }
    }

    #[test]
    fn empty_block_renders_empty_string() {
        let b = ImportBlock::new();
        assert_eq!(b.render(2), "");
    }

    #[test]
    fn dedupes_same_named_import() {
        let mut b = ImportBlock::new();
        b.extend([named("decimal", "Decimal"), named("decimal", "Decimal")]);
        let out = b.render(2);
        assert_eq!(out.matches("from decimal import Decimal").count(), 1);
    }

    #[test]
    fn merges_two_names_from_same_module() {
        let mut b = ImportBlock::new();
        b.extend([named("typing", "Optional"), named("typing", "Annotated")]);
        let out = b.render(2);
        assert!(out.contains("from typing import Annotated, Optional"));
    }

    #[test]
    fn orders_blocks_stdlib_then_third_party_then_relative() {
        // module_path_depth = 2 means we are at e.g. "<root>/bo/angebot.py" →
        // siblings under "com" are imported via "..com.adresse".
        let mut b = ImportBlock::new();
        b.extend([
            named("decimal", "Decimal"),
            named("pydantic", "BaseModel"),
            sibling(&["com", "Adresse"], "Adresse"),
        ]);
        let out = b.render(2);
        let stdlib_pos = out.find("from decimal import Decimal").unwrap();
        let third_pos = out.find("from pydantic import BaseModel").unwrap();
        let relative_pos = out.find("from ..com.adresse import Adresse").unwrap();
        assert!(stdlib_pos < third_pos);
        assert!(third_pos < relative_pos);
    }

    #[test]
    fn relative_path_dot_count_matches_depth() {
        // depth 1 (root-level module) → ".com.adresse"
        // depth 2 (one subdir)       → "..com.adresse"
        let mut b = ImportBlock::new();
        b.extend([sibling(&["com", "Adresse"], "Adresse")]);
        assert!(b.render(1).contains("from .com.adresse import Adresse"));

        let mut b2 = ImportBlock::new();
        b2.extend([sibling(&["com", "Adresse"], "Adresse")]);
        assert!(b2.render(2).contains("from ..com.adresse import Adresse"));
    }
}
