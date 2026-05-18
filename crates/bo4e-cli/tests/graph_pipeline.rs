use std::path::PathBuf;
use std::process::Command;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/bo4e-codegen/tests/fixtures/bo4e_min")
}

fn exe() -> &'static str {
    env!("CARGO_BIN_EXE_bo4e")
}

#[test]
fn extract_then_overview_then_single_produces_files() {
    let tmp = tempfile::tempdir().unwrap();
    let graph_json = tmp.path().join("graph.json");

    let out = Command::new(exe())
        .args(["graph", "extract", "-i"])
        .arg(fixture())
        .args(["-o"])
        .arg(&graph_json)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "extract failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(graph_json.exists());

    let overview = tmp.path().join("overview.dot");
    let out = Command::new(exe())
        .args(["graph", "overview", "-i"])
        .arg(&graph_json)
        .args(["-o"])
        .arg(&overview)
        .args(["--clustering", "package"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "overview failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let overview_text = std::fs::read_to_string(&overview).unwrap();
    assert!(overview_text.starts_with("digraph BO4E"));
    assert!(overview_text.contains("subgraph cluster_"));

    let singles = tmp.path().join("singles");
    let out = Command::new(exe())
        .args(["graph", "single", "-i"])
        .arg(&graph_json)
        .args(["-o"])
        .arg(&singles)
        .args(["--class", "all", "--format", "plantuml"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "single failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(singles.is_dir());
    let any_puml = walkdir::WalkDir::new(&singles)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().map(|x| x == "puml").unwrap_or(false));
    assert!(
        any_puml,
        "no .puml files written under {}",
        singles.display()
    );
}

#[test]
fn single_with_concrete_class_writes_one_file() {
    let tmp = tempfile::tempdir().unwrap();
    let graph_json = tmp.path().join("graph.json");
    Command::new(exe())
        .args(["graph", "extract", "-i"])
        .arg(fixture())
        .args(["-o"])
        .arg(&graph_json)
        .output()
        .unwrap();

    let ir: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&graph_json).unwrap()).unwrap();
    let first_class = ir["nodes"][0]["module"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    let out_file = tmp.path().join("single.dot");
    let out = Command::new(exe())
        .args(["graph", "single", "-i"])
        .arg(&graph_json)
        .args(["--class", &first_class, "-o"])
        .arg(&out_file)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "single with class failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(out_file.is_file());
}

#[test]
fn single_writes_root_level_class_at_output_dir_root() {
    let invariants = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/bo4e-codegen/tests/fixtures/bo4e_invariants");

    let tmp = tempfile::tempdir().unwrap();
    let graph_json = tmp.path().join("graph.json");
    let out = Command::new(exe())
        .args(["graph", "extract", "-i"])
        .arg(&invariants)
        .args(["-o"])
        .arg(&graph_json)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "extract failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    let singles = tmp.path().join("singles");
    let out = Command::new(exe())
        .args(["graph", "single", "-i"])
        .arg(&graph_json)
        .args(["-o"])
        .arg(&singles)
        .args(["--class", "all", "--format", "plantuml"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "single failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    let toplevel_at_root = singles.join("Toplevel.puml");
    let toplevel_nested = singles.join("Toplevel").join("Toplevel.puml");
    assert!(
        toplevel_at_root.is_file(),
        "root-level class should be written at {}, but file does not exist",
        toplevel_at_root.display(),
    );
    assert!(
        !toplevel_nested.exists(),
        "root-level class must not be nested under a same-named directory ({})",
        toplevel_nested.display(),
    );
    assert!(singles.join("bo").join("Foo.puml").is_file());
}

#[test]
fn single_class_all_wipes_output_dir_by_default_and_preserves_with_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let graph_json = tmp.path().join("graph.json");
    Command::new(exe())
        .args(["graph", "extract", "-i"])
        .arg(fixture())
        .args(["-o"])
        .arg(&graph_json)
        .output()
        .unwrap();

    let singles = tmp.path().join("singles");
    std::fs::create_dir_all(&singles).unwrap();
    let stale = singles.join("STALE.txt");

    std::fs::write(&stale, b"old").unwrap();
    let out = Command::new(exe())
        .args(["graph", "single", "-i"])
        .arg(&graph_json)
        .args(["-o"])
        .arg(&singles)
        .args(["--class", "all", "--format", "plantuml"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        !stale.exists(),
        "default run should have wiped {}",
        stale.display()
    );

    std::fs::write(&stale, b"old").unwrap();
    let out = Command::new(exe())
        .args(["graph", "single", "-i"])
        .arg(&graph_json)
        .args(["-o"])
        .arg(&singles)
        .args([
            "--class",
            "all",
            "--format",
            "plantuml",
            "--no-clear-output",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        stale.exists(),
        "--no-clear-output should have kept {}",
        stale.display()
    );
}
