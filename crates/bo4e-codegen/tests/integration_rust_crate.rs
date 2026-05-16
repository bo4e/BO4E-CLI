#![cfg(feature = "rust-crate")]

use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bo4e_min")
}

#[test]
fn writes_cargo_toml_and_src_layout() {
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
            crate_name: "bo4e_test".into(),
        },
    )
    .expect("generate");

    let cargo = std::fs::read_to_string(tmp.path().join("Cargo.toml")).unwrap();
    assert!(cargo.contains("name = \"bo4e_test\""));
    assert!(cargo.contains("version = \""));

    assert!(tmp.path().join("src/lib.rs").exists());
    assert!(tmp.path().join("src/bo/angebot.rs").exists());
    assert!(tmp.path().join("src/enums/typ.rs").exists());
    assert!(
        !tmp.path().join("src/mod.rs").exists(),
        "mod.rs should have been renamed to lib.rs"
    );
}
