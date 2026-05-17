use crate::graph::emit_common::{dotted, format_cardinality, render_link};
use crate::graph::extract::PetGraph;
use crate::models::graph::GraphIR;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;
use std::path::Path;

#[derive(Copy, Clone, Debug)]
pub enum Detail {
    None,
    Names,
    Full,
}

pub struct EmitOptions<'a> {
    /// Detail level applied to non-root nodes (or all nodes when `root` is None).
    pub detail: Detail,
    /// Detail level applied to the root node. Ignored when `root` is None.
    /// When `root` is set and `root_detail` is None, falls back to `detail`.
    pub root_detail: Option<Detail>,
    pub clusters: Option<&'a HashMap<NodeIndex, usize>>,
    /// When set, this node is rendered with `root_detail` (or `detail` as fallback);
    /// all others use `detail`. Used by per-class diagrams to highlight the root.
    pub root: Option<NodeIndex>,
    pub link_template: Option<&'a str>,
    pub cwd: &'a Path,
    pub output_dir: &'a Path,
    pub version: &'a str,
}

pub fn emit(g: &PetGraph, ir: &GraphIR, opts: &EmitOptions) -> String {
    let mut out = String::new();
    out.push_str("digraph BO4E {\n");
    out.push_str("    rankdir=LR;\n");
    out.push_str("    node [shape=record];\n\n");

    let node_by_module: HashMap<&Vec<String>, &crate::models::graph::Node> =
        ir.nodes.iter().map(|n| (&n.module, n)).collect();

    let render_node = |nx: NodeIndex, prefix: &str, out: &mut String| {
        let module = &g[nx];
        let module_dotted = dotted(module);
        let class_name = module.last().cloned().unwrap_or_default();
        let detail_for_this = match (opts.root, opts.root_detail) {
            (Some(r), Some(d)) if r == nx => d,
            _ => opts.detail,
        };
        let label = node_label(
            node_by_module.get(module).copied(),
            &class_name,
            detail_for_this,
            g,
            nx,
        );
        let pkg = module.first().cloned().unwrap_or_default();
        let url = render_link(
            opts.link_template,
            &pkg,
            &module_dotted,
            &class_name,
            opts.version,
            opts.cwd,
            opts.output_dir,
        );
        let url_attr = url.map(|u| format!(r#", URL="{}""#, u)).unwrap_or_default();
        out.push_str(&format!(
            r#"{}"{}" [label={}{}];"#,
            prefix, module_dotted, label, url_attr
        ));
        out.push('\n');
    };

    match opts.clusters {
        Some(comm) => {
            let mut by_cluster: HashMap<usize, Vec<NodeIndex>> = HashMap::new();
            for (&nx, &c) in comm {
                by_cluster.entry(c).or_default().push(nx);
            }
            let mut keys: Vec<usize> = by_cluster.keys().copied().collect();
            keys.sort_unstable();
            for c in keys {
                out.push_str(&format!("    subgraph cluster_{} {{\n", c));
                out.push_str(&format!("        label = \"Cluster {}\";\n", c));
                let mut nxs = by_cluster.remove(&c).unwrap();
                nxs.sort_by(|a, b| g[*a].cmp(&g[*b]));
                for nx in nxs {
                    render_node(nx, "        ", &mut out);
                }
                out.push_str("    }\n");
            }
            let mut orphans: Vec<NodeIndex> = g
                .node_indices()
                .filter(|nx| !comm.contains_key(nx))
                .collect();
            orphans.sort_by(|a, b| g[*a].cmp(&g[*b]));
            for nx in orphans {
                render_node(nx, "    ", &mut out);
            }
        }
        None => {
            let mut all: Vec<NodeIndex> = g.node_indices().collect();
            all.sort_by(|a, b| g[*a].cmp(&g[*b]));
            for nx in all {
                render_node(nx, "    ", &mut out);
            }
        }
    }

    out.push('\n');
    let mut edge_ixs: Vec<_> = g.edge_indices().collect();
    edge_ixs.sort_by(|a, b| {
        let (a1, a2) = g.edge_endpoints(*a).unwrap();
        let (b1, b2) = g.edge_endpoints(*b).unwrap();
        (g[a1].clone(), g[a2].clone(), g[*a].through_field.clone()).cmp(&(
            g[b1].clone(),
            g[b2].clone(),
            g[*b].through_field.clone(),
        ))
    });
    for ex in edge_ixs {
        let (a, b) = g.edge_endpoints(ex).unwrap();
        let data = &g[ex];
        let card = format_cardinality(&data.cardinality);
        out.push_str(&format!(
            r#"    "{}" -> "{}" [label="{} [{}]"];"#,
            dotted(&g[a]),
            dotted(&g[b]),
            data.through_field,
            card,
        ));
        out.push('\n');
    }
    out.push_str("}\n");
    out
}

fn node_label(
    node: Option<&crate::models::graph::Node>,
    class_name: &str,
    detail: Detail,
    g: &PetGraph,
    nx: NodeIndex,
) -> String {
    match detail {
        Detail::None => format!(r#""{}""#, class_name),
        Detail::Names | Detail::Full => {
            let mut inner = format!("{{{}|", class_name);
            if let Some(n) = node {
                let outgoing_fields: std::collections::HashSet<&str> = g
                    .edges(nx)
                    .map(|e| e.weight().through_field.as_str())
                    .collect();
                for field in &n.fields {
                    if outgoing_fields.contains(field.name.as_str()) {
                        continue;
                    }
                    match detail {
                        Detail::Names => inner.push_str(&format!("{}\\l", escape(&field.name))),
                        Detail::Full => inner.push_str(&format!(
                            "{} : {} [{}]\\l",
                            escape(&field.name),
                            escape(&field.type_repr),
                            format_cardinality(&field.cardinality),
                        )),
                        Detail::None => unreachable!(),
                    }
                }
            }
            inner.push('}');
            format!("\"{}\"", inner)
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('|', "\\|")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('<', "\\<")
        .replace('>', "\\>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::extract::to_petgraph;
    use crate::models::graph::{Cardinality, Edge, Field, GraphIR, Node};

    fn sample_ir() -> GraphIR {
        GraphIR {
            version: "v202501.0.0".parse().unwrap(),
            nodes: vec![
                Node {
                    module: vec!["bo".into(), "Angebot".into()],
                    fields: vec![
                        Field {
                            name: "adresse".into(),
                            type_repr: "Adresse".into(),
                            cardinality: Cardinality {
                                min: "0".into(),
                                max: "1".into(),
                            },
                            is_reference: true,
                        },
                        Field {
                            name: "betrag".into(),
                            type_repr: "Decimal".into(),
                            cardinality: Cardinality {
                                min: "1".into(),
                                max: "1".into(),
                            },
                            is_reference: false,
                        },
                    ],
                },
                Node {
                    module: vec!["com".into(), "Adresse".into()],
                    fields: vec![],
                },
            ],
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

    fn cwd() -> std::path::PathBuf {
        std::path::PathBuf::from("/x")
    }

    #[test]
    fn detail_none_emits_class_names_only() {
        let ir = sample_ir();
        let pg = to_petgraph(&ir);
        let s = emit(
            &pg,
            &ir,
            &EmitOptions {
                detail: Detail::None,
                root_detail: None,
                clusters: None,
                root: None,
                link_template: None,
                cwd: &cwd(),
                output_dir: &cwd(),
                version: "v1",
            },
        );
        assert!(s.contains(r#""bo.Angebot" [label="Angebot"]"#));
        assert!(s.contains(r#""com.Adresse" [label="Adresse"]"#));
        assert!(!s.contains("betrag"));
    }

    #[test]
    fn detail_full_includes_non_ref_fields_only() {
        let ir = sample_ir();
        let pg = to_petgraph(&ir);
        let s = emit(
            &pg,
            &ir,
            &EmitOptions {
                detail: Detail::Full,
                root_detail: None,
                clusters: None,
                root: None,
                link_template: None,
                cwd: &cwd(),
                output_dir: &cwd(),
                version: "v1",
            },
        );
        // betrag must appear inline (non-ref).
        assert!(s.contains("betrag : Decimal [1]"));
        // adresse must NOT appear inline (it's an edge).
        let angebot_idx = s.find("\"bo.Angebot\" [label=").unwrap();
        let after_idx_rel = s[angebot_idx..].find(';').unwrap();
        let angebot_record = &s[angebot_idx..angebot_idx + after_idx_rel];
        assert!(!angebot_record.contains("adresse"));
    }

    #[test]
    fn edge_label_contains_field_and_cardinality() {
        let ir = sample_ir();
        let pg = to_petgraph(&ir);
        let s = emit(
            &pg,
            &ir,
            &EmitOptions {
                detail: Detail::None,
                root_detail: None,
                clusters: None,
                root: None,
                link_template: None,
                cwd: &cwd(),
                output_dir: &cwd(),
                version: "v1",
            },
        );
        assert!(s.contains(r#""bo.Angebot" -> "com.Adresse" [label="adresse [0..1]"]"#));
    }

    #[test]
    fn clusters_render_subgraph_blocks() {
        let ir = sample_ir();
        let pg = to_petgraph(&ir);
        let mut comm = HashMap::new();
        for nx in pg.node_indices() {
            comm.insert(nx, if pg[nx][0] == "bo" { 0 } else { 1 });
        }
        let s = emit(
            &pg,
            &ir,
            &EmitOptions {
                detail: Detail::None,
                root_detail: None,
                clusters: Some(&comm),
                root: None,
                link_template: None,
                cwd: &cwd(),
                output_dir: &cwd(),
                version: "v1",
            },
        );
        assert!(s.contains("subgraph cluster_0"));
        assert!(s.contains("subgraph cluster_1"));
    }

    #[test]
    fn link_template_renders_url_attribute() {
        let ir = sample_ir();
        let pg = to_petgraph(&ir);
        let s = emit(
            &pg,
            &ir,
            &EmitOptions {
                detail: Detail::None,
                root_detail: None,
                clusters: None,
                root: None,
                link_template: Some("https://x/{pkg}/{class}"),
                cwd: &cwd(),
                output_dir: &cwd(),
                version: "v1",
            },
        );
        assert!(s.contains(r#"URL="https://x/bo/Angebot""#));
    }
}
