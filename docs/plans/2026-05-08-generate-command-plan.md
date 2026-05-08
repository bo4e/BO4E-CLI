# `generate` Command Implementation Plan (Plan 1 of 3)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the single-binary repo into a 3-crate Cargo workspace (`bo4e-schemas`, `bo4e-codegen`, `bo4e-cli`), add a working `python-pydantic-v2` generator, wire the `bo4e generate` subcommand end-to-end, and verify slim install via Cargo features.

**Architecture:** Workspace with `bo4e-schemas` (pure data: schema/version models + JSON I/O), `bo4e-codegen` (pure library: MiniJinja-rendered code generation, feature-gated per output type), and `bo4e-cli` (binary: clap-driven CLI that depends on the other two). Drop-in parity with the Python generator: same module/file/class/field names, same attribute surfaces. No `__future__` imports in generated output.

**Tech Stack:** Rust 2024 edition, MiniJinja 2.x, clap 4.5 (existing), serde / serde_json (existing), `thiserror` 2.x (new for `bo4e-codegen`). Python 3 in CI for AST-level integration tests.

**Spec reference:** `docs/plans/2026-05-08-generate-command-design.md`.
**Parity reference:** Python implementation at `/tmp/bo4e-cli-python` (worktree of `origin/main`).
**Branch:** All work commits directly to `rust`.

**Follow-up plans (out of scope here):**
- Plan 2: `python-pydantic-v1` generator (additive on top of this plan).
- Plan 3: `python-sql-model` generator (additive).

---

## File Structure

After this plan, the repo layout is:

```
bo4e-cli/
├── Cargo.toml                                  (workspace manifest only)
├── docs/plans/                                 (existing design + plan docs)
└── crates/
    ├── bo4e-schemas/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs
    │       ├── models/
    │       │   ├── mod.rs
    │       │   ├── schema_meta.rs              (← from src/models/schema_meta.rs)
    │       │   ├── version.rs                  (← from src/models/version.rs)
    │       │   ├── json_schema.rs              (← from src/models/json_schema.rs)
    │       │   └── macros.rs                   (← from src/models/macros.rs)
    │       ├── io/
    │       │   ├── mod.rs
    │       │   └── schemas.rs                  (← from src/io/schemas.rs, cwarn! → returned warnings)
    │       └── visitable.rs                    (← from src/utils/visitable.rs)
    ├── bo4e-codegen/
    │   ├── Cargo.toml                          (features: python, python-pydantic-v1/v2/sql-model)
    │   └── src/
    │       ├── lib.rs                          (public: generate, OutputType, Options, Error)
    │       ├── error.rs
    │       ├── output_type.rs
    │       ├── env.rs                          (MiniJinja env + embedded vs disk loader)
    │       ├── naming.rs                       (module-name, field-name conversions)
    │       ├── python/
    │       │   ├── mod.rs                      (shared python helpers)
    │       │   ├── types.rs                    (JSON Schema → Python type strings)
    │       │   ├── imports.rs                  (import collector)
    │       │   └── pydantic_v2.rs              (gated by python-pydantic-v2 feature)
    │       └── templates/
    │           └── python/pydantic_v2/
    │               ├── BaseModel.jinja2
    │               ├── Enum.jinja2
    │               └── __init__.jinja2
    └── bo4e-cli/
        ├── Cargo.toml                          (depends on schemas + codegen, re-exports features)
        └── src/                                (← all current src/ moves here)
            ├── main.rs
            ├── cli/
            │   ├── mod.rs
            │   ├── base.rs
            │   ├── repo.rs
            │   ├── generate.rs                 (new — Generate clap struct + Executable impl)
            │   ├── diff.rs / edit.rs / pull.rs (existing)
            │   └── ...
            ├── console/                        (existing)
            ├── io/
            │   ├── mod.rs                      (no longer re-exports schemas)
            │   ├── git.rs                      (existing)
            │   ├── github.rs                   (existing)
            │   └── ...                         (changes.rs, cleanse.rs, config.rs, matrix.rs stay)
            ├── models/
            │   ├── mod.rs                      (no longer contains schema_meta/version/json_schema/macros)
            │   ├── cli.rs / git.rs / changes.rs / config.rs / matrix.rs (existing)
            │   └── ...
            └── utils/
                ├── mod.rs                      (no longer contains visitable)
                └── tokio.rs                    (existing)
```

Each task below references this layout.

---

### Task 1: Convert root to a virtual Cargo workspace; move all current source under `crates/bo4e-cli/`

**Goal:** Turn the existing single-crate repo into a workspace with one member crate (`bo4e-cli`) at `crates/bo4e-cli/`. Behaviour unchanged. All existing tests pass.

**Files:**
- Modify: `Cargo.toml` (becomes a workspace manifest)
- Move: `src/` → `crates/bo4e-cli/src/`
- Move: `Cargo.lock` stays at repo root (workspace owns it)
- Create: `crates/bo4e-cli/Cargo.toml` (the per-crate manifest)
- Modify: `.devcontainer/devcontainer.json`, `.github/workflows/*.yml` (update any `cargo` working-directory references to root — workspace makes most of these unnecessary)

- [ ] **Step 1: Capture the current top-of-tree state**

```bash
cd /repos/bo4e-cli
git status -s
cargo test 2>&1 | tail -3   # baseline: confirm green
```

Expected: `test result: ok` summary lines.

- [ ] **Step 2: Move sources under `crates/bo4e-cli/`**

```bash
mkdir -p crates/bo4e-cli
git mv src crates/bo4e-cli/src
```

- [ ] **Step 3: Create `crates/bo4e-cli/Cargo.toml`**

Move the existing `[package]` and `[dependencies]` sections from the root `Cargo.toml` into this new file. Verbatim copy except for the `[package]` section, which gets `name = "bo4e-cli"`.

```toml
[package]
name = "bo4e-cli"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "bo4e"
path = "src/main.rs"

[dependencies]
# (copy the entire [dependencies] table from the previous root Cargo.toml verbatim)
```

If the existing `Cargo.toml` had a `[lints]` table, keep it here too.

- [ ] **Step 4: Replace the root `Cargo.toml` with a virtual workspace manifest**

```toml
[workspace]
resolver = "2"
members = ["crates/bo4e-cli"]

[workspace.package]
edition = "2024"
license = "MIT"
repository = "https://github.com/bo4e/BO4E-CLI"

[workspace.dependencies]
# placeholder for shared deps in later tasks
```

- [ ] **Step 5: Run the full test suite**

```bash
cargo test --workspace 2>&1 | tail -10
```

Expected: identical pass count to Step 1's baseline. `cargo build` produces `target/debug/bo4e` (binary name unchanged).

- [ ] **Step 6: Update tooling that references the old `src/` path**

Run:
```bash
grep -rn "src/" .devcontainer .github 2>/dev/null
```

For each match, decide if it should now be `crates/bo4e-cli/src/`. CI files that just run `cargo test` from the repo root need no change because the workspace member is auto-discovered.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(workspace): move bo4e-cli into crates/ as workspace member"
```

---

### Task 2: Extract `bo4e-schemas` crate

**Goal:** Move schema-related modules into a new `bo4e-schemas` library crate. `bo4e-cli` depends on it. The existing `crate::cwarn!` callsite in `io/schemas.rs` must be replaced because the moved code can't reach the CLI's console module — `read_schemas` returns warnings to the caller, which logs them.

**Files:**
- Create: `crates/bo4e-schemas/Cargo.toml`
- Create: `crates/bo4e-schemas/src/lib.rs`
- Create: `crates/bo4e-schemas/src/models/mod.rs`
- Move: `crates/bo4e-cli/src/models/schema_meta.rs` → `crates/bo4e-schemas/src/models/schema_meta.rs`
- Move: `crates/bo4e-cli/src/models/version.rs` → `crates/bo4e-schemas/src/models/version.rs`
- Move: `crates/bo4e-cli/src/models/json_schema.rs` → `crates/bo4e-schemas/src/models/json_schema.rs`
- Move: `crates/bo4e-cli/src/models/macros.rs` → `crates/bo4e-schemas/src/models/macros.rs`
- Create: `crates/bo4e-schemas/src/io/mod.rs`
- Move: `crates/bo4e-cli/src/io/schemas.rs` → `crates/bo4e-schemas/src/io/schemas.rs` (with cwarn! fix)
- Move: `crates/bo4e-cli/src/utils/visitable.rs` → `crates/bo4e-schemas/src/visitable.rs`
- Modify: `crates/bo4e-cli/Cargo.toml` (add `bo4e-schemas` path dep)
- Modify: `Cargo.toml` workspace `members`
- Modify: `crates/bo4e-cli/src/models.rs`, `src/io.rs`, `src/utils.rs` (remove moved entries)
- Modify: every file that imports the moved types

- [ ] **Step 1: Create the new crate skeleton**

```bash
mkdir -p crates/bo4e-schemas/src/{models,io}
```

Create `crates/bo4e-schemas/Cargo.toml`:

```toml
[package]
name = "bo4e-schemas"
version = "0.1.0"
edition = "2024"

