use crate::graph::PetGraph;
use petgraph::graph::NodeIndex;

pub struct FilterOptions;

pub fn apply(g: PetGraph, _opts: &FilterOptions) -> PetGraph {
    g
}

pub fn ego_graph(_g: &PetGraph, _root: NodeIndex, _radius: usize) -> PetGraph {
    unimplemented!()
}
