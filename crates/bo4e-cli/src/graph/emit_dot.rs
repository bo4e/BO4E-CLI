use crate::graph::emit_common::{
    dotted, format_cardinality, html_escape, pkg_color, pkg_color_darker, pkg_color_lighter,
    render_link,
};
use crate::graph::extract::PetGraph;
use crate::models::graph::GraphIR;
use petgraph::graph::NodeIndex;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const CLASS_NAME_POINT_SIZE: &str = "18";
const FIELD_DETAIL_POINT_SIZE: &str = "10";
const FIELD_DETAIL_COLOUR: &str = "#555555";
/// Width (in points) of the table border drawn around every node.
const NODE_BORDER_WIDTH: &str = "2";

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
    /// Graphviz layout engine name (`dot`, `neato`, `fdp`, `sfdp`, `circo`,
    /// `twopi`). `dot` keeps the legacy hierarchical layout with `rankdir=LR`;
    /// any other value emits `layout=<engine>` plus the chosen `overlap`
    /// strategy and a `splines=true` hint.
    pub layout: &'a str,
    /// Graphviz `overlap` attribute value. Only used when `layout != "dot"`
    /// (dot doesn't use overlap). Typical values: `scale` (portable),
    /// `prism` (best, needs GTS), `true` (allow overlaps).
    pub overlap: &'a str,
    /// Extra margin in points around every node, added on top of Graphviz's
    /// default `sep` of `+4`. `0` disables the override (no `sep` attribute
    /// emitted). Only applied when `layout != "dot"` (dot uses the
    /// hierarchical-layout knobs `nodesep`/`ranksep` instead).
    pub node_margin: u32,
    /// When `true`, emit `[label="<field> [<cardinality>]"]` on every edge.
    /// When `false`, edges are drawn unlabelled.
    pub edge_labels: bool,
}

pub fn emit(g: &PetGraph, ir: &GraphIR, opts: &EmitOptions) -> String {
    let mut out = String::new();
    out.push_str("digraph BO4E {\n");
    if opts.layout == "dot" {
        out.push_str("    rankdir=LR;\n");
    } else {
        out.push_str(&format!("    layout={};\n", opts.layout));
        out.push_str(&format!("    overlap={};\n", opts.overlap));
        out.push_str("    splines=true;\n");
        if opts.node_margin > 0 {
            out.push_str(&format!("    sep=\"+{}\";\n", opts.node_margin));
        }
    }
    // Each node carries an HTML-like label (`<<TABLE>…</TABLE>>`) so that the
    // class name can be sized + bolded and field details rendered in a lighter
    // colour. `shape=plaintext` removes graphviz's outer frame — the table's
    // own BORDER provides the visible outline.
    //
    // Centering quirk: with the default Times font and the default width
    // estimator, ALIGN="CENTER" class-name text drifts ~13pt left of the TD
    // centre. Two graph-level knobs help meaningfully:
    //   * `fontnames="ps"` switches Graphviz from its built-in font metrics
    //     to the PostScript Adobe metric tables, which are noticeably more
    //     accurate for bold text.
    //   * pinning every fontname to Helvetica avoids Times' bold metrics
    //     specifically (the worst-measured of the base fonts).
    // The residual misalignment is a Graphviz HTML-label issue that no
    // user-side option fully resolves.
    out.push_str("    fontnames=\"ps\";\n");
    out.push_str("    graph [fontname=\"Helvetica\"];\n");
    out.push_str("    node [shape=plaintext, fontname=\"Helvetica\"];\n");
    out.push_str("    edge [fontname=\"Helvetica\"];\n\n");

    let node_by_module: HashMap<&Vec<String>, &crate::models::graph::Node> =
        ir.nodes.iter().map(|n| (&n.module, n)).collect();

    let render_node = |nx: NodeIndex, prefix: &str, out: &mut String| {
        let module = &g[nx];
        let module_dotted = dotted(module);
        let class_name = module.last().cloned().unwrap_or_default();
        let pkg = module.first().cloned().unwrap_or_default();
        let detail_for_this = match (opts.root, opts.root_detail) {
            (Some(r), Some(d)) if r == nx => d,
            _ => opts.detail,
        };
        let palette = NodePalette {
            header: pkg_color(&pkg),
            detail: pkg_color_lighter(&pkg),
            border: pkg_color_darker(&pkg),
        };
        let label = node_html_label(
            node_by_module.get(module).copied(),
            &class_name,
            &palette,
            detail_for_this,
            g,
            nx,
        );
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
            "{}\"{}\" [label={}{}];",
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
                out.push_str("        style=invis;\n");
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
    // When labels are off, multiple edges between the same (from, to) pair
    // collapse to a single unlabelled arrow — otherwise they overdraw and
    // visually clutter the dense overview.
    let mut seen_pairs: HashSet<(NodeIndex, NodeIndex)> = HashSet::new();
    for ex in edge_ixs {
        let (a, b) = g.edge_endpoints(ex).unwrap();
        if opts.edge_labels {
            let data = &g[ex];
            let card = format_cardinality(&data.cardinality);
            out.push_str(&format!(
                r#"    "{}" -> "{}" [label="{} [{}]"];"#,
                dotted(&g[a]),
                dotted(&g[b]),
                data.through_field,
                card,
            ));
        } else {
            if !seen_pairs.insert((a, b)) {
                continue;
            }
            out.push_str(&format!(
                r#"    "{}" -> "{}";"#,
                dotted(&g[a]),
                dotted(&g[b]),
            ));
        }
        out.push('\n');
    }
    out.push_str("}\n");
    out
}

struct NodePalette<'a> {
    header: &'a str,
    detail: &'a str,
    border: &'a str,
}

