use crate::graph::extract::PetGraph;
use petgraph::graph::NodeIndex;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;

pub struct Communities {
    /// Community id per node (0-indexed, dense after compaction).
    pub of: HashMap<NodeIndex, usize>,
    /// Final modularity score for diagnostics.
    pub modularity: f64,
}

/// Run Louvain modularity on the undirected projection of `g`. Multi-edges sum.
///
/// `seed` controls tie-breaking inside the move phase. Node iteration order
/// is sorted lexicographically by the node's module path for reproducibility.
pub fn louvain(g: &PetGraph, seed: u64) -> Communities {
    if g.node_count() == 0 {
        return Communities {
            of: HashMap::new(),
            modularity: 0.0,
        };
    }

    let weights = build_undirected_weights(g);
    let degrees = node_degrees(&weights);
    // m = total number of undirected edges (each stored once in `weights`).
    // Blondel et al. define m = (1/2) Σ_{ij} A_{ij} for the full symmetric matrix;
    // since we store each undirected pair exactly once, sum(weights) already equals m.
    let m: f64 = weights.values().sum::<f64>();
    let mut rng = StdRng::seed_from_u64(seed);

    let mut comm: HashMap<NodeIndex, usize> =
        g.node_indices().enumerate().map(|(i, n)| (n, i)).collect();

    loop {
        let mut moved = false;
        let mut order: Vec<NodeIndex> = g.node_indices().collect();
        order.sort_by(|a, b| g[*a].cmp(&g[*b]));
        for n in order {
            let current = comm[&n];
            let best = best_community_for(n, current, &comm, &weights, &degrees, m, &mut rng);
            if best != current {
                comm.insert(n, best);
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }

    // Compact community ids to 0..k.
    let mut keys: Vec<usize> = comm.values().copied().collect();
    keys.sort_unstable();
    keys.dedup();
    let remap: HashMap<usize, usize> = keys.into_iter().enumerate().map(|(i, k)| (k, i)).collect();
    let final_comm: HashMap<NodeIndex, usize> = comm.iter().map(|(k, v)| (*k, remap[v])).collect();

    let q = modularity(&final_comm, &weights, &degrees, m);
    Communities {
        of: final_comm,
        modularity: q,
    }
}

fn build_undirected_weights(g: &PetGraph) -> HashMap<(NodeIndex, NodeIndex), f64> {
    let mut w: HashMap<(NodeIndex, NodeIndex), f64> = HashMap::new();
    for e in g.edge_indices() {
        let (a, b) = g.edge_endpoints(e).unwrap();
        let key = if a <= b { (a, b) } else { (b, a) };
        *w.entry(key).or_insert(0.0) += 1.0;
    }
    w
}

fn node_degrees(weights: &HashMap<(NodeIndex, NodeIndex), f64>) -> HashMap<NodeIndex, f64> {
    let mut d: HashMap<NodeIndex, f64> = HashMap::new();
    for (&(a, b), w) in weights {
        if a == b {
            *d.entry(a).or_insert(0.0) += 2.0 * w;
        } else {
            *d.entry(a).or_insert(0.0) += w;
            *d.entry(b).or_insert(0.0) += w;
        }
    }
    d
}

fn neighbour_weights(
    n: NodeIndex,
    weights: &HashMap<(NodeIndex, NodeIndex), f64>,
) -> HashMap<NodeIndex, f64> {
    let mut out: HashMap<NodeIndex, f64> = HashMap::new();
    for (&(a, b), w) in weights {
        if a == n && b != n {
            *out.entry(b).or_insert(0.0) += w;
        } else if b == n && a != n {
            *out.entry(a).or_insert(0.0) += w;
        }
    }
    out
}

fn best_community_for(
    n: NodeIndex,
    current: usize,
    comm: &HashMap<NodeIndex, usize>,
    weights: &HashMap<(NodeIndex, NodeIndex), f64>,
    degrees: &HashMap<NodeIndex, f64>,
    m: f64,
    rng: &mut StdRng,
) -> usize {
    let k_i = degrees.get(&n).copied().unwrap_or(0.0);
    let neighbours = neighbour_weights(n, weights);

    let mut sum_in: HashMap<usize, f64> = HashMap::new();
    let mut sum_tot: HashMap<usize, f64> = HashMap::new();
    for (&node, &c) in comm {
        if node == n {
            continue;
        }
        *sum_tot.entry(c).or_insert(0.0) += degrees.get(&node).copied().unwrap_or(0.0);
    }
    for (&nb, &w) in &neighbours {
        let c = comm[&nb];
        *sum_in.entry(c).or_insert(0.0) += w;
    }

    let mut candidates: Vec<usize> = sum_in.keys().copied().collect();
    candidates.push(current);
    candidates.sort_unstable();
    candidates.dedup();

    let mut best = current;
    let mut best_gain = 0.0;
    for c in &candidates {
        let s_in = sum_in.get(c).copied().unwrap_or(0.0);
        let s_tot = sum_tot.get(c).copied().unwrap_or(0.0);
        let gain = s_in / m - (k_i * s_tot) / (2.0 * m * m);
        if (gain - best_gain).abs() < 1e-12 {
            if rng.random::<bool>() {
                best = *c;
            }
        } else if gain > best_gain {
            best_gain = gain;
            best = *c;
        }
    }
    best
}

fn modularity(
    comm: &HashMap<NodeIndex, usize>,
    weights: &HashMap<(NodeIndex, NodeIndex), f64>,
    degrees: &HashMap<NodeIndex, f64>,
    m: f64,
) -> f64 {
    if m == 0.0 {
        return 0.0;
    }
    // Use the per-community form: Q = Σ_c [ L_c/m - (d_c / (2m))^2 ]
    // where L_c = sum of edge weights within community c (unique edges, each counted once),
    // and d_c = sum of node degrees within community c.
    // Blondel et al. 2008, eq. after (1). This avoids double-counting issues.
    let mut l_c: HashMap<usize, f64> = HashMap::new();
    let mut d_c: HashMap<usize, f64> = HashMap::new();
    for (&(a, b), w) in weights {
        let ca = comm[&a];
        if ca == comm[&b] {
            *l_c.entry(ca).or_insert(0.0) += w;
        }
    }
    for (&node, &deg) in degrees {
        let c = comm[&node];
        *d_c.entry(c).or_insert(0.0) += deg;
    }
    let communities: std::collections::HashSet<usize> = comm.values().copied().collect();
    let mut q = 0.0;
    for c in communities {
        let lc = l_c.get(&c).copied().unwrap_or(0.0);
        let dc = d_c.get(&c).copied().unwrap_or(0.0);
        q += lc / m - (dc / (2.0 * m)).powi(2);
    }
    q
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

    fn make_two_triangles() -> PetGraph {
        let mut g: PetGraph = PetGraph::new();
        let a = g.add_node(vec!["A".into()]);
        let b = g.add_node(vec!["B".into()]);
        let c = g.add_node(vec!["C".into()]);
        let d = g.add_node(vec!["D".into()]);
        let e = g.add_node(vec!["E".into()]);
        let f = g.add_node(vec!["F".into()]);
        let ed = || EdgeData {
            through_field: "x".into(),
            cardinality: card(),
        };
        g.add_edge(a, b, ed());
        g.add_edge(b, c, ed());
        g.add_edge(c, a, ed());
        g.add_edge(d, e, ed());
        g.add_edge(e, f, ed());
        g.add_edge(f, d, ed());
        g
    }

    #[test]
    fn two_triangles_yield_two_communities() {
        let g = make_two_triangles();
        let comms = louvain(&g, 42);
        let unique: std::collections::HashSet<_> = comms.of.values().copied().collect();
        assert_eq!(
            unique.len(),
            2,
            "expected exactly 2 communities, got {:?}",
            unique
        );
        // Modularity for two disjoint triangles is 0.5.
        assert!(
            (comms.modularity - 0.5).abs() < 0.01,
            "modularity should be ~0.5, got {}",
            comms.modularity
        );
    }

    #[test]
    fn empty_graph_returns_empty_communities() {
        let g: PetGraph = PetGraph::new();
        let comms = louvain(&g, 0);
        assert!(comms.of.is_empty());
        assert_eq!(comms.modularity, 0.0);
    }

    #[test]
    fn singleton_graph_yields_one_community() {
        let mut g: PetGraph = PetGraph::new();
        g.add_node(vec!["X".into()]);
        let comms = louvain(&g, 0);
        assert_eq!(comms.of.len(), 1);
        let unique: std::collections::HashSet<_> = comms.of.values().copied().collect();
        assert_eq!(unique.len(), 1);
    }

    #[test]
    fn same_seed_yields_same_communities() {
        let g = make_two_triangles();
        let a = louvain(&g, 17);
        let b = louvain(&g, 17);
        assert_eq!(a.of, b.of);
    }
}
