# sql-model Generator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the `python-sql-model` output type to `bo4e-codegen` so `bo4e generate -t python-sql-model` produces a drop-in replacement for the Python SQLModel generator. Also performs the pre-flight cleanup: drop the never-implemented `python-pydantic-v1` flavour and rename `python-pydantic-v2` → `python-pydantic` everywhere.

**Architecture:** Two-phase generator — a pure pre-pass walks `Schemas` and produces an immutable `SqlPlan` (BO/COM/enum tables + M:N junctions, every field classified into one of six `SqlField` variants); a render pass consumes the plan and writes Python files via vendored MiniJinja templates byte-identical to the upstream Python `custom_templates/`. Same module/file/class/field names as the Python implementation.

**Tech Stack:** Rust 2024 edition (existing). MiniJinja 2.x (existing). Python 3 in CI for AST-level integration + parity tests.

**Spec reference:** `docs/plans/2026-05-09-sql-model-generator-design.md`.
**Parity reference:** Python implementation at `/tmp/bo4e-cli-python/src/bo4e_cli/generate/python/{sql_parser.py,custom_templates/}`.
**Branch:** All work commits directly to `rust`.

---

## File Structure

After this plan, the relevant slice of the workspace looks like:

```
crates/bo4e-codegen/
├── Cargo.toml                                    (features: python, python-pydantic, python-sql-model)
├── src/
│   ├── lib.rs                                    (PythonSqlModel arm calls sql_model::generate_sql_model)
│   ├── env.rs                                    (registers both pydantic/ and sql_model/ templates)
│   ├── error.rs                                  (unchanged)
│   ├── naming.rs                                 (unchanged)
│   ├── output_type.rs                            (PythonPydantic + PythonSqlModel; v1 dropped)
│   └── python/
│       ├── mod.rs                                (exposes pydantic + sql_model behind cfg)
│       ├── imports.rs                            (unchanged)
│       ├── types.rs                              (map_pydantic_v2 → map_pydantic)
│       ├── pydantic.rs                           (renamed from pydantic_v2.rs)
│       └── sql_model/
│           ├── mod.rs                            (NEW — generate_sql_model + render helpers)
│           └── plan.rs                           (NEW — SqlPlan, SqlField, JunctionTable, build_plan)
│   └── templates/python/
│       ├── pydantic/                             (renamed from pydantic_v2/)
│       │   ├── BaseModel.jinja2
│       │   ├── Enum.jinja2
│       │   └── __init__.jinja2
│       └── sql_model/                            (NEW)
│           ├── BaseModel.jinja2                  (vendored byte-identical from upstream)
│           ├── Config.jinja2                     (vendored byte-identical)
│           ├── Enum.jinja2                       (vendored byte-identical)
│           ├── ManyLinks.jinja2                  (vendored byte-identical)
│           └── __init__.jinja2                   (authored — re-exports)
└── tests/
    ├── fixtures/
    │   ├── bo4e_min/                             (existing, untouched)
    │   └── bo4e_sql_min/                         (NEW — minimal fixture with M:N)
    ├── integration_pydantic.rs                   (renamed from integration_pydantic_v2.rs)
    ├── parity_pydantic.rs                        (renamed from parity_pydantic_v2.rs)
    ├── integration_sql_model.rs                  (NEW)
    ├── parity_sql_model.rs                       (NEW — stub, full fixture deferred)
    └── skeleton.rs                               (PythonPydanticV2 → PythonPydantic)

crates/bo4e-cli/
├── Cargo.toml                                    (features re-exported: python-pydantic, python-sql-model)
└── tests/
    └── generate_smoke.rs                         (python-pydantic-v2 → python-pydantic)

README.md                                          (v1 mentions removed; -v2 → plain pydantic)
```

Each task below references this layout.

---

### Task 1: Drop `python-pydantic-v1` everywhere

**Goal:** Remove the never-implemented v1 Cargo feature, its `OutputType` variant, its match arm in `lib.rs`, and its README mentions. Workspace builds and tests pass.

**Files:**
- Modify: `crates/bo4e-codegen/Cargo.toml`
- Modify: `crates/bo4e-cli/Cargo.toml`
- Modify: `crates/bo4e-codegen/src/output_type.rs`
- Modify: `crates/bo4e-codegen/src/lib.rs`
- Modify: `README.md`

- [ ] **Step 1: Capture green baseline**

```bash
cd /repos/bo4e-cli
cargo test --workspace 2>&1 | tail -3
```

Expected: `test result: ok` summary lines.

- [ ] **Step 2: Remove `python-pydantic-v1` from `crates/bo4e-codegen/Cargo.toml`**

Replace the `[features]` block:

```toml
[features]
default = ["python"]
python = ["python-pydantic-v1", "python-pydantic-v2", "python-sql-model"]
python-pydantic-v1 = []
python-pydantic-v2 = []
python-sql-model   = []
```

with:

```toml
[features]
default = ["python"]
python = ["python-pydantic-v2", "python-sql-model"]
python-pydantic-v2 = []
python-sql-model   = []
```

- [ ] **Step 3: Remove `python-pydantic-v1` from `crates/bo4e-cli/Cargo.toml`**

Delete the line:

```toml
python-pydantic-v1 = ["bo4e-codegen/python-pydantic-v1"]
```

- [ ] **Step 4: Remove the `PythonPydanticV1` variant from `crates/bo4e-codegen/src/output_type.rs`**

Delete these lines from the enum:

```rust
    #[cfg(feature = "python-pydantic-v1")]
    #[value(name = "python-pydantic-v1")]
    PythonPydanticV1,
```

And the matching arm in `as_str`:

```rust
            #[cfg(feature = "python-pydantic-v1")]
            Self::PythonPydanticV1 => "python-pydantic-v1",
```

- [ ] **Step 5: Remove the `PythonPydanticV1` arm + cfg gate from `crates/bo4e-codegen/src/lib.rs`**

Delete the placeholder arm:

```rust
        #[cfg(feature = "python-pydantic-v1")]
        OutputType::PythonPydanticV1 => Err(Error::OutputTypeNotCompiledIn(output_type.as_str())),
```

In the `mod python;` cfg gate and the `generate(...)` arg-attribute cfg gate, remove `feature = "python-pydantic-v1",` from both `cfg(any(...))` lists. The `mod python;` block becomes:

```rust
#[cfg(any(
    feature = "python-pydantic-v2",
    feature = "python-sql-model",
))]
mod python;
```

The `generate` arg-attribute becomes:

```rust
    #[cfg_attr(
        not(any(
            feature = "python-pydantic-v2",
            feature = "python-sql-model",
        )),
        allow(unused_variables)
    )]
    schemas: &Schemas,
```

- [ ] **Step 6: Update `README.md`** — remove `python-pydantic-v1` from the supported-languages list, the `--output-type` flag-table allowed values, and the `cargo install --features` example list. The current mentions are at:

```
README.md:247: | `--output-type` | `-t` | One of `python-pydantic-v1`, `python-pydantic-v2`, `python-sql-model` (gated by Cargo feature). |
README.md:259: Available features: `python` (umbrella), `python-pydantic-v1`, `python-pydantic-v2`, `python-sql-model`.
```

Update both to drop `python-pydantic-v1`:

```
| `--output-type` | `-t` | One of `python-pydantic-v2`, `python-sql-model` (gated by Cargo feature). |
```

```
Available features: `python` (umbrella), `python-pydantic-v2`, `python-sql-model`.
```

- [ ] **Step 7: Verify build + tests still pass**

```bash
cd /repos/bo4e-cli
cargo build --workspace 2>&1 | tail -3
cargo test --workspace 2>&1 | tail -3
```

Expected: build succeeds, `test result: ok` summary lines for every test binary.

- [ ] **Step 8: Verify no lingering v1 references in code**

```bash
cd /repos/bo4e-cli
grep -rn "python-pydantic-v1\|PythonPydanticV1\|pydantic_v1" --include="*.rs" --include="*.toml" --include="README.md"
```

Expected: only matches inside `docs/plans/2026-05-08-*` historical files (zero in code/config).

- [ ] **Step 9: Commit**

```bash
git add crates/bo4e-codegen/Cargo.toml crates/bo4e-cli/Cargo.toml \
        crates/bo4e-codegen/src/output_type.rs crates/bo4e-codegen/src/lib.rs \
        README.md
git commit -m "refactor(codegen): drop unused python-pydantic-v1 flavour

Removes the Cargo feature, OutputType variant, lib.rs match arm, and
README mentions. v1 was never implemented and there is no consumer
asking for it; carrying the name doubles surface area for the upcoming
sql-model addition.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2: Rename `python-pydantic-v2` → `python-pydantic` (Cargo features + `OutputType`)

**Goal:** Rename the Cargo feature flag, the `OutputType` enum variant, and the `ValueEnum` literal so the CLI surfaces `python-pydantic` instead of `python-pydantic-v2`. Workspace builds and tests pass.

**Files:**
- Modify: `crates/bo4e-codegen/Cargo.toml`
- Modify: `crates/bo4e-cli/Cargo.toml`
- Modify: `crates/bo4e-codegen/src/output_type.rs`

- [ ] **Step 1: Update `crates/bo4e-codegen/Cargo.toml`**

Replace the `[features]` block (post-Task-1 state) with:

```toml
[features]
default = ["python"]
python = ["python-pydantic", "python-sql-model"]
python-pydantic   = []
python-sql-model  = []
```

- [ ] **Step 2: Update `crates/bo4e-cli/Cargo.toml`**

Replace the `[features]` block with:

```toml
[features]
default = ["bo4e-codegen/default"]
python = ["bo4e-codegen/python"]
python-pydantic   = ["bo4e-codegen/python-pydantic"]
python-sql-model  = ["bo4e-codegen/python-sql-model"]
```

- [ ] **Step 3: Rename `OutputType::PythonPydanticV2` → `OutputType::PythonPydantic` in `crates/bo4e-codegen/src/output_type.rs`**

Replace the file contents with:

```rust
use clap::ValueEnum;

/// Which output type to generate. Variants are gated by Cargo features —
/// a feature compiled out removes its variant entirely so the CLI's clap
/// parser only accepts compiled-in values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[non_exhaustive]
pub enum OutputType {
    #[cfg(feature = "python-pydantic")]
    #[value(name = "python-pydantic")]
    PythonPydantic,
    #[cfg(feature = "python-sql-model")]
    #[value(name = "python-sql-model")]
    PythonSqlModel,
}

impl OutputType {
    pub fn as_str(&self) -> &'static str {
        #[allow(unreachable_patterns)]
        match self {
            #[cfg(feature = "python-pydantic")]
            Self::PythonPydantic => "python-pydantic",
            #[cfg(feature = "python-sql-model")]
            Self::PythonSqlModel => "python-sql-model",
            // Safety: when no features are compiled in, OutputType has no variants and this
            // branch is unreachable. When at least one feature is enabled, the arms above
            // cover every variant exhaustively.
            _ => unreachable!("OutputType variant not handled"),
        }
    }
}
```

- [ ] **Step 4: Verify the workspace doesn't build (expected — referrers still use the old name)**

```bash
cd /repos/bo4e-cli
cargo build --workspace 2>&1 | tail -20
```

Expected: errors in `lib.rs`, `pydantic_v2.rs`, `env.rs`, tests — the cleanup of those is Task 3.

- [ ] **Step 5: Commit**

```bash
git add crates/bo4e-codegen/Cargo.toml crates/bo4e-cli/Cargo.toml \
        crates/bo4e-codegen/src/output_type.rs
git commit -m "refactor(codegen): rename python-pydantic-v2 feature to python-pydantic

There is no v3 on the pydantic roadmap; the -v2 suffix carries no
future-proofing value and clutters the CLI. Drops the feature flag
and OutputType variant rename only — referrers (lib.rs, the pydantic
module, templates dir, tests, README) are renamed in the next commit
to keep this diff narrow and bisectable.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

(The workspace is intentionally broken at this commit; Task 3 fixes it. Acceptable for a multi-step rename — the next commit is small.)

---

### Task 3: Rename Rust modules, files, functions, and templates dir

**Goal:** Get the workspace building again by completing the Task-2 rename across all referrers: `pydantic_v2.rs` → `pydantic.rs`, `generate_pydantic_v2` → `generate_pydantic`, `map_pydantic_v2` → `map_pydantic`, templates dir `pydantic_v2/` → `pydantic/`, all `cfg` gates and `include_str!` paths.

**Files:**
- Modify: `crates/bo4e-codegen/src/lib.rs`
- Modify: `crates/bo4e-codegen/src/python/mod.rs`
- Move: `crates/bo4e-codegen/src/python/pydantic_v2.rs` → `crates/bo4e-codegen/src/python/pydantic.rs`
- Modify: `crates/bo4e-codegen/src/python/pydantic.rs` (post-rename)
- Modify: `crates/bo4e-codegen/src/python/types.rs`
- Modify: `crates/bo4e-codegen/src/env.rs`
- Move: `crates/bo4e-codegen/src/templates/python/pydantic_v2/` → `crates/bo4e-codegen/src/templates/python/pydantic/`
- Move: `crates/bo4e-codegen/tests/integration_pydantic_v2.rs` → `crates/bo4e-codegen/tests/integration_pydantic.rs`
- Move: `crates/bo4e-codegen/tests/parity_pydantic_v2.rs` → `crates/bo4e-codegen/tests/parity_pydantic.rs`
- Modify: `crates/bo4e-codegen/tests/integration_pydantic.rs`
- Modify: `crates/bo4e-codegen/tests/parity_pydantic.rs`
- Modify: `crates/bo4e-codegen/tests/skeleton.rs`
- Modify: `crates/bo4e-cli/tests/generate_smoke.rs`

- [ ] **Step 1: Move the source file and templates dir**

```bash
cd /repos/bo4e-cli
git mv crates/bo4e-codegen/src/python/pydantic_v2.rs crates/bo4e-codegen/src/python/pydantic.rs
git mv crates/bo4e-codegen/src/templates/python/pydantic_v2 crates/bo4e-codegen/src/templates/python/pydantic
```

