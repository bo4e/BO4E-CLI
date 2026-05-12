#![cfg(feature = "rust-crate")]

use std::path::PathBuf;
use std::process::Command;

#[test]
fn bo4e_generate_rust_crate_with_custom_crate_name() {
    let fixture: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("crates/bo4e-codegen/tests/fixtures/bo4e_min");
    let tmp = tempfile::tempdir().unwrap();
    let exe = env!("CARGO_BIN_EXE_bo4e");
    let out = Command::new(exe)
        .arg("generate")
        .args(["-i", fixture.to_str().unwrap()])
        .args(["-o", tmp.path().to_str().unwrap()])
        .arg("rust-crate")
        .args(["--crate-name", "my_bo4e_test"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cargo = std::fs::read_to_string(tmp.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("name = \"my_bo4e_test\""));
}
