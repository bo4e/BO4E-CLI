//! Per-file `use` block renderer for the Rust output.

use crate::imports::Import;
use std::collections::BTreeSet;

/// Collects [`Import`] values for one Rust source file and renders the `use` block.
#[derive(Debug, Default)]
pub(crate) struct UseBlock {
    items: BTreeSet<Import>,
}

impl UseBlock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn extend<I: IntoIterator<Item = Import>>(&mut self, items: I) {
        self.items.extend(items);
    }

    /// Render the use block. Sibling imports use `super::` repeated `depth` times.
    /// `depth` = 1 means the file is at the root (next to `mod.rs` / `lib.rs`).
    /// `depth` = 2 means the file lives one subdirectory down, etc.
    pub fn render(&self, depth: usize) -> String {
        debug_assert!(depth >= 1, "depth must be >= 1");

        // Named: shared grouping → `use module::Name;` / `use module::{A, B};`.
        let named_lines: Vec<String> = crate::imports::group_named_by_module(&self.items)
            .iter()
            .map(|(module, names)| {
                if names.len() == 1 {
                    let only = names.iter().next().unwrap();
                    format!("use {module}::{only};")
                } else {
                    let names_csv: String = names
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("use {module}::{{{names_csv}}};")
                }
            })
            .collect();

        // Sibling: `use super::super::…::lower::Name;` (path segments lower-cased).
        let mut sibling: BTreeSet<String> = BTreeSet::new();
        for item in &self.items {
            if let Import::Sibling { module, name } = item {
                let Some((last, head)) = module.split_last() else {
                    continue;
                };
                let supers: String = "super::".repeat(depth);
                let path: String = head
                    .iter()
                    .map(|s| s.to_ascii_lowercase())
                    .chain(std::iter::once(last.to_ascii_lowercase()))
                    .collect::<Vec<_>>()
                    .join("::");
                sibling.insert(format!("use {supers}{path}::{name};"));
            }
        }

        crate::imports::stitch_nonempty_blocks(
            &[
                named_lines.join("\n"),
                sibling.iter().cloned().collect::<Vec<_>>().join("\n"),
            ],
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
    fn empty_renders_empty() {
        assert_eq!(UseBlock::new().render(2), "");
    }

    #[test]
    fn single_named() {
        let mut b = UseBlock::new();
        b.extend([named("serde", "Serialize")]);
        assert_eq!(b.render(2), "use serde::Serialize;");
    }

    #[test]
    fn merges_two_names_same_module_with_braces() {
        let mut b = UseBlock::new();
        b.extend([named("serde", "Deserialize"), named("serde", "Serialize")]);
        assert_eq!(b.render(2), "use serde::{Deserialize, Serialize};");
    }

    #[test]
    fn chrono_grouped_imports() {
        let mut b = UseBlock::new();
        b.extend([named("chrono", "DateTime"), named("chrono", "Utc")]);
        assert_eq!(b.render(2), "use chrono::{DateTime, Utc};");
    }

    #[test]
    fn sibling_depth_2() {
        let mut b = UseBlock::new();
        b.extend([sibling(&["com", "Adresse"], "Adresse")]);
        assert_eq!(b.render(2), "use super::super::com::adresse::Adresse;");
    }

    #[test]
    fn sibling_depth_1_root_module() {
        let mut b = UseBlock::new();
        b.extend([sibling(&["enums", "Typ"], "Typ")]);
        assert_eq!(b.render(1), "use super::enums::typ::Typ;");
    }

    #[test]
    fn named_then_sibling_separated_by_blank_line() {
        let mut b = UseBlock::new();
        b.extend([
            named("serde", "Serialize"),
            named("serde", "Deserialize"),
            sibling(&["com", "Adresse"], "Adresse"),
        ]);
        let expected = "\
use serde::{Deserialize, Serialize};

use super::super::com::adresse::Adresse;";
        assert_eq!(b.render(2), expected);
    }
}