[dependencies]
itertools = "0.13"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
walkdir = "2"
```

(Copy exact crate versions from the existing `crates/bo4e-cli/Cargo.toml` to avoid version skew.)

- [ ] **Step 2: Create `crates/bo4e-schemas/src/lib.rs`**

```rust
pub mod models;
pub mod io;
mod visitable;

// re-exports for ergonomic use sites
pub use models::schema_meta::{Schema, Schemas};
pub use models::version::{DirtyVersion, Version};
pub use visitable::Visitable;
```

(Adjust the `Version` re-export name to match what `version.rs` actually exports — verify via `grep "^pub" crates/bo4e-cli/src/models/version.rs` before this step.)

- [ ] **Step 3: Move the four model files into the new crate**

```bash
git mv crates/bo4e-cli/src/models/schema_meta.rs crates/bo4e-schemas/src/models/schema_meta.rs
git mv crates/bo4e-cli/src/models/version.rs     crates/bo4e-schemas/src/models/version.rs
git mv crates/bo4e-cli/src/models/json_schema.rs crates/bo4e-schemas/src/models/json_schema.rs
git mv crates/bo4e-cli/src/models/macros.rs      crates/bo4e-schemas/src/models/macros.rs
```

Create `crates/bo4e-schemas/src/models/mod.rs`:

```rust
pub mod schema_meta;
pub mod version;
pub mod json_schema;
pub(crate) mod macros;
```

- [ ] **Step 4: Move `visitable.rs`**

```bash
git mv crates/bo4e-cli/src/utils/visitable.rs crates/bo4e-schemas/src/visitable.rs
```

- [ ] **Step 5: Move `io/schemas.rs` and convert `cwarn!` into a warnings return**

```bash
git mv crates/bo4e-cli/src/io/schemas.rs crates/bo4e-schemas/src/io/schemas.rs
```

Create `crates/bo4e-schemas/src/io/mod.rs`:

```rust
pub mod schemas;
```

In `crates/bo4e-schemas/src/io/schemas.rs`:
- Update internal `use` paths: `crate::models::...` (was already this; `crate` now means `bo4e_schemas`).
- Replace the `crate::cwarn!` call with a returned warnings vector. Change `read_schemas` from `Result<Schemas, String>` to `Result<ReadSchemasOutput, String>` where:

```rust
pub struct ReadSchemasOutput {
    pub schemas: Schemas,
    /// Non-fatal warnings encountered during traversal (e.g. unreadable entries).
    pub warnings: Vec<String>,
}

pub fn read_schemas(input_dir: &std::path::Path) -> Result<ReadSchemasOutput, String> {
    let version = read_version_file(input_dir)?;
    let mut schemas = Schemas::new(version);
    let mut warnings: Vec<String> = Vec::new();

    for entry in WalkDir::new(input_dir).into_iter().filter_map(|e| match e {
        Ok(e) => Some(e),
        Err(err) => {
            warnings.push(format!("skipping unreadable entry: {}", err));
            None
        }
    }).filter(|e| {
        e.path().is_file()
            && e.path().extension().is_some_and(|ext| ext == "json")
            && !e.file_name().to_string_lossy().starts_with('.')
    }) {
        // ... (rest of body unchanged from the previous implementation)
    }

    Ok(ReadSchemasOutput { schemas, warnings })
}
```

Update the existing `read_schemas` tests in this file to destructure `ReadSchemasOutput`:

```rust
let out = read_schemas(dir.path()).unwrap();
assert_eq!(out.schemas.schemas().len(), 2);
assert!(out.schemas.get_by_name("Angebot").is_some());
assert!(out.warnings.is_empty());
```

- [ ] **Step 6: Update `crates/bo4e-cli/src/models.rs`, `io.rs`, `utils.rs`**

Remove the now-moved declarations from each `mod.rs`-equivalent file:

```rust
// crates/bo4e-cli/src/models.rs — DELETE these lines:
// pub mod schema_meta;
// pub mod version;
// pub mod json_schema;
// pub(crate) mod macros;
// (keep cli, git, changes, config, matrix)
```

```rust
// crates/bo4e-cli/src/io.rs — DELETE the schemas line:
// pub mod schemas;
// (keep changes, cleanse, config, git, github, matrix)
```

```rust
// crates/bo4e-cli/src/utils.rs — DELETE the visitable line:
// pub mod visitable;
// (keep tokio)
```

- [ ] **Step 7: Add the `bo4e-schemas` path dependency in `crates/bo4e-cli/Cargo.toml`**

Append to `[dependencies]`:

```toml
bo4e-schemas = { path = "../bo4e-schemas" }
```

- [ ] **Step 8: Add `bo4e-schemas` to the workspace members**

In root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/bo4e-cli", "crates/bo4e-schemas"]
```

- [ ] **Step 9: Rewrite all `crate::models::{schema_meta,version,json_schema,macros}` and `crate::utils::visitable` and `crate::io::schemas` imports inside `crates/bo4e-cli/`**

Run a search to enumerate sites:

```bash
grep -rln "crate::models::schema_meta\|crate::models::version\|crate::models::json_schema\|crate::models::macros\|crate::utils::visitable\|crate::io::schemas" crates/bo4e-cli/src
```

For each file in the result, replace the prefix `crate::` with `bo4e_schemas::`. Examples:

```rust
// before
use crate::models::schema_meta::Schemas;
use crate::models::version::DirtyVersion;
use crate::utils::visitable::Visitable;
use crate::io::schemas::read_schemas;

// after
use bo4e_schemas::models::schema_meta::Schemas;
use bo4e_schemas::models::version::DirtyVersion;
use bo4e_schemas::Visitable;
use bo4e_schemas::io::schemas::read_schemas;
```

The `read_schemas` callers (`cli/diff.rs`, `cli/edit.rs`, plus any tests) must also destructure `ReadSchemasOutput`. Pattern:

```rust
let out = bo4e_schemas::io::schemas::read_schemas(input_dir).map_err(|e| e.to_string())?;
for w in &out.warnings {
    crate::cwarn!("{w}");
}
let schemas = out.schemas;
```

- [ ] **Step 10: Run the full test suite**

```bash
cargo test --workspace 2>&1 | tail -10
```

Expected: identical pass count to Task 1 Step 5. Zero warnings.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "refactor(workspace): extract bo4e-schemas crate from bo4e-cli"
```

---

### Task 3: Create the `bo4e-codegen` crate skeleton with feature flags and a no-op `generate()`

**Goal:** Add the third workspace member with its full public API surface (`generate`, `OutputType`, `Options`, `Error`), MiniJinja dependency, and feature flags. `generate()` returns `Err(Error::OutputTypeNotCompiledIn(_))` for any compiled-in variant. The `bo4e-cli` crate re-exports the same features.

**Files:**
- Create: `crates/bo4e-codegen/Cargo.toml`
- Create: `crates/bo4e-codegen/src/lib.rs`
- Create: `crates/bo4e-codegen/src/error.rs`
- Create: `crates/bo4e-codegen/src/output_type.rs`
- Create: `crates/bo4e-codegen/src/env.rs` (stub; expanded in Task 7)
- Create: `crates/bo4e-codegen/tests/skeleton.rs`
- Modify: `crates/bo4e-cli/Cargo.toml` (add codegen dep + feature passthrough)
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Write the failing test first**

Create `crates/bo4e-codegen/tests/skeleton.rs`:

```rust
use std::path::PathBuf;

#[test]
fn generate_with_compiled_out_variant_returns_specific_error() {
    // We intentionally call with no schemas in a temp dir.
    let tmp = tempfile::tempdir().unwrap();
    let schemas = bo4e_schemas::Schemas::new("v202401.0.0".parse().unwrap());

    // The variant we pass MUST be one that is compiled in (otherwise the cfg-gate
    // strips it from the enum). pydantic-v2 is compiled in by default.
    let out = bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydanticV2,
        tmp.path(),
        &bo4e_codegen::Options {
            clear_output: false,
            templates_dir: None,
        },
    );

    // Skeleton stage: every variant returns OutputTypeNotCompiledIn until Task 8 wires v2.
    assert!(matches!(out, Err(bo4e_codegen::Error::OutputTypeNotCompiledIn(_))));
}
```

- [ ] **Step 2: Verify the test fails to compile (no crate yet)**

```bash
cargo test -p bo4e-codegen 2>&1 | tail -3
```

Expected: `error: package ID specification 'bo4e-codegen' did not match any packages`.

- [ ] **Step 3: Create `crates/bo4e-codegen/Cargo.toml`**

```toml
[package]
name = "bo4e-codegen"
version = "0.1.0"
edition = "2024"

[features]
default = ["python"]
python = ["python-pydantic-v1", "python-pydantic-v2", "python-sql-model"]
python-pydantic-v1 = []
python-pydantic-v2 = []
python-sql-model   = []

