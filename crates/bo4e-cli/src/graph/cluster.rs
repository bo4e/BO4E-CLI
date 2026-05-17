use crate::graph::PetGraph;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;

pub struct Communities {
    pub of: HashMap<NodeIndex, usize>,
    pub modularity: f64,
}

pub fn louvain(_g: &PetGraph, _seed: u64) -> Communities {
    unimplemented!()
}
