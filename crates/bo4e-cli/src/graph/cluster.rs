use crate::graph::extract::PetGraph;
use petgraph::graph::NodeIndex;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
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
///
/// The graph is built without node removals, so `NodeIndex::index()` is a dense
/// `0..n`. We key everything on that `usize` and use flat `Vec`s / adjacency
/// lists instead of hashing `NodeIndex` on every lookup and rescanning all
/// edges for every node's neighbours.
pub fn louvain(g: &PetGraph, seed: u64) -> Communities {
    let n = g.node_count();
    if n == 0 {
        return Communities {
            of: HashMap::new(),
            modularity: 0.0,
        };
    }

    let weights = build_undirected_weights(g);
    let adjacency = build_adjacency(&weights, n);
    let degrees = node_degrees(&weights, n);
    // m = total number of undirected edges (each stored once in `weights`).
    // Blondel et al. define m = (1/2) Σ_{ij} A_{ij} for the full symmetric matrix;
    // since we store each undirected pair exactly once, sum(weights) already equals m.
    let m: f64 = weights.values().sum::<f64>();
    let mut rng = StdRng::seed_from_u64(seed);

    let mut comm: Vec<usize> = (0..n).collect();

    // Deterministic visiting order: lexicographic by the node's module path.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| g[NodeIndex::new(a)].cmp(&g[NodeIndex::new(b)]));

    loop {
        let mut moved = false;
        for &node in &order {
            let current = comm[node];
            let best =
                best_community_for(node, current, &comm, &adjacency, &degrees, m, n, &mut rng);
            if best != current {
                comm[node] = best;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }

    // Compact community ids to 0..k.
    let mut keys: Vec<usize> = comm.clone();
    keys.sort_unstable();
    keys.dedup();
    let remap: HashMap<usize, usize> = keys.into_iter().enumerate().map(|(i, k)| (k, i)).collect();
    for c in comm.iter_mut() {
        *c = remap[c];
    }

    let q = modularity(&weights, &comm, &degrees, m);
    let of: HashMap<NodeIndex, usize> = (0..n).map(|i| (NodeIndex::new(i), comm[i])).collect();
    Communities { of, modularity: q }
}

/// Summed pairwise edge weights, each undirected pair stored once with the
/// lower index first. Self-loops are kept under `(i, i)`.
fn build_undirected_weights(g: &PetGraph) -> HashMap<(usize, usize), f64> {
    let mut w: HashMap<(usize, usize), f64> = HashMap::new();
    for e in g.edge_indices() {
        let (a, b) = g.edge_endpoints(e).unwrap();
        let (a, b) = (a.index(), b.index());
        let key = if a <= b { (a, b) } else { (b, a) };
        *w.entry(key).or_insert(0.0) += 1.0;
    }
    w
}

/// Neighbour adjacency list (self-loops excluded, since a node is never its own
/// neighbour in the move phase). Each undirected pair appears in both endpoints.
fn build_adjacency(weights: &HashMap<(usize, usize), f64>, n: usize) -> Vec<Vec<(usize, f64)>> {
    let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for (&(a, b), &w) in weights {
        if a == b {
            continue;
        }
        adj[a].push((b, w));
        adj[b].push((a, w));
    }
    adj
}

fn node_degrees(weights: &HashMap<(usize, usize), f64>, n: usize) -> Vec<f64> {
    let mut d = vec![0.0f64; n];
    for (&(a, b), &w) in weights {
        if a == b {
            d[a] += 2.0 * w;
        } else {
            d[a] += w;
            d[b] += w;
        }
    }
    d
}

#[allow(clippy::too_many_arguments)]
fn best_community_for(
    node: usize,
    current: usize,
    comm: &[usize],
    adjacency: &[Vec<(usize, f64)>],
    degrees: &[f64],
    m: f64,
    n: usize,
    rng: &mut StdRng,
) -> usize {
    let k_i = degrees[node];

    // sum_tot[c] = total degree of every node (except `node`) in community c.
    // Community ids live in 0..n (we only ever reuse existing ids), so a flat
    // Vec indexed by community id replaces the per-node HashMap.
    let mut sum_tot = vec![0.0f64; n];
    for (other, &c) in comm.iter().enumerate() {
        if other == node {
            continue;
        }
        sum_tot[c] += degrees[other];
    }

    // sum_in[c] = total edge weight from `node` into community c. Only the
    // communities of `node`'s neighbours are touched.
    let mut sum_in = vec![0.0f64; n];
    let mut candidates: Vec<usize> = Vec::with_capacity(adjacency[node].len() + 1);
    for &(nb, w) in &adjacency[node] {
        let c = comm[nb];
        if sum_in[c] == 0.0 {
            candidates.push(c);
        }
        sum_in[c] += w;
    }
    candidates.push(current);
    candidates.sort_unstable();
    candidates.dedup();

    let mut best = current;
    let mut best_gain = 0.0;
    for &c in &candidates {
        let s_in = sum_in[c];
        let s_tot = sum_tot[c];
        let gain = s_in / m - (k_i * s_tot) / (2.0 * m * m);
        if (gain - best_gain).abs() < 1e-12 {
            if rng.random::<bool>() {
                best = c;
            }
        } else if gain > best_gain {
            best_gain = gain;
            best = c;
        }
    }
    best
}

fn modularity(
    weights: &HashMap<(usize, usize), f64>,
    comm: &[usize],
    degrees: &[f64],
    m: f64,
) -> f64 {
    if m == 0.0 {
        return 0.0;
    }
    // Use the per-community form: Q = Σ_c [ L_c/m - (d_c / (2m))^2 ]
    // where L_c = sum of edge weights within community c (unique edges, each counted once),
    // and d_c = sum of node degrees within community c.
    // Blondel et al. 2008, eq. after (1). This avoids double-counting issues.
    let k = comm.iter().copied().max().map(|c| c + 1).unwrap_or(0);
    let mut l_c = vec![0.0f64; k];
    let mut d_c = vec![0.0f64; k];
    for (&(a, b), &w) in weights {
        if comm[a] == comm[b] {
            l_c[comm[a]] += w;
        }
    }
    for (node, &deg) in degrees.iter().enumerate() {
        d_c[comm[node]] += deg;
    }
    let mut q = 0.0;
    for c in 0..k {
        q += l_c[c] / m - (d_c[c] / (2.0 * m)).powi(2);
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
