#![cfg(feature = "python-pydantic-v2")]

use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bo4e_min")
}

#[test]
fn generates_expected_files_for_minimal_fixture() {
    let tmp = tempfile::tempdir().unwrap();
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir()).expect("read_schemas");
    let schemas = out.schemas;

    bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydanticV2,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: true,
            templates_dir: None,
        },
    )
    .expect("generate");

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
    let tmp = tempfile::tempdir().unwrap();
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir()).expect("read_schemas");
    let schemas = out.schemas;

    bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydanticV2,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: true,
            templates_dir: None,
        },
    )
    .expect("generate");

    let angebot = std::fs::read_to_string(tmp.path().join("bo/angebot.py")).unwrap();
    assert!(angebot.contains("class Angebot(BaseModel):"));
    assert!(angebot.contains("from pydantic import BaseModel"));
    assert!(!angebot.contains("__future__"));

    let typ = std::fs::read_to_string(tmp.path().join("enum/typ.py")).unwrap();
    assert!(typ.contains("class Typ(StrEnum):"));
    assert!(!typ.contains("__future__"));

    let init = std::fs::read_to_string(tmp.path().join("__init__.py")).unwrap();
    assert!(init.contains("from .bo.angebot import Angebot"));
}

#[test]
fn ban_future_imports_globally() {
    let tmp = tempfile::tempdir().unwrap();
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir()).expect("read_schemas");
    let schemas = out.schemas;

    bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydanticV2,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: true,
            templates_dir: None,
        },
    )
    .expect("generate");

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
