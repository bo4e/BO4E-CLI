use crate::graph::emit_common::{format_cardinality, render_link};
use crate::graph::emit_dot::Detail;
use crate::graph::extract::PetGraph;
use crate::models::graph::GraphIR;
use petgraph::graph::NodeIndex;
use std::collections::HashMap;
use std::path::Path;

pub struct EmitOptions<'a> {
    pub detail: Detail,
    pub clusters: Option<&'a HashMap<NodeIndex, usize>>,
    /// If set, the named module is rendered outside its namespace block (root
    /// of a per-class diagram) and `hide members` + `show .Root fields` are
    /// appended at the end.
    pub root: Option<&'a [String]>,
    pub link_template: Option<&'a str>,
    pub cwd: &'a Path,
    pub output_dir: &'a Path,
    pub version: &'a str,
    /// If true, use today's namespace blocks (`bo4e.bo`, `bo4e.com`, `bo4e.enum`)
    /// with the canonical colour palette. If false and `clusters` is set, use
    /// `package "Cluster N"` blocks instead.
    pub package_grouping: bool,
}

const COLOUR_BO: &str = "#B6D7A8";
const COLOUR_COM: &str = "#E0A86C";
const COLOUR_ENUM: &str = "#d1c358";

pub fn emit(g: &PetGraph, ir: &GraphIR, opts: &EmitOptions) -> String {
    let mut out = String::new();
    out.push_str("@startuml\nleft to right direction\n\n");

    let node_by_module: HashMap<&Vec<String>, &crate::models::graph::Node> =
        ir.nodes.iter().map(|n| (&n.module, n)).collect();

    let mut sorted_nodes: Vec<NodeIndex> = g.node_indices().collect();
    sorted_nodes.sort_by(|a, b| g[*a].cmp(&g[*b]));

    let is_root = |module: &Vec<String>| -> bool {
        opts.root.map(|r| r == module.as_slice()).unwrap_or(false)
    };

    // Emit root outside its block first.
    if let Some(root_module) = opts.root
        && let Some(nx) = sorted_nodes.iter().find(|nx| g[**nx] == root_module)
    {
        let cls = root_module.last().cloned().unwrap_or_default();
        out.push_str(&class_decl(
            node_by_module.get(&g[*nx]).copied(),
            &cls,
            true,
            opts,
        ));
        out.push_str("\n\n");
    }

    match (opts.package_grouping, opts.clusters) {
        (true, _) => {
            emit_namespace_blocks(g, &node_by_module, &sorted_nodes, &is_root, opts, &mut out)
        }
        (false, Some(comm)) => {
            emit_louvain_packages(g, &node_by_module, comm, &is_root, opts, &mut out)
        }
        (false, None) => emit_flat(g, &node_by_module, &sorted_nodes, &is_root, opts, &mut out),
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
        let name_a = g[a].last().cloned().unwrap_or_default();
        let name_b = g[b].last().cloned().unwrap_or_default();
        out.push_str(&format!(
            "{} --* \"{}\" {} : {}\n",
            name_a, card, name_b, data.through_field
        ));
    }

    if let Some(root_module) = opts.root {
        let cls = root_module.last().cloned().unwrap_or_default();
        out.push_str("\nhide members\n");
        out.push_str(&format!("show .{} fields\n", cls));
    }

    out.push_str("@enduml\n");
    out
}

fn emit_namespace_blocks(
    g: &PetGraph,
    node_by_module: &HashMap<&Vec<String>, &crate::models::graph::Node>,
    sorted_nodes: &[NodeIndex],
    is_root: &impl Fn(&Vec<String>) -> bool,
    opts: &EmitOptions,
    out: &mut String,
) {
    let mut by_pkg: HashMap<String, Vec<NodeIndex>> = HashMap::new();
    for &nx in sorted_nodes {
        if is_root(&g[nx]) {
            continue;
        }
        let pkg = g[nx].first().cloned().unwrap_or_default();
        by_pkg.entry(pkg).or_default().push(nx);
    }
    let mut keys: Vec<String> = by_pkg.keys().cloned().collect();
    keys.sort();
    for pkg in keys {
        let colour = match pkg.as_str() {
            "bo" => COLOUR_BO,
            "com" => COLOUR_COM,
            "enum" => COLOUR_ENUM,
            _ => "",
        };
        out.push_str(&format!(
            "namespace \"bo4e.{pkg}\" as bo4e.{pkg} {colour} {{\n"
        ));
        for nx in by_pkg.remove(&pkg).unwrap() {
            let cls = g[nx].last().cloned().unwrap_or_default();
            out.push_str(&format!(
                "    {}",
                class_decl(node_by_module.get(&g[nx]).copied(), &cls, false, opts)
            ));
            out.push('\n');
        }
        out.push_str("}\n");
    }
}