- [ ] **Step 2: Move the test files**

```bash
cd /repos/bo4e-cli
git mv crates/bo4e-codegen/tests/integration_pydantic_v2.rs crates/bo4e-codegen/tests/integration_pydantic.rs
git mv crates/bo4e-codegen/tests/parity_pydantic_v2.rs crates/bo4e-codegen/tests/parity_pydantic.rs
```

- [ ] **Step 3: Update `crates/bo4e-codegen/src/python/mod.rs`**

Replace the file contents with:

```rust
pub(crate) mod imports;
pub(crate) mod types;

#[cfg(feature = "python-pydantic")]
pub(crate) mod pydantic;
```

- [ ] **Step 4: Rename function in `crates/bo4e-codegen/src/python/pydantic.rs`**

Replace every occurrence of `generate_pydantic_v2` with `generate_pydantic` and every reference to `map_pydantic_v2` with `map_pydantic`. Update the `use` statement at the top of the file:

```rust
use crate::python::types::{Import, map_pydantic};
```

Update the `pub(crate) fn` signature:

```rust
pub(crate) fn generate_pydantic(
    schemas: &Schemas,
    output_dir: &Path,
    env: &Environment<'static>,
) -> Result<Vec<PathBuf>, Error> {
```

Update all template-name keys inside this file from `"python/pydantic_v2/..."` to `"python/pydantic/..."` (there are three: `BaseModel.jinja2`, `Enum.jinja2`, `__init__.jinja2`).

Update the call inside `render_object`:

```rust
        let mapped = map_pydantic(prop_schema);
```

(One occurrence; was previously `map_pydantic_v2`.)

- [ ] **Step 5: Rename `map_pydantic_v2` in `crates/bo4e-codegen/src/python/types.rs`**

Replace the function signature:

```rust
pub fn map_pydantic(schema_type: &SchemaType) -> MappedType {
```

Replace every recursive call inside the function body and every test invocation (`map_pydantic_v2` → `map_pydantic`). A single search-and-replace within the file is sufficient.

Update the doc-comment header lines that mention "pydantic-v2":

```rust
//! The pydantic dialect emits:
```

(was `//! The pydantic-v2 dialect emits:`).

- [ ] **Step 6: Update `crates/bo4e-codegen/src/lib.rs`**

Replace every `python-pydantic-v2` cfg literal with `python-pydantic`. Replace the `OutputType::PythonPydanticV2` arm and its body:

```rust
        #[cfg(feature = "python-pydantic")]
        OutputType::PythonPydantic => {
            python::pydantic::generate_pydantic(schemas, output_dir, &env)?;
            Ok(())
        }
```

The full match block now reads:

```rust
    #[allow(unreachable_patterns)]
    match output_type {
        #[cfg(feature = "python-pydantic")]
        OutputType::PythonPydantic => {
            python::pydantic::generate_pydantic(schemas, output_dir, &env)?;
            Ok(())
        }
        #[cfg(feature = "python-sql-model")]
        OutputType::PythonSqlModel => Err(Error::OutputTypeNotCompiledIn(output_type.as_str())),
        // When all python features are compiled out, OutputType has no variants and
        // this match has no arms; the wildcard keeps the code well-formed.
        _ => unreachable!("OutputType variant not handled"),
    }
```

The `mod python;` cfg gate becomes:

```rust
#[cfg(any(
    feature = "python-pydantic",
    feature = "python-sql-model",
))]
mod python;
```

The `generate` arg-attribute becomes:

```rust
    #[cfg_attr(
        not(any(
            feature = "python-pydantic",
            feature = "python-sql-model",
        )),
        allow(unused_variables)
    )]
    schemas: &Schemas,
```

- [ ] **Step 7: Update `crates/bo4e-codegen/src/env.rs`**

Replace every `"python-pydantic-v2"` cfg literal with `"python-pydantic"`. Replace every `templates/python/pydantic_v2/` path with `templates/python/pydantic/`. Replace every template-name key from `"python/pydantic_v2/..."` to `"python/pydantic/..."`. The full `load_embedded` function becomes:

```rust
#[allow(unused_variables)]
fn load_embedded(env: &mut minijinja::Environment<'static>) -> Result<(), Error> {
    #[cfg(feature = "python-pydantic")]
    {
        env.add_template(
            "python/pydantic/BaseModel.jinja2",
            include_str!("templates/python/pydantic/BaseModel.jinja2"),
        )?;
        env.add_template(
            "python/pydantic/Enum.jinja2",
            include_str!("templates/python/pydantic/Enum.jinja2"),
        )?;
        env.add_template(
            "python/pydantic/__init__.jinja2",
            include_str!("templates/python/pydantic/__init__.jinja2"),
        )?;
    }
    Ok(())
}
```

Update the test (`embedded_pydantic_v2_init_template_renders` → `embedded_pydantic_init_template_renders`) and its template key (`"python/pydantic_v2/__init__.jinja2"` → `"python/pydantic/__init__.jinja2"`). The `disk_loader_loads_templates_from_supplied_directory` test similarly updates its sub-directory and template-name strings to use `pydantic` instead of `pydantic_v2`.

- [ ] **Step 8: Update `crates/bo4e-codegen/tests/integration_pydantic.rs`**

Replace `feature = "python-pydantic-v2"` with `feature = "python-pydantic"`. Replace `bo4e_codegen::OutputType::PythonPydanticV2` with `bo4e_codegen::OutputType::PythonPydantic`. The first three lines of the file become:

```rust
#![cfg(feature = "python-pydantic")]
```

And the `generate(...)` call becomes:

```rust
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
```

- [ ] **Step 9: Update `crates/bo4e-codegen/tests/parity_pydantic.rs`**

Same pattern: `feature = "python-pydantic-v2"` → `feature = "python-pydantic"`; `OutputType::PythonPydanticV2` → `OutputType::PythonPydantic`.

- [ ] **Step 10: Update `crates/bo4e-codegen/tests/skeleton.rs`**

Replace the file contents with:

```rust
#[cfg(feature = "python-pydantic")]
#[test]
fn generate_pydantic_writes_at_least_one_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut schemas = bo4e_schemas::Schemas::new("v202401.0.0".parse().unwrap());

    let mut s = bo4e_schemas::Schema::new(vec!["enum".into(), "Typ".into()], None).unwrap();
    s.load_schema(r#"{"type":"string","title":"Typ","enum":["A","B"]}"#.into());
    schemas
        .add_schema(std::rc::Rc::new(std::cell::RefCell::new(s)))
        .unwrap();

    bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydantic,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: false,
            templates_dir: None,
        },
    )
    .expect("generate");

    let typ_py = tmp.path().join("enum/typ.py");
    assert!(typ_py.exists(), "expected {:?} to exist", typ_py);
    let body = std::fs::read_to_string(&typ_py).unwrap();
    assert!(body.contains("class Typ"));
}
```

- [ ] **Step 11: Update `crates/bo4e-cli/tests/generate_smoke.rs`**

Replace the file contents with:

```rust
#![cfg(feature = "python-pydantic")]

use std::process::Command;
use std::path::PathBuf;

#[test]
fn bo4e_generate_writes_output_directory() {
    let fixture: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()                // crates/
        .parent().unwrap()                // repo root
        .join("crates/bo4e-codegen/tests/fixtures/bo4e_min");
    assert!(fixture.exists(), "fixture dir not vendored");

    let tmp = tempfile::tempdir().unwrap();
    let exe = env!("CARGO_BIN_EXE_bo4e");

    let out = Command::new(exe)
        .arg("generate")
        .args(["-i", fixture.to_str().unwrap()])
        .args(["-o", tmp.path().to_str().unwrap()])
        .args(["-t", "python-pydantic"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "bo4e generate failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );

    assert!(tmp.path().join("bo/angebot.py").exists());
    assert!(tmp.path().join("__version__.py").exists());
}
```

- [ ] **Step 12: Update `README.md`**

Replace every `python-pydantic-v2` mention (4 occurrences post-Task-1) with `python-pydantic`:

| Before | After |
| --- | --- |
| `bo4e generate -i ./bo4e_schemas_edited -o ./bo4e_schemas_python -t python-pydantic-v2` | `bo4e generate -i ./bo4e_schemas_edited -o ./bo4e_schemas_python -t python-pydantic` |
| `One of `python-pydantic-v2`, `python-sql-model``  | `One of `python-pydantic`, `python-sql-model`` |
| `cargo install bo4e-cli --no-default-features --features python-pydantic-v2` | `cargo install bo4e-cli --no-default-features --features python-pydantic` |
| `Available features: `python` (umbrella), `python-pydantic-v2`, `python-sql-model`.` | `Available features: `python` (umbrella), `python-pydantic`, `python-sql-model`.` |

- [ ] **Step 13: Verify build + tests pass**

```bash
cd /repos/bo4e-cli
cargo build --workspace 2>&1 | tail -3
cargo test --workspace 2>&1 | tail -10
```

Expected: build succeeds, every test binary reports `test result: ok`.

- [ ] **Step 14: Verify zero v2 references in code/config**

```bash
cd /repos/bo4e-cli
grep -rn "python-pydantic-v2\|PythonPydanticV2\|pydantic_v2\|map_pydantic_v2\|generate_pydantic_v2" \
  --include="*.rs" --include="*.toml" --include="README.md" --include="*.jinja2"
```

Expected: zero matches (all occurrences moved into `docs/plans/2026-05-08-*` historical docs, which `grep` skips since none match the `--include` filters).

- [ ] **Step 15: Verify slim install works under the new feature name**

```bash
cd /repos/bo4e-cli
cargo install --path crates/bo4e-cli --no-default-features --features python-pydantic --force --locked 2>&1 | tail -3
~/.cargo/bin/bo4e generate --help 2>&1 | grep -A1 'output-type'
```

Expected install output: `Installed package` summary. Expected `--help` output: `--output-type` line shows `[possible values: python-pydantic]` (only the compiled-in variant).

- [ ] **Step 16: Commit**

```bash
git add crates/bo4e-codegen/src/python/mod.rs crates/bo4e-codegen/src/python/pydantic.rs \
        crates/bo4e-codegen/src/python/types.rs crates/bo4e-codegen/src/lib.rs \
        crates/bo4e-codegen/src/env.rs \
        crates/bo4e-codegen/src/templates/python/pydantic \
        crates/bo4e-codegen/tests/integration_pydantic.rs \
        crates/bo4e-codegen/tests/parity_pydantic.rs \
        crates/bo4e-codegen/tests/skeleton.rs \
        crates/bo4e-cli/tests/generate_smoke.rs \
        README.md
git commit -m "refactor(codegen): rename pydantic_v2 module/templates to pydantic

Completes the python-pydantic-v2 → python-pydantic rename:
modules (pydantic_v2.rs → pydantic.rs), template directory
(templates/python/pydantic_v2/ → templates/python/pydantic/),
exported function (generate_pydantic_v2 → generate_pydantic),
type-mapper function (map_pydantic_v2 → map_pydantic),
test files, and README mentions.

Slim install verified: cargo install ... --features python-pydantic
produces a binary whose --output-type accepts only python-pydantic.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 4: Add `bo4e_sql_min` test fixture

**Goal:** Vendor a minimal synthesised JSON-Schema fixture under `crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/` that exercises every distinct `SqlField` variant exactly once. The fixture is consumed by sql-model unit + integration tests *and* by the existing pydantic integration test (cross-fixture coverage in Task 13).

**Files:**
- Create: `crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/.version`
- Create: `crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/bo/Angebot.json`
- Create: `crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/com/Adresse.json`
- Create: `crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/enum/Typ.json`

- [ ] **Step 1: Create the version sentinel**

```bash
mkdir -p /repos/bo4e-cli/crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/{bo,com,enum}
printf '202401.4.0' > /repos/bo4e-cli/crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/.version
```

- [ ] **Step 2: Create `enum/Typ.json`**

```json
{
  "title": "Typ",
  "type": "string",
  "enum": ["ANGEBOT", "VERTRAG"],
  "description": "Geschäftsobjekt-Typ."
}
```

Save to `crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/enum/Typ.json`.

- [ ] **Step 3: Create `com/Adresse.json`**

```json
{
  "title": "Adresse",
  "description": "Postal address (trimmed for sql-model fixture).",
  "additionalProperties": true,
  "type": "object",
  "required": [],
  "properties": {
    "_id": {
      "title": " Id",
      "default": null,
      "anyOf": [
        { "type": "string" },
        { "type": "null" }
      ]
    },
    "ort": {
      "title": "Ort",
      "type": "string"
    }
  }
}
```

Save to `crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/com/Adresse.json`.

- [ ] **Step 4: Create `bo/Angebot.json`**

This BO covers every `SqlField` variant: a 1:1 nullable reference (`adresse`), an M:N reference (`adressen`), an enum reference with default (`_typ`), a nullable scalar (`angebotsnummer`), a nullable datetime (`angebotsdatum`), a non-nullable scalar array (`werte`), a nullable Any (`extras`), a non-nullable list-of-Any (`anhaenge`).

```json
{
  "title": "Angebot",
  "description": "Angebot fixture exercising every SqlField variant.",
  "additionalProperties": true,
  "type": "object",
  "required": ["werte", "anhaenge"],
  "properties": {
    "_id": {
      "title": " Id",
      "default": null,
      "anyOf": [
        { "type": "string" },
        { "type": "null" }
      ]
    },
    "_typ": {
      "default": "ANGEBOT",
      "anyOf": [
        { "$ref": "../enum/Typ.json#" },
        { "type": "null" }
      ]
    },
    "adresse": {
      "default": null,
      "anyOf": [
        { "$ref": "../com/Adresse.json#" },
        { "type": "null" }
      ]
    },
    "adressen": {
      "type": "array",
      "items": { "$ref": "../com/Adresse.json#" }
    },
    "angebotsnummer": {
      "title": "Angebotsnummer",
      "default": null,
      "anyOf": [
        { "type": "string" },
        { "type": "null" }
      ]
    },
    "angebotsdatum": {
      "title": "Angebotsdatum",
      "default": null,
      "anyOf": [
        { "type": "string", "format": "date-time" },
        { "type": "null" }
      ]
    },
    "werte": {
      "type": "array",
      "items": {
        "type": "number",
        "format": "decimal"
      }
    },
    "extras": {
      "default": null,
      "anyOf": [
        {},
        { "type": "null" }
      ]
    },
    "anhaenge": {
      "type": "array",
      "items": {}
    }
  }
}
```

Save to `crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/bo/Angebot.json`.

- [ ] **Step 5: Verify the fixture loads via the existing schema reader**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-schemas read_schemas 2>&1 | tail -5
```