[dependencies]
bo4e-schemas = { path = "../bo4e-schemas" }
minijinja    = "2"
serde_json   = "1"
thiserror    = "2"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Create `crates/bo4e-codegen/src/error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("template render error: {0}")]
    TemplateRender(#[from] minijinja::Error),

    #[error("template not found: {name}")]
    TemplateNotFound { name: String },

    #[error("schema lookup miss: {0}")]
    SchemaLookup(String),

    #[error("output type {0} is not compiled in (enable the corresponding Cargo feature)")]
    OutputTypeNotCompiledIn(&'static str),

    #[error("schema model error: {0}")]
    Schema(String),
}
```

- [ ] **Step 5: Create `crates/bo4e-codegen/src/output_type.rs`**

```rust
use clap::ValueEnum;

/// Which output type to generate. Variants are gated by Cargo features —
/// a feature compiled out removes its variant entirely so the CLI's clap
/// parser only accepts compiled-in values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[non_exhaustive]
pub enum OutputType {
    #[cfg(feature = "python-pydantic-v1")]
    #[value(name = "python-pydantic-v1")]
    PythonPydanticV1,
    #[cfg(feature = "python-pydantic-v2")]
    #[value(name = "python-pydantic-v2")]
    PythonPydanticV2,
    #[cfg(feature = "python-sql-model")]
    #[value(name = "python-sql-model")]
    PythonSqlModel,
}

impl OutputType {
    pub fn as_str(&self) -> &'static str {
        match self {
            #[cfg(feature = "python-pydantic-v1")]
            Self::PythonPydanticV1 => "python-pydantic-v1",
            #[cfg(feature = "python-pydantic-v2")]
            Self::PythonPydanticV2 => "python-pydantic-v2",
            #[cfg(feature = "python-sql-model")]
            Self::PythonSqlModel => "python-sql-model",
        }
    }
}
```

The `clap` import is needed because clap's `ValueEnum` derive lives there. Add `clap = { version = "4.5", features = ["derive"] }` to `crates/bo4e-codegen/Cargo.toml` `[dependencies]`.

- [ ] **Step 6: Create `crates/bo4e-codegen/src/env.rs` stub**

```rust
use crate::error::Error;
use std::path::Path;

pub(crate) fn make_environment(
    _templates_dir: Option<&Path>,
) -> Result<minijinja::Environment<'static>, Error> {
    Ok(minijinja::Environment::new())
}
```

(Expanded in Task 7.)

- [ ] **Step 7: Create `crates/bo4e-codegen/src/lib.rs`**

```rust
mod env;
mod error;
mod output_type;

pub use error::Error;
pub use output_type::OutputType;

use bo4e_schemas::Schemas;
use std::path::Path;

#[derive(Debug, Default)]
pub struct Options<'a> {
    pub clear_output: bool,
    pub templates_dir: Option<&'a Path>,
}

pub fn generate(
    _schemas: &Schemas,
    output_type: OutputType,
    _output_dir: &Path,
    _options: &Options,
) -> Result<(), Error> {
    Err(Error::OutputTypeNotCompiledIn(output_type.as_str()))
}
```

- [ ] **Step 8: Add the new crate to the workspace and to `bo4e-cli`'s deps**

Root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/bo4e-cli", "crates/bo4e-schemas", "crates/bo4e-codegen"]
```

Append to `crates/bo4e-cli/Cargo.toml` `[dependencies]`:

```toml
bo4e-codegen = { path = "../bo4e-codegen", default-features = false }
```

Add a `[features]` section to `crates/bo4e-cli/Cargo.toml`:

```toml
[features]
default = ["bo4e-codegen/default"]
python = ["bo4e-codegen/python"]
python-pydantic-v1 = ["bo4e-codegen/python-pydantic-v1"]
python-pydantic-v2 = ["bo4e-codegen/python-pydantic-v2"]
python-sql-model   = ["bo4e-codegen/python-sql-model"]
```

- [ ] **Step 9: Verify the skeleton test passes**

```bash
cargo test -p bo4e-codegen 2>&1 | tail -5
```

Expected: `test generate_with_compiled_out_variant_returns_specific_error ... ok` plus zero failures from the parent suite.

- [ ] **Step 10: Verify slim feature combinations build**

```bash
cargo build -p bo4e-codegen --no-default-features --features python-pydantic-v2
cargo build -p bo4e-codegen --no-default-features --features python-pydantic-v1
cargo build -p bo4e-codegen --no-default-features --features python-sql-model
cargo build -p bo4e-codegen --no-default-features
```

Expected: all four commands exit 0.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat(codegen): add bo4e-codegen crate skeleton with feature flags and stub generate()"
```

---

### Task 4: Naming utilities (module-name + field-name conversions)

**Goal:** Add `bo4e_codegen::naming` with two pure functions: `module_file_name` (for `Angebot` → `angebot`) and `to_snake_case` (for `marktlokationsId` → `marktlokations_id`). Drop-in parity uses these to derive Python module file names and snake_case field names.

**Files:**
- Create: `crates/bo4e-codegen/src/naming.rs`
- Modify: `crates/bo4e-codegen/src/lib.rs` (declare module, no re-export — internal use)

- [ ] **Step 1: Write the failing tests**

Create `crates/bo4e-codegen/src/naming.rs`:

```rust
//! Pure naming conversions used by all output types.

/// Lower-case the schema's last module segment to form its Python module file name.
/// `module_file_name(&["bo", "Angebot"])` → `"angebot"`.
pub fn module_file_name(module: &[String]) -> String {
    module.last().map(|s| s.to_ascii_lowercase()).unwrap_or_default()
}

/// Convert a JSON property name (typically camelCase) into snake_case for use as a
/// Python field name. Acronyms are treated as case-preserving runs.
/// `to_snake_case("marktlokationsId")` → `"marktlokations_id"`.
/// `to_snake_case("URL")` → `"url"`.
/// `to_snake_case("APIVersion")` → `"api_version"`.
pub fn to_snake_case(name: &str) -> String {
    // implementation in Step 3
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_file_name_lowercases_last_segment() {
        let m = vec!["bo".to_string(), "Angebot".to_string()];
        assert_eq!(module_file_name(&m), "angebot");
    }

    #[test]
    fn module_file_name_handles_single_segment() {
        let m = vec!["Typ".to_string()];
        assert_eq!(module_file_name(&m), "typ");
    }

    #[test]
    fn module_file_name_handles_already_lowercase() {
        let m = vec!["enum".to_string(), "marktrolle".to_string()];
        assert_eq!(module_file_name(&m), "marktrolle");
    }

    #[test]
    fn snake_case_basic_camel_case() {
        assert_eq!(to_snake_case("marktlokationsId"), "marktlokations_id");
    }

    #[test]
    fn snake_case_pascal_case() {
        assert_eq!(to_snake_case("MarktLokation"), "markt_lokation");
    }

    #[test]
    fn snake_case_acronym_at_start() {
        assert_eq!(to_snake_case("APIVersion"), "api_version");
    }

    #[test]
    fn snake_case_all_caps_acronym_alone() {
        assert_eq!(to_snake_case("URL"), "url");
    }

    #[test]
    fn snake_case_already_snake_case_passthrough() {
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }

    #[test]
    fn snake_case_with_digits() {
        assert_eq!(to_snake_case("v2Version"), "v2_version");
    }
}
```

Add `pub mod naming;` to `crates/bo4e-codegen/src/lib.rs` (just after `mod env;`).

- [ ] **Step 2: Run tests; expect failures from `to_snake_case`**

```bash
cargo test -p bo4e-codegen --lib naming 2>&1 | tail -20
```

Expected: 6 of 8 fail (the `to_snake_case` cases all return `""`).

- [ ] **Step 3: Implement `to_snake_case`**

Replace the empty stub with:

```rust
pub fn to_snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let chars: Vec<char> = name.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_is_lower_or_digit = i > 0
                && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let next_is_lower = i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase();
            let prev_is_upper = i > 0 && chars[i - 1].is_ascii_uppercase();
            // Insert underscore before an uppercase that begins a new word:
            // either after a lower/digit, or when an acronym ends and a new
            // capitalised word begins (UPPER followed by Upper+lower).
            if i > 0 && (prev_is_lower_or_digit || (prev_is_upper && next_is_lower)) {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
```

- [ ] **Step 4: Run tests again; expect all green**

```bash
cargo test -p bo4e-codegen --lib naming 2>&1 | tail -5
```

Expected: `test result: ok. 8 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/bo4e-codegen/src/lib.rs crates/bo4e-codegen/src/naming.rs
git commit -m "feat(codegen): add naming utilities (module_file_name, to_snake_case)"
```

---

### Task 5: JSON Schema → Python type mapping (pydantic-v2 dialect)

**Goal:** Map JSON Schema fragments to pydantic-v2 Python type strings. Output is a string like `str`, `int`, `Decimal`, `list[Angebot]`, `Typ`, `Adresse | None`. Imports are tracked via a side-channel struct (filled in Task 6).

This is a non-trivial step — the implementer must read the Python parity reference. Direct the implementer there and provide the test driver and minimal first cases.