/// Build a Graphviz HTML-like label for a node:
///   <<TABLE BORDER="2" COLOR="<border>" BGCOLOR="<header>" …>
///     <TR><TD ALIGN="CENTER"><FONT POINT-SIZE="18"><B>ClassName</B></FONT></TD></TR>
///     (one row per field, BGCOLOR="<detail>", when detail-level != None)
///     (one row per enum value, same lighter row, when detail-level == Full
///      and the node carries enum_values)
///   </TABLE>>
///
/// Why explicit `ALIGN="CENTER"` on the class-name TD: Graphviz centres TD
/// content by default, but when sibling rows use `ALIGN="LEFT"` certain
/// versions render the header slightly off-axis. Stating the alignment
/// explicitly is a no-cost belt-and-braces fix.
fn node_html_label(
    node: Option<&crate::models::graph::Node>,
    class_name: &str,
    palette: &NodePalette,
    detail: Detail,
    g: &PetGraph,
    nx: NodeIndex,
) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        r#"<<TABLE BORDER="{}" CELLBORDER="0" CELLSPACING="0" CELLPADDING="4" COLOR="{}" BGCOLOR="{}">"#,
        NODE_BORDER_WIDTH, palette.border, palette.header,
    ));
    s.push_str(&format!(
        r#"<TR><TD ALIGN="CENTER"><FONT POINT-SIZE="{}"><B>{}</B></FONT></TD></TR>"#,
        CLASS_NAME_POINT_SIZE,
        html_escape(class_name)
    ));

    if !matches!(detail, Detail::None)
        && let Some(n) = node
    {
        let outgoing_fields: HashSet<&str> = g
            .edges(nx)
            .map(|e| e.weight().through_field.as_str())
            .collect();
        for field in &n.fields {
            if outgoing_fields.contains(field.name.as_str()) {
                continue;
            }
            let body = match detail {
                Detail::Names => html_escape(&field.name),
                Detail::Full => format!(
                    "{} : {} [{}]",
                    html_escape(&field.name),
                    html_escape(&field.type_repr),
                    html_escape(&format_cardinality(&field.cardinality)),
                ),
                Detail::None => unreachable!(),
            };
            s.push_str(&detail_row(&body, palette.detail));
        }
        // Enum variants only render at Detail::Full; they're typically empty
        // for class nodes (only StrEnum root schemas carry them).
        if matches!(detail, Detail::Full) {
            for v in &n.enum_values {
                s.push_str(&detail_row(&html_escape(v), palette.detail));
            }
        }
    }

    s.push_str("</TABLE>>");
    s
}

