use crate::models::graph::GraphIR;
use std::path::Path;

pub fn read_graph(path: &Path) -> Result<GraphIR, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse {} as GraphIR: {}", path.display(), e))
}

pub fn write_graph_json(graph: &GraphIR, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
    }
    let text = serde_json::to_string_pretty(graph)
        .map_err(|e| format!("Failed to serialize GraphIR: {}", e))?;
    std::fs::write(path, text)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

pub fn write_graph_graphml(graph: &GraphIR, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
    }
    let mut out = String::new();
    out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    out.push('\n');
    out.push_str(r#"<graphml xmlns="http://graphml.graphdrawing.org/xmlns">"#);
    out.push('\n');
    out.push_str(r#"  <key id="field" for="edge" attr.name="through_field" attr.type="string"/>"#);
    out.push('\n');
    out.push_str(r#"  <key id="card" for="edge" attr.name="cardinality" attr.type="string"/>"#);
    out.push('\n');
    out.push_str(r#"  <graph edgedefault="directed">"#);
    out.push('\n');
    for n in &graph.nodes {
        let id = n.module.join(".");
        out.push_str(&format!(r#"    <node id="{}"/>"#, xml_escape(&id)));
        out.push('\n');
    }
    for e in &graph.edges {
        let from = e.from.join(".");
        let to = e.to.join(".");
        let card = format!("{}..{}", e.cardinality.min, e.cardinality.max);
        out.push_str(&format!(
            r#"    <edge source="{}" target="{}">"#,
            xml_escape(&from),
            xml_escape(&to)
        ));
        out.push('\n');
        out.push_str(&format!(
            r#"      <data key="field">{}</data>"#,
            xml_escape(&e.through_field)
        ));
        out.push('\n');
        out.push_str(&format!(
            r#"      <data key="card">{}</data>"#,
            xml_escape(&card)
        ));
        out.push('\n');
        out.push_str("    </edge>\n");
    }
    out.push_str("  </graph>\n</graphml>\n");
    std::fs::write(path, out)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::graph::{Cardinality, Edge, Field, GraphIR, Node};

    fn sample() -> GraphIR {
        GraphIR {
            version: "v202501.0.0".parse().unwrap(),
            nodes: vec![Node {
                module: vec!["bo".into(), "Angebot".into()],
                fields: vec![Field {
                    name: "adresse".into(),
                    type_repr: "Adresse".into(),
                    cardinality: Cardinality { min: "0".into(), max: "1".into() },
                    is_reference: true,
                }],
            }],
            edges: vec![Edge {
                from: vec!["bo".into(), "Angebot".into()],
                to: vec!["com".into(), "Adresse".into()],
                through_field: "adresse".into(),
                cardinality: Cardinality { min: "0".into(), max: "1".into() },
            }],
        }
    }

    #[test]
    fn json_roundtrip_via_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("graph.json");
        let g = sample();
        write_graph_json(&g, &p).unwrap();
        let back = read_graph(&p).unwrap();
        assert_eq!(g, back);
    }

    #[test]
    fn graphml_contains_node_and_edge_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("graph.graphml");
        write_graph_graphml(&sample(), &p).unwrap();
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(r#"<node id="bo.Angebot"/>"#));
        assert!(s.contains(r#"source="bo.Angebot""#));
        assert!(s.contains(r#"target="com.Adresse""#));
        assert!(s.contains(r#"<data key="field">adresse</data>"#));
        assert!(s.contains(r#"<data key="card">0..1</data>"#));
    }

    #[test]
    fn read_missing_file_errors_with_path() {
        let err = read_graph(std::path::Path::new("/no/such/file.json")).unwrap_err();
        assert!(err.contains("/no/such/file.json"));
    }
}
