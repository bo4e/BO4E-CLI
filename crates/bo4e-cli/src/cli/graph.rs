use crate::cli::base::Executable;
use crate::graph::extract::extract;
use crate::io::cleanse::clear_dir_if_needed;
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
    Single(SingleArgs),
}

/// Build a directed graph of BO4E class references and write it as JSON or GraphML.
#[derive(Args)]
pub struct ExtractArgs {
    /// Directory of BO4E JSON schemas (typically the output of `bo4e pull`).
    #[arg(short = 'i', long = "input", required = true)]
    pub input_dir: PathBuf,
    /// Output file path. The suffix is not enforced — use `.json` for the
    /// internal GraphIR (consumed by `overview` / `single`) or `.graphml`
    /// for external tools such as Gephi or yEd.
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
    /// GraphIR JSON file produced by `bo4e graph extract`.
    #[arg(short = 'i', long = "input", required = true)]
    pub input_graph: PathBuf,
    /// Output file for the rendered diagram (DOT or PlantUML source).
    #[arg(short = 'o', long = "output", required = true)]
    pub output_file: PathBuf,
    /// Rendering language: `dot` (Graphviz) or `plantuml`.
    #[arg(long = "format", default_value = "dot")]
    pub format: DiagramFormat,
    /// Per-class detail: `none` (just the name), `names` (field names),
    /// or `full` (field names + types).
    #[arg(long = "detail", default_value = "none")]
    pub detail: DetailLevel,
    /// Visual grouping. `louvain` detects communities by modularity;
    /// `components` colours weakly-connected components; `package` groups
    /// nodes by BO4E package (bo, com, enum, …); `none` disables grouping.
    #[arg(long = "clustering", default_value = "louvain")]
    pub clustering: ClusteringMode,
    /// RNG seed for `--clustering louvain`. Default: randomised each run.
    /// Set this for reproducible layouts.
    #[arg(long = "seed")]
    pub seed: Option<u64>,
    /// Include-only glob over dotted module paths (e.g. `bo.*`,
    /// `*.Angebot`). Repeatable; a node is kept if it matches any pattern.
    #[arg(long = "include")]
    pub include: Vec<String>,
    /// Exclude glob over dotted module paths. Applied after `--include`.
    /// Repeatable.
    #[arg(long = "exclude")]
    pub exclude: Vec<String>,
    /// Restrict the graph to nodes reachable from this class (forward BFS).
    /// Accepts a bare name (`Angebot`) or dotted path (`bo.Angebot`).
    #[arg(long = "reachable-from")]
    pub reachable_from: Option<String>,
    /// URL template for clickable class nodes. See `--help` for placeholders
    /// and worked examples.
    #[arg(long = "link-base", long_help = LINK_BASE_LONG_HELP_OVERVIEW)]
    pub link_base: Option<String>,
}

const LINK_BASE_LONG_HELP_OVERVIEW: &str =
    "URL template for clickable class nodes. When set, each class node \
becomes a hyperlink in the rendered diagram.

Placeholders (expanded per node):
  {pkg}         BO4E package, e.g. `bo`
  {module}      dotted module path, e.g. `bo.Angebot`
  {class}       class name, e.g. `Angebot`
  {version}     BO4E version of the source schemas
  {cwd[.abs|.rel|.uri|.posix|.name]}         current working directory
  {output_dir[.abs|.rel|.uri|.posix|.name]}  parent directory of `-o`

Examples:
  # Link to the official BO4E-Python API docs:
  --link-base \"https://bo4e.github.io/BO4E-python/{version}/api/{module}.html\"

  # Link to a sibling SVG written next to the diagram:
  --link-base \"{pkg}/{class}.svg\"

  # Anchor into a single docs page:
  --link-base \"https://docs.example.com/bo4e#{class}\"

  # Open locally rendered HTML by file URI:
  --link-base \"{output_dir.uri}/{pkg}/{class}.html\"

Pass `none` or an empty string to disable links explicitly.";

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