**Parity reference reading list (read these BEFORE implementing):**
- `/tmp/bo4e-cli-python/src/bo4e_cli/generate/python/parser.py` (338 lines) — the type-mapping logic.
- `/tmp/bo4e-cli-python/src/bo4e_cli/generate/python/imports.py` (133 lines) — collected imports per case.
- `/tmp/bo4e-cli-python/src/bo4e_cli/generate/python/custom_templates/BaseModel.jinja2` — what the templates expect to receive.

**Files:**
- Create: `crates/bo4e-codegen/src/python/mod.rs`
- Create: `crates/bo4e-codegen/src/python/types.rs`
- Modify: `crates/bo4e-codegen/src/lib.rs` (declare `mod python;` gated by `#[cfg(any(...python features))]`)

- [ ] **Step 1: Wire the python sub-module gated by the feature**

Add to `crates/bo4e-codegen/src/lib.rs`:

```rust
#[cfg(any(
    feature = "python-pydantic-v1",
    feature = "python-pydantic-v2",
    feature = "python-sql-model",
))]
mod python;
```

Create `crates/bo4e-codegen/src/python/mod.rs`:

```rust
pub(crate) mod types;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/bo4e-codegen/src/python/types.rs`:

```rust
//! JSON Schema → Python type-string mapping.
//!
//! Each function returns a `MappedType { rendered, imports }` where `rendered`
//! is the type as it should appear inline in generated code, and `imports` is
//! the set of imports it depends on. The caller (the per-output-type generator)
//! merges these imports into the file's import block.

use bo4e_schemas::models::json_schema::SchemaType;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedType {
    pub rendered: String,
    pub imports: BTreeSet<Import>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Import {
    /// `from <module> import <name>`
    Named { module: String, name: String },
    /// Relative import from a sibling generated module: `from ..<sub>.<file> import <Class>`.
    /// `from_root` is `true` if the import path is relative to the generation root.
    Sibling { module: Vec<String>, name: String },
}

pub fn map_pydantic_v2(schema_type: &SchemaType) -> MappedType {
    // implementation in Step 4
    MappedType { rendered: String::new(), imports: BTreeSet::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo4e_schemas::models::json_schema::{SchemaType, /* whatever variant constructors exist */};

    // Each test creates a SchemaType fragment and asserts the mapped output.
    // These tests are intentionally per-case so the implementer can extend the
    // mapping incrementally and see exactly which cases regress.

    #[test]
    fn maps_string_type() {
        // Construct the SchemaType variant for { "type": "string" }
        // Use whichever constructor / Default + field-set pattern bo4e_schemas exposes.
        // Read crates/bo4e-schemas/src/models/json_schema.rs to see the variants.
        // Expected result: MappedType { rendered: "str", imports: empty }
        // Replace the placeholder below once the SchemaType API is read.
        // assert_eq!(map_pydantic_v2(&t), MappedType { rendered: "str".into(), imports: BTreeSet::new() });
    }

    #[test]
    fn maps_integer_type() {
        // expected: rendered "int", no imports
    }

    #[test]
    fn maps_number_type() {
        // expected: rendered "float", no imports
    }

    #[test]
    fn maps_boolean_type() {
        // expected: rendered "bool", no imports
    }

    #[test]
    fn maps_optional_string_to_pipe_none() {
        // expected: rendered "str | None", no imports
        // (pydantic-v2 uses PEP 604 union syntax since we banned __future__)
    }

    #[test]
    fn maps_array_of_strings() {
        // expected: rendered "list[str]", no imports
    }

    #[test]
    fn maps_decimal() {
        // JSON Schema { "type": "number", "format": "decimal" } → "Decimal"
        // imports: { Named { module: "decimal", name: "Decimal" } }
    }

    #[test]
    fn maps_datetime() {
        // { "type": "string", "format": "date-time" } → "datetime"
        // imports: { Named { module: "datetime", name: "datetime" } }
    }

    #[test]
    fn maps_ref_to_sibling_class() {
        // A $ref like "#/$defs/Adresse" or whatever BO4E uses → "Adresse"
        // imports: { Sibling { module: vec!["com", "Adresse"], name: "Adresse" } }
        // This case requires reading parser.py to mirror exactly how Python resolves refs.
    }
}
```

The implementer fills in the test bodies after reading `crates/bo4e-schemas/src/models/json_schema.rs` for the `SchemaType` variants.

- [ ] **Step 3: Read the parity reference**

```bash
sed -n '1,200p' /tmp/bo4e-cli-python/src/bo4e_cli/generate/python/parser.py
sed -n '1,140p' /tmp/bo4e-cli-python/src/bo4e_cli/generate/python/imports.py
```

Take notes on each type-mapping rule, especially:
- Ref resolution: how `$ref` strings map to sibling module paths.
- Format handlers: `date-time`, `date`, `time`, `decimal`, `uuid`.
- Enum vs object discrimination.
- Nullable handling (JSON Schema `null` in `type` array vs missing from `required`).

- [ ] **Step 4: Implement `map_pydantic_v2` to satisfy the trivial cases first**

Implement enough of `map_pydantic_v2` to pass the `string`/`integer`/`number`/`boolean` cases. Run the tests:

```bash
cargo test -p bo4e-codegen --lib python::types 2>&1 | tail -10
```

Expected: 4 tests pass (the trivial cases).

- [ ] **Step 5: Add the remaining cases incrementally**

For each remaining test (`Optional`, `array`, `Decimal`, `datetime`, `$ref`), add the matching code path and re-run the test. Commit after each green case (per TDD rhythm) so failures stay isolated.

```bash
cargo test -p bo4e-codegen --lib python::types 2>&1 | tail -10
```

- [ ] **Step 6: Verify all type tests pass**

```bash
cargo test -p bo4e-codegen --lib python::types 2>&1 | tail -5
```

Expected: every test in `python::types::tests` passes.

- [ ] **Step 7: Commit**

```bash
git add crates/bo4e-codegen/src/lib.rs crates/bo4e-codegen/src/python/
git commit -m "feat(codegen): map JSON Schema to Python pydantic-v2 type strings"
```

---

### Task 6: Import collector

**Goal:** Add `bo4e_codegen::python::imports::ImportBlock` that collects `Import` values (from Task 5), deduplicates, and renders a deterministic import block: stdlib first, then third-party (`pydantic`), then relative imports, alphabetised within each block.

**Files:**
- Create: `crates/bo4e-codegen/src/python/imports.rs`
- Modify: `crates/bo4e-codegen/src/python/mod.rs` (re-export)

- [ ] **Step 1: Write the failing tests**

Create `crates/bo4e-codegen/src/python/imports.rs`:

```rust
use crate::python::types::Import;
use std::collections::BTreeSet;

/// A registry of imports collected while rendering a single module file.
/// `render()` produces the deterministic import block.
#[derive(Debug, Default)]
pub struct ImportBlock {
    items: BTreeSet<Import>,
}

impl ImportBlock {
    pub fn new() -> Self { Self::default() }

    pub fn extend<I: IntoIterator<Item = Import>>(&mut self, items: I) {
        self.items.extend(items);
    }

    pub fn render(&self, module_path_depth: usize) -> String {
        // Implemented in Step 3.
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(module: &str, name: &str) -> Import {
        Import::Named { module: module.into(), name: name.into() }
    }

    fn sibling(module: &[&str], name: &str) -> Import {
        Import::Sibling {
            module: module.iter().map(|s| s.to_string()).collect(),
            name: name.to_string(),
        }
    }

    #[test]
    fn empty_block_renders_empty_string() {
        let b = ImportBlock::new();
        assert_eq!(b.render(2), "");
    }

    #[test]
    fn dedupes_same_named_import() {
        let mut b = ImportBlock::new();
        b.extend([named("decimal", "Decimal"), named("decimal", "Decimal")]);
        let out = b.render(2);
        assert_eq!(out.matches("from decimal import Decimal").count(), 1);
    }

    #[test]
    fn merges_two_names_from_same_module() {
        let mut b = ImportBlock::new();
        b.extend([named("typing", "Optional"), named("typing", "Annotated")]);
        let out = b.render(2);
        assert!(out.contains("from typing import Annotated, Optional"));
    }

    #[test]
    fn orders_blocks_stdlib_then_third_party_then_relative() {
        // module_path_depth = 2 means we are at e.g. "<root>/bo/angebot.py" →
        // siblings under "com" are imported via "..com.adresse".
        let mut b = ImportBlock::new();
        b.extend([
            named("decimal", "Decimal"),
            named("pydantic", "BaseModel"),
            sibling(&["com", "Adresse"], "Adresse"),
        ]);
        let out = b.render(2);
        let stdlib_pos    = out.find("from decimal import Decimal").unwrap();
        let third_pos     = out.find("from pydantic import BaseModel").unwrap();
        let relative_pos  = out.find("from ..com.adresse import Adresse").unwrap();
        assert!(stdlib_pos < third_pos);
        assert!(third_pos < relative_pos);
    }

    #[test]
    fn relative_path_dot_count_matches_depth() {
        // depth 1 (root-level module) → ".com.adresse"
        // depth 2 (one subdir)       → "..com.adresse"
        let mut b = ImportBlock::new();
        b.extend([sibling(&["com", "Adresse"], "Adresse")]);
        assert!(b.render(1).contains("from .com.adresse import Adresse"));

        let mut b2 = ImportBlock::new();
        b2.extend([sibling(&["com", "Adresse"], "Adresse")]);
        assert!(b2.render(2).contains("from ..com.adresse import Adresse"));
    }
}
```

