use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

fn kroki_url() -> String {
    std::env::var("KROKI_URL").unwrap_or_else(|_| "http://localhost:8000".to_string())
}

fn kroki_reachable(url: &str) -> bool {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(1))
        .build();
    agent.get(url).call().is_ok()
}

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

fn post_to_kroki(url: &str, source: &str, kind: &str) -> Result<(), String> {
    let body = serde_json::json!({
        "diagram_source": source,
        "diagram_type": kind,
        "output_format": "svg",
    });
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let resp = agent
        .post(url)
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| format!("kroki POST failed: {e}"))?;
    if resp.status() == 200 {
        Ok(())
    } else {
        Err(format!("kroki returned status {}", resp.status()))
    }
}

#[test]
#[ignore]
fn extract_overview_single_outputs_are_valid_for_kroki() {
    let url = kroki_url();
    if !kroki_reachable(&url) {
        println!("[skipped — no Kroki at {url}]");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let graph_json = tmp.path().join("g.json");
    let out = Command::new(exe())
        .args(["graph", "extract", "-i"])
        .arg(fixture())
        .args(["-o"])
        .arg(&graph_json)
        .output()
        .unwrap();
    assert!(out.status.success());

    for fmt in &["dot", "plantuml"] {
        let overview = tmp.path().join(format!("overview.{fmt}"));
        let out = Command::new(exe())
            .args(["graph", "overview", "-i"])
            .arg(&graph_json)
            .args(["-o"])
            .arg(&overview)
            .args(["--format", fmt])
            .output()
            .unwrap();
        assert!(out.status.success(), "overview {fmt} failed");
        let source = std::fs::read_to_string(&overview).unwrap();
        let kind = if *fmt == "dot" {
            "graphviz"
        } else {
            "plantuml"
        };
        post_to_kroki(&url, &source, kind)
            .unwrap_or_else(|e| panic!("Kroki rejected overview {fmt} output: {e}"));
    }

    let ir: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&graph_json).unwrap()).unwrap();
    let class = ir["nodes"][0]["module"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    for fmt in &["dot", "plantuml"] {
        let single = tmp.path().join(format!("single.{fmt}"));
        let out = Command::new(exe())
            .args(["graph", "single", "-i"])
            .arg(&graph_json)
            .args(["--class", &class, "-o"])
            .arg(&single)
            .args(["--format", fmt])
            .output()
            .unwrap();
        assert!(out.status.success(), "single {fmt} failed");
        let source = std::fs::read_to_string(&single).unwrap();
        let kind = if *fmt == "dot" {
            "graphviz"
        } else {
            "plantuml"
        };
        post_to_kroki(&url, &source, kind)
            .unwrap_or_else(|e| panic!("Kroki rejected single {fmt} output: {e}"));
    }
}
