# `generate` Command — Design

**Status:** Design approved. Implementation plan to be written next via `writing-plans` skill.
**Branch:** All work commits directly to `rust`. A draft PR `rust → main` may be opened if useful for review; no separate feature branch is created.
**Parity reference:** `origin/main` of this repo (Python implementation), worktree-checked out at `/tmp/bo4e-cli-python` for cross-reference during implementation.

## Goal

Port the Python `bo4e generate` subcommand to Rust as a drop-in replacement. The generated Python code must be importable at the same module paths and expose the same attribute surfaces as the Python implementation's output. Generation logic is redesigned from scratch; only the *result* matches Python — the implementation does not.

## Non-Goals

- Languages other than Python in v1.
- User-customizable templates beyond a `--templates-dir` override.
- Byte-equivalent output. We match structurally, not lexically.
- Supporting circular references in generated SQL models. BO4E no longer contains them.
- Migrating any other existing CLI command. `repo versions` and others keep working unchanged.

## CLI Surface

`bo4e generate` keeps Python's flag set verbatim, plus one Rust-only addition (`--templates-dir`):

| Flag | Short | Type | Default | Notes |
|---|---|---|---|---|
| `--input` | `-i` | path | required | Directory containing input JSON schemas. |
| `--output` | `-o` | path | required | Directory to write generated Python code to. |
| `--output-type` | `-t` | enum | required | One of `python-pydantic-v1`, `python-pydantic-v2`, `python-sql-model`. Variants are gated by Cargo features. |
| `--clear-output` / `--no-clear-output` | — | bool | `true` | Clear `--output` before writing. Implemented via `clap::ArgAction::SetFalse` on `--no-clear-output`, mirroring the existing `--no-validate-releases` convention from `repo versions`. |
| `--templates-dir` | — | path | `None` | Override embedded templates with a directory of Jinja templates. |

Behaviour with no `--templates-dir`: templates are loaded from the binary via `include_str!`. With `--templates-dir`: MiniJinja loads templates from the supplied directory using the same logical names (e.g. `python/pydantic_v2/BaseModel.jinja2`).

## Workspace Structure

The single-binary repo becomes a Cargo workspace at the repo root. Crates live under `crates/`.

```
bo4e-cli/                          (workspace root, virtual)
├── Cargo.toml                     (workspace manifest + shared profile/lints)
└── crates/
    ├── bo4e-schemas/              (lib)
    │   └── src/
    │       ├── lib.rs
    │       ├── models/            (← src/models/{schema_meta, version}.rs moves here)
    │       └── io.rs              (← src/io/schemas.rs moves here)
    ├── bo4e-codegen/               (lib, depends on bo4e-schemas)
    │   └── src/
    │       ├── lib.rs              (public: generate, OutputType, Options, Error)
    │       ├── python/             (gated per python-* feature)
    │       │   ├── mod.rs
    │       │   ├── pydantic_v1.rs
    │       │   ├── pydantic_v2.rs
    │       │   └── sql_model.rs
    │       └── templates/          (.jinja2 files; include_str!'d per feature)
    └── bo4e-cli/                   (bin, depends on schemas + codegen)
        └── src/
            ├── main.rs
            ├── cli/                (clap subcommands; gains generate.rs)
            ├── console/            (CLI-only concern)
            ├── io/                 (git.rs, github.rs)
            ├── models/git.rs       (Reference enum — CLI input concept)
            └── ...
```

### Why this split

- **`bo4e-schemas`** owns "what BO4E schemas *are*": schema/version models and JSON read/write. Both other crates depend on it.
- **`bo4e-codegen`** owns "given schemas, produce generated code." Pure library: no CLI plumbing, no console output, no `cargo install` knowledge. Returns errors to its caller.
- **`bo4e-cli`** owns the CLI shell: clap parsing, console module, levels, git/github work. The new `generate` subcommand is a thin wrapper over `bo4e_codegen::generate(...)`.

If during implementation we discover that `bo4e-codegen` needs something currently in `bo4e-cli`, we either hoist it into `bo4e-schemas` (if schema-related) or create a new `bo4e-utils` crate. **`bo4e-codegen` must not depend on `bo4e-cli`.**

## Feature Flags

`bo4e-codegen` declares per-output-type features and a `python` umbrella:

```toml
[features]
default = ["python"]
python = ["python-pydantic-v1", "python-pydantic-v2", "python-sql-model"]
python-pydantic-v1 = []
python-pydantic-v2 = []
python-sql-model   = []
```

`bo4e-cli` re-exports them so end users install with the same names:

```toml
[features]
default = ["bo4e-codegen/default"]
python = ["bo4e-codegen/python"]
python-pydantic-v1 = ["bo4e-codegen/python-pydantic-v1"]
python-pydantic-v2 = ["bo4e-codegen/python-pydantic-v2"]
python-sql-model   = ["bo4e-codegen/python-sql-model"]
```

Slim install:
```
cargo install bo4e-cli --no-default-features --features python-pydantic-v2
```

The `OutputType` enum's variants are `#[cfg(feature = "...")]`-gated, so a slim install's `--output-type` clap parser only accepts compiled-in variants. A user passing an excluded variant gets a clap-level "invalid value" error; a programmatic call into `bo4e_codegen::generate` with a compiled-out variant returns `Error::OutputTypeNotCompiledIn`.

## `bo4e-codegen` Public API

```rust
pub use error::Error;
pub use output_type::OutputType;

pub fn generate(
    schemas: &Schemas,
    output_type: OutputType,
    output_dir: &Path,
    options: &Options,
) -> Result<(), Error>;

pub struct Options<'a> {
    pub clear_output: bool,
    pub templates_dir: Option<&'a Path>,
}

#[non_exhaustive]
pub enum OutputType {
    #[cfg(feature = "python-pydantic-v1")] PythonPydanticV1,
    #[cfg(feature = "python-pydantic-v2")] PythonPydanticV2,
    #[cfg(feature = "python-sql-model")]   PythonSqlModel,
}
```

### Design rationale

- **`Schemas` taken by reference, not loaded internally.** The CLI calls `bo4e_schemas::io::read_schemas(input_dir)?` first, then hands the result to `bo4e-codegen`. Separating I/O from generation makes codegen unit-testable without disk and avoids duplicating the schema-loading logic.
- **`Options` struct, not positional args.** Lets us grow the API (e.g., add `format_with_black`, `extra_imports`) without breaking callers.
- **`#[non_exhaustive]` `OutputType`.** Future languages add variants; downstream callers can't write exhaustive matches that break on new variants.
- **`Error` from `thiserror`.** Variants enumerated in the implementation plan: at minimum `Io`, `TemplateRender`, `TemplateNotFound`, `SchemaLookup`, `OutputTypeNotCompiledIn`. Codegen does **no** logging — it returns. The CLI calls `crate::cwarn!` / `crate::cerror!` for user-facing output.

## Template Engine

**MiniJinja**, chosen because:

- Author is Armin Ronacher (Jinja2's original creator), so syntax is the closest match to Python's Jinja2 templates we're cross-referencing for parity.
- Lightweight (~150 KB compiled, minimal transitive dependencies).
- Excellent serde integration — schema models can be passed straight to template contexts.
- Runtime engine plays well with both `include_str!` and a disk loader for the `--templates-dir` override.

Tera and Askama were considered. Tera diverges from Jinja2 in subtle places (loop semantics) which would force more template porting work. Askama is compile-time and would complicate feature-gated language selection plus the `--templates-dir` override path.

## Template Loading

```rust
fn make_environment(opts: &Options) -> Result<minijinja::Environment<'static>, Error> {
    let mut env = minijinja::Environment::new();
    if let Some(dir) = opts.templates_dir {
        env.set_loader(minijinja::path_loader(dir));
    } else {
        load_embedded(&mut env)?;
    }
    Ok(env)
}

fn load_embedded(env: &mut minijinja::Environment<'static>) -> Result<(), Error> {
    #[cfg(feature = "python-pydantic-v1")]
    {
        env.add_template(
            "python/pydantic_v1/BaseModel.jinja2",
            include_str!("templates/python/pydantic_v1/BaseModel.jinja2"),
        )?;
        // ... other v1 templates
    }
    #[cfg(feature = "python-pydantic-v2")] { /* analogous */ }
    #[cfg(feature = "python-sql-model")]   { /* analogous */ }
    Ok(())
}
```

Override semantics: `--templates-dir` causes MiniJinja to look up the same logical names on disk. Users can copy `crates/bo4e-codegen/src/templates/` to a directory, edit, and point `--templates-dir` at it. Missing-template lookups surface as `Error::TemplateNotFound { name }`.

## Drop-in Parity Contract

The output must be importable at the same paths and expose the same attribute surfaces as Python's generator.

### Module layout

- `bo/Angebot.json` → `<output>/bo/angebot.py` containing `class Angebot(BaseModel)`. Last path segment is lowercased to form the Python module file name; class keeps PascalCase from the JSON `title`.
- `enum/Typ.json` → `<output>/enum/typ.py` containing `class Typ(StrEnum)`.
- `<output>/__version__.py` exporting `__version__: str = "<DirtyVersion>"`.
- `<output>/__init__.py` re-exporting all generated classes (matches Python's `bo4e_init_file_content`).
- `<output>/<subdir>/__init__.py` per submodule (`bo`, `com`, `enum`, etc.) so `from bo4e.bo.angebot import Angebot` resolves.

### Class internals

- Class name = JSON `title`.
- Field names = JSON property names converted to snake_case, with `Field(alias=<original>)` (pydantic-v2) or `Field(alias=...)` plus `Config.allow_population_by_field_name = True` (v1) so JSON in/out keeps the original key.
- Field types map JSON Schema → Python types using the same conventions Python uses (`str`, `int`, `float`, `bool`, `datetime`, `Decimal`, `list[T]`, `Annotated[...]` where Python adds it, etc.). Optionality syntax depends on output type: `python-pydantic-v1` uses `Optional[T]`, `python-pydantic-v2` and `python-sql-model` use `T | None`. See the `__future__` ban section below for the full rule.
- Field defaults: properties not in the JSON Schema's `required` list default to `None`; the special `version` field defaults to `__version__` imported from `..__version__`; enum-valued fields have no default.
- Imports section is assembled deterministically: stdlib, then third-party (`pydantic` / `sqlmodel`), then relative imports, alphabetised within each block.

### `__future__` ban

We do **not** emit `from __future__ import annotations`. Annotations evaluate at runtime. Concretely:

- `python-pydantic-v1`: `Optional[T]`, `Union[A, B]` (Python ≤ 3.9 friendly).
- `python-pydantic-v2`: `T | None`, `A | B` (Python ≥ 3.10 syntax, evaluates fine at runtime).
- Both: `list[T]`, `dict[K, V]` instead of `List[T]`, `Dict[K, V]` (work at runtime since Python 3.9).

### Reference behaviour

For ambiguous cases not enumerated above, the four Python templates (`BaseModel.jinja2`, `Config.jinja2`, `Enum.jinja2`, `ManyLinks.jinja2`) plus `parser.py` / `sql_parser.py` / `imports.py` in `/tmp/bo4e-cli-python/src/bo4e_cli/generate/python/` are the spec. We match the *output*, not the implementation choices. We do not copy `datamodel-code-generator` behaviour; only the surface result.

### Explicit non-promises

- Whitespace and blank-line counts.
- Comment text.
- Order of fields within a class beyond JSON property insertion order.

## Testing Strategy

Three layers, each catching a distinct failure class.

### 1. Unit tests in `bo4e-codegen` (Rust-only)

- Module-name conversion (`Angebot` → `angebot`, edge cases).
- Field-name conversion (`marktlokationsId` → `marktlokations_id`).
- Import deduplication / ordering.
- Type-mapping table: JSON Schema fragment → expected Python type string.
- Per-template render: feed a synthetic `Schema`, render, assert key substrings (`class Angebot(BaseModel):`, `version: str = __version__`, `Field(alias="marktlokationsId")`).

### 2. Integration tests in `bo4e-codegen` (require `python3` in CI)

- Vendor a small subset of `bo4e_rel_refs` under `crates/bo4e-codegen/tests/fixtures/`.
- Run `generate(...)` to a `tempfile::TempDir`.
- Shell out to `python3 -c "import ast; print(ast.dump(ast.parse(open(...).read())))"` and assert structural elements (class name, base classes, field annotations) against expected values.
- Mirrors the Python repo's own test pattern (`unittests/cli/generate/test_python.py`).
- Skipped locally if `python3` is unavailable; gated on its presence in CI.

### 3. End-to-end parity test (require `python3`)

- Given a fixture input directory, run our generator into `out_rust/` and the Python generator into `out_python/`.
- For each `.py` file, parse both with Python's `ast`, walk the AST, assert class names, base classes, field annotations, and `Field(alias=...)` values match. Whitespace and comments are ignored because we compare nodes, not text.
- This is the canonical drop-in proof. If it passes for v1, v2, and sql-model, the design works.

### Explicitly *not* doing

- Byte-diff against Python output.
- Run the Python generator inside Rust unit tests.
- Re-implement Python's AST walker — we use Python for the AST step.

## Migration Plan

All work commits directly to `rust`. A draft PR `rust → main` may be opened for ongoing review; no separate feature branch is created.

### Phase 1 — workspace skeleton, no behavioural change

- Convert repo root `Cargo.toml` into a virtual workspace.
- Move all existing source under `crates/bo4e-cli/`.
- Update CI configs, devcontainer paths, and any tooling that references `src/`.
- `cargo test --workspace` passes; the existing test suite remains green.
- One commit, mostly path renames.

### Phase 2 — extract `bo4e-schemas`

- Create `crates/bo4e-schemas/` with `Cargo.toml` and `src/lib.rs`.
- Move `crates/bo4e-cli/src/models/{schema_meta,version}.rs` → `crates/bo4e-schemas/src/models/`.
- Move `crates/bo4e-cli/src/io/schemas.rs` → `crates/bo4e-schemas/src/io.rs`.
- Add `bo4e-schemas` as a path dependency of `bo4e-cli`. Update imports.
- Existing tests still pass.

### Phase 3 — `bo4e-codegen` skeleton

- Create `crates/bo4e-codegen/` with the public API surface (`generate`, `Options`, `OutputType`, `Error`), `minijinja` dependency, feature flags wired to empty `cfg`-gated modules.
- `generate(...)` returns `Err(Error::OutputTypeNotCompiledIn(_))` for every variant.
- Add a no-op test asserting the error path.
- Workspace builds with each feature combination.

### Phase 4 — implement output types

Order: `python-pydantic-v2` → `python-pydantic-v1` → `python-sql-model`. Per output type:

1. Vendor the corresponding Python `.jinja2` templates into `crates/bo4e-codegen/src/templates/python/<type>/`.
2. Adapt to MiniJinja syntax (most should be near-identical).
3. Implement type-mapping + import-collection in `crates/bo4e-codegen/src/python/<type>.rs`.
4. Add unit tests + integration tests (Python `ast`-based).
5. Add AST-level parity test against Python output.

Each output type ships as its own commit (or small PR) with green tests.

### Phase 5 — wire `generate` subcommand

- Add `crates/bo4e-cli/src/cli/generate.rs` with the `Generate` clap struct and `Executable` impl that calls `bo4e_codegen::generate(...)`.
- Register in `crates/bo4e-cli/src/cli/mod.rs` alongside `Repo`.
- Update `README.md` (the existing `generate` section documents Python flags; same flags, mostly a status update from "Python" to "Rust").
- End-to-end smoke test: run `bo4e generate -i fixtures/ -o tmp/ -t python-pydantic-v2` and assert output dir is non-empty.

### Phase 6 — install verification

- Document feature usage in README (`cargo install bo4e-cli --no-default-features --features python-pydantic-v2`).
- Verify `cargo install --path crates/bo4e-cli` works with each feature combination.

## Open Questions / Risks

- **Python `ast.dump()` stability across Python versions.** AST node attributes shift between minor versions. The CI parity test should pin `python3` to a known version (or, more robustly, compare a normalised projection of `ast` nodes rather than raw `ast.dump` output).
- **MiniJinja vs Python Jinja2 syntax gaps.** Most Jinja2 syntax is supported; specific filters or macros in the Python templates may not be. Implementation phase 4 will surface these per template; mitigations are either implementing custom filters in MiniJinja or adapting the template.
- **`--templates-dir` override stability.** Users who override templates take responsibility for keeping them in sync with future schema changes. Documented limitation.

## References

- Python implementation: `origin/main` of this repo, worktree at `/tmp/bo4e-cli-python`.
- Existing Rust patterns: `crates/bo4e-cli/src/cli/repo.rs` (clap subcommand wiring); `crates/bo4e-cli/src/console/console.rs` (level-gated output).
- MiniJinja docs: https://docs.rs/minijinja/.