fn detail_row(body: &str, bg_color: &str) -> String {
    format!(
        r#"<TR><TD ALIGN="LEFT" BGCOLOR="{}"><FONT POINT-SIZE="{}" COLOR="{}">{}</FONT></TD></TR>"#,
        bg_color, FIELD_DETAIL_POINT_SIZE, FIELD_DETAIL_COLOUR, body
    )
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
                    enum_values: vec![],
                },
                Node {
                    module: vec!["com".into(), "Adresse".into()],
                    fields: vec![],
                    enum_values: vec![],
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
                layout: "dot",
                overlap: "scale",
                node_margin: 0,
                edge_labels: true,
            },
        );
        // HTML labels: class name lives in `<B>...</B>` with the package BGCOLOR.
        assert!(s.contains("<B>Angebot</B>"));
        assert!(s.contains("<B>Adresse</B>"));
        // bo gets the bo palette colour, com gets the com palette colour.
        assert!(s.contains(r##"BGCOLOR="#B6D7A8""##));
        assert!(s.contains(r##"BGCOLOR="#E0A86C""##));
        // Detail::None must not surface field rows.
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
                layout: "dot",
                overlap: "scale",
                node_margin: 0,
                edge_labels: true,
            },
        );
        // betrag must appear inline (non-ref).
        assert!(s.contains("betrag : Decimal [1]"));
        // adresse must NOT appear inline (it's an edge).
        let angebot_idx = s.find("\"bo.Angebot\"").unwrap();
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
                layout: "dot",
                overlap: "scale",
                node_margin: 0,
                edge_labels: true,
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
                layout: "dot",
                overlap: "scale",
                node_margin: 0,
                edge_labels: true,
            },
        );
        assert!(s.contains("subgraph cluster_0"));
        assert!(s.contains("subgraph cluster_1"));
        assert!(s.contains("style=invis;"));
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
                layout: "dot",
                overlap: "scale",
                node_margin: 0,
                edge_labels: true,
            },
        );
        assert!(s.contains(r#"URL="https://x/bo/Angebot""#));
    }

    #[test]
    fn non_dot_layout_emits_layout_attr_and_drops_rankdir() {
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
                layout: "neato",
                overlap: "scale",
                node_margin: 0,
                edge_labels: true,
            },
        );
        assert!(s.contains("layout=neato;"));
        assert!(s.contains("overlap=scale;"));
        assert!(!s.contains("rankdir=LR;"));
        assert!(!s.contains("sep="));
    }

    #[test]
    fn overlap_value_is_threaded_through() {
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
                layout: "sfdp",
                overlap: "prism",
                node_margin: 0,
                edge_labels: true,
            },
        );
        assert!(s.contains("layout=sfdp;"));
        assert!(s.contains("overlap=prism;"));
    }

    #[test]
    fn node_margin_emits_sep_attribute_on_non_dot_layouts() {
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
                layout: "neato",
                overlap: "prism",
                node_margin: 20,
                edge_labels: true,
            },
        );
        assert!(s.contains("sep=\"+20\";"));
    }

    #[test]
    fn nodes_render_with_colored_border_and_lighter_detail_rows() {
        let ir = GraphIR {
            version: "v202501.0.0".parse().unwrap(),
            nodes: vec![
                Node {
                    module: vec!["bo".into(), "Angebot".into()],
                    fields: vec![Field {
                        name: "betrag".into(),
                        type_repr: "Decimal".into(),
                        cardinality: Cardinality {
                            min: "1".into(),
                            max: "1".into(),
                        },
                        is_reference: false,
                    }],
                    enum_values: vec![],
                },
                // Top-level (no-package) class — exercises the grey fallback.
                Node {
                    module: vec!["ZusatzAttribut".into()],
                    fields: vec![],
                    enum_values: vec![],
                },
            ],
            edges: vec![],
        };
        let pg = to_petgraph(&ir);
        let s = emit(
            &pg,
            &ir,
            &EmitOptions {
                detail: Detail::Names,
                root_detail: None,
                clusters: None,
                root: None,
                link_template: None,
                cwd: &cwd(),
                output_dir: &cwd(),
                version: "v1",
                layout: "neato",
                overlap: "prism",
                node_margin: 50,
                edge_labels: false,
            },
        );
        // bo node: header is the main palette colour, border is the darker shade,
        // field row has the lighter shade as its TD BGCOLOR.
        assert!(s.contains(r##"BGCOLOR="#B6D7A8""##));
        assert!(s.contains(r##"COLOR="#6D8164""##));
        assert!(s.contains(r##"BGCOLOR="#DBEBD4""##));
        // Top-level ZusatzAttribut hits the muted-grey fallback for header + border.
        assert!(s.contains(r##"BGCOLOR="#D9D9D9""##));
        assert!(s.contains(r##"COLOR="#8C8C8C""##));
        // Class names are emitted at 18 pt and explicitly centered.
        assert!(s.contains(r#"ALIGN="CENTER""#));
        assert!(s.contains(r#"POINT-SIZE="18""#));
        // 2-pixel border baked into every TABLE.
        assert!(s.contains(r#"BORDER="2""#));
    }

    #[test]
    fn detail_full_renders_enum_values_for_str_enum_nodes() {
        let ir = GraphIR {
            version: "v202501.0.0".parse().unwrap(),
            nodes: vec![Node {
                module: vec!["enum".into(), "Preisstatus".into()],
                fields: vec![],
                enum_values: vec!["ENDGUELTIG".into(), "VORLAEUFIG".into()],
            }],
            edges: vec![],
        };
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
                layout: "neato",
                overlap: "prism",
                node_margin: 50,
                edge_labels: false,
            },
        );
        assert!(s.contains("<B>Preisstatus</B>"));
        assert!(s.contains("ENDGUELTIG"));
        assert!(s.contains("VORLAEUFIG"));
        // Variants use the lighter detail font.
        assert!(s.contains(r##"COLOR="#555555""##));
        // Enum package uses its palette colour.
        assert!(s.contains(r##"BGCOLOR="#d1c358""##));
    }

    #[test]
    fn detail_names_omits_enum_values() {
        let ir = GraphIR {
            version: "v202501.0.0".parse().unwrap(),
            nodes: vec![Node {
                module: vec!["enum".into(), "Preisstatus".into()],
                fields: vec![],
                enum_values: vec!["ENDGUELTIG".into()],
            }],
            edges: vec![],
        };
        let pg = to_petgraph(&ir);
        let s = emit(
            &pg,
            &ir,
            &EmitOptions {
                detail: Detail::Names,
                root_detail: None,
                clusters: None,
                root: None,
                link_template: None,
                cwd: &cwd(),
                output_dir: &cwd(),
                version: "v1",
                layout: "neato",
                overlap: "prism",
                node_margin: 0,
                edge_labels: false,
            },
        );
        assert!(s.contains("<B>Preisstatus</B>"));
        assert!(!s.contains("ENDGUELTIG"));
    }

    #[test]
    fn parallel_edges_are_deduped_when_labels_off() {
        let ir = GraphIR {
            version: "v202501.0.0".parse().unwrap(),
            nodes: vec![
                Node {
                    module: vec!["bo".into(), "A".into()],
                    fields: vec![],
                    enum_values: vec![],
                },
                Node {
                    module: vec!["bo".into(), "B".into()],
                    fields: vec![],
                    enum_values: vec![],
                },
            ],
            edges: vec![
                Edge {
                    from: vec!["bo".into(), "A".into()],
                    to: vec!["bo".into(), "B".into()],
                    through_field: "x".into(),
                    cardinality: Cardinality {
                        min: "1".into(),
                        max: "1".into(),
                    },
                },
                Edge {
                    from: vec!["bo".into(), "A".into()],
                    to: vec!["bo".into(), "B".into()],
                    through_field: "y".into(),
                    cardinality: Cardinality {
                        min: "1".into(),
                        max: "1".into(),
                    },
                },
            ],
        };
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
                layout: "neato",
                overlap: "prism",
                node_margin: 0,
                edge_labels: false,
            },
        );
        let count = s.matches(r#""bo.A" -> "bo.B""#).count();
        assert_eq!(count, 1, "expected exactly one A->B arrow; got: {s}");
    }

    #[test]
    fn parallel_edges_are_kept_when_labels_on() {
        let ir = GraphIR {
            version: "v202501.0.0".parse().unwrap(),
            nodes: vec![
                Node {
                    module: vec!["bo".into(), "A".into()],
                    fields: vec![],
                    enum_values: vec![],
                },
                Node {
                    module: vec!["bo".into(), "B".into()],
                    fields: vec![],
                    enum_values: vec![],
                },
            ],
            edges: vec![
                Edge {
                    from: vec!["bo".into(), "A".into()],
                    to: vec!["bo".into(), "B".into()],
                    through_field: "x".into(),
                    cardinality: Cardinality {
                        min: "1".into(),
                        max: "1".into(),
                    },
                },
                Edge {
                    from: vec!["bo".into(), "A".into()],
                    to: vec!["bo".into(), "B".into()],
                    through_field: "y".into(),
                    cardinality: Cardinality {
                        min: "1".into(),
                        max: "1".into(),
                    },
                },
            ],
        };
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
                layout: "neato",
                overlap: "prism",
                node_margin: 0,
                edge_labels: true,
            },
        );
        let count = s.matches(r#""bo.A" -> "bo.B""#).count();
        assert_eq!(count, 2, "labelled edges must not dedupe");
    }

    #[test]
    fn edge_labels_false_omits_label_attribute() {
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
                layout: "neato",
                overlap: "prism",
                node_margin: 0,
                edge_labels: false,
            },
        );
        assert!(s.contains(r#""bo.Angebot" -> "com.Adresse";"#));
        assert!(!s.contains("label=\"adresse"));
    }

    #[test]
    fn node_margin_is_ignored_for_dot_layout() {
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
                layout: "dot",
                overlap: "scale",
                node_margin: 20,
                edge_labels: true,
            },
        );
        assert!(!s.contains("sep="));
    }

    #[test]
    fn dot_layout_ignores_overlap_attribute() {
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
                layout: "dot",
                overlap: "prism",
                node_margin: 0,
                edge_labels: true,
            },
        );
        assert!(!s.contains("overlap="));
        assert!(s.contains("rankdir=LR;"));
    }

    #[test]
    fn dot_layout_keeps_rankdir_lr() {
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
                layout: "dot",
                overlap: "scale",
                node_margin: 0,
                edge_labels: true,
            },
        );
        assert!(s.contains("rankdir=LR;"));
        assert!(!s.contains("layout="));
    }
}
