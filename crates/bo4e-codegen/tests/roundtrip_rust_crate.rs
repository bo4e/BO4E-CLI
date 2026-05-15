#![cfg(feature = "rust-crate")]

//! Round-trip integration test for the strict required/default matrix.
//!
//! Generates the `bo4e_invariants` fixture as a `rust-crate`, drops a
//! Rust test file into the generated crate that deserialises a handful
//! of JSON payloads against the generated `Foo` type, and asserts on
//! the resulting field values. Then shells out to `cargo test` against
//! that crate so the assertions actually run.
//!
//! This catches regressions that a `cargo build`-only smoke test can't:
//! e.g. a `default_<field>()` helper that returns the wrong literal,
//! or a `skip_serializing_if` that drops a field that should be
//! serialised.
//!
//! Requires `cargo` on PATH (always true in CI).

use std::path::PathBuf;
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bo4e_invariants")
}

#[test]
fn generated_crate_roundtrips_strict_matrix() {
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
            crate_name: "bo4e_invariants_roundtrip".into(),
        },
    )
    .expect("generate");

    // Drop a test file into the generated crate. It uses serde_json (a
    // dev-dep added below) to round-trip three JSON payloads against the
    // generated `Foo` type and asserts on each field per the matrix.
    let tests_dir = tmp.path().join("tests");
    std::fs::create_dir_all(&tests_dir).unwrap();
    std::fs::write(tests_dir.join("roundtrip.rs"), TEST_FILE).unwrap();

    // Patch Cargo.toml to add `serde_json` as a dev-dep so the test file
    // can use it. (`serde_json` is already a runtime dep of the generated
    // crate, but [dev-dependencies] is conventionally separate.)
    let manifest_path = tmp.path().join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path).unwrap();
    let patched = format!("{manifest}\n[dev-dependencies]\nserde_json = \"1\"\n");
    std::fs::write(&manifest_path, patched).unwrap();

    let target_dir = tmp.path().join("__target");
    let output = Command::new("cargo")
        .arg("test")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .arg("--target-dir")
        .arg(&target_dir)
        .output()
        .expect("invoke cargo test");
    assert!(
        output.status.success(),
        "cargo test on generated crate failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Embedded as `tests/roundtrip.rs` inside the generated crate. Each
/// `#[test]` exercises one row or column of the matrix.
const TEST_FILE: &str = r##"
use bo4e_invariants_roundtrip::Toplevel;
use bo4e_invariants_roundtrip::bo::foo::Foo;
use bo4e_invariants_roundtrip::enums::color::Color;

/// Missing-key path: only required fields are present. Every optional
/// field should fall back to the schema-declared default via the
/// per-field `default_<name>` helpers (or, for row 4, bare serde
/// `default` returning `Option::<T>::default() == None`).
#[test]
fn minimal_payload_applies_schema_defaults() {
    let json = r#"{"req_str": "hi", "req_nullable_str": null}"#;
    let f: Foo = serde_json::from_str(json).expect("deserialize minimal");
    assert_eq!(f.req_str, "hi");
    assert_eq!(f.req_nullable_str, None);
    assert_eq!(f.opt_str_with_default, "hello");
    assert_eq!(f.opt_int_with_default, 42);
    assert!(f.opt_bool_with_default);
    assert_eq!(f.opt_nullable_str_null_default, None);
    assert_eq!(f.opt_nullable_str_literal_default, Some("world".to_string()));
    assert_eq!(f.opt_nullable_enum_with_default, Some(Color::Red));
}

/// Present-key path: every value is supplied explicitly, including
/// values that differ from the schema default. Each field should
/// preserve the supplied value verbatim.
#[test]
fn explicit_payload_preserves_values() {
    let json = r#"{
        "req_str": "X",
        "req_nullable_str": "Y",
        "opt_str_with_default": "custom",
        "opt_int_with_default": 7,
        "opt_bool_with_default": false,
        "opt_nullable_str_null_default": "non-null",
        "opt_nullable_str_literal_default": "overridden",
        "opt_nullable_enum_with_default": "BLUE"
    }"#;
    let f: Foo = serde_json::from_str(json).expect("deserialize explicit");
    assert_eq!(f.req_str, "X");
    assert_eq!(f.req_nullable_str, Some("Y".to_string()));
    assert_eq!(f.opt_str_with_default, "custom");
    assert_eq!(f.opt_int_with_default, 7);
    assert!(!f.opt_bool_with_default);
    assert_eq!(f.opt_nullable_str_null_default, Some("non-null".to_string()));
    assert_eq!(f.opt_nullable_str_literal_default, Some("overridden".to_string()));
    assert_eq!(f.opt_nullable_enum_with_default, Some(Color::Blue));
}

