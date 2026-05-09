#![cfg(feature = "python-pydantic")]

use std::process::Command;
use std::path::PathBuf;

#[test]
fn bo4e_generate_writes_output_directory() {
    let fixture: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()                // crates/
        .parent().unwrap()                // repo root
        .join("crates/bo4e-codegen/tests/fixtures/bo4e_min");
    assert!(fixture.exists(), "fixture dir not vendored");

    let tmp = tempfile::tempdir().unwrap();
    let exe = env!("CARGO_BIN_EXE_bo4e");

    let out = Command::new(exe)
        .arg("generate")
        .args(["-i", fixture.to_str().unwrap()])
        .args(["-o", tmp.path().to_str().unwrap()])
        .args(["-t", "python-pydantic"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "bo4e generate failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    assert!(tmp.path().join("bo/angebot.py").exists());
    assert!(tmp.path().join("__version__.py").exists());
}
