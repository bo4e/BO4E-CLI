#![cfg(feature = "rust-plain")]

use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bo4e_min")
}

fn generate_into_tmp() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir()).expect("read_schemas");
    bo4e_codegen::rust::plain::generate(
        &out.schemas,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: true,
            templates_dir: None,
        },
    )
    .expect("generate");
    tmp
}

#[test]
fn writes_expected_files() {
    let tmp = generate_into_tmp();
    for rel in [
        "bo/angebot.rs",
        "com/adresse.rs",
        "enums/typ.rs",
        "mod.rs",
        "bo/mod.rs",
        "com/mod.rs",
        "enums/mod.rs",
    ] {
        let p = tmp.path().join(rel);
        assert!(p.exists(), "expected {rel} to exist");
    }
}

#[test]
fn angebot_has_struct_and_sibling_use() {
    let tmp = generate_into_tmp();
    let body = std::fs::read_to_string(tmp.path().join("bo/angebot.rs")).unwrap();
    assert!(body.contains("pub struct Angebot"), "got:\n{body}");
    assert!(
        body.contains("use super::super::com::adresse::Adresse;"),
        "got:\n{body}"
    );
    assert!(body.contains("pub enum AngebotTyp"), "got:\n{body}");
    assert!(body.contains("impl Default for Angebot"), "got:\n{body}");
}

#[test]
fn typ_is_str_enum_with_serde_renames() {
    let tmp = generate_into_tmp();
    let body = std::fs::read_to_string(tmp.path().join("enums/typ.rs")).unwrap();
    assert!(body.contains("pub enum Typ"), "got:\n{body}");
    assert!(body.contains("#[serde(rename = \"ANGEBOT\")]"));
    assert!(body.contains("Angebot,"));
}

#[test]
fn root_mod_rs_lists_top_packages_and_version() {
    let tmp = generate_into_tmp();
    let body = std::fs::read_to_string(tmp.path().join("mod.rs")).unwrap();
    assert!(body.contains("pub mod bo;"));
    assert!(body.contains("pub mod com;"));
    assert!(body.contains("pub mod enums;"));
    assert!(body.contains("pub const VERSION:"));
}