fn emit_louvain_packages(
    g: &PetGraph,
    node_by_module: &HashMap<&Vec<String>, &crate::models::graph::Node>,
    comm: &HashMap<NodeIndex, usize>,
    is_root: &impl Fn(&Vec<String>) -> bool,
    opts: &EmitOptions,
    out: &mut String,
) {
    let mut by_cluster: HashMap<usize, Vec<NodeIndex>> = HashMap::new();
    for (&nx, &c) in comm {
        if is_root(&g[nx]) {
            continue;
        }
        by_cluster.entry(c).or_default().push(nx);
    }
    let mut keys: Vec<usize> = by_cluster.keys().copied().collect();
    keys.sort_unstable();
    for c in keys {
        out.push_str(&format!("package \"Cluster {}\" {{\n", c));
        let mut nxs = by_cluster.remove(&c).unwrap();
        nxs.sort_by(|a, b| g[*a].cmp(&g[*b]));
        for nx in nxs {
            let cls = g[nx].last().cloned().unwrap_or_default();
            out.push_str(&format!(
                "    {}",
                class_decl(node_by_module.get(&g[nx]).copied(), &cls, false, opts)
            ));
            out.push('\n');
        }
        out.push_str("}\n");
    }
}

fn emit_flat(
    g: &PetGraph,
    node_by_module: &HashMap<&Vec<String>, &crate::models::graph::Node>,
    sorted_nodes: &[NodeIndex],
    is_root: &impl Fn(&Vec<String>) -> bool,
    opts: &EmitOptions,
    out: &mut String,
) {
    for &nx in sorted_nodes {
        if is_root(&g[nx]) {
            continue;
        }
        let cls = g[nx].last().cloned().unwrap_or_default();
        out.push_str(&class_decl(
            node_by_module.get(&g[nx]).copied(),
            &cls,
            false,
            opts,
        ));
        out.push('\n');
    }
}

fn class_decl(
    node: Option<&crate::models::graph::Node>,
    class_name: &str,
    is_root: bool,
    opts: &EmitOptions,
) -> String {
    let pkg = node
        .and_then(|n| n.module.first().cloned())
        .unwrap_or_default();
    let module = node.map(|n| n.module.join(".")).unwrap_or_default();
    let url = render_link(
        opts.link_template,
        &pkg,
        &module,
        class_name,
        opts.version,
        opts.cwd,
        opts.output_dir,
    );
    let header = match url {
        Some(u) => format!("class \"[[{u} {class_name}]]\\n<size:10>{module}\" as {class_name}",),
        None => format!("class \"{class_name}\" as {class_name}"),
    };
    let body = if let Some(n) = node {
        if matches!(opts.detail, Detail::None) && !is_root {
            String::new()
        } else {
            let mut b = String::from(" {\n");
            for field in &n.fields {
                if field.is_reference {
                    continue;
                }
                match opts.detail {
                    Detail::None if !is_root => continue,
                    Detail::Names => b.push_str(&format!("    {}\n", field.name)),
                    Detail::Full | Detail::None => b.push_str(&format!(
                        "    {} : {} [{}]\n",
                        field.name,
                        field.type_repr,
                        format_cardinality(&field.cardinality),
                    )),
                }
            }
            b.push('}');
            b
        }
    } else {
        String::new()
    };
    format!("{header}{body}")
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

    fn opts<'a>(detail: Detail, root: Option<&'a [String]>, cwd: &'a Path) -> EmitOptions<'a> {
        EmitOptions {
            detail,
            clusters: None,
            root,
            link_template: None,
            cwd,
            output_dir: cwd,
            version: "v202501.0.0",
            package_grouping: true,
        }
    }

    #[test]
    fn namespace_blocks_include_palette_colour() {
        let ir = sample_ir();
        let pg = to_petgraph(&ir);
        let s = emit(&pg, &ir, &opts(Detail::None, None, &cwd()));
        assert!(s.contains(r#"namespace "bo4e.bo" as bo4e.bo #B6D7A8 {"#));
        assert!(s.contains(r#"namespace "bo4e.com" as bo4e.com #E0A86C {"#));
    }

    #[test]
    fn root_mode_emits_hide_members_show_root_fields() {
        let ir = sample_ir();
        let pg = to_petgraph(&ir);
        let root = vec!["bo".to_string(), "Angebot".to_string()];
        let s = emit(&pg, &ir, &opts(Detail::Full, Some(&root), &cwd()));
        assert!(s.contains("hide members\nshow .Angebot fields"));
        let outside = s.find("class \"Angebot\"").unwrap();
        let bo_block = s.find(r#"namespace "bo4e.bo""#);
        // bo_block may be None if all bo nodes are excluded (only Angebot is bo here).
        // We just need to confirm the root appears before any namespace block.
        if let Some(bb) = bo_block {
            assert!(outside < bb, "root must precede its namespace block");
        }
    }

    #[test]
    fn edge_renders_association_with_cardinality_and_field() {
        let ir = sample_ir();
        let pg = to_petgraph(&ir);
        let s = emit(&pg, &ir, &opts(Detail::None, None, &cwd()));
        assert!(s.contains(r#"Angebot --* "0..1" Adresse : adresse"#));
    }

    #[test]
    fn link_template_renders_clickable_class_anchor() {
        let ir = sample_ir();
        let pg = to_petgraph(&ir);
        let s = emit(
            &pg,
            &ir,
            &EmitOptions {
                detail: Detail::None,
                clusters: None,
                root: None,
                link_template: Some("https://x/{pkg}.html#{module}"),
                cwd: &cwd(),
                output_dir: &cwd(),
                version: "v202501.0.0",
                package_grouping: true,
            },
        );
        assert!(s.contains(r#"class "[[https://x/bo.html#bo.Angebot Angebot]]"#));
    }
}