Expected: `test result: ok` from the `bo4e-schemas` crate. If new test failures surface (the schema reader can't deserialise the fixture), capture the error message — it indicates a `bo4e-schemas` deserialisation gap that must be flagged separately (per spec "Out of Scope: bo4e-schemas serde-deserialisation gaps"). If a gap blocks loading, simplify the fixture (remove the offending field) and file a follow-up issue, but proceed — no other fixture is blocked by this one.

- [ ] **Step 6: Commit**

```bash
git add crates/bo4e-codegen/tests/fixtures/bo4e_sql_min
git commit -m "test(codegen): add bo4e_sql_min fixture covering every SqlField variant

Synthesised fixture (not from upstream bo4e_rel_refs) sized to be the
smallest input that exercises 1:1 reference, M:N reference, enum
reference with default, nullable scalar, scalar array, Any, and
list[Any]. Reused by both the sql-model integration test (Task 12)
and a cross-fixture pydantic check (Task 13).

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 5: Scaffold `sql_model` module + `SqlPlan` data types

**Goal:** Lay down the empty `sql_model::mod` and `sql_model::plan` modules with all type definitions from the spec. No build behaviour, no rendering — just the data types so subsequent tasks can fill in `build_plan` and the renderer.

**Files:**
- Modify: `crates/bo4e-codegen/src/python/mod.rs`
- Create: `crates/bo4e-codegen/src/python/sql_model/mod.rs`
- Create: `crates/bo4e-codegen/src/python/sql_model/plan.rs`

- [ ] **Step 1: Update `crates/bo4e-codegen/src/python/mod.rs`**

Add the new sub-module behind its feature flag. Replace the file with:

```rust
pub(crate) mod imports;
pub(crate) mod types;

#[cfg(feature = "python-pydantic")]
pub(crate) mod pydantic;

#[cfg(feature = "python-sql-model")]
pub(crate) mod sql_model;
```

- [ ] **Step 2: Create `crates/bo4e-codegen/src/python/sql_model/mod.rs`**

```rust
//! python-sql-model generator orchestration.
//!
//! Two-phase: a pure pre-pass walks `Schemas` and produces an immutable
//! [`plan::SqlPlan`]; a render pass consumes the plan and writes Python files
//! via vendored MiniJinja templates.

pub(crate) mod plan;

// Re-export the entry point so lib.rs can call into us via `python::sql_model::generate_sql_model`.
// Filled in by Task 11.
#[allow(unused_imports)]
pub(crate) use plan::SqlPlan;
```

- [ ] **Step 3: Create `crates/bo4e-codegen/src/python/sql_model/plan.rs`**

```rust
//! SQL-model build plan: an immutable description of all tables, fields, and junctions
//! produced by walking a [`bo4e_schemas::Schemas`].
//!
//! `build_plan` is pure — it has no side effects and writes no files. The renderer in
//! [`super`] consumes the plan and produces source.

#![allow(dead_code)] // Filled in across Tasks 6, 7. Wired up in Task 11.

use std::collections::BTreeMap;

/// All tables and junctions produced by the pre-pass.
#[derive(Debug)]
pub(crate) struct SqlPlan {
    /// All BO/COM/enum tables, keyed by their module path
    /// (e.g. `["bo", "Angebot"]` matching `bo4e_schemas::models::schema_meta::Schema::module`).
    pub(crate) tables: BTreeMap<Vec<String>, TablePlan>,
    /// All M:N junction tables that need to land in `<output>/many.py`.
    pub(crate) junctions: Vec<JunctionTable>,
}

#[derive(Debug)]
pub(crate) struct TablePlan {
    /// Same module-path key as in `SqlPlan.tables` (e.g. `["bo", "Angebot"]`).
    pub(crate) module: Vec<String>,
    pub(crate) class_name: String,
    pub(crate) is_enum: bool,
    /// Schema-level `description`, used for the class docstring.
    pub(crate) description: Option<String>,
    /// For enum tables, the StrEnum members. Empty for object tables.
    pub(crate) enum_members: Vec<String>,
    /// For object tables, the fields in JSON-property insertion order. Empty for enum tables.
    pub(crate) sql_fields: Vec<SqlField>,
}

/// One field on a `TablePlan`. The pre-pass classifies every JSON property into
/// exactly one of these variants.
#[derive(Debug)]
pub(crate) enum SqlField {
    /// Plain scalar; renders as `name: type_ = Field(default=...)`.
    Scalar {
        name: String,
        /// Type expression as it appears inline (already includes `| None` for nullable).
        type_: String,
        nullable: bool,
        /// Already-quoted Python default expression, or `None` for required.
        default: Option<String>,
        title: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>_id: UUID = Field(default=None, foreign_key="adresse.id")`.
    /// Sibling of a `Relationship` entry that follows immediately in `sql_fields`.
    ForeignKey {
        /// The FK column name, already `_id`-suffixed (e.g. `"adresse_id"`).
        name: String,
        target_class: String,
        target_table: String,
        nullable: bool,
        /// `Some("SET NULL")` when nullable, `None` when required.
        ondelete: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>: Adresse | None = Relationship(sa_relationship_kwargs={...})`.
    /// Sibling of a `ForeignKey` entry that precedes it in `sql_fields`.
    Relationship {
        name: String,
        target_class: String,
        owner_class: String,
        /// The matching FK field name on the owner (`"adresse_id"`).
        fk_field_name: String,
        nullable: bool,
        docstring: Option<String>,
    },
    /// `<name>: list[Adresse] = Relationship(link_model=AngebotAdressenLink)`.
    /// The junction class is appended to `SqlPlan.junctions`.
    ManyRelationship {
        name: String,
        target_class: String,
        link_class: String,
        nullable: bool,
        docstring: Option<String>,
    },
    /// `<name>: Typ | None = Field(default=Typ.ANGEBOT, sa_column=Column(Enum(Typ, name="typ")))`.
    EnumColumn {
        name: String,
        enum_class: String,
        is_list: bool,
        nullable: bool,
        /// e.g. `Some("Typ.ANGEBOT")` or `None`.
        default: Option<String>,
        title: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>: list[Decimal] = Field(sa_column=Column(ARRAY(Numeric)))`.
    ScalarArray {
        name: String,
        py_inner: String,
        sa_inner: &'static str,
        nullable: bool,
        title: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>: Any | None = Field(sa_column=Column(PickleType, nullable=True))`.
    /// Or with `ARRAY(PickleType)` when `is_array`.
    AnyColumn {
        name: String,
        is_array: bool,
        nullable: bool,
        docstring: Option<String>,
    },
}

#[derive(Debug)]
pub(crate) struct JunctionTable {
    /// Class + lower-cased table name (e.g. `"AngebotAdressenLink"`).
    pub(crate) class_name: String,
    pub(crate) owner_class: String,
    pub(crate) owner_table: String,
    pub(crate) owner_id_field: String,
    pub(crate) target_class: String,
    pub(crate) target_table: String,
    pub(crate) target_id_field: String,
    /// The source field on the owner (diagnostic only — appears in the junction class docstring).
    pub(crate) source_field: String,
}

/// Build the immutable plan from a parsed `Schemas`. Pure — no I/O, no template rendering.
///
/// Filled in across Tasks 6 and 7.
pub(crate) fn build_plan(_schemas: &bo4e_schemas::Schemas) -> SqlPlan {
    SqlPlan {
        tables: BTreeMap::new(),
        junctions: Vec::new(),
    }
}
```

- [ ] **Step 4: Verify the workspace builds with the new feature gate**

```bash
cd /repos/bo4e-cli
cargo build -p bo4e-codegen --features python-sql-model 2>&1 | tail -3
cargo build --workspace 2>&1 | tail -3
```

Expected: both succeed. `dead_code` warnings are silenced by the `#![allow(dead_code)]` at the top of `plan.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/bo4e-codegen/src/python/mod.rs \
        crates/bo4e-codegen/src/python/sql_model
git commit -m "feat(codegen): scaffold sql_model module with SqlPlan types

Lays down the empty sql_model::mod and sql_model::plan modules with
all type definitions (SqlPlan, TablePlan, SqlField with six variants,
JunctionTable). build_plan is a no-op; subsequent tasks fill it in.
No behaviour change for the existing pydantic flavour.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 6: Implement `build_plan` for the simple cases (object scaffolding, id, plain scalars)

**Goal:** `build_plan` walks all schemas, creates `TablePlan` entries with `module`, `class_name`, `description`, and the synthesised `id` field. Plain scalar properties (`str`, `int`, `Decimal`, `datetime`, etc.) are classified as `Scalar`. Skip every non-trivial case (references, arrays, enum refs, Any) for Task 7. Enum schemas produce `TablePlan { is_enum: true, enum_members: [...] }`.

**Files:**
- Modify: `crates/bo4e-codegen/src/python/sql_model/plan.rs`

- [ ] **Step 1: Write the failing tests for `build_plan` basics**

Append to the bottom of `crates/bo4e-codegen/src/python/sql_model/plan.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::{Schema, Schemas};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn fixture_schemas() -> Schemas {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/bo4e_sql_min");
        bo4e_schemas::io::schemas::read_schemas(&path)
            .expect("read bo4e_sql_min")
            .schemas
    }

    fn enum_schema(name: &str, members: &[&str]) -> Schemas {
        let mut s = Schemas::new("v202401.0.0".parse().unwrap());
        let body = format!(
            r#"{{"title":"{name}","type":"string","enum":[{}]}}"#,
            members.iter().map(|m| format!("\"{m}\"")).collect::<Vec<_>>().join(",")
        );
        let mut sch = Schema::new(vec!["enum".into(), name.into()], None).unwrap();
        sch.load_schema(body);
        s.add_schema(Rc::new(RefCell::new(sch))).unwrap();
        s
    }

    #[test]
    fn enum_schema_produces_enum_table_plan() {
        let schemas = enum_schema("Typ", &["ANGEBOT", "VERTRAG"]);
        let plan = build_plan(&schemas);
        let key = vec!["enum".to_string(), "Typ".to_string()];
        let table = plan.tables.get(&key).expect("enum table present");
        assert!(table.is_enum);
        assert_eq!(table.class_name, "Typ");
        assert_eq!(table.enum_members, vec!["ANGEBOT".to_string(), "VERTRAG".to_string()]);
        assert!(table.sql_fields.is_empty());
    }

    #[test]
    fn object_table_synthesises_primary_key_id() {
        let plan = build_plan(&fixture_schemas());
        let angebot = plan.tables.get(&vec!["bo".to_string(), "Angebot".to_string()])
            .expect("Angebot table present");
        // The first SqlField is always the synthesised primary key.
        match &angebot.sql_fields[0] {
            SqlField::Scalar { name, type_, default, .. } => {
                assert_eq!(name, "id");
                assert_eq!(type_, "uuid_pkg.UUID");
                assert_eq!(default.as_deref(), Some("Field(default_factory=uuid_pkg.uuid4, primary_key=True, alias=\"_id\", title=\" Id\")"));
            }
            other => panic!("expected Scalar id field, got {:?}", other),
        }
    }

    #[test]
    fn nullable_scalar_field_emits_none_default() {
        let plan = build_plan(&fixture_schemas());
        let angebot = plan.tables.get(&vec!["bo".to_string(), "Angebot".to_string()]).unwrap();
        let nummer = angebot.sql_fields.iter().find_map(|f| match f {
            SqlField::Scalar { name, type_, nullable, default, .. } if name == "angebotsnummer" => {
                Some((type_.clone(), *nullable, default.clone()))
            }
            _ => None,
        }).expect("angebotsnummer field present");
        assert_eq!(nummer.0, "str | None");
        assert!(nummer.1);
        assert_eq!(nummer.2.as_deref(), Some("None"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model -- --test-threads=1 sql_model::plan 2>&1 | tail -20
```

Expected: all three tests fail with messages like `expected enum table present` / `expected Angebot table present` (because `build_plan` returns an empty plan).

- [ ] **Step 3: Implement `build_plan` for objects + enums + simple scalars**

Replace the `pub(crate) fn build_plan` stub in `crates/bo4e-codegen/src/python/sql_model/plan.rs` with the implementation below. Add the necessary `use` statements at the top of the file (immediately under `use std::collections::BTreeMap;`):

```rust
use crate::naming::to_snake_case;
use crate::python::types::map_pydantic;
use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::{ObjectSchema, PrimitiveValue, SchemaRootType, SchemaType};
```

Replace the `build_plan` stub with:

```rust
pub(crate) fn build_plan(schemas: &Schemas) -> SqlPlan {
    let mut tables: BTreeMap<Vec<String>, TablePlan> = BTreeMap::new();
    let junctions: Vec<JunctionTable> = Vec::new();

    for schema_rc in schemas {
        let mut schema = schema_rc.borrow_mut();
        let module = schema.module().to_vec();
        let class_name = schema.name().to_string();
        let parsed = schema.schema().expect("schema parsed").clone();
        drop(schema);

        let table = match &parsed {
            SchemaRootType::StrEnum(e) => TablePlan {
                module: module.clone(),
                class_name: class_name.clone(),
                is_enum: true,
                description: e.base.description.clone(),
                enum_members: e.str_enum.enum_values.clone(),
                sql_fields: Vec::new(),
            },
            SchemaRootType::Object(o) => {
                let id_field = synth_id_field(&o.object);
                let mut fields = vec![id_field];
                for (prop_name, prop_schema) in o.object.properties.iter() {
                    if prop_name == "_id" {
                        continue; // already synthesised
                    }
                    if let Some(field) = simple_scalar_field(prop_name, prop_schema) {
                        fields.push(field);
                    }
                    // Reference / array / enum / Any cases are added in Task 7.
                }
                TablePlan {
                    module: module.clone(),
                    class_name: class_name.clone(),
                    is_enum: false,
                    description: o.object.base.description.clone(),
                    enum_members: Vec::new(),
                    sql_fields: fields,
                }
            }
        };
        tables.insert(module, table);
    }

    SqlPlan { tables, junctions }
}

/// Build the synthesised `id: uuid_pkg.UUID = Field(default_factory=…, primary_key=True, alias="_id")`.
/// Mirrors `add_id_field` in the Python `sql_parser.py`. Pulls the `_id` schema's `title` if present;
/// falls back to `"Primary key ID-Field"` when no `_id` property exists in the source schema.
fn synth_id_field(obj: &ObjectSchema) -> SqlField {
    let title = obj
        .properties
        .get("_id")
        .and_then(|s| literal_title(s))
        .unwrap_or_else(|| "Primary key ID-Field".to_string());
    let default = format!(
        "Field(default_factory=uuid_pkg.uuid4, primary_key=True, alias=\"_id\", title=\"{title}\")"
    );
    SqlField::Scalar {
        name: "id".to_string(),
        type_: "uuid_pkg.UUID".to_string(),
        nullable: false,
        default: Some(default),
        title: None,
        docstring: Some("The primary key of the table as a UUID4.".to_string()),
    }
}

/// Map a JSON-Schema property to a `SqlField::Scalar` if and only if the type mapper produces
/// a non-reference, non-array, non-Any type. Returns `None` otherwise (caller skips for now;
/// Task 7 wires up the remaining cases).
fn simple_scalar_field(prop_name: &str, schema: &SchemaType) -> Option<SqlField> {
    if !is_simple_scalar(schema) {
        return None;
    }
    let mapped = map_pydantic(schema);
    let nullable = mapped.rendered.contains("| None");
    let type_ = if nullable {
        mapped.rendered.clone()
    } else {
        mapped.rendered.clone()
    };
    let default = if nullable {
        Some(literal_default(schema).unwrap_or_else(|| "None".to_string()))
    } else {
        literal_default(schema)
    };
    Some(SqlField::Scalar {
        name: to_snake_case(prop_name),
        type_,
        nullable,
        default,
        title: literal_title(schema),
        docstring: literal_description(schema),
    })
}

fn is_simple_scalar(schema: &SchemaType) -> bool {
    match schema {
        SchemaType::StringSchema(_)
        | SchemaType::IntegerSchema(_)
        | SchemaType::NumberSchema(_)
        | SchemaType::BooleanSchema(_)
        | SchemaType::DecimalSchema(_)
        | SchemaType::ConstantSchema(_) => true,
        SchemaType::AnyOf(a) => {
            // Optional[<scalar>] only — reject AnyOf containing references / Any / arrays.
            a.any_of.iter().all(|t| matches!(t,
                SchemaType::StringSchema(_)
                | SchemaType::IntegerSchema(_)
                | SchemaType::NumberSchema(_)
                | SchemaType::BooleanSchema(_)
                | SchemaType::DecimalSchema(_)
                | SchemaType::ConstantSchema(_)
                | SchemaType::NullSchema(_)
            ))
        }
        _ => false,
    }
}

fn literal_default(schema: &SchemaType) -> Option<String> {
    let base = schema_base(schema);
    base.default.as_ref().map(|v| match v {
        PrimitiveValue::Null => "None".into(),
        PrimitiveValue::Bool(true) => "True".into(),
        PrimitiveValue::Bool(false) => "False".into(),
        PrimitiveValue::Integer(i) => i.to_string(),
        PrimitiveValue::Float(f) => f.to_string(),
        PrimitiveValue::String(s) => format!("\"{s}\""),
    })
}

fn literal_title(schema: &SchemaType) -> Option<String> {
    schema_base(schema).title.clone()
}

fn literal_description(schema: &SchemaType) -> Option<String> {
    schema_base(schema).description.clone()
}

fn schema_base(schema: &SchemaType) -> &bo4e_schemas::models::json_schema::TypeBase {
    match schema {
        SchemaType::StringSchema(s) => &s.base,
        SchemaType::IntegerSchema(s) => &s.base,
        SchemaType::NumberSchema(s) => &s.base,
        SchemaType::BooleanSchema(s) => &s.base,
        SchemaType::DecimalSchema(s) => &s.base,
        SchemaType::NullSchema(s) => &s.base,
        SchemaType::AnySchema(s) => &s.base,
        SchemaType::Array(s) => &s.base,
        SchemaType::AnyOf(s) => &s.base,
        SchemaType::AllOf(s) => &s.base,
        SchemaType::ConstantSchema(s) => &s.base,
        SchemaType::ReferenceSchema(s) => &s.base,
        SchemaType::Object(s) => &s.base,
        SchemaType::StrEnum(s) => &s.base,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model sql_model::plan 2>&1 | tail -10
```

Expected: all three tests pass.

- [ ] **Step 5: Verify the workspace still builds**

```bash
cd /repos/bo4e-cli
cargo build --workspace 2>&1 | tail -3
cargo test --workspace 2>&1 | tail -10
```

Expected: build succeeds; existing pydantic tests still pass.

- [ ] **Step 6: Commit**

```bash
git add crates/bo4e-codegen/src/python/sql_model/plan.rs
git commit -m "feat(codegen/sql_model): build_plan handles enums + scalar fields

Walks Schemas, creates TablePlan entries, synthesises the primary-key
id field for every object table, and classifies plain scalars (incl.
Optional<scalar>) into SqlField::Scalar. References, arrays, enum
references, and Any are skipped here and added in the next commit.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 7: Implement `build_plan` for references, arrays, enum refs, and Any

**Goal:** Extend `build_plan` to classify every remaining JSON-property variant into the right `SqlField`: 1:1 reference (FK + Relationship pair), M:N reference (ManyRelationship + JunctionTable), enum reference (EnumColumn), scalar array (ScalarArray), and Any / list[Any] (AnyColumn).

**Files:**
- Modify: `crates/bo4e-codegen/src/python/sql_model/plan.rs`

- [ ] **Step 1: Append failing tests for the remaining `SqlField` variants**

Add to the `tests` module at the bottom of `plan.rs`:

```rust
    fn angebot_table(plan: &SqlPlan) -> &TablePlan {
        plan.tables.get(&vec!["bo".to_string(), "Angebot".to_string()])
            .expect("Angebot table present")
    }

    #[test]
    fn one_to_one_reference_emits_fk_then_relationship() {
        let plan = build_plan(&fixture_schemas());
        let angebot = angebot_table(&plan);

        let fk_idx = angebot.sql_fields.iter().position(|f| matches!(f,
            SqlField::ForeignKey { name, .. } if name == "adresse_id"
        )).expect("adresse_id FK present");
        let rel_idx = angebot.sql_fields.iter().position(|f| matches!(f,
            SqlField::Relationship { name, .. } if name == "adresse"
        )).expect("adresse Relationship present");

        // FK must come immediately before its Relationship sibling.
        assert_eq!(rel_idx, fk_idx + 1, "Relationship must follow FK directly");

        match &angebot.sql_fields[fk_idx] {
            SqlField::ForeignKey { target_class, target_table, nullable, ondelete, .. } => {
                assert_eq!(target_class, "Adresse");
                assert_eq!(target_table, "adresse");
                assert!(*nullable);
                assert_eq!(ondelete.as_deref(), Some("SET NULL"));
            }
            _ => unreachable!(),
        }
        match &angebot.sql_fields[rel_idx] {
            SqlField::Relationship { target_class, owner_class, fk_field_name, nullable, .. } => {
                assert_eq!(target_class, "Adresse");
                assert_eq!(owner_class, "Angebot");
                assert_eq!(fk_field_name, "adresse_id");
                assert!(*nullable);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn many_reference_emits_many_relationship_and_junction() {
        let plan = build_plan(&fixture_schemas());
        let angebot = angebot_table(&plan);

        let many = angebot.sql_fields.iter().find_map(|f| match f {
            SqlField::ManyRelationship { name, target_class, link_class, nullable, .. }
                if name == "adressen" => Some((target_class.clone(), link_class.clone(), *nullable)),
            _ => None,
        }).expect("adressen ManyRelationship present");
        assert_eq!(many.0, "Adresse");
        assert_eq!(many.1, "AngebotAdressenLink");
        assert!(!many.2, "list[Reference] without Optional should not be nullable");

        let junction = plan.junctions.iter().find(|j| j.class_name == "AngebotAdressenLink")
            .expect("AngebotAdressenLink junction present");
        assert_eq!(junction.owner_class, "Angebot");
        assert_eq!(junction.owner_table, "angebot");
        assert_eq!(junction.owner_id_field, "angebot_id");
        assert_eq!(junction.target_class, "Adresse");
        assert_eq!(junction.target_table, "adresse");
        assert_eq!(junction.target_id_field, "adresse_id");
        assert_eq!(junction.source_field, "adressen");
    }

    #[test]
    fn enum_reference_with_default_emits_enum_column() {
        let plan = build_plan(&fixture_schemas());
        let angebot = angebot_table(&plan);
        let typ = angebot.sql_fields.iter().find_map(|f| match f {
            SqlField::EnumColumn { name, enum_class, is_list, nullable, default, .. }
                if name == "_typ" => Some((enum_class.clone(), *is_list, *nullable, default.clone())),
            _ => None,
        }).expect("_typ EnumColumn present");
        assert_eq!(typ.0, "Typ");
        assert!(!typ.1);
        assert!(typ.2);
        assert_eq!(typ.3.as_deref(), Some("Typ.ANGEBOT"));
    }

    #[test]
    fn scalar_array_of_decimal_emits_scalar_array() {
        let plan = build_plan(&fixture_schemas());
        let angebot = angebot_table(&plan);
        let werte = angebot.sql_fields.iter().find_map(|f| match f {
            SqlField::ScalarArray { name, py_inner, sa_inner, nullable, .. } if name == "werte" => {
                Some((py_inner.clone(), *sa_inner, *nullable))
            }
            _ => None,
        }).expect("werte ScalarArray present");
        assert_eq!(werte.0, "Decimal");
        assert_eq!(werte.1, "Numeric");
        assert!(!werte.2);
    }

    #[test]
    fn any_field_emits_any_column() {
        let plan = build_plan(&fixture_schemas());
        let angebot = angebot_table(&plan);
        let extras = angebot.sql_fields.iter().find_map(|f| match f {
            SqlField::AnyColumn { name, is_array, nullable, .. } if name == "extras" => {
                Some((*is_array, *nullable))
            }
            _ => None,
        }).expect("extras AnyColumn present");
        assert!(!extras.0);
        assert!(extras.1);

        let anhaenge = angebot.sql_fields.iter().find_map(|f| match f {
            SqlField::AnyColumn { name, is_array, nullable, .. } if name == "anhaenge" => {
                Some((*is_array, *nullable))
            }
            _ => None,
        }).expect("anhaenge AnyColumn present");
        assert!(anhaenge.0);
        assert!(!anhaenge.1);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model sql_model::plan 2>&1 | tail -20
```

Expected: the five new tests fail (the simple-scalar tests still pass).

- [ ] **Step 3: Implement reference / array / enum-ref / Any classification**

Inside `crates/bo4e-codegen/src/python/sql_model/plan.rs`, replace the inner property loop in `build_plan` with the dispatch below. The loop now calls into a new `classify_property` helper that returns one or two `SqlField`s plus an optional `JunctionTable`. Update the `for` body inside the `SchemaRootType::Object(o)` arm:

```rust
                let id_field = synth_id_field(&o.object);
                let mut fields = vec![id_field];
                let mut local_junctions: Vec<JunctionTable> = Vec::new();
                for (prop_name, prop_schema) in o.object.properties.iter() {
                    if prop_name == "_id" {
                        continue;
                    }
                    if is_simple_scalar(prop_schema) {
                        if let Some(field) = simple_scalar_field(prop_name, prop_schema) {
                            fields.push(field);
                        }
                        continue;
                    }
                    classify_property(
                        &class_name,
                        prop_name,
                        prop_schema,
                        schemas,
                        &mut fields,
                        &mut local_junctions,
                    );
                }
                let plan_table = TablePlan {
                    module: module.clone(),
                    class_name: class_name.clone(),
                    is_enum: false,
                    description: o.object.base.description.clone(),
                    enum_members: Vec::new(),
                    sql_fields: fields,
                };
                tables.insert(module, plan_table);
                junction_buf.extend(local_junctions);
```

Update the `build_plan` outer scope to own `junction_buf`:

```rust
pub(crate) fn build_plan(schemas: &Schemas) -> SqlPlan {
    let mut tables: BTreeMap<Vec<String>, TablePlan> = BTreeMap::new();
    let mut junction_buf: Vec<JunctionTable> = Vec::new();

    for schema_rc in schemas {
        // ... (loop body as above; replace the whole `let table = match` with the inline insert)
    }

    SqlPlan { tables, junctions: junction_buf }
}
```

Delete the now-unused `let table = match` form for the `Object` arm (replaced by the inline insert above). The `StrEnum` arm stays as-is but is moved into the loop alongside the object insert:

```rust
        match &parsed {
            SchemaRootType::StrEnum(e) => {
                tables.insert(module.clone(), TablePlan {
                    module: module.clone(),
                    class_name: class_name.clone(),
                    is_enum: true,
                    description: e.base.description.clone(),
                    enum_members: e.str_enum.enum_values.clone(),
                    sql_fields: Vec::new(),
                });
            }
            SchemaRootType::Object(o) => {
                // ... inline insert as above ...
            }
        }
```

Add the `classify_property` helper below `simple_scalar_field`:

```rust
/// Classify a non-simple-scalar JSON-Schema property and push the resulting
/// `SqlField`(s) (and any `JunctionTable`) onto the buffers. Mirrors the
/// `match` ladder in the Python `adapt_parse_for_sql_model`.
fn classify_property(
    owner_class: &str,
    prop_name: &str,
    schema: &SchemaType,
    all_schemas: &Schemas,
    fields: &mut Vec<SqlField>,
    junctions: &mut Vec<JunctionTable>,
) {
    let snake = to_snake_case(prop_name);
    let docstring = literal_description(schema);

    // Direct $ref (no nullable wrapper).
    if let SchemaType::ReferenceSchema(r) = schema {
        if let Some(target) = ref_target_class(&r.r#ref) {
            if is_enum_ref(&target, all_schemas) {
                fields.push(SqlField::EnumColumn {
                    name: snake,
                    enum_class: target,
                    is_list: false,
                    nullable: false,
                    default: None,
                    title: literal_title(schema),
                    docstring,
                });
            } else {
                push_one_to_one(owner_class, &snake, &target, false, fields);
            }
            return;
        }
    }

    // list[$ref], list[<scalar>], list[Any].
    if let SchemaType::Array(a) = schema {
        match &*a.items {
            SchemaType::ReferenceSchema(r) => {
                if let Some(target) = ref_target_class(&r.r#ref) {
                    if is_enum_ref(&target, all_schemas) {
                        fields.push(SqlField::EnumColumn {
                            name: snake,
                            enum_class: target,
                            is_list: true,
                            nullable: false,
                            default: None,
                            title: literal_title(schema),
                            docstring,
                        });
                    } else {
                        push_many_to_many(owner_class, &snake, &target, false, prop_name, fields, junctions);
                    }
                    return;
                }
            }
            SchemaType::AnySchema(_) => {
                fields.push(SqlField::AnyColumn { name: snake, is_array: true, nullable: false, docstring });
                return;
            }
            inner if matches!(inner,
                SchemaType::StringSchema(_) | SchemaType::IntegerSchema(_) | SchemaType::NumberSchema(_)
                | SchemaType::BooleanSchema(_) | SchemaType::DecimalSchema(_)
            ) => {
                let (py_inner, sa_inner) = scalar_array_inners(inner);
                fields.push(SqlField::ScalarArray {
                    name: snake,
                    py_inner,
                    sa_inner,
                    nullable: false,
                    title: literal_title(schema),
                    docstring,
                });
                return;
            }
            _ => {}
        }
    }

    // Plain Any (no nullable wrapper).
    if let SchemaType::AnySchema(_) = schema {
        fields.push(SqlField::AnyColumn { name: snake, is_array: false, nullable: false, docstring });
        return;
    }

    // AnyOf with one Null branch — i.e. Optional[<inner>].
    if let SchemaType::AnyOf(a) = schema {
        let nulls = a.any_of.iter().filter(|t| matches!(t, SchemaType::NullSchema(_))).count();
        if nulls == 1 && a.any_of.len() == 2 {
            let inner = a.any_of.iter().find(|t| !matches!(t, SchemaType::NullSchema(_))).unwrap();
            classify_optional(owner_class, prop_name, &snake, inner, schema, all_schemas, fields, junctions);
            return;
        }
    }
    // Anything else falls through silently — Task 7 covers the cases the spec
    // commits to; unsupported shapes are out of scope (see spec).
}

fn classify_optional(
    owner_class: &str,
    prop_name: &str,
    snake: &str,
    inner: &SchemaType,
    full_schema: &SchemaType,
    all_schemas: &Schemas,
    fields: &mut Vec<SqlField>,
    junctions: &mut Vec<JunctionTable>,
) {
    let docstring = literal_description(full_schema);
    let title = literal_title(full_schema);

    match inner {
        SchemaType::ReferenceSchema(r) => {
            if let Some(target) = ref_target_class(&r.r#ref) {
                if is_enum_ref(&target, all_schemas) {
                    let default = literal_default(full_schema).and_then(|d| {
                        if d == "None" {
                            None
                        } else {
                            // Strip the leading/trailing quotes from a JSON-string default.
                            let trimmed = d.trim_matches('"').to_string();
                            Some(format!("{target}.{trimmed}"))
                        }
                    });
                    fields.push(SqlField::EnumColumn {
                        name: snake.to_string(),
                        enum_class: target,
                        is_list: false,
                        nullable: true,
                        default,
                        title,
                        docstring,
                    });
                } else {
                    push_one_to_one(owner_class, snake, &target, true, fields);
                }
            }
        }
        SchemaType::Array(a) => match &*a.items {
            SchemaType::ReferenceSchema(r) => {
                if let Some(target) = ref_target_class(&r.r#ref) {
                    if is_enum_ref(&target, all_schemas) {
                        fields.push(SqlField::EnumColumn {
                            name: snake.to_string(),
                            enum_class: target,
                            is_list: true,
                            nullable: true,
                            default: None,
                            title,
                            docstring,
                        });
                    } else {
                        push_many_to_many(owner_class, snake, &target, true, prop_name, fields, junctions);
                    }
                }
            }
            SchemaType::AnySchema(_) => {
                fields.push(SqlField::AnyColumn { name: snake.to_string(), is_array: true, nullable: true, docstring });
            }
            other if matches!(other,
                SchemaType::StringSchema(_) | SchemaType::IntegerSchema(_) | SchemaType::NumberSchema(_)
                | SchemaType::BooleanSchema(_) | SchemaType::DecimalSchema(_)
            ) => {
                let (py_inner, sa_inner) = scalar_array_inners(other);
                fields.push(SqlField::ScalarArray {
                    name: snake.to_string(),
                    py_inner,
                    sa_inner,
                    nullable: true,
                    title,
                    docstring,
                });
            }
            _ => {}
        },
        SchemaType::AnySchema(_) => {
            fields.push(SqlField::AnyColumn { name: snake.to_string(), is_array: false, nullable: true, docstring });
        }
        _ => {}
    }
}

fn push_one_to_one(
    owner_class: &str,
    snake: &str,
    target_class: &str,
    nullable: bool,
    fields: &mut Vec<SqlField>,
) {
    let fk_name = format!("{snake}_id");
    let target_table = target_class.to_ascii_lowercase();
    fields.push(SqlField::ForeignKey {
        name: fk_name.clone(),
        target_class: target_class.to_string(),
        target_table,
        nullable,
        ondelete: if nullable { Some("SET NULL".to_string()) } else { None },
        docstring: Some(format!(
            "The id to implement the relationship (field {snake} references {target_class})."
        )),
    });
    fields.push(SqlField::Relationship {
        name: snake.to_string(),
        target_class: target_class.to_string(),
        owner_class: owner_class.to_string(),
        fk_field_name: fk_name,
        nullable,
        docstring: None,
    });
}

fn push_many_to_many(
    owner_class: &str,
    snake: &str,
    target_class: &str,
    nullable: bool,
    source_field: &str,
    fields: &mut Vec<SqlField>,
    junctions: &mut Vec<JunctionTable>,
) {
    let pascal_field = pascal_case(snake);
    let link_class = format!("{owner_class}{pascal_field}Link");
    fields.push(SqlField::ManyRelationship {
        name: snake.to_string(),
        target_class: target_class.to_string(),
        link_class: link_class.clone(),
        nullable,
        docstring: None,
    });
    let owner_table = owner_class.to_ascii_lowercase();
    let target_table = target_class.to_ascii_lowercase();
    junctions.push(JunctionTable {
        class_name: link_class,
        owner_class: owner_class.to_string(),
        owner_table: owner_table.clone(),
        owner_id_field: format!("{owner_table}_id"),
        target_class: target_class.to_string(),
        target_table: target_table.clone(),
        target_id_field: format!("{target_table}_id"),
        source_field: source_field.to_string(),
    });
}

fn scalar_array_inners(inner: &SchemaType) -> (String, &'static str) {
    match inner {
        SchemaType::StringSchema(_) => ("str".into(), "String"),
        SchemaType::IntegerSchema(_) => ("int".into(), "Integer"),
        SchemaType::NumberSchema(_) => ("float".into(), "Float"),
        SchemaType::BooleanSchema(_) => ("bool".into(), "Boolean"),
        SchemaType::DecimalSchema(_) => ("Decimal".into(), "Numeric"),
        _ => unreachable!("scalar_array_inners called with non-scalar inner"),
    }
}

fn ref_target_class(ref_str: &str) -> Option<String> {
    // "../com/Adresse.json#" → "Adresse"
    let path = ref_str.split('#').next().unwrap_or(ref_str);
    let last = path.rsplit('/').next()?;
    last.strip_suffix(".json").map(|s| s.to_string())
}

fn is_enum_ref(target_class: &str, all_schemas: &Schemas) -> bool {
    for schema_rc in all_schemas {
        let mut s = schema_rc.borrow_mut();
        if s.name() != target_class {
            continue;
        }
        return matches!(s.schema(), Ok(SchemaRootType::StrEnum(_)));
    }
    false
}

/// snake_case → PascalCase. `"adressen"` → `"Adressen"`; `"liefer_adressen"` → `"LieferAdressen"`.
fn pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect()
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model sql_model::plan 2>&1 | tail -10
```

Expected: all eight tests pass (3 from Task 6 + 5 new).

- [ ] **Step 5: Verify the workspace still builds and tests pass**

```bash
cd /repos/bo4e-cli
cargo test --workspace 2>&1 | tail -10
```

Expected: every test binary reports `test result: ok`.

- [ ] **Step 6: Commit**

```bash
git add crates/bo4e-codegen/src/python/sql_model/plan.rs
git commit -m "feat(codegen/sql_model): build_plan classifies refs/arrays/enums/Any

Adds the dispatch from JSON Schema property to SqlField for the six
remaining variants: 1:1 reference (FK + Relationship pair), M:N
reference (ManyRelationship + JunctionTable), enum reference with
default, scalar array (with SQLAlchemy type lookup), Any, and
list[Any]. Mirrors the case ladder in the Python sql_parser.py.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 8: Vendor sql-model templates and register them in `env.rs`

**Goal:** Vendor the four upstream Jinja2 templates (`BaseModel.jinja2`, `Config.jinja2`, `Enum.jinja2`, `ManyLinks.jinja2`) byte-identical from `/tmp/bo4e-cli-python/src/bo4e_cli/generate/python/custom_templates/`, plus author one new `__init__.jinja2`. Register all five via `include_str!` in `env.rs`.

**Files:**
- Create: `crates/bo4e-codegen/src/templates/python/sql_model/BaseModel.jinja2`
- Create: `crates/bo4e-codegen/src/templates/python/sql_model/Config.jinja2`
- Create: `crates/bo4e-codegen/src/templates/python/sql_model/Enum.jinja2`
- Create: `crates/bo4e-codegen/src/templates/python/sql_model/ManyLinks.jinja2`
- Create: `crates/bo4e-codegen/src/templates/python/sql_model/__init__.jinja2`
- Modify: `crates/bo4e-codegen/src/env.rs`

- [ ] **Step 1: Vendor the four upstream templates byte-identical**

```bash
cd /repos/bo4e-cli
mkdir -p crates/bo4e-codegen/src/templates/python/sql_model
cp /tmp/bo4e-cli-python/src/bo4e_cli/generate/python/custom_templates/BaseModel.jinja2 \
   crates/bo4e-codegen/src/templates/python/sql_model/BaseModel.jinja2
cp /tmp/bo4e-cli-python/src/bo4e_cli/generate/python/custom_templates/Config.jinja2 \
   crates/bo4e-codegen/src/templates/python/sql_model/Config.jinja2
cp /tmp/bo4e-cli-python/src/bo4e_cli/generate/python/custom_templates/Enum.jinja2 \
   crates/bo4e-codegen/src/templates/python/sql_model/Enum.jinja2
cp /tmp/bo4e-cli-python/src/bo4e_cli/generate/python/custom_templates/ManyLinks.jinja2 \
   crates/bo4e-codegen/src/templates/python/sql_model/ManyLinks.jinja2
```

Verify the copies are byte-identical:

```bash
diff -q /tmp/bo4e-cli-python/src/bo4e_cli/generate/python/custom_templates/ \
        crates/bo4e-codegen/src/templates/python/sql_model/ \
   | grep -v "Only in.*sql_model.*__init__"
```

Expected: no output (byte-identical for the four vendored files; `__init__.jinja2` only exists on the Rust side and is filtered out).

- [ ] **Step 2: Author `__init__.jinja2`**

Same shape as the pydantic flavour's `__init__.jinja2`. Save to `crates/bo4e-codegen/src/templates/python/sql_model/__init__.jinja2`:

```jinja
{% for cls in classes -%}
from .{{ cls.module_path | join('.') }} import {{ cls.name }}
{% endfor -%}
{% for link in links -%}
from .many import {{ link }}
{% endfor %}
```

(`links` is rendered separately so the orchestrator can pass an empty list when there are no junctions.)

- [ ] **Step 3: Register all five templates in `crates/bo4e-codegen/src/env.rs`**

Inside `load_embedded`, append a new `#[cfg(feature = "python-sql-model")]` block alongside the existing pydantic block. The function becomes:

```rust
#[allow(unused_variables)]
fn load_embedded(env: &mut minijinja::Environment<'static>) -> Result<(), Error> {
    #[cfg(feature = "python-pydantic")]
    {
        env.add_template(
            "python/pydantic/BaseModel.jinja2",
            include_str!("templates/python/pydantic/BaseModel.jinja2"),
        )?;
        env.add_template(
            "python/pydantic/Enum.jinja2",
            include_str!("templates/python/pydantic/Enum.jinja2"),
        )?;
        env.add_template(
            "python/pydantic/__init__.jinja2",
            include_str!("templates/python/pydantic/__init__.jinja2"),
        )?;
    }

    #[cfg(feature = "python-sql-model")]
    {
        env.add_template(
            "python/sql_model/BaseModel.jinja2",
            include_str!("templates/python/sql_model/BaseModel.jinja2"),
        )?;
        env.add_template(
            "python/sql_model/Config.jinja2",
            include_str!("templates/python/sql_model/Config.jinja2"),
        )?;
        env.add_template(
            "python/sql_model/Enum.jinja2",
            include_str!("templates/python/sql_model/Enum.jinja2"),
        )?;
        env.add_template(
            "python/sql_model/ManyLinks.jinja2",
            include_str!("templates/python/sql_model/ManyLinks.jinja2"),
        )?;
        env.add_template(
            "python/sql_model/__init__.jinja2",
            include_str!("templates/python/sql_model/__init__.jinja2"),
        )?;
    }

    Ok(())
}
```

- [ ] **Step 4: Add a render-smoke unit test for each template**

Append inside the existing `#[cfg(test)] mod tests` block in `env.rs`:

```rust
    #[cfg(feature = "python-sql-model")]
    #[test]
    fn embedded_sql_model_many_links_template_renders() {
        let env = make_environment(None).expect("env builds");
        let tpl = env.get_template("python/sql_model/ManyLinks.jinja2").unwrap();
        let out = tpl.render(context! {
            links => vec![context! {
                table_name => "AngebotAdressenLink",
                cls1 => "Angebot",
                cls2 => "Adresse",
                rel_field_name1 => "adressen",
                id_field_name1 => "angebot_id",
                id_field_name2 => "adresse_id",
            }]
        }).unwrap();
        assert!(out.contains("class AngebotAdressenLink(SQLModel, table=True):"), "got: {out}");
        assert!(out.contains("angebot_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"angebot.id\""), "got: {out}");
        assert!(out.contains("adresse_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"adresse.id\""), "got: {out}");
    }

    #[cfg(feature = "python-sql-model")]
    #[test]
    fn embedded_sql_model_init_template_renders() {
        let env = make_environment(None).expect("env builds");
        let tpl = env.get_template("python/sql_model/__init__.jinja2").unwrap();
        let out = tpl.render(context! {
            classes => vec![context!{ name => "Angebot", module_path => vec!["bo", "angebot"] }],
            links => vec!["AngebotAdressenLink"],
        }).unwrap();
        assert!(out.contains("from .bo.angebot import Angebot"));
        assert!(out.contains("from .many import AngebotAdressenLink"));
    }
```

- [ ] **Step 5: Run the new tests + the full workspace**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model 2>&1 | tail -10
cargo test --workspace 2>&1 | tail -5
```

Expected: the new tests pass; existing tests remain green.

- [ ] **Step 6: Commit**

```bash
git add crates/bo4e-codegen/src/templates/python/sql_model \
        crates/bo4e-codegen/src/env.rs
git commit -m "feat(codegen/sql_model): vendor templates + register in env

Vendors BaseModel, Config, Enum, ManyLinks byte-identical from the
upstream Python custom_templates; authors __init__.jinja2 (the Python
generator builds __init__.py from a helper, not a template). All five
registered via include_str! in env.rs; render-smoke tests for
ManyLinks and __init__ assert the templates work end-to-end.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 9: Render an SQL class file (object → `<output>/<sub>/<name>.py`)

**Goal:** Implement a `render_table` function in `sql_model::mod` that takes a `TablePlan`, builds the SQL imports + per-field `(annotation, definition, description)` triples, and renders `BaseModel.jinja2` using its `SQL=true`-style context. The function returns a `String` (the file body); writing to disk happens in Task 11.

**Files:**
- Modify: `crates/bo4e-codegen/src/python/sql_model/mod.rs`

- [ ] **Step 1: Append failing test for `render_table`**

Add to the bottom of `crates/bo4e-codegen/src/python/sql_model/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::make_environment;

    fn fixture_plan() -> SqlPlan {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/bo4e_sql_min");
        let schemas = bo4e_schemas::io::schemas::read_schemas(&path)
            .expect("read bo4e_sql_min")
            .schemas;
        plan::build_plan(&schemas)
    }

    #[test]
    fn render_table_object_emits_sqlmodel_class() {
        let env = make_environment(None).unwrap();
        let plan = fixture_plan();
        let angebot = plan.tables.get(&vec!["bo".to_string(), "Angebot".to_string()])
            .expect("Angebot table");
        let body = render_table(&env, angebot, 2).expect("render");
        assert!(body.contains("class Angebot(SQLModel, table=True):"), "got:\n{body}");
        assert!(body.contains("id: uuid_pkg.UUID = Field(default_factory=uuid_pkg.uuid4, primary_key=True"), "got:\n{body}");
        assert!(body.contains("adresse_id: uuid_pkg.UUID | None = Field(default=None, foreign_key=\"adresse.id\""), "got:\n{body}");
        assert!(body.contains("adresse: Adresse | None = Relationship("), "got:\n{body}");
        assert!(body.contains("adressen: list[Adresse] = Relationship(link_model=AngebotAdressenLink)"), "got:\n{body}");
        assert!(body.contains("_typ: Typ | None = Field"), "got:\n{body}");
        assert!(body.contains("werte: list[Decimal] = Field(sa_column=Column(ARRAY(Numeric)))"), "got:\n{body}");
        assert!(body.contains("extras: Any | None = Field(sa_column=Column(PickleType, nullable=True))"), "got:\n{body}");
        assert!(body.contains("anhaenge: list[Any] = Field(sa_column=Column(ARRAY(PickleType), nullable=False))"), "got:\n{body}");
        assert!(body.contains("import uuid as uuid_pkg"), "got:\n{body}");
        assert!(body.contains("from typing import Any"), "got:\n{body}");
        assert!(body.contains("from sqlmodel import Field, Relationship, SQLModel"), "got:\n{body}");
        assert!(body.contains("from ..com.adresse import Adresse"), "got:\n{body}");
        assert!(body.contains("from ..many import AngebotAdressenLink"), "got:\n{body}");
        assert!(body.contains("from ..enum.typ import Typ"), "got:\n{body}");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model sql_model::tests::render_table 2>&1 | tail -10
```

Expected: fails with `cannot find function render_table in this scope`.

- [ ] **Step 3: Implement `render_table` and supporting structs in `sql_model/mod.rs`**

Replace the `crates/bo4e-codegen/src/python/sql_model/mod.rs` contents with:

```rust
//! python-sql-model generator orchestration.
//!
//! Two-phase: a pure pre-pass walks `Schemas` and produces an immutable
//! [`plan::SqlPlan`]; a render pass consumes the plan and writes Python files
//! via vendored MiniJinja templates.

pub(crate) mod plan;

use crate::error::Error;
use crate::naming::module_file_name;
use minijinja::{Environment, context};
use plan::{SqlField, SqlPlan, TablePlan};
use serde::Serialize;
use std::collections::BTreeSet;

#[allow(dead_code)] // Wired up in Task 11.
pub(crate) use plan::SqlPlan as _SqlPlanReexport;

/// Per-field row passed to `BaseModel.jinja2`'s `SQL.fields` loop.
#[derive(Debug, Serialize)]
struct SqlFieldRow {
    annotation: String,
    definition: String,
    description: Option<String>,
}

/// One SQL import passed to `BaseModel.jinja2`'s `SQL.imports` loop.
#[derive(Debug, Serialize)]
struct SqlImport {
    from_: String,
    import_: String,
    alias: Option<String>,
}

/// Render a table's source as a Python module body.
/// `depth` is the relative-import depth (1 = root-level module, 2 = one subdir, …).
pub(crate) fn render_table(
    env: &Environment<'_>,
    table: &TablePlan,
    depth: usize,
) -> Result<String, Error> {
    if table.is_enum {
        return render_enum(env, table);
    }

    let mut imports: BTreeSet<SqlImport> = BTreeSet::new();
    imports.insert(SqlImport { from_: "uuid".into(), import_: "uuid".into(), alias: Some("uuid_pkg".into()) });
    imports.insert(SqlImport { from_: "sqlmodel".into(), import_: "Field".into(), alias: None });
    imports.insert(SqlImport { from_: "sqlmodel".into(), import_: "SQLModel".into(), alias: None });

    let mut fields: Vec<(String, SqlFieldRow)> = Vec::new();

    for sql_field in &table.sql_fields {
        match sql_field {
            SqlField::Scalar { name, type_, default, docstring, .. } => {
                let definition = match default {
                    Some(d) if d.starts_with("Field(") => d.clone(),
                    Some(d) => format!("Field(default={d})"),
                    None => "Field(...)".to_string(),
                };
                fields.push((name.clone(), SqlFieldRow {
                    annotation: type_.clone(),
                    definition,
                    description: docstring.clone(),
                }));
            }
            SqlField::ForeignKey { name, target_table, nullable, ondelete, docstring, .. } => {
                let annotation = if *nullable {
                    "uuid_pkg.UUID | None".to_string()
                } else {
                    "uuid_pkg.UUID".to_string()
                };
                let mut args = String::new();
                if *nullable {
                    args.push_str("default=None, ");
                }
                args.push_str(&format!("foreign_key=\"{target_table}.id\""));
                if let Some(od) = ondelete {
                    args.push_str(&format!(", ondelete=\"{od}\""));
                }
                let definition = if *nullable {
                    format!("Field({args})")
                } else {
                    format!("Field(..., foreign_key=\"{target_table}.id\")")
                };
                fields.push((name.clone(), SqlFieldRow {
                    annotation,
                    definition,
                    description: docstring.clone(),
                }));
            }
            SqlField::Relationship { name, target_class, owner_class, fk_field_name, nullable, docstring } => {
                let annotation = if *nullable {
                    format!("{target_class} | None")
                } else {
                    target_class.clone()
                };
                let definition = format!(
                    "Relationship(sa_relationship_kwargs={{\"foreign_keys\": [\"{owner_class}.{fk_field_name}\"]}})"
                );
                imports.insert(SqlImport { from_: "sqlmodel".into(), import_: "Relationship".into(), alias: None });
                imports.insert(target_relative_import(target_class, depth));
                fields.push((name.clone(), SqlFieldRow {
                    annotation,
                    definition,
                    description: docstring.clone(),
                }));
            }
            SqlField::ManyRelationship { name, target_class, link_class, nullable, docstring } => {
                let annotation = if *nullable {
                    format!("list[{target_class}] | None")
                } else {
                    format!("list[{target_class}]")
                };
                let definition = format!("Relationship(link_model={link_class})");
                imports.insert(SqlImport { from_: "sqlmodel".into(), import_: "Relationship".into(), alias: None });
                imports.insert(target_relative_import(target_class, depth));
                imports.insert(SqlImport {
                    from_: format!("{}many", ".".repeat(depth)),
                    import_: link_class.clone(),
                    alias: None,
                });
                fields.push((name.clone(), SqlFieldRow {
                    annotation,
                    definition,
                    description: docstring.clone(),
                }));
            }
            SqlField::EnumColumn { name, enum_class, is_list, nullable, default, docstring, .. } => {
                let mut annotation = if *is_list {
                    format!("list[{enum_class}]")
                } else {
                    enum_class.clone()
                };
                if *nullable {
                    annotation.push_str(" | None");
                }
                let enum_table_name = enum_class.to_ascii_lowercase();
                let sa_column = if *is_list {
                    format!("Column(ARRAY(Enum({enum_class}, name=\"{enum_table_name}\")))")
                } else {
                    format!("Column(Enum({enum_class}, name=\"{enum_table_name}\"))")
                };
                let mut args = String::new();
                if let Some(d) = default {
                    args.push_str(&format!("default={d}, "));
                }
                args.push_str(&format!("sa_column={sa_column}"));
                let definition = format!("Field({args})");
                imports.insert(SqlImport { from_: "sqlalchemy".into(), import_: "Column".into(), alias: None });
                imports.insert(SqlImport { from_: "sqlalchemy".into(), import_: "Enum".into(), alias: None });
                if *is_list {
                    imports.insert(SqlImport { from_: "sqlalchemy".into(), import_: "ARRAY".into(), alias: None });
                }
                imports.insert(enum_relative_import(enum_class, depth));
                fields.push((name.clone(), SqlFieldRow {
                    annotation,
                    definition,
                    description: docstring.clone(),
                }));
            }
            SqlField::ScalarArray { name, py_inner, sa_inner, nullable, docstring, .. } => {
                let annotation = if *nullable {
                    format!("list[{py_inner}] | None")
                } else {
                    format!("list[{py_inner}]")
                };
                let definition = format!("Field(sa_column=Column(ARRAY({sa_inner})))");
                imports.insert(SqlImport { from_: "sqlalchemy".into(), import_: "ARRAY".into(), alias: None });
                imports.insert(SqlImport { from_: "sqlalchemy".into(), import_: "Column".into(), alias: None });
                imports.insert(SqlImport { from_: "sqlalchemy".into(), import_: (*sa_inner).into(), alias: None });
                if py_inner == "Decimal" {
                    imports.insert(SqlImport { from_: "decimal".into(), import_: "Decimal".into(), alias: None });
                }
                fields.push((name.clone(), SqlFieldRow {
                    annotation,
                    definition,
                    description: docstring.clone(),
                }));
            }
            SqlField::AnyColumn { name, is_array, nullable, docstring } => {
                let annotation = if *is_array {
                    if *nullable { "list[Any] | None".to_string() } else { "list[Any]".to_string() }
                } else if *nullable {
                    "Any | None".to_string()
                } else {
                    "Any".to_string()
                };
                let definition = if *is_array {
                    format!("Field(sa_column=Column(ARRAY(PickleType), nullable={}))", py_bool(*nullable))
                } else {
                    format!("Field(sa_column=Column(PickleType, nullable={}))", py_bool(*nullable))
                };
                imports.insert(SqlImport { from_: "typing".into(), import_: "Any".into(), alias: None });
                imports.insert(SqlImport { from_: "sqlalchemy".into(), import_: "Column".into(), alias: None });
                imports.insert(SqlImport { from_: "sqlalchemy".into(), import_: "PickleType".into(), alias: None });
                if *is_array {
                    imports.insert(SqlImport { from_: "sqlalchemy".into(), import_: "ARRAY".into(), alias: None });
                }
                fields.push((name.clone(), SqlFieldRow {
                    annotation,
                    definition,
                    description: docstring.clone(),
                }));
            }
        }
    }

    let imports_vec: Vec<SqlImport> = imports.into_iter().collect();

    let tpl = env.get_template("python/sql_model/BaseModel.jinja2")?;
    let rendered = tpl.render(context! {
        decorators => Vec::<String>::new(),
        class_name => table.class_name.clone(),
        base_class => "SQLModel",
        description => table.description.clone().unwrap_or_else(|| table.class_name.clone()),
        fields => Vec::<String>::new(),
        methods => Vec::<String>::new(),
        config => None::<String>,
        SQL => context! {
            imports => imports_vec,
            fields => fields,
        },
    })?;
    Ok(rendered)
}

fn render_enum(env: &Environment<'_>, table: &TablePlan) -> Result<String, Error> {
    let members: Vec<minijinja::Value> = table.enum_members.iter().map(|v| {
        context! {
            name => v.clone(),
            default => format!("\"{v}\""),
            docstring => None::<String>,
        }
    }).map(minijinja::Value::from_serialize).collect();

    let tpl = env.get_template("python/sql_model/Enum.jinja2")?;
    let rendered = tpl.render(context! {
        decorators => Vec::<String>::new(),
        class_name => table.class_name.clone(),
        base_class => "StrEnum",
        description => table.description.clone(),
        fields => members,
    })?;
    Ok(format!("from enum import StrEnum\n\n{}", rendered.trim_start_matches('\n')))
}

fn target_relative_import(target_class: &str, depth: usize) -> SqlImport {
    let target_table = target_class.to_ascii_lowercase();
    SqlImport {
        from_: format!("{}com.{}", ".".repeat(depth), target_table),
        import_: target_class.to_string(),
        alias: None,
    }
}

fn enum_relative_import(enum_class: &str, depth: usize) -> SqlImport {
    let enum_table = enum_class.to_ascii_lowercase();
    SqlImport {
        from_: format!("{}enum.{}", ".".repeat(depth), enum_table),
        import_: enum_class.to_string(),
        alias: None,
    }
}

fn py_bool(b: bool) -> &'static str { if b { "True" } else { "False" } }

// Required for SqlImport in BTreeSet.
impl Ord for SqlImport {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.from_.as_str(), self.import_.as_str(), self.alias.as_deref())
            .cmp(&(other.from_.as_str(), other.import_.as_str(), other.alias.as_deref()))
    }
}
impl PartialOrd for SqlImport {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
impl PartialEq for SqlImport {
    fn eq(&self, other: &Self) -> bool { self.cmp(other) == std::cmp::Ordering::Equal }
}
impl Eq for SqlImport {}
```

**Sub-detail — relative-import targeting.** `target_relative_import` assumes the target sits under `<output>/com/`, which is true for every BO referencing a COM in the BO4E schema set. If a future schema references a class in a different subpackage (e.g. a BO referencing another BO), this helper renders the wrong path. The fixture exercises only the BO→COM case; other cases are out of scope per the spec ("circular references between BOs"). A follow-up plan can generalise the helper by passing the full target module path through `SqlField::Relationship` / `ManyRelationship`.

- [ ] **Step 4: Run the test to verify it passes**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model sql_model::tests::render_table 2>&1 | tail -10
```

Expected: pass. The body assertions check class header, every SqlField variant rendered, and every relative import.

- [ ] **Step 5: Verify the workspace still builds and tests pass**

```bash
cd /repos/bo4e-cli
cargo test --workspace 2>&1 | tail -10
```

Expected: every test binary reports `test result: ok`.

- [ ] **Step 6: Commit**

```bash
git add crates/bo4e-codegen/src/python/sql_model/mod.rs
git commit -m "feat(codegen/sql_model): render_table emits SQLModel class body

Builds the SQL.imports + SQL.fields context for BaseModel.jinja2 and
renders one Python module per TablePlan. Covers every SqlField variant:
Scalar (incl. synthesised id), ForeignKey, Relationship, ManyRelationship,
EnumColumn (scalar/list, nullable/default), ScalarArray, AnyColumn
(scalar/list, nullable). Enum tables route through render_enum which
emits a StrEnum class.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 10: Render `many.py`, `__init__.py`, `__version__.py`, and per-subdir `__init__.py`

**Goal:** Add the helpers that produce the package-level files: `many.py` (junction classes via `ManyLinks.jinja2`), `__init__.py` (re-exports via `__init__.jinja2`), `__version__.py` (constant), and the empty `__init__.py` files for each first-level subpackage.

**Files:**
- Modify: `crates/bo4e-codegen/src/python/sql_model/mod.rs`

- [ ] **Step 1: Append failing tests for the package-level renderers**

Add inside the existing `mod tests` in `sql_model/mod.rs`:

```rust
    #[test]
    fn render_many_py_emits_one_class_per_junction() {
        let env = make_environment(None).unwrap();
        let plan = fixture_plan();
        let body = render_many(&env, &plan.junctions).expect("render");
        assert!(body.contains("class AngebotAdressenLink(SQLModel, table=True):"), "got:\n{body}");
        assert!(body.contains("angebot_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"angebot.id\""));
        assert!(body.contains("adresse_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key=\"adresse.id\""));
    }

    #[test]
    fn render_init_includes_classes_and_links() {
        let env = make_environment(None).unwrap();
        let plan = fixture_plan();
        let body = render_init(&env, &plan).expect("render");
        assert!(body.contains("from .bo.angebot import Angebot"));
        assert!(body.contains("from .com.adresse import Adresse"));
        assert!(body.contains("from .enum.typ import Typ"));
        assert!(body.contains("from .many import AngebotAdressenLink"));
    }

    #[test]
    fn render_version_emits_constant() {
        let body = render_version("202401.4.0");
        assert_eq!(body.trim(), "__version__: str = \"202401.4.0\"");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model sql_model::tests::render 2>&1 | tail -15
```

Expected: three failures (`cannot find function render_many` etc).

- [ ] **Step 3: Implement the helpers**

Append to `crates/bo4e-codegen/src/python/sql_model/mod.rs`:

```rust
use plan::JunctionTable;

/// Render `<output>/many.py`. Returns an empty string when there are no junctions
/// (caller should not write the file in that case).
pub(crate) fn render_many(env: &Environment<'_>, junctions: &[JunctionTable]) -> Result<String, Error> {
    if junctions.is_empty() {
        return Ok(String::new());
    }
    let links: Vec<minijinja::Value> = junctions.iter().map(|j| {
        context! {
            table_name => j.class_name.clone(),
            cls1 => j.owner_class.clone(),
            cls2 => j.target_class.clone(),
            rel_field_name1 => j.source_field.clone(),
            id_field_name1 => j.owner_id_field.clone(),
            id_field_name2 => j.target_id_field.clone(),
        }
    }).map(minijinja::Value::from_serialize).collect();

    let tpl = env.get_template("python/sql_model/ManyLinks.jinja2")?;
    let rendered = tpl.render(context! { links => links })?;
    Ok(rendered)
}

/// Render `<output>/__init__.py` re-exporting every class and every junction.
pub(crate) fn render_init(env: &Environment<'_>, plan: &SqlPlan) -> Result<String, Error> {
    let classes: Vec<minijinja::Value> = plan.tables.values().map(|t| {
        let module_path: Vec<String> = t.module.iter()
            .take(t.module.len().saturating_sub(1))
            .map(|s| s.to_ascii_lowercase())
            .chain(std::iter::once(module_file_name(&t.module)))
            .collect();
        context! {
            name => t.class_name.clone(),
            module_path => module_path,
        }
    }).map(minijinja::Value::from_serialize).collect();

    let links: Vec<String> = plan.junctions.iter().map(|j| j.class_name.clone()).collect();

    let tpl = env.get_template("python/sql_model/__init__.jinja2")?;
    let rendered = tpl.render(context! {
        classes => classes,
        links => links,
    })?;
    Ok(rendered)
}

pub(crate) fn render_version(version: &str) -> String {
    format!("__version__: str = \"{version}\"\n")
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model sql_model::tests::render 2>&1 | tail -15
```

Expected: all three new tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/bo4e-codegen/src/python/sql_model/mod.rs
git commit -m "feat(codegen/sql_model): render package-level files

Adds render_many (junction classes via ManyLinks.jinja2), render_init
(__init__.py via __init__.jinja2 re-exporting all classes + links),
and render_version (the __version__.py constant). render_many returns
an empty string when there are no junctions; the caller is responsible
for skipping the file write in that case.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 11: Implement `generate_sql_model` orchestration + wire the lib.rs match arm

**Goal:** Add the top-level `generate_sql_model(schemas, output_dir, env)` function that walks the plan, calls `render_table` per table, writes each rendered string to its file, then writes `many.py`, `__init__.py`, `__version__.py`, and the per-subpackage `__init__.py`. Flip the placeholder arm in `lib.rs` to call this function.

**Files:**
- Modify: `crates/bo4e-codegen/src/python/sql_model/mod.rs`
- Modify: `crates/bo4e-codegen/src/lib.rs`

- [ ] **Step 1: Implement `generate_sql_model` in `sql_model/mod.rs`**

Append:

```rust
use bo4e_schemas::Schemas;
use std::path::{Path, PathBuf};

pub(crate) fn generate_sql_model(
    schemas: &Schemas,
    output_dir: &Path,
    env: &Environment<'_>,
) -> Result<Vec<PathBuf>, Error> {
    std::fs::create_dir_all(output_dir)?;
    let mut written: Vec<PathBuf> = Vec::new();
    let plan = plan::build_plan(schemas);

    // ── Per-class files ────────────────────────────────────────────────────────
    for table in plan.tables.values() {
        let path_segments: Vec<String> = table.module.iter()
            .take(table.module.len().saturating_sub(1))
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let mut out_dir = output_dir.to_path_buf();
        for seg in &path_segments {
            out_dir.push(seg);
        }
        std::fs::create_dir_all(&out_dir)?;
        let file_name = format!("{}.py", module_file_name(&table.module));
        let depth = path_segments.len() + 1;
        let body = render_table(env, table, depth)?;
        let out_path = out_dir.join(&file_name);
        std::fs::write(&out_path, body)?;
        written.push(out_path);
    }

    // ── many.py at the root (only if there are junctions) ──────────────────────
    if !plan.junctions.is_empty() {
        let many = render_many(env, &plan.junctions)?;
        let many_path = output_dir.join("many.py");
        std::fs::write(&many_path, many)?;
        written.push(many_path);
    }

    // ── __init__.py + __version__.py at the root ───────────────────────────────
    let init_body = render_init(env, &plan)?;
    let init_path = output_dir.join("__init__.py");
    std::fs::write(&init_path, init_body)?;
    written.push(init_path);

    let version_path = output_dir.join("__version__.py");
    std::fs::write(&version_path, render_version(&schemas.version.to_string()))?;
    written.push(version_path);

    // ── Empty __init__.py per first-level subdirectory ─────────────────────────
    let mut subdirs: BTreeSet<String> = BTreeSet::new();
    for table in plan.tables.values() {
        if table.module.len() > 1 {
            subdirs.insert(table.module[0].to_ascii_lowercase());
        }
    }
    for sub in subdirs {
        let p = output_dir.join(&sub).join("__init__.py");
        if !p.exists() {
            std::fs::write(&p, "")?;
            written.push(p);
        }
    }

    Ok(written)
}
```

Remove the now-unused `_SqlPlanReexport` placeholder and its `#[allow(dead_code)]` line at the top of the file — `generate_sql_model` is the public entry point.

- [ ] **Step 2: Wire `lib.rs`**

In `crates/bo4e-codegen/src/lib.rs`, replace the placeholder match arm:

```rust
        #[cfg(feature = "python-sql-model")]
        OutputType::PythonSqlModel => Err(Error::OutputTypeNotCompiledIn(output_type.as_str())),
```

with:

```rust
        #[cfg(feature = "python-sql-model")]
        OutputType::PythonSqlModel => {
            python::sql_model::generate_sql_model(schemas, output_dir, &env)?;
            Ok(())
        }
```

- [ ] **Step 3: Verify the workspace builds**

```bash
cd /repos/bo4e-cli
cargo build --workspace 2>&1 | tail -3
cargo test --workspace 2>&1 | tail -10
```

Expected: build succeeds; all unit tests pass. The new orchestration is exercised end-to-end by Task 12's integration test.

- [ ] **Step 4: Commit**

```bash
git add crates/bo4e-codegen/src/python/sql_model/mod.rs crates/bo4e-codegen/src/lib.rs
git commit -m "feat(codegen/sql_model): wire generate_sql_model end-to-end

Adds generate_sql_model orchestration that walks the SqlPlan, calls
render_table per table, writes per-class files at the right paths,
writes many.py (when junctions exist), __init__.py, __version__.py,
and the empty per-subpackage __init__.py files. Flips the lib.rs
match arm from OutputTypeNotCompiledIn to the real call.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 12: Add the sql-model integration test (Python AST)

**Goal:** Vendor an integration test that exercises `generate(...)` end-to-end against `bo4e_sql_min`, asserts the directory tree, and shells out to `python3 -c "import ast; ..."` to verify the generated `.py` files parse as Python and contain the expected class structure.

**Files:**
- Create: `crates/bo4e-codegen/tests/integration_sql_model.rs`

- [ ] **Step 1: Write the integration test**

Save to `crates/bo4e-codegen/tests/integration_sql_model.rs`:

```rust
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

    assert!(body.contains("id: uuid_pkg.UUID = Field(default_factory=uuid_pkg.uuid4, primary_key=True"));
    assert!(body.contains("adresse_id: uuid_pkg.UUID | None = Field(default=None, foreign_key=\"adresse.id\""));
    assert!(body.contains("adresse: Adresse | None = Relationship("));
    assert!(body.contains("adressen: list[Adresse] = Relationship(link_model=AngebotAdressenLink)"));
    assert!(body.contains("_typ: Typ | None = Field"));
    assert!(body.contains("werte: list[Decimal] = Field(sa_column=Column(ARRAY(Numeric)))"));
    assert!(body.contains("extras: Any | None = Field(sa_column=Column(PickleType, nullable=True))"));
    assert!(body.contains("anhaenge: list[Any] = Field(sa_column=Column(ARRAY(PickleType), nullable=False))"));
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
```

- [ ] **Step 2: Run the test**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model --test integration_sql_model 2>&1 | tail -20
```

Expected: all four tests pass. If the AST-parse test fails, examine `tmp` paths printed in the assertion error — common failures are missing imports or wrong relative-import depth.

- [ ] **Step 3: Verify the full workspace stays green**

```bash
cd /repos/bo4e-cli
cargo test --workspace 2>&1 | tail -10
```

Expected: every test binary reports `test result: ok`.

- [ ] **Step 4: Commit**

```bash
git add crates/bo4e-codegen/tests/integration_sql_model.rs
git commit -m "test(codegen/sql_model): integration test against bo4e_sql_min

Asserts: every expected file lands in the right path, angebot.py
contains the expected SQLModel class header + every SqlField variant
+ every relative import, many.py contains the junction class, no
__future__ imports anywhere, and python3 -c \"ast.parse(...)\" parses
every generated .py file (skipped when python3 is unavailable).

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 13: Cross-fixture coverage — feed `bo4e_sql_min` to the pydantic integration test

**Goal:** Add a sub-test to `integration_pydantic.rs` asserting the existing pydantic generator successfully renders the richer `bo4e_sql_min` fixture too. The richer fixture has every variant the pydantic generator already handles (M:N becomes `list[Adresse]`, Any becomes `Any`, etc.); this guards against regressions when extending shared helpers.

**Files:**
- Modify: `crates/bo4e-codegen/tests/integration_pydantic.rs`

- [ ] **Step 1: Append cross-fixture sub-test**

Add to the bottom of `crates/bo4e-codegen/tests/integration_pydantic.rs`:

```rust
#[test]
fn pydantic_renders_richer_sql_fixture_without_error() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bo4e_sql_min");
    let tmp = tempfile::tempdir().unwrap();
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture).expect("read_schemas");

    bo4e_codegen::generate(
        &out.schemas,
        bo4e_codegen::OutputType::PythonPydantic,
        tmp.path(),
        &bo4e_codegen::Options { clear_output: true, templates_dir: None },
    ).expect("generate");

    let angebot = std::fs::read_to_string(tmp.path().join("bo/angebot.py")).unwrap();
    // The pydantic flavour renders M:N as list[Adresse] | None (no junction concept);
    // Any as Any | None; list[Decimal] as list[Decimal] | None.
    assert!(angebot.contains("class Angebot(BaseModel):"), "got:\n{angebot}");
    assert!(angebot.contains("adressen: list[Adresse]"), "got:\n{angebot}");
    assert!(angebot.contains("extras: Any | None"), "got:\n{angebot}");
    assert!(angebot.contains("werte: list[Decimal]"), "got:\n{angebot}");
    assert!(!angebot.contains("__future__"));
    assert!(!angebot.contains("table=True"), "pydantic flavour must not emit table=True");
}
```

- [ ] **Step 2: Run the test**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-pydantic --test integration_pydantic pydantic_renders_richer_sql_fixture_without_error 2>&1 | tail -20
```

Expected: pass. **If it fails**, the failure indicates a real semantic gap in the existing pydantic generator that the richer fixture surfaces. Per the spec ("If the pydantic generator surfaces gaps in the existing implementation when fed this fixture, those are real defects to fix as part of the cleanup or out-of-scope follow-ups (decided per defect)"):

  - If the gap is small (e.g. a missing import or a default-value mishandling), fix it inline as part of this task and re-run.
  - If the gap is structural (e.g. the enum-default rendering as `"ANGEBOT"` string vs `Typ.ANGEBOT` reference), file a follow-up issue and weaken the assertion to skip the affected line; record the limitation in a comment immediately above the assertion.

- [ ] **Step 3: Verify the workspace stays green**

```bash
cd /repos/bo4e-cli
cargo test --workspace 2>&1 | tail -10
```

Expected: every test binary reports `test result: ok`.

- [ ] **Step 4: Commit**

```bash
git add crates/bo4e-codegen/tests/integration_pydantic.rs
git commit -m "test(codegen/pydantic): cross-fixture coverage with bo4e_sql_min

Asserts the existing pydantic generator successfully renders the
richer sql-model fixture: M:N becomes list[Adresse], Any becomes
Any | None, list[Decimal] passes through, and no table=True is emitted.
Provides a regression guard when shared helpers (types, imports) are
extended in the future.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 14: Add the parity-test stub for sql-model

**Goal:** Add a parity-test scaffold that mirrors `parity_pydantic.rs` (run our generator + Python's, walk both ASTs, compare). The full Python-side fixture wiring is out of scope per the spec; the stub reuses `bo4e_sql_min` and asserts only that the Rust output parses as Python, with a `// TODO` comment pointing to the future Python-side comparison.

**Files:**
- Create: `crates/bo4e-codegen/tests/parity_sql_model.rs`

- [ ] **Step 1: Write the parity-test stub**

Save to `crates/bo4e-codegen/tests/parity_sql_model.rs`:

```rust
#![cfg(feature = "python-sql-model")]

// Parity stub: full Python-side comparison is deferred to a follow-up plan.
// For now we assert the Rust output parses as Python and contains the expected
// class shapes. When the upstream Python image is wired in CI, this test grows
// to call the Python generator into a sibling tempdir and walk both ASTs.

use std::path::PathBuf;
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bo4e_sql_min")
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
    bo4e_codegen::generate(
        &out.schemas,
        bo4e_codegen::OutputType::PythonSqlModel,
        tmp.path(),
        &bo4e_codegen::Options { clear_output: true, templates_dir: None },
    ).unwrap();

    let angebot = tmp.path().join("bo/angebot.py");
    let script = format!(r#"
import ast
src = open({path:?}).read()
tree = ast.parse(src)
classes = [n for n in ast.walk(tree) if isinstance(n, ast.ClassDef)]
assert len(classes) == 1, f"expected 1 class, got {{len(classes)}}"
assert classes[0].name == "Angebot", classes[0].name
bases = [b.id if isinstance(b, ast.Name) else getattr(b, 'attr', '?') for b in classes[0].bases]
assert "SQLModel" in bases, f"expected SQLModel in bases, got {{bases}}"
keywords = {{kw.arg: kw.value for kw in classes[0].keywords}}
assert "table" in keywords, f"expected table=True keyword, got {{list(keywords)}}"
print("ok")
"#, path = angebot.to_string_lossy());

    let output = Command::new("python3").arg("-c").arg(&script).output().unwrap();
    assert!(
        output.status.success(),
        "python3 failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
```

- [ ] **Step 2: Run the test**

```bash
cd /repos/bo4e-cli
cargo test -p bo4e-codegen --features python-sql-model --test parity_sql_model 2>&1 | tail -15
```

Expected: pass when `python3` is on PATH; otherwise prints "skipping parity test" and exits 0.

- [ ] **Step 3: Commit**

```bash
git add crates/bo4e-codegen/tests/parity_sql_model.rs
git commit -m "test(codegen/sql_model): parity-test stub vs Python AST

Stub mirrors parity_pydantic.rs but compares only against the Rust
output's AST shape (1 class named Angebot, base SQLModel, table=True
keyword). Full byte/AST comparison against the Python implementation's
output is deferred to a follow-up plan once the upstream Python image
is wired into CI.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 15: README updates + slim install verification + final workspace check

**Goal:** Document `python-sql-model` in the README (slim install example, generate command example), and verify both the default-features install and the sql-model-only install produce a binary that exposes the right `--output-type` values.

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Update `README.md`**

Add a sql-model-only install example next to the existing pydantic-only example. The current state (after Tasks 1–3) has:

```
cargo install bo4e-cli --no-default-features --features python-pydantic
```

Add immediately below:

```
cargo install bo4e-cli --no-default-features --features python-sql-model
```

In the example command block (currently `bo4e generate -i ./bo4e_schemas_edited -o ./bo4e_schemas_python -t python-pydantic`), add a second example line directly underneath:

```
bo4e generate -i ./bo4e_schemas_edited -o ./bo4e_schemas_sql -t python-sql-model
```

The flag-table value-list line already enumerates `python-sql-model` from Task 1's edit; no further change needed there.

- [ ] **Step 2: Verify slim install (sql-model only)**

```bash
cd /repos/bo4e-cli
cargo install --path crates/bo4e-cli --no-default-features --features python-sql-model --force --locked 2>&1 | tail -3
~/.cargo/bin/bo4e generate --help 2>&1 | grep -A1 'output-type'
```

Expected install output: `Installed package` summary. Expected `--help` output: `--output-type` line shows `[possible values: python-sql-model]` (no other variants, since pydantic was compiled out).

- [ ] **Step 3: Verify default install (both flavours)**

```bash
cd /repos/bo4e-cli
cargo install --path crates/bo4e-cli --force --locked 2>&1 | tail -3
~/.cargo/bin/bo4e generate --help 2>&1 | grep -A1 'output-type'
```

Expected: `--output-type` shows `[possible values: python-pydantic, python-sql-model]`.

- [ ] **Step 4: End-to-end smoke**

```bash
cd /repos/bo4e-cli
TMPDIR=$(mktemp -d)
~/.cargo/bin/bo4e generate \
  -i crates/bo4e-codegen/tests/fixtures/bo4e_sql_min \
  -o "$TMPDIR" \
  -t python-sql-model
ls "$TMPDIR/bo" "$TMPDIR/com" "$TMPDIR/enum" "$TMPDIR"/many.py "$TMPDIR"/__init__.py "$TMPDIR"/__version__.py
```

Expected: every listed path exists (no errors).

- [ ] **Step 5: Full workspace test pass**

```bash
cd /repos/bo4e-cli
cargo test --workspace 2>&1 | tail -10
```

Expected: every test binary reports `test result: ok`.

- [ ] **Step 6: Commit**

```bash
git add README.md
git commit -m "docs: README mentions for python-sql-model install + example

Adds a sql-model-only \`cargo install --features\` example and a
second \`bo4e generate -t python-sql-model\` example. Verified that
default install exposes both python-pydantic and python-sql-model
output types and that slim installs of either feature alone restrict
--output-type to that single variant.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Self-Review

**1. Spec coverage** — every spec section maps to a task:

| Spec section | Task(s) |
| --- | --- |
| Pre-flight cleanup: drop v1 | Task 1 |
| Pre-flight cleanup: rename v2 → pydantic (Cargo, OutputType) | Task 2 |
| Pre-flight cleanup: rename modules/files/templates/tests | Task 3 |
| Cleanup verification (slim install, grep) | Tasks 1, 3 |
| CLI surface (no new flags) | (no-op — Task 11 wires existing variant) |
| Workspace structure | Tasks 5, 8 |
| Why a shared mapper / two-file module | (informational; honoured by Tasks 5, 9) |
| Feature flags | Tasks 1, 2 |
| Public API (no change) | (no-op) |
| SqlPlan data model | Task 5 |
| Pre-pass invariants 1 (synth id), 7 (plain scalars) | Task 6 |
| Pre-pass invariants 2 (1:1 ref), 3 (M:N), 4 (enum), 5 (Any), 6 (scalar array) | Task 7 |
| Vendored templates | Task 8 |
| Render orchestration | Tasks 9, 10, 11 |
| Test fixture | Task 4 |
| CLI wiring (lib.rs arm) | Task 11 |
| Drop-in parity contract | Tasks 9, 12 |
| Testing strategy: unit tests in plan.rs | Tasks 6, 7 |
| Testing strategy: per-template render smoke | Task 8 |
| Testing strategy: integration test | Task 12 |
| Testing strategy: cross-fixture pydantic | Task 13 |
| Testing strategy: parity stub | Task 14 |
| Out of scope (circular refs, schemas serde gaps, migration gen) | Task 9 step 3 sub-detail; Task 4 step 5 |
| Migration plan / phases | Task ordering 1–15 |
| Open questions / risks | (informational; honoured by Task 14 stub + Task 9 sub-detail) |

**2. Placeholder scan** — searched for `TBD`, `TODO`, `implement later`, `appropriate`, `handle edge cases`. The only matches are: a Task-13 directive instructing the engineer to file a follow-up issue if a structural gap surfaces (legitimate runbook step, not a placeholder); the Task-14 file body comment that documents what the stub defers (intentional code comment, not a plan-failure). All steps either show the actual code or give an exact bash command.

**3. Type consistency** — verified across tasks:

- `SqlPlan { tables: BTreeMap<Vec<String>, TablePlan>, junctions: Vec<JunctionTable> }` introduced in Task 5; consumed identically in Tasks 6, 7, 9, 10, 11.
- `SqlField` variant names (`Scalar`, `ForeignKey`, `Relationship`, `ManyRelationship`, `EnumColumn`, `ScalarArray`, `AnyColumn`) match between Task 5 (definition), Task 6 (`Scalar` only), Task 7 (six new), Task 9 (`render_table` match arms), and Task 12 (assertions about emitted output).
- `JunctionTable` field names (`class_name`, `owner_class`, `owner_table`, `owner_id_field`, `target_class`, `target_table`, `target_id_field`, `source_field`) match between Tasks 5, 7, 8 (template smoke test), 10 (`render_many`), 12 (assertions).
- Function names: `build_plan` (Tasks 5, 6, 7, 9, 10, 11), `render_table` (Tasks 9, 11), `render_many` (Tasks 10, 11), `render_init` (Tasks 10, 11), `render_version` (Tasks 10, 11), `generate_sql_model` (Tasks 11, 12, 14).
- Template names: `python/sql_model/{BaseModel,Config,Enum,ManyLinks,__init__}.jinja2` consistent between Task 8 (registration) and Tasks 8 smoke tests, 9 (`render_table`), 10 (`render_many`, `render_init`).
- Cargo feature `python-sql-model` consistent across Tasks 1, 2, 5, 8, 11, 12, 14, 15.
- `OutputType::PythonSqlModel` referenced consistently by Tasks 11, 12, 14.
- `bo4e_sql_min` fixture path `crates/bo4e-codegen/tests/fixtures/bo4e_sql_min` consistent across Tasks 4, 6, 9, 12, 13, 14, 15.

No mismatches found.