/// Render per-class diagrams. Outputs a file when --class names a single class,
/// or a directory of files when --class all.
#[derive(Args)]
pub struct SingleArgs {
    /// GraphIR JSON file produced by `bo4e graph extract`.
    #[arg(short = 'i', long = "input", required = true)]
    pub input_graph: PathBuf,
    /// Class to render. Use a bare name (`Angebot`) or a dotted module path
    /// (`bo.Angebot`). Pass `all` to render every class in the graph.
    #[arg(long = "class", default_value = "all")]
    pub class: String,
    /// Output target. With `--class <NAME>`: path to the single output file.
    /// With `--class all`: directory to populate with one file per class
    /// (mirroring the BO4E package layout: `bo/`, `com/`, `enum/`, …).
    #[arg(short = 'o', long = "output", required = true)]
    pub output_target: PathBuf,
    /// Rendering language: `dot` (Graphviz) or `plantuml`.
    #[arg(long = "format", default_value = "dot")]
    pub format: DiagramFormat,
    /// Detail level for the focused class: `none`, `names`, or `full`
    /// (field names + types).
    #[arg(long = "detail-root", default_value = "full")]
    pub detail_root: DetailLevel,
    /// Detail level for neighbour classes: `none`, `names`, or `full`.
    #[arg(long = "detail-neighbours", default_value = "none")]
    pub detail_neighbours: DetailLevel,
    /// Visual grouping inside the per-class diagram. `package` groups
    /// neighbours by BO4E package; `none` disables grouping.
    /// (`louvain` / `components` are rejected — the per-class ego graph is
    /// too small for them to be meaningful.)
    #[arg(long = "clustering", default_value = "package", value_parser = single_clustering_parser)]
    pub clustering: ClusteringMode,
    /// Include-only glob over dotted module paths. Overrides the default
    /// per-package scope (which keeps siblings in the same package). Repeatable.
    #[arg(long = "include")]
    pub include: Vec<String>,
    /// Exclude glob over dotted module paths. Applied after `--include`.
    /// Repeatable.
    #[arg(long = "exclude")]
    pub exclude: Vec<String>,
    /// BFS radius around the focused class: 1 = direct neighbours,
    /// 2 = neighbours-of-neighbours, and so on.
    #[arg(long = "radius", default_value = "1")]
    pub radius: usize,
    /// URL template for clickable class nodes. See `--help` for placeholders
    /// and worked examples.
    #[arg(long = "link-base", long_help = LINK_BASE_LONG_HELP_SINGLE)]
    pub link_base: Option<String>,
    /// Don't clear the output directory before writing diagrams. Only
    /// relevant with `--class all`; for single-class targets the output file
    /// is overwritten in place regardless.
    #[arg(short = 'c', long = "no-clear-output", default_value_t = false)]
    pub no_clear_output: bool,
}

const LINK_BASE_LONG_HELP_SINGLE: &str =
    "URL template for clickable class nodes. When set, each class node \
becomes a hyperlink in the rendered diagram.

Placeholders (expanded per node):
  {pkg}         BO4E package, e.g. `bo`
  {module}      dotted module path, e.g. `bo.Angebot`
  {class}       class name, e.g. `Angebot`
  {version}     BO4E version of the source schemas
  {cwd[.abs|.rel|.uri|.posix|.name]}         current working directory
  {output_dir[.abs|.rel|.uri|.posix|.name]}  `-o` (in `--class all` mode) \
or its parent

Examples:
  # Cross-link every per-class SVG to its siblings:
  --link-base \"{class}.svg\"

  # Link to the official BO4E-Python API docs:
  --link-base \"https://bo4e.github.io/BO4E-python/{version}/api/{module}.html\"

  # Anchor into a single docs page:
  --link-base \"https://docs.example.com/bo4e#{class}\"

  # Open locally rendered HTML by file URI:
  --link-base \"{output_dir.uri}/{pkg}/{class}.html\"

Pass `none` or an empty string to disable links explicitly.";

fn single_clustering_parser(s: &str) -> Result<ClusteringMode, String> {
    match s {
        "package" => Ok(ClusteringMode::Package),
        "none" => Ok(ClusteringMode::None),
        "louvain" | "components" => Err(format!(
            "--clustering {s} is not available on 'bo4e graph single' \
             (the per-class ego graph is too small). Allowed: package | none."
        )),
        other => Err(format!("invalid clustering mode: {other}")),
    }
}

