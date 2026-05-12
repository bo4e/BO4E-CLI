//! Shared import-statement data model used by per-language renderers.
//!
//! Both Python (`python/imports.rs::ImportBlock`) and Rust
//! (`rust/imports.rs::UseBlock`) consume this type. The data model is
//! language-neutral; each language owns its own rendering.

/// A single import statement that a mapped type depends on.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Import {
    /// Import of a name from a named (external) module.
    /// Python: `from <module> import <name>`. Rust: `use <module>::<name>;`.
    Named { module: String, name: String },
    /// Relative import from a sibling generated module, preserving the
    /// original case of path segments. The rendering layer decides how to
    /// translate `module` into language-specific path syntax and how many
    /// levels of relative-prefix (`.`/`super::`) to emit.
    ///
    /// Example: a `$ref` to `"../bo/Geschaeftspartner.json"` produces
    /// `Sibling { module: vec!["bo", "Geschaeftspartner"], name: "Geschaeftspartner" }`.
    Sibling { module: Vec<String>, name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_imports_are_ordered_by_module_then_name() {
        let mut v = vec![
            Import::Named { module: "typing".into(), name: "Optional".into() },
            Import::Named { module: "decimal".into(), name: "Decimal".into() },
            Import::Named { module: "typing".into(), name: "Annotated".into() },
        ];
        v.sort();
        assert_eq!(
            v[0],
            Import::Named { module: "decimal".into(), name: "Decimal".into() }
        );
        assert_eq!(
            v[1],
            Import::Named { module: "typing".into(), name: "Annotated".into() }
        );
        assert_eq!(
            v[2],
            Import::Named { module: "typing".into(), name: "Optional".into() }
        );
    }

    #[test]
    fn sibling_and_named_can_coexist_in_sorted_set() {
        use std::collections::BTreeSet;
        let mut s = BTreeSet::new();
        s.insert(Import::Named { module: "decimal".into(), name: "Decimal".into() });
        s.insert(Import::Sibling {
            module: vec!["bo".into(), "Adresse".into()],
            name: "Adresse".into(),
        });
        assert_eq!(s.len(), 2);
    }
}
