#![cfg(feature = "python-pydantic")]

use std::path::PathBuf;
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bo4e_min")
}

fn python3_available() -> bool {
    Command::new("python3").arg("--version").output().is_ok()
}

#[test]
fn generated_angebot_parses_as_python_and_has_expected_class() {
    if !python3_available() {
        eprintln!("python3 not available, skipping parity test");
        return;
    }

    let tmp = tempfile::tempdir().unwrap();
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir()).unwrap();

    bo4e_codegen::generate(
        &out.schemas,
        bo4e_codegen::OutputType::PythonPydantic,
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
import ast, sys
src = open({path:?}).read()
tree = ast.parse(src)
classes = [n for n in ast.walk(tree) if isinstance(n, ast.ClassDef)]
assert len(classes) == 1, f"expected 1 class, got {{len(classes)}}"
assert classes[0].name == "Angebot", classes[0].name
bases = [b.id if isinstance(b, ast.Name) else getattr(b, "attr", "?") for b in classes[0].bases]
assert "BaseModel" in bases, f"expected BaseModel in bases, got {{bases}}"
print("ok")
"#,
        path = angebot.to_string_lossy().to_string()
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
