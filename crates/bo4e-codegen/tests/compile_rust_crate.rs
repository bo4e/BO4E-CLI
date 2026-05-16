#![cfg(feature = "rust-crate")]

//! Compile-time correctness signal: generate a crate and shell out to `cargo build`
//! inside it. Catches generated-code regressions that unit tests miss.
//!
//! Requires `cargo` on PATH (always true in CI for a Rust workspace).

use std::path::PathBuf;
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bo4e_min")
}

#[test]
fn generated_crate_compiles() {
    let tmp = tempfile::tempdir().unwrap();
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir()).expect("read_schemas");
    bo4e_codegen::rust::crate_::generate(
        &out.schemas,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: true,
            templates_dir: None,
        },
        &bo4e_codegen::RustCrateOptions {
            crate_name: "bo4e_compile_smoke".into(),
        },
    )
    .expect("generate");

    // Use a separate target dir so we don't pollute the parent target/.
    let target_dir = tmp.path().join("__target");
    let output = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(tmp.path().join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        .output()
        .expect("invoke cargo");
    assert!(
        output.status.success(),
        "cargo build of generated crate failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
