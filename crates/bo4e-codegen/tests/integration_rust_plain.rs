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
    // After the strict-schema rework, `_version` is treated like any other
    // optional+nullable field: the schema's `default: "v…"` literal drives
    // the Default impl. The previous `default_version()` helper indirection
    // is gone, so no special `use super::…default_version;` line should
    // appear and no `fn default_version()` should be defined per-file.
    assert!(
        !body.contains("default_version"),
        "no default_version helper should appear; schema literal drives the default, got:\n{body}"
    );
}

#[test]
fn root_mod_rs_carries_version_const_only() {
    let tmp = generate_into_tmp();
    let body = std::fs::read_to_string(tmp.path().join("mod.rs")).unwrap();
    // The root module exposes the public `VERSION: &str` constant for
    // downstream consumers, but no longer defines the `default_version()`
    // helper — `_version` fields take their default straight from the
    // schema's `default` literal (validated by the strict required/default
    // invariant in `crate::validate`).
    assert!(
        body.contains("pub const VERSION: &str ="),
        "expected VERSION const at root, got:\n{body}"
    );
    assert!(
        !body.contains("default_version"),
        "default_version helper should be gone after the _version strip, got:\n{body}"
    );
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

/// Regression: `mod.rs` reexports must use the schema's real PascalCase class
/// name, not one reconstructed by uppercasing the first char of the lowercased
/// file stem. The fixture set only contains single-word names (`Angebot`,
/// `Adresse`), which happen to round-trip through that broken reconstruction —
/// a multi-word name like `PreisblattDienstleistung` would lose its internal
/// capital and yield `Preisblattdienstleistung` (which doesn't exist as a
/// struct), so the generated crate would fail to compile.
#[test]
fn mod_rs_reexport_preserves_internal_camel_case() {
    let tmp = tempfile::tempdir().unwrap();
    let mut schemas = bo4e_schemas::Schemas::new("v202401.0.0".parse().unwrap());

    let mut s =
        bo4e_schemas::Schema::new(vec!["bo".into(), "PreisblattDienstleistung".into()], None)
            .unwrap();
    s.load_schema(
        r#"{"type":"object","title":"PreisblattDienstleistung","properties":{},"required":[]}"#
            .into(),
    );
    schemas
        .add_schema(std::rc::Rc::new(std::cell::RefCell::new(s)))
        .unwrap();

    bo4e_codegen::rust::plain::generate(
        &schemas,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: false,
            templates_dir: None,
        },
    )
    .expect("generate");

    let bo_mod = std::fs::read_to_string(tmp.path().join("bo/mod.rs")).unwrap();
    assert!(
        bo_mod.contains("pub use preisblattdienstleistung::PreisblattDienstleistung;"),
        "expected reexport of the real PascalCase class name, got:\n{bo_mod}",
    );
}
