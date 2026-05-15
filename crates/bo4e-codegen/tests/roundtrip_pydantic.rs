#![cfg(feature = "python-pydantic")]

//! Round-trip integration test for the strict required/default matrix
//! on the pydantic side. Mirror of `roundtrip_rust_crate.rs`: generates
//! the `bo4e_invariants` fixture, drops a Python pytest-style script
//! that exercises every matrix row against the generated `Foo` class,
//! and shells out to `python3` to run it.
//!
//! Skips gracefully if `python3` isn't on PATH (mirrors the existing
//! parity_pydantic.rs behaviour). The script itself doesn't depend on
//! pydantic being installed — it imports the generated module and
//! attempts to instantiate / serialise `Foo`. If pydantic isn't
//! present locally, the import fails and the test reports a clear
//! skip-and-message rather than a misleading failure.

use std::path::PathBuf;
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bo4e_invariants")
}

/// Probe for a Python 3 with **pydantic v2** installed. The generated
/// code calls `model_validate` (a v2 API), so probing only `import
/// pydantic` would let v1 environments run the test and fail in
/// confusing ways; we explicitly require `pydantic.VERSION` to start
/// with `2.`.
fn python3_with_pydantic_v2() -> Option<PathBuf> {
    let py = std::env::var("PYTHON3").unwrap_or_else(|_| "python3".to_string());
    let probe = Command::new(&py)
        .arg("-c")
        .arg("import pydantic, sys; sys.exit(0 if pydantic.VERSION.split('.')[0] == '2' else 2)")
        .output()
        .ok()?;
    if probe.status.success() {
        Some(PathBuf::from(py))
    } else {
        None
    }
}

#[test]
fn generated_pydantic_roundtrips_strict_matrix() {
    let Some(py) = python3_with_pydantic_v2() else {
        eprintln!("python3 with pydantic v2 not available; skipping roundtrip_pydantic test");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    // Generate into a Python-valid subdirectory name rather than the tempdir
    // itself: `tempfile::tempdir()` uses prefix `.tmp` and Python rejects
    // dotted basenames when used as a package name.
    let pkg_root = tmp.path().join("bo4e_pkg");
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir()).expect("read_schemas");
    bo4e_codegen::python::pydantic::generate(
        &out.schemas,
        &pkg_root,
        &bo4e_codegen::Options {
            clear_output: true,
            templates_dir: None,
        },
    )
    .expect("generate");

    let script_path = tmp.path().join("_roundtrip_test.py");
    std::fs::write(&script_path, TEST_SCRIPT).unwrap();

    let output = Command::new(&py)
        .arg(&script_path)
        .arg(&pkg_root)
        .output()
        .expect("invoke python3");
    assert!(
        output.status.success(),
        "pydantic roundtrip failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// pytest-style assertions but driven by plain `assert` so we don't need
/// pytest as a dependency. Argv[1] is the generated-package root.
const TEST_SCRIPT: &str = r#"
import sys, pathlib
pkg_root = pathlib.Path(sys.argv[1])
# Make the generated package importable by its directory name.
sys.path.insert(0, str(pkg_root.parent))
pkg_name = pkg_root.name

# Import via importlib so the package can have any tempdir-generated name.
import importlib
foo_mod = importlib.import_module(f"{pkg_name}.bo.foo")
color_mod = importlib.import_module(f"{pkg_name}.enum.color")
toplevel_mod = importlib.import_module(f"{pkg_name}.toplevel")
Foo = foo_mod.Foo
Color = color_mod.Color
Toplevel = toplevel_mod.Toplevel

# Root-level schema wiring: `Toplevel` lives at the package root
# (no subdirectory) and must be importable both via its module path
# and via the package's `__init__.py` re-export.
t = Toplevel.model_validate({"name": "abc"})
assert t.name == "abc"
assert t.tag == "default-tag"
pkg_mod = importlib.import_module(pkg_name)
assert hasattr(pkg_mod, "Toplevel"), "Toplevel must be re-exported from root __init__"

# 1. Missing-key path: every optional field falls back to schema default.
f = Foo.model_validate({"_id": None, "req_str": "hi", "req_nullable_str": None})
assert f.req_str == "hi"
assert f.req_nullable_str is None
assert f.opt_str_with_default == "hello"
assert f.opt_int_with_default == 42
assert f.opt_bool_with_default is True
assert f.opt_nullable_str_null_default is None
assert f.opt_nullable_str_literal_default == "world"
assert f.opt_nullable_enum_with_default == Color.RED

# 2. Explicit values: each one preserved.
f = Foo.model_validate({
    "req_str": "X",
    "req_nullable_str": "Y",
    "opt_str_with_default": "custom",
    "opt_int_with_default": 7,
    "opt_bool_with_default": False,
    "opt_nullable_str_null_default": "non-null",
    "opt_nullable_str_literal_default": "overridden",
    "opt_nullable_enum_with_default": "BLUE",
})
assert f.req_str == "X"
assert f.req_nullable_str == "Y"
assert f.opt_str_with_default == "custom"
assert f.opt_int_with_default == 7
assert f.opt_bool_with_default is False
assert f.opt_nullable_str_null_default == "non-null"
assert f.opt_nullable_str_literal_default == "overridden"
assert f.opt_nullable_enum_with_default == Color.BLUE

# 3. Explicit null overrides literal default on a nullable field.
f = Foo.model_validate({
    "req_str": "X",
    "req_nullable_str": None,
    "opt_nullable_str_literal_default": None,
    "opt_nullable_enum_with_default": None,
})
assert f.opt_nullable_str_literal_default is None
assert f.opt_nullable_enum_with_default is None

# 4. Typed-format defaults: missing keys produce *typed* values,
# not raw strings. date/uuid/Decimal constructors are generated
# at render time (immutable types, no mutable-default trap).
from datetime import date
from decimal import Decimal
from uuid import UUID
f = Foo.model_validate({"req_str": "x", "req_nullable_str": None})
# Matrix row 3 (non-nullable + literal default): pydantic returns
# the typed value directly, not Optional. The default is a real
# `date(2024, 1, 15)` / `UUID(...)` / `Decimal("1.23")` instance,
# not a string that pydantic coerces — proves the immutable typed
# constructors in literal_default.
assert isinstance(f.opt_date_with_default, date), type(f.opt_date_with_default)
assert f.opt_date_with_default == date(2024, 1, 15)
assert isinstance(f.opt_uuid_with_default, UUID), type(f.opt_uuid_with_default)
assert f.opt_uuid_with_default == UUID("550e8400-e29b-41d4-a716-446655440000")
assert isinstance(f.opt_decimal_with_default, Decimal), type(f.opt_decimal_with_default)
assert f.opt_decimal_with_default == Decimal("1.23")

print("ok")
"#;