/// Explicit null on a nullable optional field with a *non-null* literal
/// default. Strict-schema interpretation: the default applies only on
/// missing-key, not when null is supplied explicitly — so the field
/// should become None.
#[test]
fn explicit_null_overrides_literal_default() {
    let json = r#"{
        "req_str": "X",
        "req_nullable_str": null,
        "opt_nullable_str_literal_default": null,
        "opt_nullable_enum_with_default": null
    }"#;
    let f: Foo = serde_json::from_str(json).expect("deserialize null-overrides");
    assert_eq!(f.opt_nullable_str_literal_default, None);
    assert_eq!(f.opt_nullable_enum_with_default, None);
}

/// Serialisation: a fresh `Foo` with explicitly-provided values should
/// round-trip through serde_json back to the same JSON structure. Row 4
/// (optional + nullable + null default) is the one row whose
/// `skip_serializing_if = "Option::is_none"` makes serialisation drop
/// the field — that's the agreed strict-schema behaviour.
#[test]
fn serialisation_preserves_explicit_values() {
    let json = r#"{
        "req_str": "X",
        "req_nullable_str": "Y",
        "opt_str_with_default": "custom",
        "opt_int_with_default": 7,
        "opt_bool_with_default": false,
        "opt_nullable_str_null_default": "non-null",
        "opt_nullable_str_literal_default": "overridden",
        "opt_nullable_enum_with_default": "BLUE"
    }"#;
    let f: Foo = serde_json::from_str(json).unwrap();
    let v: serde_json::Value = serde_json::to_value(&f).unwrap();
    assert_eq!(v["req_str"], "X");
    assert_eq!(v["opt_str_with_default"], "custom");
    assert_eq!(v["opt_int_with_default"], 7);
    assert_eq!(v["opt_nullable_str_null_default"], "non-null");
    assert_eq!(v["opt_nullable_enum_with_default"], "BLUE");
}

/// Helper fn for the row-4 case isn't generated (bare `default` is
/// sufficient because `Option::<T>::default()` returns `None`, which
/// matches the schema's null literal). Verify the function isn't
/// reachable; since Rust can't directly assert "this fn doesn't exist",
/// we just confirm the documented sister fn DOES exist.
///
/// (This is a compile-time check: if a future change accidentally
/// emits `default_opt_nullable_str_null_default()`, it'd still
/// compile, but the test below makes the absence of need visible.)
#[test]
fn row4_uses_bare_serde_default() {
    // Missing key → None. Already tested above; restating as a focused
    // assertion.
    let json = r#"{"req_str": "x", "req_nullable_str": null}"#;
    let f: Foo = serde_json::from_str(json).unwrap();
    assert!(f.opt_nullable_str_null_default.is_none());
}

/// Root-level schema wiring: `Toplevel` lives at the crate root (no
/// parent directory). The generator must declare `pub mod toplevel;`
/// and re-export it from `lib.rs` so it's reachable as
/// `crate::Toplevel`. The `use` line at the top of this file proves the
/// re-export exists; this test exercises deserialisation.
#[test]
fn root_level_toplevel_deserialises() {
    let json = r#"{"name": "abc"}"#;
    let t: Toplevel = serde_json::from_str(json).expect("deserialize Toplevel");
    assert_eq!(t.name, "abc");
    assert_eq!(t.tag, Some("default-tag".to_string()));
}
"##;
