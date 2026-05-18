#![cfg(feature = "python-sql-model")]

// Parity stub: full Python-side comparison is deferred to a follow-up plan.
// For now we assert the Rust output parses as Python and contains the expected
// class shapes. When the upstream Python image is wired in CI, this test grows
// to call the Python generator into a sibling tempdir and walk both ASTs.

use std::path::PathBuf;
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bo4e_sql_min")
}

fn python3_available() -> bool {
    Command::new("python3").arg("--version").output().is_ok()
}

#[test]
fn generated_angebot_parses_as_python_and_has_sqlmodel_class() {
    if !python3_available() {
        eprintln!("python3 not available, skipping parity test");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir()).unwrap();
    bo4e_codegen::python::sql_model::generate(
        &out.schemas,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: true,
            templates_dir: None,
        },
    )
    .unwrap();

    let angebot = tmp.path().join("bo/angebot.py");
    let script = format!(
        r#"
import ast
src = open({path:?}).read()
tree = ast.parse(src)
classes = [n for n in ast.walk(tree) if isinstance(n, ast.ClassDef)]
# Inline literal fields can produce synthetic single-member enums (e.g.
# `AngebotTyp`) emitted above the table class, so just locate `Angebot`.
angebot = next((c for c in classes if c.name == "Angebot"), None)
assert angebot is not None, f"Angebot class missing; got {{[c.name for c in classes]}}"
bases = [b.id if isinstance(b, ast.Name) else getattr(b, 'attr', '?') for b in angebot.bases]
assert "SQLModel" in bases, f"expected SQLModel in bases, got {{bases}}"
keywords = {{kw.arg: kw.value for kw in angebot.keywords}}
assert "table" in keywords, f"expected table=True keyword, got {{list(keywords)}}"
print("ok")
"#,
        path = angebot.to_string_lossy()
    );

    let output = Command::new("python3")
        .arg("-c")
        .arg(&script)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "python3 failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
