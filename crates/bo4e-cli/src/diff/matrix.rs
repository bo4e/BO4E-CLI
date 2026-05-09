use crate::diff::filters::has_critical;
use crate::models::changes::{Change, ChangeType, Changes};
use crate::models::matrix::{
    Compatibility, CompatibilityMatrix, CompatibilityMatrixEntry, CompatibilitySymbol,
    CompatibilityText,
};
use crate::models::schema_meta::Schemas;
use indexmap::IndexMap;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Debug)]
pub struct VersionChain {
    pub nodes: Vec<ChainNode>,
    pub edges: Vec<ChainEdge>,
}

#[derive(Debug)]
pub struct ChainNode {
    pub version_key: String,
    pub schemas: Schemas,
}

#[derive(Debug)]
pub struct ChainEdge {
    pub changes: Changes,
}

pub fn build_chain(diffs: Vec<Changes>) -> Result<VersionChain, String> {
    if diffs.is_empty() {
        return Err("Cannot build a version chain from zero diffs.".to_string());
    }

    let mut nodes: HashMap<String, Schemas> = HashMap::new();
    let mut insert_node = |key: String, s: Schemas| -> Result<(), String> {
        if let Some(existing) = nodes.get(&key) {
            if existing != &s {
                return Err(format!(
                    "Node {} already exists with different attributes.",
                    key
                ));
            }
            return Ok(());
        }
        nodes.insert(key, s);
        Ok(())
    };

    let mut out_edge: HashMap<String, usize> = HashMap::new();
    let mut in_keys: HashSet<String> = HashSet::new();

    for (idx, d) in diffs.iter().enumerate() {
        let old_key = d.old_version().to_string();
        let new_key = d.new_version().to_string();
        insert_node(old_key.clone(), d.old_schemas.clone())?;
        insert_node(new_key.clone(), d.new_schemas.clone())?;
        if out_edge.insert(old_key.clone(), idx).is_some() {
            return Err(format!("Duplicate outgoing edge from version {}.", old_key));
        }
        if !in_keys.insert(new_key.clone()) {
            return Err(format!("Duplicate incoming edge to version {}.", new_key));
        }
    }

    let starts: Vec<&String> = nodes.keys().filter(|k| !in_keys.contains(*k)).collect();
    if starts.len() != 1 {
        return Err(format!(
            "Expected exactly one start node, found {}.",
            starts.len()
        ));
    }
    let start = starts[0].clone();

    let ends: Vec<&String> = nodes.keys().filter(|k| !out_edge.contains_key(*k)).collect();
    if ends.len() != 1 {
        return Err(format!("Expected exactly one end node, found {}.", ends.len()));
    }
    let end = ends[0].clone();

    let mut nodes_ordered: Vec<ChainNode> = Vec::new();
    let mut edges_ordered: Vec<ChainEdge> = Vec::new();
    let mut cursor = start.clone();
    nodes_ordered.push(ChainNode {
        version_key: cursor.clone(),
        schemas: nodes[&cursor].clone(),
    });

    while cursor != end {
        let next_idx = *out_edge
            .get(&cursor)
            .ok_or_else(|| format!("Disconnected chain: no outgoing edge from {}.", cursor))?;
        let edge_changes = diffs[next_idx].clone();
        let next_key = edge_changes.new_version().to_string();
        edges_ordered.push(ChainEdge {
            changes: edge_changes,
        });
        cursor = next_key;
        nodes_ordered.push(ChainNode {
            version_key: cursor.clone(),
            schemas: nodes[&cursor].clone(),
        });
    }

    if edges_ordered.len() != diffs.len() {
        return Err(
            "Disconnected chain: not all diffs are reachable from the start.".to_string(),
        );
    }

    Ok(VersionChain {
        nodes: nodes_ordered,
        edges: edges_ordered,
    })
}

