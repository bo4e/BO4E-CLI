//! Opt-in integration tests that run against the full BO4E schema set.
//!
//! Hydrate `.tmp/bo4e_latest/` first via `scripts/fetch-bo4e-fixture.sh`, then run:
//!   cargo test -p bo4e-cli --test full_bo4e -- --ignored
//!
//! These tests are NOT run by default `cargo test` — `#[ignore]` keeps them out.

use bo4e_schemas::models::json_schema::SchemaRootType;
use std::path::{Path, PathBuf};

const FIXTURE_ROOT: &str = ".tmp/bo4e_latest";

fn fixture_root() -> PathBuf {
    // Tests run from the crate dir; walk up to repo root.
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_dir.parent().and_then(Path::parent).unwrap();
    workspace_root.join(FIXTURE_ROOT)
}

fn require_fixture(root: &Path) {
    if !root.join(".version").exists() {
        panic!(
            "missing {} — run scripts/fetch-bo4e-fixture.sh first",
            root.display()
        );
    }
}

fn walk_json_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn recurse(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("readdir") {
            let entry = entry.expect("dirent");
            let p = entry.path();
            if p.is_dir() {
                recurse(&p, out);
            } else if p.extension().and_then(|s| s.to_str()) == Some("json") {
                out.push(p);
            }
        }
    }
    recurse(root, &mut out);
    out
}

#[test]
#[ignore = "requires .tmp/bo4e_latest; run scripts/fetch-bo4e-fixture.sh first"]
fn parse_every_schema() {
    let root = fixture_root();
    require_fixture(&root);
    let files = walk_json_files(&root);
    assert!(!files.is_empty(), "fixture has no JSON files at {}", root.display());

    let mut failures: Vec<(PathBuf, String)> = Vec::new();
    for path in &files {
        let raw = std::fs::read_to_string(path).expect("read json");
        match serde_json::from_str::<SchemaRootType>(&raw) {
            Ok(_) => {}
            Err(e) => failures.push((path.clone(), e.to_string())),
        }
    }
    if !failures.is_empty() {
        let mut report = format!(
            "{}/{} schemas failed to parse:\n",
            failures.len(),
            files.len()
        );
        for (p, e) in &failures {
            report.push_str(&format!("  {} → {}\n", p.display(), e));
        }
        panic!("{}", report);
    }
}

use bo4e_cli::cli::base::{Cli, Executable};
use clap::Parser;

fn ensure_console() {
    use bo4e_cli::console::console::{CONSOLE, Console, Level};
    let _ = CONSOLE.set(Console::new(Level::Normal));
}

fn drive_pipeline(generate_target: &str) {
    let root = fixture_root();
    require_fixture(&root);

    ensure_console();

    let workdir = tempfile::tempdir().unwrap();
    let edited  = workdir.path().join("edited");
    let diff_f  = workdir.path().join("diff.json");
    let gen_out = workdir.path().join("generated");

    // edit
    Cli::try_parse_from([
        "bo4e", "edit", "-i", root.to_str().unwrap(),
        "-o", edited.to_str().unwrap(),
    ])
    .unwrap()
    .run()
    .expect("edit");

    // diff (compare edited against itself — empty diff, exercises the read+write paths)
    Cli::try_parse_from([
        "bo4e", "diff", "schemas",
        edited.to_str().unwrap(),
        edited.to_str().unwrap(),
        "-o", diff_f.to_str().unwrap(),
    ])
    .unwrap()
    .run()
    .expect("diff");

    // generate
    Cli::try_parse_from([
        "bo4e", "generate", "-i", edited.to_str().unwrap(),
        "-o", gen_out.to_str().unwrap(),
        "-t", generate_target,
    ])
    .unwrap()
    .run()
    .expect("generate");

    assert!(gen_out.exists(), "generate produced no output");
}

#[test]
#[ignore = "requires .tmp/bo4e_latest; run scripts/fetch-bo4e-fixture.sh first"]
fn pull_to_edit_to_diff_to_generate_pydantic() {
    drive_pipeline("python-pydantic");
}

#[test]
#[ignore = "requires .tmp/bo4e_latest; run scripts/fetch-bo4e-fixture.sh first"]
fn pull_to_edit_to_diff_to_generate_sql_model() {
    drive_pipeline("python-sql-model");
}