impl Executable for Graph {
    fn run(&self) -> Result<(), String> {
        match &self.command {
            GraphSubcommand::Extract(a) => run_extract(a),
            GraphSubcommand::Overview(a) => run_overview(a),
            GraphSubcommand::Single(a) => run_single(a),
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

fn run_single(a: &SingleArgs) -> Result<(), String> {
    use crate::graph::emit_dot::{self, Detail as DotDetail};
    use crate::graph::emit_plantuml;
    use crate::graph::extract::{from_petgraph_with_fields, to_petgraph};
    use crate::graph::filter::{
        FilterOptions, apply, default_scope_for, ego_graph, retain_edges_incident_on,
    };
    use crate::io::graph::read_graph;

    let ir = read_graph(&a.input_graph)?;
    let pg = to_petgraph(&ir);

    // Resolve --class to a list of (module, NodeIndex) pairs.
    let targets: Vec<(Vec<String>, petgraph::graph::NodeIndex)> = if a.class == "all" {
        pg.node_indices().map(|nx| (pg[nx].clone(), nx)).collect()
    } else {
        let needle = a.class.clone();
        let found: Vec<_> = pg
            .node_indices()
            .filter(|nx| {
                pg[*nx].last().map(|s| s.as_str()) == Some(&needle) || pg[*nx].join(".") == needle
            })
            .map(|nx| (pg[nx].clone(), nx))
            .collect();
        if found.is_empty() {
            let known: Vec<String> = pg.node_indices().map(|nx| pg[nx].join(".")).collect();
            return Err(format!(
                "Unknown class {:?}. Known: {}",
                needle,
                known.join(", ")
            ));
        }
        found
    };

    // Validate output_target shape relative to --class.
    let single_target = a.class != "all";
    if single_target && a.output_target.is_dir() {
        return Err(format!(
            "--class {} expects -o to be a file path, but {} is a directory",
            a.class,
            a.output_target.display()
        ));
    }
    if !single_target && a.output_target.is_file() {
        return Err(format!(
            "--class all expects -o to be a directory, but {} is a file",
            a.output_target.display()
        ));
    }

    if !single_target {
        clear_dir_if_needed(&a.output_target, !a.no_clear_output).map_err(|e| e.to_string())?;
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    for (module, nx) in targets {
        let pkg = module.first().cloned().unwrap_or_default();
        let mut opts = FilterOptions::new();
        if a.include.is_empty() {
            for pat in default_scope_for(&pkg) {
                opts = opts.include_glob(pat)?;
            }
        } else {
            for pat in &a.include {
                opts = opts.include_glob(pat)?;
            }
        }
        for pat in &a.exclude {
            opts = opts.exclude_glob(pat)?;
        }

        let mut sub = ego_graph(&pg, nx, a.radius);
        sub = apply(sub, &opts);
        let root_in_sub = match sub.node_indices().find(|n| sub[*n] == module) {
            Some(n) => n,
            None => continue,
        };
        sub = retain_edges_incident_on(sub, root_in_sub);

        let ir_sub = from_petgraph_with_fields(&sub, &ir);
        let detail = |level: DetailLevel| match level {
            DetailLevel::None => DotDetail::None,
            DetailLevel::Names => DotDetail::Names,
            DetailLevel::Full => DotDetail::Full,
        };

        let (text, ext) = match a.format {
            DiagramFormat::Dot => (
                emit_dot::emit(
                    &sub,
                    &ir_sub,
                    &emit_dot::EmitOptions {
                        detail: detail(a.detail_neighbours),
                        root_detail: Some(detail(a.detail_root)),
                        clusters: None,
                        root: Some(root_in_sub),
                        link_template: a.link_base.as_deref(),
                        cwd: &cwd,
                        output_dir: &a.output_target,
                        version: &ir.version.to_string(),
                    },
                ),
                "dot",
            ),
            DiagramFormat::Plantuml => (
                emit_plantuml::emit(
                    &sub,
                    &ir_sub,
                    &emit_plantuml::EmitOptions {
                        detail: detail(a.detail_root),
                        clusters: None,
                        root: Some(module.as_slice()),
                        link_template: a.link_base.as_deref(),
                        cwd: &cwd,
                        output_dir: &a.output_target,
                        version: &ir.version.to_string(),
                        package_grouping: matches!(a.clustering, ClusteringMode::Package),
                    },
                ),
                "puml",
            ),
        };

        let out_path = if single_target {
            a.output_target.clone()
        } else {
            let cls = module.last().cloned().unwrap_or_default();
            let mut p = a.output_target.clone();
            if module.len() > 1 {
                let pkg = module.first().cloned().unwrap_or_default();
                p = p.join(&pkg);
            }
            p.join(format!("{cls}.{ext}"))
        };
        if let Some(parent) = out_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
        }
        std::fs::write(&out_path, text)
            .map_err(|e| format!("Failed to write {}: {}", out_path.display(), e))?;
        crate::cprint_verbose!("Wrote {}", out_path.display());
    }

    crate::cprint_normal!("Wrote diagrams to {}", a.output_target.display());
    Ok(())
}