pub fn create_compatibility_matrix(
    chain: &VersionChain,
    use_emotes: bool,
) -> CompatibilityMatrix {
    let mut modules: BTreeSet<Vec<String>> = BTreeSet::new();
    for node in &chain.nodes {
        for m in node.schemas.modules() {
            modules.insert(m.clone());
        }
    }

    // Sort by lowercased path tuple, matching Python's `sorted(..., key=lambda m: tuple(p.lower()))`.
    let mut sorted: Vec<Vec<String>> = modules.into_iter().collect();
    sorted.sort_by_key(|m| m.iter().map(|p| p.to_lowercase()).collect::<Vec<_>>());

    let mut root: IndexMap<String, Vec<CompatibilityMatrixEntry>> = IndexMap::new();
    for module in &sorted {
        // Traces emitted by `diff::diff` are `/<module/parts>` for class-level changes and
        // `/<module/parts>/<field>...` for descendants — no `#` separator anywhere. We match
        // either an exact class-level trace or a descendant trace via a `/`-anchored prefix
        // so that `enum.Typ` does not accidentally match `enum.TypX`.
        let class_path_str = format!("/{}", module.join("/"));
        let descendant_prefix = format!("{}/", class_path_str);
        let belongs = |trace: &str| trace == class_path_str || trace.starts_with(&descendant_prefix);
        let mut entries: Vec<CompatibilityMatrixEntry> = Vec::with_capacity(chain.edges.len());

        for (i, edge) in chain.edges.iter().enumerate() {
            let node_a = &chain.nodes[i];
            let node_b = &chain.nodes[i + 1];

            let filtered: Vec<&Change> = edge
                .changes
                .changes
                .iter()
                .filter(|c| belongs(&c.old_trace) || belongs(&c.new_trace))
                .collect();

            let symbol = determine_compatibility(&filtered, &node_b.schemas, module, use_emotes);
            entries.push(CompatibilityMatrixEntry {
                previous_version: node_a.schemas.version.clone(),
                next_version: node_b.schemas.version.clone(),
                compatibility: symbol,
            });
        }

        root.insert(module.join("."), entries);
    }

    CompatibilityMatrix { root }
}