Wire it up: in `crates/bo4e-codegen/src/python/mod.rs`:

```rust
pub(crate) mod types;
pub(crate) mod imports;
```

- [ ] **Step 2: Run tests; expect failures**

```bash
cargo test -p bo4e-codegen --lib python::imports 2>&1 | tail -10
```

Expected: 4 of 5 fail; only `empty_block_renders_empty_string` passes.

- [ ] **Step 3: Implement `render`**

Replace the body with:

```rust
pub fn render(&self, module_path_depth: usize) -> String {
    use std::collections::BTreeMap;

    let mut stdlib: BTreeMap<&String, BTreeSet<&String>> = BTreeMap::new();
    let mut third_party: BTreeMap<&String, BTreeSet<&String>> = BTreeMap::new();
    let mut relative: BTreeMap<String, BTreeSet<&String>> = BTreeMap::new();

    let stdlib_modules = &[
        "decimal", "datetime", "uuid", "typing", "enum", "collections",
    ];

    for item in &self.items {
        match item {
            Import::Named { module, name } => {
                let bucket = if stdlib_modules.iter().any(|m| *m == module || module.starts_with(&format!("{m}."))) {
                    &mut stdlib
                } else {
                    &mut third_party
                };
                bucket.entry(module).or_default().insert(name);
            }
            Import::Sibling { module, name } => {
                let dots: String = std::iter::repeat('.').take(module_path_depth).collect();
                let last_idx = module.len() - 1;
                let dotted: String = module[..last_idx]
                    .iter()
                    .chain(std::iter::once(&module[last_idx].to_ascii_lowercase()))
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(".");
                let key = format!("{dots}{dotted}");
                relative.entry(key).or_default().insert(name);
            }
        }
    }

    fn fmt_block(block: &BTreeMap<&String, BTreeSet<&String>>) -> String {
        block
            .iter()
            .map(|(module, names)| {
                let names_csv = names.iter().cloned().cloned().collect::<Vec<_>>().join(", ");
                format!("from {module} import {names_csv}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn fmt_relative(block: &BTreeMap<String, BTreeSet<&String>>) -> String {
        block
            .iter()
            .map(|(module, names)| {
                let names_csv = names.iter().cloned().cloned().collect::<Vec<_>>().join(", ");
                format!("from {module} import {names_csv}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    [fmt_block(&stdlib), fmt_block(&third_party), fmt_relative(&relative)]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
```

The two `fmt_*` helpers differ in their key type (`&String` vs `String`); they're kept separate to avoid generic gymnastics that obscure the intent.

- [ ] **Step 4: Run tests; expect all green**

```bash
cargo test -p bo4e-codegen --lib python::imports 2>&1 | tail -5
```

Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/bo4e-codegen/src/python/imports.rs crates/bo4e-codegen/src/python/mod.rs
git commit -m "feat(codegen): add import collector with deterministic block ordering"
```

---

### Task 7: Vendor pydantic-v2 templates and wire MiniJinja loaders (embedded + `--templates-dir`)

**Goal:** Copy the three Python templates needed for pydantic-v2 (`BaseModel.jinja2`, `Enum.jinja2`, `__init__.jinja2`) into `crates/bo4e-codegen/src/templates/python/pydantic_v2/`. Wire MiniJinja's environment to load them via `include_str!` by default and via `path_loader(dir)` when `Options::templates_dir` is set. Render a "hello world" template to prove the wiring.

**Files:**
- Create: `crates/bo4e-codegen/src/templates/python/pydantic_v2/BaseModel.jinja2`
- Create: `crates/bo4e-codegen/src/templates/python/pydantic_v2/Enum.jinja2`
- Create: `crates/bo4e-codegen/src/templates/python/pydantic_v2/__init__.jinja2`
- Modify: `crates/bo4e-codegen/src/env.rs` (real loader logic)
- Add inline tests inside `env.rs`

- [ ] **Step 1: Vendor and adapt templates**

```bash
cp /tmp/bo4e-cli-python/src/bo4e_cli/generate/python/custom_templates/BaseModel.jinja2 \
   crates/bo4e-codegen/src/templates/python/pydantic_v2/BaseModel.jinja2
cp /tmp/bo4e-cli-python/src/bo4e_cli/generate/python/custom_templates/Enum.jinja2 \
   crates/bo4e-codegen/src/templates/python/pydantic_v2/Enum.jinja2
```

Create `crates/bo4e-codegen/src/templates/python/pydantic_v2/__init__.jinja2` with:

```jinja2
{% for cls in classes -%}
from .{{ cls.module_path | join('.') }} import {{ cls.name }}
{% endfor %}
```

The vendored Python templates may use Jinja2 features MiniJinja doesn't support exactly. Two known divergence points to watch for and adjust:
- `{% set %}` with `{% endset %}` blocks — supported.
- Custom Python filters (`isinstance`, `len(...)` in expressions) — `len(x)` is `x|length` in Jinja2/MiniJinja.

After vendoring, run `cargo build -p bo4e-codegen` (it won't render anything yet, just check the templates compile if your loader pre-parses them — see Step 2). Adjustments needed will surface in Task 8.

- [ ] **Step 2: Implement the real loader in `env.rs`**

Replace `crates/bo4e-codegen/src/env.rs` with:

```rust
use crate::error::Error;
use std::path::Path;

pub(crate) fn make_environment(
    templates_dir: Option<&Path>,
) -> Result<minijinja::Environment<'static>, Error> {
    let mut env = minijinja::Environment::new();
    if let Some(dir) = templates_dir {
        env.set_loader(minijinja::path_loader(dir));
    } else {
        load_embedded(&mut env)?;
    }
    Ok(env)
}

