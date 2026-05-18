use crate::graph::extract::PetGraph;
use globset::{Glob, GlobMatcher};
use petgraph::graph::NodeIndex;
use petgraph::visit::Bfs;
use std::collections::HashSet;

pub struct FilterOptions {
    pub include: Vec<GlobMatcher>,
    pub exclude: Vec<GlobMatcher>,
    pub reachable_from: Option<Vec<String>>,
}

impl FilterOptions {
    pub fn new() -> Self {
        Self {
            include: vec![],
            exclude: vec![],
            reachable_from: None,
        }
    }

    pub fn include_glob(mut self, pattern: &str) -> Result<Self, String> {
        self.include.push(
            Glob::new(pattern)
                .map_err(|e| format!("Bad include glob {:?}: {}", pattern, e))?
                .compile_matcher(),
        );
        Ok(self)
    }

    pub fn exclude_glob(mut self, pattern: &str) -> Result<Self, String> {
        self.exclude.push(
            Glob::new(pattern)
                .map_err(|e| format!("Bad exclude glob {:?}: {}", pattern, e))?
                .compile_matcher(),
        );
        Ok(self)
    }
}

impl Default for FilterOptions {
    fn default() -> Self {
        Self::new()
    }
}

pub fn apply(mut g: PetGraph, opts: &FilterOptions) -> PetGraph {
    if let Some(start_module) = &opts.reachable_from {
        let start = g.node_indices().find(|nx| g[*nx] == *start_module);
        let reachable: HashSet<NodeIndex> = match start {
            Some(s) => {
                let mut set = HashSet::new();
                let mut bfs = Bfs::new(&g, s);
                while let Some(n) = bfs.next(&g) {
                    set.insert(n);
                }
                set
            }
            None => HashSet::new(),
        };
        g.retain_nodes(|_, n| reachable.contains(&n));
    }

    g.retain_nodes(|gref, n| {
        let path = gref[n].join(".");
        let included = opts.include.is_empty() || opts.include.iter().any(|gl| gl.is_match(&path));
        let excluded = opts.exclude.iter().any(|gl| gl.is_match(&path));
        included && !excluded
    });

    g
}

/// Keep only edges where at least one endpoint equals `root`.
pub fn retain_edges_incident_on(mut g: PetGraph, root: NodeIndex) -> PetGraph {
    g.retain_edges(|gref, e| {
        let (a, b) = gref.edge_endpoints(e).unwrap();
        a == root || b == root
    });
    g
}

pub fn ego_graph(g: &PetGraph, root: NodeIndex, radius: usize) -> PetGraph {
    let kept = bfs_with_depth_limit(g, root, radius);
    let mut out = g.clone();
    out.retain_nodes(|_, n| kept.contains(&n));
    out
}

fn bfs_with_depth_limit(g: &PetGraph, root: NodeIndex, radius: usize) -> HashSet<NodeIndex> {
    let mut kept = HashSet::new();
    kept.insert(root);
    let mut frontier: Vec<NodeIndex> = vec![root];
    for _ in 0..radius {
        let mut next: Vec<NodeIndex> = Vec::new();
        for &n in &frontier {
            for nb in g.neighbors(n) {
                if kept.insert(nb) {
                    next.push(nb);
                }
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    kept
}

/// Per-package scope defaults for `bo4e graph single`. Returns the glob patterns
/// to apply when the user passed no explicit `--include`.
pub fn default_scope_for(root_pkg: &str) -> Vec<&'static str> {
    match root_pkg {
        "bo" | "com" => vec!["bo.*", "com.*", "enum.*"],
        "enum" => vec!["enum.*"],
        _ => vec!["*"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::extract::EdgeData;
    use crate::models::graph::Cardinality;

    fn card() -> Cardinality {
        Cardinality {
            min: "0".into(),
            max: "1".into(),
        }
    }

    fn make() -> (PetGraph, NodeIndex, NodeIndex, NodeIndex, NodeIndex) {
        let mut g: PetGraph = PetGraph::new();
        let a = g.add_node(vec!["bo".into(), "Angebot".into()]);
        let b = g.add_node(vec!["com".into(), "Adresse".into()]);
        let c = g.add_node(vec!["enum".into(), "Typ".into()]);
        let d = g.add_node(vec!["bo".into(), "Dokument".into()]);
        g.add_edge(
            a,
            b,
            EdgeData {
                through_field: "adresse".into(),
                cardinality: card(),
            },
        );
        g.add_edge(
            a,
            c,
            EdgeData {
                through_field: "typ".into(),
                cardinality: card(),
            },
        );
        g.add_edge(
            d,
            b,
            EdgeData {
                through_field: "adresse".into(),
                cardinality: card(),
            },
        );
        (g, a, b, c, d)
    }

    #[test]
    fn include_glob_keeps_only_matches() {
        let (g, _, _, _, _) = make();
        let opts = FilterOptions::new().include_glob("bo.*").unwrap();
        let out = apply(g, &opts);
        let modules: Vec<_> = out.node_indices().map(|nx| out[nx].clone()).collect();
        assert_eq!(modules.len(), 2);
        assert!(modules.iter().all(|m| m[0] == "bo"));
    }

    #[test]
    fn exclude_glob_drops_matches() {
        let (g, _, _, _, _) = make();
        let opts = FilterOptions::new().exclude_glob("enum.*").unwrap();
        let out = apply(g, &opts);
        let modules: Vec<_> = out.node_indices().map(|nx| out[nx].clone()).collect();
        assert_eq!(modules.len(), 3);
        assert!(!modules.iter().any(|m| m[0] == "enum"));
    }

    #[test]
    fn reachable_from_restricts_to_descendants() {
        let (g, _, _, _, _) = make();
        let opts = FilterOptions {
            include: vec![],
            exclude: vec![],
            reachable_from: Some(vec!["bo".into(), "Angebot".into()]),
        };
        let out = apply(g, &opts);
        let modules: Vec<_> = out.node_indices().map(|nx| out[nx].clone()).collect();
        // Reachable from Angebot: Angebot, Adresse, Typ. Not Dokument.
        assert_eq!(modules.len(), 3);
        assert!(
            !modules
                .iter()
                .any(|m| m == &vec!["bo".to_string(), "Dokument".to_string()])
        );
    }

    #[test]
    fn ego_graph_radius_one_keeps_root_and_neighbours() {
        let (g, a, _, _, _) = make();
        let out = ego_graph(&g, a, 1);
        // From Angebot, neighbours = Adresse, Typ.
        assert_eq!(out.node_count(), 3);
    }

    #[test]
    fn retain_edges_incident_on_root_drops_unrelated_edges() {
        let (g, a, _, _, _) = make();
        let out = retain_edges_incident_on(g, a);
        // Only edges touching `a` remain: a->b and a->c. The d->b edge is gone.
        assert_eq!(out.edge_count(), 2);
    }

    #[test]
    fn default_scope_for_known_packages() {
        assert_eq!(default_scope_for("bo"), vec!["bo.*", "com.*", "enum.*"]);
        assert_eq!(default_scope_for("com"), vec!["bo.*", "com.*", "enum.*"]);
        assert_eq!(default_scope_for("enum"), vec!["enum.*"]);
        assert_eq!(default_scope_for("other"), vec!["*"]);
    }
}