fn determine_compatibility(
    filtered: &[&Change],
    node_b: &Schemas,
    module: &[String],
    use_emotes: bool,
) -> Compatibility {
    let module_vec = module.to_vec();
    let exists_in_new = node_b.modules().iter().any(|m| **m == module_vec);

    // Single-change short-circuits run before the empty-filter check so that a sole
    // ClassAdded/ClassRemoved is reported even when the module's presence in node_b
    // would otherwise dictate a different cell.
    if filtered.len() == 1 {
        match filtered[0].r#type {
            ChangeType::ClassRemoved => return wrap_symbol(use_emotes, Sym::Removed),
            ChangeType::ClassAdded => return wrap_symbol(use_emotes, Sym::Added),
            _ => {}
        }
    }
    if !exists_in_new {
        return wrap_symbol(use_emotes, Sym::NonExistent);
    }
    if filtered.is_empty() {
        return wrap_symbol(use_emotes, Sym::ChangeNone);
    }

    debug_assert!(
        filtered
            .iter()
            .all(|c| !matches!(c.r#type, ChangeType::ClassAdded | ChangeType::ClassRemoved)),
        "ClassAdded/ClassRemoved must be the sole change in filtered list",
    );

    if has_critical(filtered.iter().copied()) {
        wrap_symbol(use_emotes, Sym::ChangeCritical)
    } else {
        wrap_symbol(use_emotes, Sym::ChangeNonCritical)
    }
}

#[derive(Copy, Clone)]
enum Sym {
    ChangeNone,
    ChangeNonCritical,
    ChangeCritical,
    NonExistent,
    Added,
    Removed,
}

fn wrap_symbol(use_emotes: bool, s: Sym) -> Compatibility {
    if use_emotes {
        Compatibility::Symbol(match s {
            Sym::ChangeNone => CompatibilitySymbol::ChangeNone,
            Sym::ChangeNonCritical => CompatibilitySymbol::ChangeNonCritical,
            Sym::ChangeCritical => CompatibilitySymbol::ChangeCritical,
            Sym::NonExistent => CompatibilitySymbol::NonExistent,
            Sym::Added => CompatibilitySymbol::Added,
            Sym::Removed => CompatibilitySymbol::Removed,
        })
    } else {
        Compatibility::Text(match s {
            Sym::ChangeNone => CompatibilityText::ChangeNone,
            Sym::ChangeNonCritical => CompatibilityText::ChangeNonCritical,
            Sym::ChangeCritical => CompatibilityText::ChangeCritical,
            Sym::NonExistent => CompatibilityText::NonExistent,
            Sym::Added => CompatibilityText::Added,
            Sym::Removed => CompatibilityText::Removed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::changes::{Change, ChangeType, Changes};
    use crate::models::schema_meta::{Schema, Schemas};
    use crate::models::version::DirtyVersion;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn schema_from(module: &[&str]) -> Rc<RefCell<Schema>> {
        let m: Vec<String> = module.iter().map(|s| s.to_string()).collect();
        Rc::new(RefCell::new(Schema::new(m, None).unwrap()))
    }

    fn coll(version: &str, modules: &[&[&str]]) -> Schemas {
        let v: DirtyVersion = version.parse().unwrap();
        let mut s = Schemas::new(v);
        for m in modules {
            s.add_schema(schema_from(m)).unwrap();
        }
        s
    }

    fn changes_between(old_v: &str, new_v: &str, items: Vec<Change>) -> Changes {
        Changes {
            old_schemas: coll(old_v, &[&["bo", "Angebot"]]),
            new_schemas: coll(new_v, &[&["bo", "Angebot"]]),
            changes: items,
        }
    }

    #[test]
    fn test_build_chain_orders_three_unsorted_diffs() {
        let d_ab = changes_between("v202401.0.1", "v202401.0.2", vec![]);
        let d_bc = changes_between("v202401.0.2", "v202401.1.0", vec![]);
        let d_cd = changes_between("v202401.1.0", "v202402.0.0", vec![]);
        let chain = build_chain(vec![d_cd, d_ab, d_bc]).unwrap();
        let keys: Vec<_> = chain.nodes.iter().map(|n| n.version_key.clone()).collect();
        assert_eq!(
            keys,
            vec![
                "v202401.0.1".to_string(),
                "v202401.0.2".to_string(),
                "v202401.1.0".to_string(),
                "v202402.0.0".to_string(),
            ]
        );
        assert_eq!(chain.edges.len(), 3);
    }

    #[test]
    fn test_build_chain_rejects_two_starts() {
        let d1 = changes_between("v202401.0.1", "v202401.0.2", vec![]);
        let d2 = changes_between("v202401.1.0", "v202402.0.0", vec![]);
        let err = build_chain(vec![d1, d2]).unwrap_err();
        assert!(err.to_lowercase().contains("start") || err.to_lowercase().contains("disconnected"));
    }

    #[test]
    fn test_build_chain_rejects_duplicate_outgoing_edge() {
        let d1 = changes_between("v202401.0.1", "v202401.0.2", vec![]);
        let d2 = changes_between("v202401.0.1", "v202401.1.0", vec![]);
        let err = build_chain(vec![d1, d2]).unwrap_err();
        assert!(err.to_lowercase().contains("outgoing") || err.to_lowercase().contains("duplicate"));
    }

    #[test]
    fn test_build_chain_rejects_node_attribute_mismatch() {
        let d1 = Changes {
            old_schemas: coll("v202401.0.1", &[&["bo", "Angebot"]]),
            new_schemas: coll("v202401.0.2", &[&["bo", "Angebot"]]),
            changes: vec![],
        };
        let d2 = Changes {
            old_schemas: coll("v202401.0.2", &[&["enum", "Typ"]]),
            new_schemas: coll("v202401.1.0", &[&["bo", "Angebot"]]),
            changes: vec![],
        };
        let err = build_chain(vec![d1, d2]).unwrap_err();
        assert!(err.to_lowercase().contains("different attributes"));
    }

    #[test]
    fn test_create_matrix_emits_unchanged_and_added() {
        let class_added = Change {
            r#type: ChangeType::ClassAdded,
            old: None,
            new: None,
            old_trace: String::new(),
            new_trace: "/enum/Typ".to_string(),
        };
        let d = Changes {
            old_schemas: coll("v202401.0.1", &[&["bo", "Angebot"]]),
            new_schemas: coll("v202401.0.2", &[&["bo", "Angebot"], &["enum", "Typ"]]),
            changes: vec![class_added],
        };
        let chain = build_chain(vec![d]).unwrap();
        let matrix = create_compatibility_matrix(&chain, false);

        let bo_row = matrix.root.get("bo.Angebot").unwrap();
        assert_eq!(bo_row.len(), 1);
        assert!(matches!(
            bo_row[0].compatibility,
            Compatibility::Text(CompatibilityText::ChangeNone)
        ));
        let enum_row = matrix.root.get("enum.Typ").unwrap();
        assert!(matches!(
            enum_row[0].compatibility,
            Compatibility::Text(CompatibilityText::Added)
        ));
    }
}
