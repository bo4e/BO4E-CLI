//! Import collector for the per-module Python renderer.
//!
//! Collects [`Import`] values produced by [`crate::python::types`] mapping, deduplicates
//! them, and renders a deterministic import block.

use crate::imports::Import;
use std::collections::BTreeSet;

/// A registry of imports collected while rendering a single module file.
/// `render()` produces the deterministic import block.
#[derive(Debug, Default)]
pub struct ImportBlock {
    items: BTreeSet<Import>,
}

impl ImportBlock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn extend<I: IntoIterator<Item = Import>>(&mut self, items: I) {
        self.items.extend(items);
    }

    pub fn render(&self, module_path_depth: usize) -> String {
        debug_assert!(
            module_path_depth >= 1,
            "module_path_depth must be >= 1 (root-level module is depth 1)"
        );

        use std::collections::BTreeMap;

        const STDLIB_MODULES: &[&str] = &[
            "decimal",
            "datetime",
            "uuid",
            "typing",
            "enum",
            "collections",
        ];
        let is_stdlib = |module: &str| -> bool {
            STDLIB_MODULES
                .iter()
                .any(|m| *m == module || module.starts_with(&format!("{m}.")))
        };

        // Named: partition the shared module-grouped map into Python's
        // stdlib / third-party buckets.
        let named = crate::imports::group_named_by_module(&self.items);
        let (stdlib, third_party): (BTreeMap<_, _>, BTreeMap<_, _>) = named
            .into_iter()
            .partition(|(module, _)| is_stdlib(module.as_str()));

        // Sibling: dot-separated relative paths, with the leaf segment
        // lower-cased (Python module file names are snake/lower).
        let mut relative: BTreeMap<String, BTreeSet<&String>> = BTreeMap::new();
        for item in &self.items {
            if let Import::Sibling { module, name } = item {
                let Some((last, head)) = module.split_last() else {
                    continue;
                };
                let dots: String = ".".repeat(module_path_depth);
                let dotted: String = head
                    .iter()
                    .cloned()
                    .chain(std::iter::once(last.to_ascii_lowercase()))
                    .collect::<Vec<_>>()
                    .join(".");
                relative
                    .entry(format!("{dots}{dotted}"))
                    .or_default()
                    .insert(name);
            }
        }

        fn fmt<K: std::fmt::Display>(block: &BTreeMap<K, BTreeSet<&String>>) -> String {
            block
                .iter()
                .map(|(module, names)| {
                    let names_csv = names
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("from {module} import {names_csv}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        crate::imports::stitch_nonempty_blocks(
            &[fmt(&stdlib), fmt(&third_party), fmt(&relative)],
            "\n\n",
        )
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

    #[test]
    fn renders_exact_block_with_separators() {
        let mut b = ImportBlock::new();
        b.extend([
            named("decimal", "Decimal"),
            named("typing", "Annotated"),
            named("typing", "Optional"),
            named("pydantic", "BaseModel"),
            sibling(&["com", "Adresse"], "Adresse"),
            sibling(&["bo", "Geschaeftspartner"], "Geschaeftspartner"),
        ]);
        let expected = "\
from decimal import Decimal
from typing import Annotated, Optional

from pydantic import BaseModel

from ..bo.geschaeftspartner import Geschaeftspartner
from ..com.adresse import Adresse";
        assert_eq!(b.render(2), expected);
    }
}
