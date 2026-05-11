#![cfg(feature = "python-pydantic")]

// NOTE: Each fixture object schema must include `"required": []` until
// bo4e-schemas adds `#[serde(default)]` on `ObjectSchema::required`.
// See crates/bo4e-schemas/src/models/json_schema.rs.

use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bo4e_min")
}

fn generate_into_tmp() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir()).expect("read_schemas");
    let schemas = out.schemas;

    bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydantic,
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
fn generates_expected_files_for_minimal_fixture() {
    let tmp = generate_into_tmp();

    for rel in [
        "bo/angebot.py",
        "com/adresse.py",
        "enum/typ.py",
        "__version__.py",
        "__init__.py",
    ] {
        let p = tmp.path().join(rel);
        assert!(p.exists(), "expected {rel} to exist");
    }
}

#[test]
fn generated_classes_have_expected_names_and_imports() {
    let tmp = generate_into_tmp();

    let angebot = std::fs::read_to_string(tmp.path().join("bo/angebot.py")).unwrap();
    assert!(angebot.contains("class Angebot(BaseModel):"));
    assert!(angebot.contains("from pydantic import BaseModel"));
    assert!(!angebot.contains("__future__"));
    assert!(
        angebot.contains("Adresse"),
        "expected bo/angebot.py to reference the Adresse class via cross-file $ref, got:\n{angebot}"
    );
    assert!(
        angebot.contains("from ..com.adresse import Adresse"),
        "expected bo/angebot.py to import Adresse from ..com.adresse, got:\n{angebot}"
    );

    let typ = std::fs::read_to_string(tmp.path().join("enum/typ.py")).unwrap();
    assert!(typ.contains("class Typ(StrEnum):"));
    assert!(!typ.contains("__future__"));

    let init = std::fs::read_to_string(tmp.path().join("__init__.py")).unwrap();
    assert!(init.contains("from .bo.angebot import Angebot"));
}

#[test]
fn ban_future_imports_globally() {
    let tmp = generate_into_tmp();

    for entry in walkdir::WalkDir::new(tmp.path())
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "py"))
    {
        let body = std::fs::read_to_string(entry.path()).unwrap();
        assert!(
            !body.contains("__future__"),
            "found __future__ import in {:?}",
            entry.path()
        );
    }
}

#[test]
fn pydantic_renders_richer_sql_fixture_without_error() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bo4e_sql_min");
    let tmp = tempfile::tempdir().unwrap();
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture).expect("read_schemas");

    bo4e_codegen::generate(
        &out.schemas,
        bo4e_codegen::OutputType::PythonPydantic,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: true,
            templates_dir: None,
        },
    )
    .expect("generate");

    let angebot = std::fs::read_to_string(tmp.path().join("bo/angebot.py")).unwrap();
    // The pydantic flavour renders M:N as list[Adresse] | None (no junction concept);
    // Any subsumes None so it stays bare; list[Decimal] as list[Decimal] | None.
    assert!(
        angebot.contains("class Angebot(BaseModel):"),
        "got:\n{angebot}"
    );
    assert!(
        angebot.contains("adressen: list[Adresse]"),
        "got:\n{angebot}"
    );
    assert!(
        angebot.contains("extras: Any") && !angebot.contains("Any | None"),
        "got:\n{angebot}"
    );
    assert!(angebot.contains("werte: list[Decimal]"), "got:\n{angebot}");
    assert!(!angebot.contains("__future__"));
    assert!(
        !angebot.contains("table=True"),
        "pydantic flavour must not emit table=True"
    );
}
