use petgraph::Graph;

use crate::models::graph::Cardinality;

pub type PetGraph = Graph<Vec<String>, EdgeData>;

#[derive(Debug, Clone)]
pub struct EdgeData {
    pub through_field: String,
    pub cardinality: Cardinality,
}

pub fn extract(
    _: &bo4e_schemas::models::schema_meta::Schemas,
) -> Result<crate::models::graph::GraphIR, String> {
    Err("not implemented yet".into())
}
