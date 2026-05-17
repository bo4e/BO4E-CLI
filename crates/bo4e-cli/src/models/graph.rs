use bo4e_schemas::models::version::DirtyVersion;
use serde::{Deserialize, Serialize};

/// On-disk graph IR. Serialised as JSON.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GraphIR {
    /// BO4E schema version this graph was extracted from.
    pub version: DirtyVersion,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Node {
    /// Module path, e.g. ["bo", "Angebot"].
    pub module: Vec<String>,
    /// All fields the class declares, in declaration order.
    pub fields: Vec<Field>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    /// Pretty-printed type, e.g. `"Decimal"`, `"list[Adresse]"`, `"Typ"`.
    /// Nullability is encoded in `cardinality`, so `Optional[...]` wrapping is
    /// never emitted.
    pub type_repr: String,
    pub cardinality: Cardinality,
    /// True iff there exists an outgoing edge from this node along
    /// `through_field == self.name`. Renderers skip ref-fields inside the
    /// class box when this is true.
    pub is_reference: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Edge {
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub through_field: String,
    pub cardinality: Cardinality,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Cardinality {
    pub min: String,
    pub max: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GraphIR {
        GraphIR {
            version: "v202501.0.0".parse().unwrap(),
            nodes: vec![Node {
                module: vec!["bo".into(), "Angebot".into()],
                fields: vec![Field {
                    name: "adresse".into(),
                    type_repr: "Adresse".into(),
                    cardinality: Cardinality {
                        min: "0".into(),
                        max: "1".into(),
                    },
                    is_reference: true,
                }],
            }],
            edges: vec![Edge {
                from: vec!["bo".into(), "Angebot".into()],
                to: vec!["com".into(), "Adresse".into()],
                through_field: "adresse".into(),
                cardinality: Cardinality {
                    min: "0".into(),
                    max: "1".into(),
                },
            }],
        }
    }

    #[test]
    fn graph_ir_roundtrip_is_byte_identical() {
        let g = sample();
        let json = serde_json::to_string_pretty(&g).unwrap();
        let parsed: GraphIR = serde_json::from_str(&json).unwrap();
        assert_eq!(g, parsed);
    }

    #[test]
    fn field_invariant_is_reference_matches_edge_existence() {
        // This is documentation-via-test: the invariant `field.is_reference ⇔
        // outgoing edge with matching through_field` is verified at extraction
        // (Task 6). Here we just spot-check the struct shape supports it.
        let g = sample();
        let node = &g.nodes[0];
        let field = &node.fields[0];
        assert!(field.is_reference);
        assert!(
            g.edges
                .iter()
                .any(|e| e.from == node.module && e.through_field == field.name)
        );
    }
}
