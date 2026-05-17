use crate::cli::base::Executable;
use crate::graph::extract::extract;
use crate::io::graph::{write_graph_graphml, write_graph_json};
use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Generate diagrams and machine-readable graphs from BO4E schemas.
#[derive(Args)]
pub struct Graph {
    #[command(subcommand)]
    pub command: GraphSubcommand,
}

#[derive(Subcommand)]
pub enum GraphSubcommand {
    Extract(ExtractArgs),
    Overview(OverviewArgs),
}

/// Build a directed graph of BO4E class references and write it as JSON or GraphML.
#[derive(Args)]
pub struct ExtractArgs {
    /// BO4E schemas directory (the kind written by `bo4e pull`).
    #[arg(short = 'i', long = "input", required = true)]
    pub input_dir: PathBuf,
    /// Output file.
    #[arg(short = 'o', long = "output", required = true)]
    pub output_file: PathBuf,
    /// Output format.
    #[arg(long = "format", default_value = "json")]
    pub format: GraphFormat,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum GraphFormat {
    Json,
    Graphml,
}

/// Render the big-picture overview diagram for all classes in a graph.json.
#[derive(Args)]
pub struct OverviewArgs {
    #[arg(short = 'i', long = "input", required = true)]
    pub input_graph: PathBuf,
    #[arg(short = 'o', long = "output", required = true)]
    pub output_file: PathBuf,
    #[arg(long = "format", default_value = "dot")]
    pub format: DiagramFormat,
    #[arg(long = "detail", default_value = "none")]
    pub detail: DetailLevel,
    #[arg(long = "clustering", default_value = "louvain")]
    pub clustering: ClusteringMode,
    #[arg(long = "seed")]
    pub seed: Option<u64>,
    #[arg(long = "include")]
    pub include: Vec<String>,
    #[arg(long = "exclude")]
    pub exclude: Vec<String>,
    #[arg(long = "reachable-from")]
    pub reachable_from: Option<String>,
    #[arg(long = "link-base")]
    pub link_base: Option<String>,
}

#[derive(ValueEnum, Clone, Debug, Copy)]
pub enum DiagramFormat {
    Dot,
    Plantuml,
}

#[derive(ValueEnum, Clone, Debug, Copy)]
pub enum DetailLevel {
    None,
    Names,
    Full,
}

#[derive(ValueEnum, Clone, Debug, Copy)]
pub enum ClusteringMode {
    Louvain,
    Components,
    Package,
    None,
}

impl Executable for Graph {
    fn run(&self) -> Result<(), String> {
        match &self.command {
            GraphSubcommand::Extract(a) => run_extract(a),
            GraphSubcommand::Overview(a) => run_overview(a),
        }
    }
}

fn run_extract(a: &ExtractArgs) -> Result<(), String> {
    let out = bo4e_schemas::io::schemas::read_schemas(&a.input_dir)?;
    for w in &out.warnings {
        crate::cwarn!("{w}");
    }
    let schemas = out.schemas;
    let g = extract(&schemas)?;
    match a.format {
        GraphFormat::Json => write_graph_json(&g, &a.output_file)?,
        GraphFormat::Graphml => write_graph_graphml(&g, &a.output_file)?,
    }
    crate::cprint_normal!("Wrote graph to {}", a.output_file.display());
    Ok(())
}

fn run_overview(a: &OverviewArgs) -> Result<(), String> {
    use crate::graph::cluster::louvain;
    use crate::graph::emit_dot::{self, Detail as DotDetail};
    use crate::graph::emit_plantuml;
    use crate::graph::extract::{from_petgraph_with_fields, to_petgraph};
    use crate::graph::filter::{FilterOptions, apply};
    use crate::io::graph::read_graph;
    use std::collections::HashMap;

    let ir = read_graph(&a.input_graph)?;
    let pg = to_petgraph(&ir);

    let mut opts = FilterOptions::new();
    for glob in &a.include {
        opts = opts.include_glob(glob)?;
    }
    for glob in &a.exclude {
        opts = opts.exclude_glob(glob)?;
    }
    if let Some(rf) = &a.reachable_from {
        opts.reachable_from = Some(rf.split('.').map(|s| s.to_string()).collect());
    }
    let pg = apply(pg, &opts);
    let ir_filtered = from_petgraph_with_fields(&pg, &ir);

    let clusters: Option<HashMap<petgraph::graph::NodeIndex, usize>> = match a.clustering {
        ClusteringMode::Louvain => {
            let seed = a.seed.unwrap_or_else(rand::random);
            let comms = louvain(&pg, seed);
            Some(comms.of)
        }
        ClusteringMode::Components => {
            use petgraph::unionfind::UnionFind;
            let n = pg.node_count();
            let mut uf: UnionFind<usize> = UnionFind::new(n);
            for e in pg.edge_indices() {
                let (a, b) = pg.edge_endpoints(e).unwrap();
                uf.union(a.index(), b.index());
            }
            // Compact root labels to dense 0..k ids.
            let mut root_to_id: std::collections::HashMap<usize, usize> =
                std::collections::HashMap::new();
            let mut next_id: usize = 0;
            let mut m: HashMap<petgraph::graph::NodeIndex, usize> = HashMap::new();
            for nx in pg.node_indices() {
                let root = uf.find(nx.index());
                let id = *root_to_id.entry(root).or_insert_with(|| {
                    let v = next_id;
                    next_id += 1;
                    v
                });
                m.insert(nx, id);
            }
            Some(m)
        }
        ClusteringMode::Package => {
            let mut pkg_to_id: HashMap<String, usize> = HashMap::new();
            let mut next: usize = 0;
            let mut m = HashMap::new();
            let mut ixs: Vec<_> = pg.node_indices().collect();
            ixs.sort_by(|x, y| pg[*x].cmp(&pg[*y]));
            for nx in ixs {
                let pkg = pg[nx].first().cloned().unwrap_or_default();
                let id = *pkg_to_id.entry(pkg).or_insert_with(|| {
                    let v = next;
                    next += 1;
                    v
                });
                m.insert(nx, id);
            }
            Some(m)
        }
        ClusteringMode::None => None,
    };

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let output_dir = a
        .output_file
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    let detail = match a.detail {
        DetailLevel::None => DotDetail::None,
        DetailLevel::Names => DotDetail::Names,
        DetailLevel::Full => DotDetail::Full,
    };
    let version_str = ir.version.to_string();

    let text = match a.format {
        DiagramFormat::Dot => emit_dot::emit(
            &pg,
            &ir_filtered,
            &emit_dot::EmitOptions {
                detail,
                root_detail: None,
                clusters: clusters.as_ref(),
                root: None,
                link_template: a.link_base.as_deref(),
                cwd: &cwd,
                output_dir: &output_dir,
                version: &version_str,
            },
        ),
        DiagramFormat::Plantuml => emit_plantuml::emit(
            &pg,
            &ir_filtered,
            &emit_plantuml::EmitOptions {
                detail,
                clusters: clusters.as_ref(),
                root: None,
                link_template: a.link_base.as_deref(),
                cwd: &cwd,
                output_dir: &output_dir,
                version: &version_str,
                package_grouping: matches!(a.clustering, ClusteringMode::Package),
            },
        ),
    };

    if let Some(parent) = a.output_file.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
    }
    std::fs::write(&a.output_file, text)
        .map_err(|e| format!("Failed to write {}: {}", a.output_file.display(), e))?;
    crate::cprint_normal!("Wrote overview to {}", a.output_file.display());
    Ok(())
}
