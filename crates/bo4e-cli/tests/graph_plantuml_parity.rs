use std::path::PathBuf;
use std::process::Command;

fn fixture_schemas() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/bo4e-codegen/tests/fixtures/bo4e_min")
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/graph/golden/plantuml")
}

fn exe() -> &'static str {
    env!("CARGO_BIN_EXE_bo4e")
}

fn run_parity(class: &str) {
    let tmp = tempfile::tempdir().unwrap();
    let graph_json = tmp.path().join("g.json");
    let out_file = tmp.path().join(format!("{class}.puml"));

    let out = Command::new(exe())
        .args(["graph", "extract", "-i"])
        .arg(fixture_schemas())
        .args(["-o"])
        .arg(&graph_json)
        .output()
        .unwrap();
    assert!(out.status.success(), "extract failed: {:?}", out);

    let out = Command::new(exe())
        .args(["graph", "single", "-i"])
        .arg(&graph_json)
        .args(["--class", class, "-o"])
        .arg(&out_file)
        .args(["--format", "plantuml"])
        .output()
        .unwrap();
    assert!(out.status.success(), "single failed: {:?}", out);

    let actual = std::fs::read_to_string(&out_file).unwrap();
    let golden_path = golden_dir().join(format!("{class}.puml"));
    let expected = std::fs::read_to_string(&golden_path)
        .unwrap_or_else(|e| panic!("Missing golden {}: {e}", golden_path.display()));
    // Normalise CRLF -> LF so the test passes on Windows runners where git's
    // `core.autocrlf` may have rewritten line endings in the checked-out
    // golden file. The emitter always produces LF.
    assert_eq!(
        actual.trim().replace("\r\n", "\n"),
        expected.trim().replace("\r\n", "\n"),
        "PlantUML output drifted from golden for class {class}.\nTo regenerate, run:\n  cargo run -p bo4e-cli -- graph single -i <graph.json> --class {class} -o {}",
        golden_path.display(),
    );
}

#[test]
fn plantuml_parity_angebot() {
    run_parity("Angebot");
}
