//! Per-language filesystem layout helpers: turn a schema module path
//! (e.g. `["bo", "Angebot"]`) into an output directory + file name + depth.
//!
//! These helpers are language-neutral; the caller passes the file extension
//! (`"py"` for Python, `"rs"` for Rust) and the resulting `file_name` includes
//! that extension. The leaf module name is lowercased (matches BO4E PascalCase
//! → snake-style file naming convention for both languages).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Lower-case the schema's last module segment to form its module file stem.
/// `module_file_name(&["bo", "Angebot"])` → `"angebot"`.
pub fn module_file_name(module: &[String]) -> String {
    module
        .last()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default()
}

/// Compute the output directory, file name, and import depth for a schema
/// with the given module path. Pure — does not touch the filesystem.
///
/// Returns `(out_dir, file_name, depth)` where:
/// - `out_dir` = `output_dir` joined with the lowercased preceding path segments
/// - `file_name` = `module_file_name(module) + "." + extension`
/// - `depth` = `path_segments.len() + 1` (1 = root-level module, 2 = one subdir, …)
pub fn module_paths(
    output_dir: &Path,
    module: &[String],
    extension: &str,
) -> (PathBuf, String, usize) {
    let path_segments: Vec<String> = module
        .iter()
        .take(module.len().saturating_sub(1))
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let mut out_dir = output_dir.to_path_buf();
    for seg in &path_segments {
        out_dir.push(seg);
    }
    let file_name = format!("{}.{extension}", module_file_name(module));
    let depth = path_segments.len() + 1;
    (out_dir, file_name, depth)
}

/// Collect the set of first-level subpackage directory names from an iterator of module paths.
/// A module of length 1 (e.g. `["__version__"]`) is at the root and contributes nothing.
pub fn first_level_subdirs<'a, I>(modules: I) -> BTreeSet<String>
where
    I: IntoIterator<Item = &'a [String]>,
{
    modules
        .into_iter()
        .filter(|m| m.len() > 1)
        .map(|m| m[0].to_ascii_lowercase())
        .collect()
}

/// Convenience wrapper for the common pydantic/rust-plain pattern of collecting
/// the module path of every schema in `schemas` into a side `Vec`, just to feed
/// [`first_level_subdirs`]. The intermediate `Vec<Vec<String>>` exists purely
/// because each `s.borrow()` is a short-lived `Ref` that can't be slice-borrowed
/// across the `first_level_subdirs` call.
#[cfg(any(
    feature = "python-pydantic",
    feature = "python-sql-model",
    feature = "rust-plain",
))]
pub fn first_level_subdirs_from_schemas(schemas: &bo4e_schemas::Schemas) -> BTreeSet<String> {
    let modules: Vec<Vec<String>> = schemas
        .iter()
        .map(|s| s.borrow().module().to_vec())
        .collect();
    first_level_subdirs(modules.iter().map(Vec::as_slice))
}

/// A leaf module: one schema file at some directory in the output tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LeafModule {
    /// Lowercased file stem (no extension).
    pub leaf: String,
    /// Original PascalCase class name from the schema title.
    pub class_name: String,
}

/// Direct contents of one directory in the generated module tree.
#[derive(Debug, Default, Clone)]
pub struct ModuleNode {
    /// Leaf schema files placed directly in this directory.
    pub leaves: Vec<LeafModule>,
    /// Direct-child subdirectory segment names (lowercased). Each entry has
    /// its own [`ModuleNode`] in [`ModuleTree::nodes`] keyed by the parent
    /// path joined with the segment.
    pub children: std::collections::BTreeSet<String>,
}

/// A tree view of where every schema lives in the output, including
/// root-level files (schemas with `module.len() == 1`) and arbitrary
/// nested depths. Built once from a [`bo4e_schemas::Schemas`] and used by
/// the per-flavour generators to emit `mod.rs` / `__init__.py` at every
/// directory level.
#[derive(Debug, Default, Clone)]
pub struct ModuleTree {
    /// Maps a *lowercased* directory path (`[]` = output root) to its
    /// direct contents. Every directory that contains at least one leaf
    /// or one child gets an entry.
    pub nodes: std::collections::BTreeMap<Vec<String>, ModuleNode>,
}

