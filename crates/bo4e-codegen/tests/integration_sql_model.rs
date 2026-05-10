#![cfg(feature = "python-sql-model")]

use std::path::PathBuf;
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bo4e_sql_min")
}

fn generate_into_tmp() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir()).expect("read_schemas");
    bo4e_codegen::generate(
        &out.schemas,
        bo4e_codegen::OutputType::PythonSqlModel,
        tmp.path(),
        &bo4e_codegen::Options { clear_output: true, templates_dir: None },
    ).expect("generate");
    tmp
}

#[test]
fn generates_expected_files() {
    let tmp = generate_into_tmp();
    for rel in [
        "bo/angebot.py",
        "com/adresse.py",
        "enum/typ.py",
        "many.py",
        "__init__.py",
        "__version__.py",
        "bo/__init__.py",
        "com/__init__.py",
        "enum/__init__.py",
    ] {
        let p = tmp.path().join(rel);
        assert!(p.exists(), "expected {rel} to exist");
    }
}

#[test]
fn generated_angebot_contains_all_field_kinds_and_imports() {
    let tmp = generate_into_tmp();
    let body = std::fs::read_to_string(tmp.path().join("bo/angebot.py")).unwrap();

    assert!(body.contains("class Angebot(SQLModel, table=True):"), "got:\n{body}");
    assert!(body.contains("import uuid as uuid_pkg"), "got:\n{body}");
    assert!(body.contains("from sqlmodel import Field, Relationship, SQLModel"), "got:\n{body}");
    assert!(body.contains("from ..com.adresse import Adresse"), "got:\n{body}");
    assert!(body.contains("from ..many import AngebotAdressenLink"), "got:\n{body}");
    assert!(body.contains("from ..enum.typ import Typ"), "got:\n{body}");

    assert!(body.contains("id_: uuid_pkg.UUID = Field(alias=\"_id\", default_factory=uuid_pkg.uuid4, primary_key=True"));
    assert!(body.contains("adresse_id: uuid_pkg.UUID | None = Field(default=None, foreign_key=\"adresse.id\""));
    assert!(body.contains("adresse: Adresse | None = Relationship("));
    assert!(body.contains("adressen: list[Adresse] = Relationship(link_model=AngebotAdressenLink)"));
    assert!(body.contains("typ: Typ | None = Field(alias=\"_typ\","));
    assert!(body.contains("werte: list[Decimal] = Field(sa_column=Column(ARRAY(Numeric)))"));
    assert!(body.contains("extras: Any = Field(sa_column=Column(PickleType, nullable=True))") && !body.contains("extras: Any | None"));
    assert!(body.contains("anhaenge: list[Any] = Field(sa_column=Column(ARRAY(PickleType), nullable=False))"));
    assert!(body.contains("model_config = ConfigDict(alias_generator=to_camel, populate_by_name=True, use_attribute_docstrings=True)"));
    assert!(body.contains("from pydantic import ConfigDict"));
    assert!(body.contains("from pydantic.alias_generators import to_camel"));
    assert!(body.starts_with("\"\"\"Contains class Angebot.\"\"\""));
    assert!(!body.contains("__future__"));
}

#[test]
fn generated_many_py_has_junction_class() {
    let tmp = generate_into_tmp();
    let body = std::fs::read_to_string(tmp.path().join("many.py")).unwrap();
    assert!(body.contains("class AngebotAdressenLink(SQLModel, table=True):"), "got:\n{body}");
    assert!(body.contains("angebot_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"angebot.id\""));
    assert!(body.contains("adresse_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"adresse.id\""));
}

#[test]
fn ast_parses_every_generated_python_file() {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("python3 not available, skipping ast parse test");
        return;
    }
    let tmp = generate_into_tmp();
    for rel in ["bo/angebot.py", "com/adresse.py", "enum/typ.py", "many.py", "__init__.py"] {
        let path = tmp.path().join(rel);
        let script = format!(
            "import ast, sys; ast.parse(open({:?}).read()); print('ok')",
            path.to_string_lossy()
        );
        let out = Command::new("python3").arg("-c").arg(&script).output().unwrap();
        assert!(
            out.status.success(),
            "ast.parse failed for {rel}:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}