#[allow(unused_variables)]
fn load_embedded(env: &mut minijinja::Environment<'static>) -> Result<(), Error> {
    #[cfg(feature = "python-pydantic-v2")]
    {
        env.add_template(
            "python/pydantic_v2/BaseModel.jinja2",
            include_str!("templates/python/pydantic_v2/BaseModel.jinja2"),
        )?;
        env.add_template(
            "python/pydantic_v2/Enum.jinja2",
            include_str!("templates/python/pydantic_v2/Enum.jinja2"),
        )?;
        env.add_template(
            "python/pydantic_v2/__init__.jinja2",
            include_str!("templates/python/pydantic_v2/__init__.jinja2"),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use minijinja::context;

    #[cfg(feature = "python-pydantic-v2")]
    #[test]
    fn embedded_pydantic_v2_init_template_renders() {
        let env = make_environment(None).expect("env builds");
        let tpl = env.get_template("python/pydantic_v2/__init__.jinja2")
            .expect("template registered");
        let out = tpl.render(context!{
            classes => vec![
                context!{ name => "Angebot", module_path => vec!["bo", "angebot"] }
            ]
        }).unwrap();
        assert!(out.contains("from .bo.angebot import Angebot"));
    }

    #[test]
    fn disk_loader_loads_templates_from_supplied_directory() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("python/pydantic_v2");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("Hello.jinja2"), "Hello, {{ name }}!").unwrap();

        let env = make_environment(Some(dir.path())).unwrap();
        let tpl = env.get_template("python/pydantic_v2/Hello.jinja2").unwrap();
        let out = tpl.render(context!{ name => "Welt" }).unwrap();
        assert_eq!(out, "Hello, Welt!");
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p bo4e-codegen --lib env 2>&1 | tail -10
```

Expected: both tests pass. If `embedded_pydantic_v2_init_template_renders` fails because the vendored `BaseModel.jinja2` or `Enum.jinja2` has Jinja2-specific syntax MiniJinja doesn't accept, the failure will be on `add_template` (loader-side). Fix the offending construct in the template file; commit the adjustment.

- [ ] **Step 4: Commit**

```bash
git add crates/bo4e-codegen/src/env.rs crates/bo4e-codegen/src/templates/
git commit -m "feat(codegen): wire MiniJinja loader (embedded + --templates-dir override)"
```

---

### Task 8: pydantic-v2 generator orchestration

**Goal:** Add `crates/bo4e-codegen/src/python/pydantic_v2.rs`. Implement a function `generate_pydantic_v2(schemas, output_dir, env) -> Result<Vec<PathBuf>, Error>` that, for each schema:
- Decides whether the schema is an enum (has `enum` field) or a regular object.
- Builds a template context (class name, fields with snake_case + alias, mapped types, collected imports).
- Renders the appropriate template (`BaseModel.jinja2` or `Enum.jinja2`).
- Writes the output to `<output_dir>/<module_path>/<module_file>.py`.

Wire `lib.rs::generate(...)` to dispatch to this function for `OutputType::PythonPydanticV2`.

**Files:**
- Create: `crates/bo4e-codegen/src/python/pydantic_v2.rs`
- Modify: `crates/bo4e-codegen/src/python/mod.rs` (declare the new module gated by feature)
- Modify: `crates/bo4e-codegen/src/lib.rs` (dispatch for v2; switch off the OutputTypeNotCompiledIn for v2)
- Update: `crates/bo4e-codegen/tests/skeleton.rs` (the test from Task 3 must now expect `Ok(())` for v2; convert it to a positive smoke test)

- [ ] **Step 1: Update the skeleton test to a positive smoke test**

Replace the body of `crates/bo4e-codegen/tests/skeleton.rs`:

```rust
use std::path::PathBuf;

#[cfg(feature = "python-pydantic-v2")]
#[test]
fn generate_pydantic_v2_writes_at_least_one_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut schemas = bo4e_schemas::Schemas::new("v202401.0.0".parse().unwrap());

    // Add one minimal enum schema. Use bo4e_schemas's Schema::new() and load_schema()
    // with a small JSON payload — see crates/bo4e-schemas/src/io/schemas.rs tests for a pattern.
    let mut s = bo4e_schemas::Schema::new(
        vec!["enum".into(), "Typ".into()],
        None,
    ).unwrap();
    s.load_schema(r#"{"type":"string","title":"Typ","enum":["A","B"]}"#.into());
    schemas.add_schema(std::rc::Rc::new(std::cell::RefCell::new(s))).unwrap();

    bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydanticV2,
        tmp.path(),
        &bo4e_codegen::Options { clear_output: false, templates_dir: None },
    ).expect("generate");

    let typ_py = tmp.path().join("enum/typ.py");
    assert!(typ_py.exists(), "expected {:?} to exist", typ_py);
    let body = std::fs::read_to_string(&typ_py).unwrap();
    assert!(body.contains("class Typ"));
}
```

Run it; expected to fail because `lib.rs::generate` still returns `OutputTypeNotCompiledIn`.

```bash
cargo test -p bo4e-codegen --test skeleton 2>&1 | tail -5
```

Expected: failure.

- [ ] **Step 2: Wire the dispatch in `lib.rs`**

Replace the `generate` function in `crates/bo4e-codegen/src/lib.rs`:

```rust
pub fn generate(
    schemas: &Schemas,
    output_type: OutputType,
    output_dir: &Path,
    options: &Options,
) -> Result<(), Error> {
    if options.clear_output {
        clear_dir_if_exists(output_dir)?;
    }

    let env = env::make_environment(options.templates_dir)?;

    match output_type {
        #[cfg(feature = "python-pydantic-v2")]
        OutputType::PythonPydanticV2 => {
            python::pydantic_v2::generate_pydantic_v2(schemas, output_dir, &env)?;
            Ok(())
        }
        // Other compiled-in variants get the not-implemented-yet error in this plan.
        #[cfg(feature = "python-pydantic-v1")]
        OutputType::PythonPydanticV1 => Err(Error::OutputTypeNotCompiledIn(output_type.as_str())),
        #[cfg(feature = "python-sql-model")]
        OutputType::PythonSqlModel   => Err(Error::OutputTypeNotCompiledIn(output_type.as_str())),
    }
}

fn clear_dir_if_exists(dir: &Path) -> Result<(), Error> {
    if dir.exists() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            } else {
                std::fs::remove_file(entry.path())?;
            }
        }
    } else {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
```

(Note: The `OutputTypeNotCompiledIn` error name is a misnomer for the v1/sql-model arms here since they *are* compiled in but not yet implemented. Keep the variant for now; Plan 2 and Plan 3 replace these arms with real implementations.)

Add `mod python;` declaration in `lib.rs` if not already present (it was added in Task 5 gated by `cfg(any(...))`).

Add `pub(crate) mod pydantic_v2;` to `crates/bo4e-codegen/src/python/mod.rs`, gated:

```rust
#[cfg(feature = "python-pydantic-v2")]
pub(crate) mod pydantic_v2;
```

- [ ] **Step 3: Implement `generate_pydantic_v2`**

Create `crates/bo4e-codegen/src/python/pydantic_v2.rs`:

```rust
use crate::error::Error;
use crate::naming::{module_file_name, to_snake_case};
use crate::python::imports::ImportBlock;
use crate::python::types::map_pydantic_v2;
use bo4e_schemas::Schemas;
use bo4e_schemas::models::json_schema::SchemaRootType;
use minijinja::{Environment, context};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct FieldCtx {
    name_snake: String,
    name_alias: String,
    type_str: String,
    /// Default expression rendered as a Python expression (e.g. "None", "__version__").
    /// `None` means the field has no default and is required.
    default_expr: Option<String>,
}

#[derive(Debug, Serialize)]
struct ClassCtx {
    name: String,
    fields: Vec<FieldCtx>,
}

#[derive(Debug, Serialize)]
struct EnumCtx {
    name: String,
    members: Vec<String>,
}

pub(crate) fn generate_pydantic_v2(
    schemas: &Schemas,
    output_dir: &Path,
    env: &Environment<'static>,
) -> Result<Vec<PathBuf>, Error> {
    let mut written = Vec::new();
    let version_str = schemas.version.to_string(); // adjust if the API differs

    for schema_rc in schemas {
        let mut schema = schema_rc.borrow_mut();
        let module = schema.module().to_vec();
        let class_name = schema.name().to_string();
        let path_segments = module
            .iter()
            .take(module.len() - 1)
            .map(|s| s.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let file_name = format!("{}.py", module_file_name(&module));

        let mut out_path = output_dir.to_path_buf();
        for seg in &path_segments {
            out_path.push(seg);
        }
        std::fs::create_dir_all(&out_path)?;
        out_path.push(&file_name);

        // Resolve the parsed JSON Schema once.
        let parsed = schema.schema().map_err(Error::Schema)?.clone();

        let depth = path_segments.len() + 1; // +1 because relative imports go up to the gen root

        let rendered = match &parsed {
            SchemaRootType::Enum(e) => {
                let ctx = EnumCtx {
                    name: class_name.clone(),
                    members: e.values_as_strings(), // adjust to actual API
                };
                let tpl = env.get_template("python/pydantic_v2/Enum.jinja2")?;
                tpl.render(context!{ cls => ctx })?
            }
            SchemaRootType::Object(o) => {
                let mut imports = ImportBlock::new();
                let mut fields = Vec::new();
                imports.extend([crate::python::types::Import::Named {
                    module: "pydantic".into(),
                    name: "BaseModel".into(),
                }]);

                for (prop_name, prop_schema) in o.properties_iter() {
                    let mapped = map_pydantic_v2(&prop_schema);
                    imports.extend(mapped.imports.iter().cloned());

                    let is_required = o.required().contains(prop_name);
                    let (type_str, default_expr) = if is_required {
                        (mapped.rendered.clone(), None)
                    } else {
                        (format!("{} | None", mapped.rendered), Some("None".into()))
                    };

                    let mut name_snake = to_snake_case(prop_name);
                    let needs_alias = name_snake != *prop_name;

                    let default_with_alias = if needs_alias {
                        match &default_expr {
                            Some(d) => Some(format!(r#"Field({}, alias="{}")"#, d, prop_name)),
                            None => Some(format!(r#"Field(alias="{}")"#, prop_name)),
                        }
                    } else {
                        default_expr.clone()
                    };
                    if needs_alias {
                        imports.extend([crate::python::types::Import::Named {
                            module: "pydantic".into(),
                            name: "Field".into(),
                        }]);
                    }
                    if prop_name == "version" {
                        // special-case from the spec: default to imported __version__
                        // imports the symbol from "..__version__"
                        // (drop_in_parity_contract, "Field defaults" bullet)
                    }

                    fields.push(FieldCtx {
                        name_snake,
                        name_alias: prop_name.clone(),
                        type_str,
                        default_expr: default_with_alias,
                    });
                }

                let ctx = ClassCtx { name: class_name.clone(), fields };
                let tpl = env.get_template("python/pydantic_v2/BaseModel.jinja2")?;
                tpl.render(context!{
                    cls => ctx,
                    imports => imports.render(depth),
                })?
            }
            other => {
                return Err(Error::Schema(format!(
                    "unsupported schema root type for {}: {:?}",
                    class_name, other
                )));
            }
        };

        std::fs::write(&out_path, rendered)?;
        written.push(out_path);
    }

    // Write __version__.py at the root.
    let version_path = output_dir.join("__version__.py");
    std::fs::write(&version_path, format!("__version__: str = \"{version_str}\"\n"))?;
    written.push(version_path);

    // Write __init__.py per directory containing files.
    let init_tpl = env.get_template("python/pydantic_v2/__init__.jinja2")?;
    let init_classes: Vec<_> = schemas
        .iter()
        .map(|s| {
            let s = s.borrow();
            let module = s.module().to_vec();
            let lower: Vec<String> = module.iter().map(|m| m.to_ascii_lowercase()).collect();
            context!{ name => s.name().to_string(), module_path => lower }
        })
        .collect();
    let init_body = init_tpl.render(context!{ classes => init_classes })?;
    let init_path = output_dir.join("__init__.py");
    std::fs::write(&init_path, init_body)?;
    written.push(init_path);

    // Write empty __init__.py for each subdirectory we wrote into.
    let mut subdirs = std::collections::BTreeSet::new();
    for schema_rc in schemas {
        let s = schema_rc.borrow();
        let module = s.module();
        if module.len() > 1 {
            subdirs.insert(module[0].to_ascii_lowercase());
        }
    }
    for sub in subdirs {
        let p = output_dir.join(&sub).join("__init__.py");
        if !p.exists() {
            std::fs::write(&p, "")?;
        }
    }

    Ok(written)
}
```

Several method names above (`SchemaRootType::Enum`, `SchemaRootType::Object`, `o.properties_iter()`, `o.required()`, `e.values_as_strings()`) are placeholders — the implementer reads `crates/bo4e-schemas/src/models/json_schema.rs` and substitutes the actual variant names and accessors. The structure is correct; the API names may shift.

Likewise the `version` special-case (commented stub) needs the implementer to add the actual `Field(default=__version__)` injection. The Python parity reference (`/tmp/bo4e-cli-python/src/bo4e_cli/generate/python/parser.py`) shows the exact behaviour.

- [ ] **Step 4: Run the smoke test**

```bash
cargo test -p bo4e-codegen --test skeleton 2>&1 | tail -10
```

Expected: passes. If template compilation fails, edit the offending `.jinja2` file in `crates/bo4e-codegen/src/templates/python/pydantic_v2/` to use MiniJinja-supported syntax, and re-run.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(codegen): implement python-pydantic-v2 generator end-to-end"
```

---

### Task 9: Rust-only integration test for pydantic-v2 against a vendored fixture

**Goal:** Vendor a small subset of the Python repo's `bo4e_rel_refs` fixture, run our generator, and assert structural properties of the generated files. This catches regressions without requiring `python3` to be installed.

**Files:**
- Create: `crates/bo4e-codegen/tests/fixtures/bo4e_min/.version`
- Create: `crates/bo4e-codegen/tests/fixtures/bo4e_min/bo/Angebot.json`
- Create: `crates/bo4e-codegen/tests/fixtures/bo4e_min/com/Adresse.json`
- Create: `crates/bo4e-codegen/tests/fixtures/bo4e_min/enum/Typ.json`
- Create: `crates/bo4e-codegen/tests/integration_pydantic_v2.rs`

- [ ] **Step 1: Vendor the fixture**

Copy three minimal schemas from `/tmp/bo4e-cli-python/unittests/test_data/bo4e_rel_refs/`. Pick the smallest examples that exercise:
- A schema with a primitive-typed field (e.g. `Angebot.version`).
- A schema referencing another schema (Angebot → Adresse).
- An enum (`Typ`).

```bash
mkdir -p crates/bo4e-codegen/tests/fixtures/bo4e_min/{bo,com,enum}
cp /tmp/bo4e-cli-python/unittests/test_data/bo4e_rel_refs/.version \
   crates/bo4e-codegen/tests/fixtures/bo4e_min/.version
cp /tmp/bo4e-cli-python/unittests/test_data/bo4e_rel_refs/bo/Angebot.json \
   crates/bo4e-codegen/tests/fixtures/bo4e_min/bo/Angebot.json
cp /tmp/bo4e-cli-python/unittests/test_data/bo4e_rel_refs/com/Adresse.json \
   crates/bo4e-codegen/tests/fixtures/bo4e_min/com/Adresse.json
cp /tmp/bo4e-cli-python/unittests/test_data/bo4e_rel_refs/enum/Typ.json \
   crates/bo4e-codegen/tests/fixtures/bo4e_min/enum/Typ.json
```

Trim each `.json` if it pulls in too many transitive refs — the goal is the smallest set that builds successfully through our generator.

- [ ] **Step 2: Write the integration test**

Create `crates/bo4e-codegen/tests/integration_pydantic_v2.rs`:

```rust
#![cfg(feature = "python-pydantic-v2")]

use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bo4e_min")
}

#[test]
fn generates_expected_files_for_minimal_fixture() {
    let tmp = tempfile::tempdir().unwrap();
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir())
        .expect("read_schemas");
    let schemas = out.schemas;

    bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydanticV2,
        tmp.path(),
        &bo4e_codegen::Options { clear_output: true, templates_dir: None },
    ).expect("generate");

    for rel in ["bo/angebot.py", "com/adresse.py", "enum/typ.py", "__version__.py", "__init__.py"] {
        let p = tmp.path().join(rel);
        assert!(p.exists(), "expected {rel} to exist");
    }
}

#[test]
fn generated_classes_have_expected_names_and_imports() {
    let tmp = tempfile::tempdir().unwrap();
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir())
        .expect("read_schemas");
    let schemas = out.schemas;

    bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydanticV2,
        tmp.path(),
        &bo4e_codegen::Options { clear_output: true, templates_dir: None },
    ).expect("generate");

    let angebot = std::fs::read_to_string(tmp.path().join("bo/angebot.py")).unwrap();
    assert!(angebot.contains("class Angebot(BaseModel):"));
    assert!(angebot.contains("from pydantic import BaseModel"));
    // No __future__ imports allowed.
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
    let out = bo4e_schemas::io::schemas::read_schemas(&fixture_dir())
        .expect("read_schemas");
    let schemas = out.schemas;

    bo4e_codegen::generate(
        &schemas,
        bo4e_codegen::OutputType::PythonPydanticV2,
        tmp.path(),
        &bo4e_codegen::Options { clear_output: true, templates_dir: None },
    ).expect("generate");

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
```

Add `walkdir = "2"` to `crates/bo4e-codegen/Cargo.toml` `[dev-dependencies]` so the third test compiles.

- [ ] **Step 3: Run integration tests**

```bash
cargo test -p bo4e-codegen --test integration_pydantic_v2 2>&1 | tail -10
```

Expected: 3 passed. If a test fails because the vendored fixture exercises a JSON Schema feature not yet covered in `map_pydantic_v2` (Task 5), add the missing case in `python/types.rs`, re-run.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "test(codegen): add integration test for python-pydantic-v2 against minimal fixture"
```

---

### Task 10: Python AST parity test (optional, requires `python3`)

**Goal:** Parse our generated output with Python's `ast` module and assert structural equivalence to what the Python generator produces. The test is skipped if `python3` is not on `$PATH`.

**Files:**
- Create: `crates/bo4e-codegen/tests/parity_pydantic_v2.rs`

- [ ] **Step 1: Write the parity test**

Create `crates/bo4e-codegen/tests/parity_pydantic_v2.rs`:

```rust
#![cfg(feature = "python-pydantic-v2")]

use std::path::PathBuf;
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bo4e_min")
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
        bo4e_codegen::OutputType::PythonPydanticV2,
        tmp.path(),
        &bo4e_codegen::Options { clear_output: true, templates_dir: None },
    ).unwrap();

    let angebot = tmp.path().join("bo/angebot.py");
    let script = format!(r#"
import ast, sys
src = open({path:?}).read()
tree = ast.parse(src)
classes = [n for n in ast.walk(tree) if isinstance(n, ast.ClassDef)]
assert len(classes) == 1, f"expected 1 class, got {{len(classes)}}"
assert classes[0].name == "Angebot", classes[0].name
bases = [b.id if isinstance(b, ast.Name) else getattr(b, "attr", "?") for b in classes[0].bases]
assert "BaseModel" in bases, f"expected BaseModel in bases, got {{bases}}"
print("ok")
"#, path = angebot.to_string_lossy().to_string());

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
cargo test -p bo4e-codegen --test parity_pydantic_v2 2>&1 | tail -10
```

Expected (with `python3` available): 1 passed.
Expected (without `python3`): 1 passed (test exits early after printing skip notice).

- [ ] **Step 3: Commit**

```bash
git add crates/bo4e-codegen/tests/parity_pydantic_v2.rs
git commit -m "test(codegen): add Python AST parity check for python-pydantic-v2"
```

---

### Task 11: Wire the `Generate` clap subcommand in `bo4e-cli`

**Goal:** Add `crates/bo4e-cli/src/cli/generate.rs` with a `Generate` clap struct, an `Executable` impl that calls `bo4e_codegen::generate(...)`, and register it in `cli/mod.rs` next to `Repo`. End-to-end smoke test via `assert_cmd`.

**Files:**
- Create: `crates/bo4e-cli/src/cli/generate.rs`
- Modify: `crates/bo4e-cli/src/cli.rs` or `crates/bo4e-cli/src/cli/mod.rs` (register subcommand)
- Modify: `crates/bo4e-cli/Cargo.toml` (`[dev-dependencies]` add `assert_cmd` if not present, and `tempfile`)
- Create: `crates/bo4e-cli/tests/generate_smoke.rs`

- [ ] **Step 1: Write the failing CLI test first**

Create `crates/bo4e-cli/tests/generate_smoke.rs`:

```rust
#![cfg(feature = "python-pydantic-v2")]

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
        .args(["-t", "python-pydantic-v2"])
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

`CARGO_BIN_EXE_bo4e` is set automatically by Cargo when running `tests/*.rs` against a binary crate.

- [ ] **Step 2: Add `tempfile` to `crates/bo4e-cli/Cargo.toml` dev-dependencies if missing**

```bash
grep -A 5 "\[dev-dependencies\]" crates/bo4e-cli/Cargo.toml
```

If `tempfile` is not present, add `tempfile = "3"`.

- [ ] **Step 3: Run the test; expect it to fail**

```bash
cargo test -p bo4e-cli --test generate_smoke 2>&1 | tail -5
```

Expected: failure with `error: unrecognized subcommand 'generate'` from clap.

- [ ] **Step 4: Add `crates/bo4e-cli/src/cli/generate.rs`**

```rust
use crate::cli::base::Executable;
use clap::Args;
use std::path::PathBuf;

/// Generate code from BO4E JSON schemas. Same flag set as the Python CLI plus
/// an optional `--templates-dir` override for the embedded MiniJinja templates.
#[derive(Args)]
pub struct Generate {
    /// Directory containing input JSON schemas.
    #[arg(short = 'i', long = "input")]
    pub input: PathBuf,

    /// Directory to write generated code to.
    #[arg(short = 'o', long = "output")]
    pub output: PathBuf,

    /// Output type. Variants are gated by Cargo features.
    #[arg(short = 't', long = "output-type", value_enum)]
    pub output_type: bo4e_codegen::OutputType,

    /// Skip clearing the output directory before writing.
    #[arg(long = "no-clear-output", action = clap::ArgAction::SetFalse, default_value_t = true)]
    pub clear_output: bool,

    /// Override embedded templates with a directory of Jinja templates.
    #[arg(long = "templates-dir")]
    pub templates_dir: Option<PathBuf>,
}

impl Executable for Generate {
    fn run(&self) -> Result<(), String> {
        let out = bo4e_schemas::io::schemas::read_schemas(&self.input)
            .map_err(|e| format!("failed to read schemas: {e}"))?;
        for w in &out.warnings {
            crate::cwarn!("{w}");
        }

        bo4e_codegen::generate(
            &out.schemas,
            self.output_type,
            &self.output,
            &bo4e_codegen::Options {
                clear_output: self.clear_output,
                templates_dir: self.templates_dir.as_deref(),
            },
        )
        .map_err(|e| e.to_string())
    }
}
```

- [ ] **Step 5: Register the subcommand**

Open the existing CLI subcommand registry. Find where `Repo` is registered (use `grep -rn "Repo(.*Repo)" crates/bo4e-cli/src/cli`). The registry is a clap `Subcommand` enum — add `Generate(Generate)` next to `Repo(Repo)`. Example diff:

```rust
// crates/bo4e-cli/src/cli/mod.rs (or cli.rs — wherever the enum lives)
pub mod generate;
pub use generate::Generate;

#[derive(Subcommand)]
pub enum Command {
    Repo(Repo),
    Generate(Generate),
    // ... others
}
```

In the `match` that dispatches `Command` variants to `Executable::run(...)`, add the new arm:

```rust
Command::Generate(g) => g.run(),
```

- [ ] **Step 6: Re-run the smoke test**

```bash
cargo test -p bo4e-cli --test generate_smoke 2>&1 | tail -10
```

Expected: passes.

- [ ] **Step 7: Run the full suite**

```bash
cargo test --workspace 2>&1 | tail -10
```

Expected: all tests pass, zero warnings.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(cli): wire bo4e generate subcommand for python-pydantic-v2"
```

---

### Task 12: README update + cargo install verification

**Goal:** Document the Rust `generate` command (replacing the Python documentation in README), document the feature-gated install workflow, and verify each feature combo cleanly installs.

**Files:**
- Modify: `README.md` (replace or update the existing `generate` section, lines documenting Python flags)

- [ ] **Step 1: Locate the existing `generate` section in README**

```bash
grep -n "generate" README.md | head
```

(The summary referenced lines 227-245 documenting the Python `bo4e generate` flags — verify the current line range.)

- [ ] **Step 2: Update the `generate` section**

Replace the existing block with:

```markdown
### `generate`

Generate Python code from BO4E JSON schemas.

```
bo4e generate -i <input-dir> -o <output-dir> -t <output-type> [--no-clear-output] [--templates-dir <dir>]
```

Flags:

| Flag | Short | Description |
|---|---|---|
| `--input` | `-i` | Directory containing input JSON schemas. |
| `--output` | `-o` | Directory to write generated code to. |
| `--output-type` | `-t` | One of `python-pydantic-v1`, `python-pydantic-v2`, `python-sql-model` (gated by Cargo feature). |
| `--no-clear-output` |  | Skip clearing the output directory before writing (default: clear). |
| `--templates-dir` |  | Override embedded templates with a directory of Jinja templates. |

#### Slim install via Cargo features

By default `cargo install bo4e-cli` includes all three Python output types. To install with only the type you need:

```
cargo install bo4e-cli --no-default-features --features python-pydantic-v2
```

Available features: `python` (umbrella), `python-pydantic-v1`, `python-pydantic-v2`, `python-sql-model`.
```

- [ ] **Step 3: Verify cargo install works for each feature combo (locally)**

```bash
cargo install --path crates/bo4e-cli --no-default-features --features python-pydantic-v2 --root /tmp/bo4e-test-v2
/tmp/bo4e-test-v2/bin/bo4e generate --help | grep -i "output-type"
```

Expected: only `python-pydantic-v2` appears as an accepted value.

```bash
cargo install --path crates/bo4e-cli --root /tmp/bo4e-test-default
/tmp/bo4e-test-default/bin/bo4e generate --help | grep -i "output-type"
```

Expected: all three Python output types appear.

(Cleanup: `rm -rf /tmp/bo4e-test-v2 /tmp/bo4e-test-default` afterwards.)

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs(generate): document Rust generate command and feature-gated install"
```

- [ ] **Step 5: Open a draft PR `rust → main` if one does not already exist**

```bash
gh pr list --repo bo4e/BO4E-CLI --head rust --base main --state open
```

If no open PR is returned:

```bash
gh pr create --draft --base main --head rust \
  --title "Rust port of BO4E-CLI (in progress)" \
  --body "$(cat <<'EOF'
## Summary

Rolling port of the Python BO4E-CLI to Rust. This branch carries the workspace conversion
plus the `python-pydantic-v2` generator end-to-end. Follow-up plans add `python-pydantic-v1`
and `python-sql-model`.

## Test plan

- [ ] `cargo test --workspace` passes
- [ ] `bo4e generate -i fixtures -o tmp -t python-pydantic-v2` produces importable Python
- [ ] AST parity test against Python output passes when `python3` is available
EOF
)"
```

If a PR is already open, leave it alone.

---

## Self-Review

**Spec coverage:** Mapped each spec section to a task.

| Spec section | Implementing task(s) |
|---|---|
| Workspace structure | T1, T2, T3 |
| Feature flags | T3 (codegen + cli passthrough) |
| `bo4e-codegen` public API | T3 |
| Template engine (MiniJinja) | T3 (dep), T7 (loader + embed) |
| Template loading (embedded + `--templates-dir`) | T7 |
| Drop-in parity contract — module layout | T8, T9 |
| Drop-in parity contract — class internals | T5 (types), T6 (imports), T8 (orchestration) |
| `__future__` ban | T9 (third integration test enforces) |
| Testing — unit | T4, T5, T6, T7 |
| Testing — Rust integration | T9 |
| Testing — Python AST parity | T10 |
| `generate` CLI subcommand | T11 |
| Install verification | T12 |
| Migration — branch strategy | All tasks commit to `rust`; T12 opens draft PR |

**Type consistency check:** `Schemas`, `Schema`, `DirtyVersion` re-exported from `bo4e_schemas`'s lib root and used consistently across tasks. `OutputType`, `Options`, `Error` defined once in T3 and used unchanged in T8/T11. `Import` and `MappedType` defined in T5 and consumed in T6/T8. The `read_schemas` return-type change (`Result<ReadSchemasOutput, String>`) is introduced in T2 Step 5 and the new struct is used in T9, T11.

**Placeholder scan:** No "TBD" / "TODO" markers. Each step shows code or exact commands. The two remaining "implementer reads X and fills in Y" loops (Task 5 type-mapping rules; Task 8 schema accessor names) are unavoidable — they require reading 500+ lines of Python and the `bo4e-schemas` JSON Schema model API, which is outside this plan's reach to enumerate. Each such loop has concrete tests and points to the parity reference.

---

## Execution Handoff

**Plan complete and saved to `docs/plans/2026-05-08-generate-command-plan.md`.** Two execution options:

1. **Subagent-Driven (recommended)** — Fresh subagent per task, two-stage review (spec compliance, then code quality) between tasks, fast iteration.
2. **Inline Execution** — Tasks executed in this session via `executing-plans`, batch execution with checkpoints.

Which approach?