impl ModuleTree {
    /// Build the tree from a [`bo4e_schemas::Schemas`] collection.
    ///
    /// - Directory segments are lowercased to match the on-disk paths.
    /// - Leaf file stems are also lowercased (`Angebot.json` →
    ///   `angebot`); the original PascalCase title is preserved as
    ///   [`LeafModule::class_name`] for re-exports.
    /// - Schemas whose module path is empty are skipped (defensive).
    #[cfg(any(
        feature = "python-pydantic",
        feature = "python-sql-model",
        feature = "rust-plain",
    ))]
    pub fn from_schemas(schemas: &bo4e_schemas::Schemas) -> Self {
        let mut tree = Self::default();
        for schema_rc in schemas {
            let s = schema_rc.borrow();
            let module = s.module();
            if module.is_empty() {
                continue;
            }
            let class_name = s.name().to_string();
            tree.insert(module, class_name);
        }
        tree
    }

    /// Insert one schema into the tree. Sets up parent nodes for every
    /// intermediate directory in the path so `mod.rs` files can be emitted
    /// at every level.
    pub fn insert(&mut self, module: &[String], class_name: String) {
        // Lowercased directory segments (everything except the leaf).
        let dir: Vec<String> = module
            .iter()
            .take(module.len().saturating_sub(1))
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let leaf_stem = module_file_name(module);

        // Ensure the leaf's parent directory has an entry; add the leaf.
        self.nodes
            .entry(dir.clone())
            .or_default()
            .leaves
            .push(LeafModule {
                leaf: leaf_stem,
                class_name,
            });

        // Walk up the parent chain, recording each `child` segment on its
        // parent's `children` set. We stop one level above the leaf
        // because we just inserted the leaf itself.
        let mut current = dir;
        while !current.is_empty() {
            let segment = current.last().cloned().expect("non-empty checked above");
            let parent: Vec<String> = current[..current.len() - 1].to_vec();
            self.nodes
                .entry(parent.clone())
                .or_default()
                .children
                .insert(segment);
            current = parent;
        }
    }

    /// Iterate `(dir_path, node)` pairs in deterministic (BTreeMap) order.
    pub fn iter(&self) -> impl Iterator<Item = (&Vec<String>, &ModuleNode)> {
        self.nodes.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn module_file_name_lowercases_last_segment() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        assert_eq!(module_file_name(&m), "angebot");
    }

    #[test]
    fn module_file_name_handles_single_segment() {
        let m = vec!["Typ".to_string()];
        assert_eq!(module_file_name(&m), "typ");
    }

    #[test]
    fn module_paths_python_extension() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        let (dir, file, depth) = module_paths(Path::new("/tmp/out"), &m, "py");
        assert_eq!(dir, Path::new("/tmp/out/bo"));
        assert_eq!(file, "angebot.py");
        assert_eq!(depth, 2);
    }

    #[test]
    fn module_paths_rust_extension() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        let (dir, file, depth) = module_paths(Path::new("/tmp/out"), &m, "rs");
        assert_eq!(dir, Path::new("/tmp/out/bo"));
        assert_eq!(file, "angebot.rs");
        assert_eq!(depth, 2);
    }

    #[test]
    fn module_paths_root_level_has_depth_1() {
        let m = vec!["Standalone".to_string()];
        let (dir, file, depth) = module_paths(Path::new("/tmp/out"), &m, "rs");
        assert_eq!(dir, Path::new("/tmp/out"));
        assert_eq!(file, "standalone.rs");
        assert_eq!(depth, 1);
    }

    #[test]
    fn module_tree_collects_root_and_nested_leaves() {
        let mut t = ModuleTree::default();
        t.insert(
            &["ZusatzAttribut".to_string()],
            "ZusatzAttribut".to_string(),
        );
        t.insert(
            &["bo".to_string(), "Angebot".to_string()],
            "Angebot".to_string(),
        );
        t.insert(
            &["foo".to_string(), "bar".to_string(), "Baz".to_string()],
            "Baz".to_string(),
        );

        // Root node has the root-level leaf and the two top-level subdirs as children.
        let root = t.nodes.get(&Vec::<String>::new()).expect("root node");
        assert_eq!(root.leaves.len(), 1);
        assert_eq!(root.leaves[0].leaf, "zusatzattribut");
        assert_eq!(root.leaves[0].class_name, "ZusatzAttribut");
        assert!(root.children.contains("bo"));
        assert!(root.children.contains("foo"));

        // `bo` has one leaf, no nested children.
        let bo = t.nodes.get(&vec!["bo".to_string()]).expect("bo node");
        assert_eq!(bo.leaves.len(), 1);
        assert_eq!(bo.leaves[0].class_name, "Angebot");
        assert!(bo.children.is_empty());

        // `foo` has no direct leaves but does have a child `bar`.
        let foo = t.nodes.get(&vec!["foo".to_string()]).expect("foo node");
        assert!(foo.leaves.is_empty());
        assert!(foo.children.contains("bar"));

        // `foo/bar` has the deeply-nested Baz leaf.
        let bar = t
            .nodes
            .get(&vec!["foo".to_string(), "bar".to_string()])
            .expect("foo/bar node");
        assert_eq!(bar.leaves.len(), 1);
        assert_eq!(bar.leaves[0].class_name, "Baz");
    }

    #[test]
    fn first_level_subdirs_collects_unique_lowercased() {
        let a = vec!["bo".to_string(), "Angebot".to_string()];
        let b = vec!["BO".to_string(), "Andere".to_string()];
        let c = vec!["com".to_string(), "Adresse".to_string()];
        let d = vec!["__version__".to_string()];
        let set = first_level_subdirs([a.as_slice(), b.as_slice(), c.as_slice(), d.as_slice()]);
        assert_eq!(set.len(), 2);
        assert!(set.contains("bo"));
        assert!(set.contains("com"));
    }
}
