# STRUCTURE.md — `bo4e-codegen`

Generates source code from a `bo4e_schemas::Schemas` collection. Output families: Python (`python-pydantic`, `python-sql-model`) and Rust (`rust-plain`, `rust-crate`); the architecture is built to grow.

## Purpose

- Take typed BO4E schemas in, write a complete language package out.
- Be small and self-contained: templates are embedded into the binary via `include_str!`, but a `--templates-dir` override is supported for ad-hoc tweaks.
- Surface per-flavour public entry points (`python::pydantic::generate`, `python::sql_model::generate`, `rust::plain::generate`, `rust::crate_::generate`) that `bo4e-cli` consumes.

## Layout

```
src/
├── lib.rs           # `Options`, `RustCrateOptions`, `GenerateOutput`,
│                    #   `clear_dir_if_exists`, `rename_in_written`,
│                    #   `for_each_schema_file` + `SchemaCtx` (shared per-flavour iteration)
├── env.rs           # MiniJinja Environment builder; embedded + disk template loaders
├── error.rs         # `Error` (thiserror) — includes `UnsupportedSchemaShape`
├── naming.rs        # Pure naming helpers: to_snake_case, to_pascal_case, sanitize_member_name
├── layout.rs        # Output-tree layout helpers: module_file_name, module_paths,
│                    #   first_level_subdirs, first_level_subdirs_from_schemas
├── refs.rs          # JSON-Schema $ref helpers: parse_ref, schema_base, enum_ref_target
├── imports.rs       # Shared `Import` enum (Named / Sibling) + language-neutral
│                    #   helpers: `group_named_by_module`, `stitch_nonempty_blocks`
├── python/
│   ├── mod.rs       # Python-specific helpers (PYTHON_RESERVED, python_attr_name,
│   │                #   root_init_module_docstring, write_empty_subdir_inits)
│   ├── imports.rs   # ImportBlock — renders the per-file `from … import …` header
│   ├── types.rs     # JSON-Schema → Python type-hint mapping (`map_pydantic`), default formatting
│   ├── pydantic.rs  # `pub fn generate` — pydantic flavour (per-class context + Jinja render)
│   └── sql_model/
│       ├── mod.rs      # `pub fn generate` — sql-model flavour orchestrator
│       ├── plan.rs     # Pure two-phase planner: schemas → `SqlPlan { tables, junctions }`
│       └── renderer.rs # Consumes `SqlPlan`, renders tables, many.py, __init__
├── rust/
│   ├── mod.rs       # RUST_RESERVED keyword list, rust_field_name helper
│   ├── imports.rs   # UseBlock — renders the per-file `use ...;` header
│   ├── types.rs     # JSON-Schema → Rust type mapping (`map_rust`, `literal_default_rust`), `UnsupportedShape`
│   ├── render.rs    # `render_object` orchestration (discriminator detection,
│   │                #   field rendering, serde attrs) + `DefaultImplOutcome`.
│   │                #   Calls per-shape Jinja templates for struct / enum / Default-impl bodies.
│   ├── plain/mod.rs # `pub fn generate` — rust-plain flavour
│   └── crate_/mod.rs # `pub fn generate` — rust-crate flavour (wraps plain output with Cargo.toml + lib.rs)
└── templates/
    ├── python/
    │   ├── pydantic/  # BaseModel, Enum, __init__ (vendored from data-model-code-generator)
    │   └── sql_model/ # BaseModel, Enum, Config, ManyLinks, __init__
    └── rust/
        ├── plain/    # Struct, Enum, DefaultImpl, ModRs, RootModRs
        │             #   (consumed by both rust-plain and rust-crate)
        └── crate_/   # CargoToml
```

Tests live in `tests/` (integration: `integration_pydantic.rs`, `integration_sql_model.rs`, `parity_*.rs`, `skeleton.rs`) and inline in each module.

## Public API

```rust
pub struct Options<'a> {
    pub clear_output: bool,
    pub templates_dir: Option<&'a Path>,
}

#[cfg(feature = "rust-crate")]
pub struct RustCrateOptions {
    pub crate_name: String,
}

// Per-flavour entry points (each behind its Cargo feature):
pub fn python::pydantic::generate(schemas, output_dir, &Options) -> Result<Vec<PathBuf>, Error>;
pub fn python::sql_model::generate(schemas, output_dir, &Options) -> Result<Vec<PathBuf>, Error>;
pub fn rust::plain::generate(schemas, output_dir, &Options) -> Result<Vec<PathBuf>, Error>;
pub fn rust::crate_::generate(schemas, output_dir, &Options, &RustCrateOptions) -> Result<Vec<PathBuf>, Error>;
```

Each per-flavour `generate` clears or creates the output dir, builds a MiniJinja environment, and writes all files. The returned `Vec<PathBuf>` is every file written — the CLI uses this list for logging. The CLI's subcommand enum (`GenerateFlavour` in `cli/generate.rs`) is the only runtime dispatcher; the library has no equivalent.

## Shared orchestration helpers (`lib.rs`)

The per-schema iterate-render-write skeleton lives in `lib.rs` so every flavour shares the borrow lifecycle and path computation:

- `for_each_schema_file(schemas, output_dir, ext, render_fn)` — borrows each `Rc<RefCell<Schema>>`, snapshots `module` + `class_name`, computes `(out_dir, file_name, depth)` via `layout::module_paths`, drops the borrow, calls `render_fn(&SchemaCtx) -> Result<String, Error>`, writes the body, and returns every path written. Flavours that need extra per-file state (diagnostics, mod.rs reexport maps, …) capture it in the closure. Currently used by `python::pydantic::generate` and `rust::plain::generate`; `sql_model` iterates `plan.tables` instead and doesn't fit this shape.
- `rename_in_written(from, to, &mut written)` — renames `from` → `to` on disk and patches any matching entries in `written`. Works for both single-file (exact match) and directory (prefix-match) renames; no-ops idempotently when the source is missing or the target already exists. Used by `rust::plain::generate` (`enum/` → `enums/`) and `rust::crate_::generate` (`mod.rs` → `lib.rs`).
- `clear_dir_if_exists(dir)` — drives `Options::clear_output`.

## Feature flags

```
[features]
default            = ["python", "rust"]
python             = ["python-pydantic", "python-sql-model"]
rust               = ["rust-plain", "rust-crate"]
python-pydantic    = []
python-sql-model   = []
rust-plain         = []
rust-crate         = []
```

Per-flavour `generate` functions and template `include_str!` calls are `#[cfg(feature = …)]`-gated. Building with `--no-default-features --features python-pydantic` ships only the pydantic generator and its templates.

## How a generator runs (pydantic example)

1. `python::pydantic::generate` clears the output dir and iterates over `schemas`. For each schema:
   - Compute `(out_dir, file_name, depth)` via `layout::module_paths`.
   - Build a per-class context struct (`PydanticField` / `EnumMember`) that mirrors the vendored `BaseModel.jinja2` / `Enum.jinja2` shape.
   - Render with MiniJinja; prepend an `ImportBlock` (the vendored template doesn't emit a pydantic import header — see the file-level docstring in `python/pydantic.rs` for the deliberate workarounds).
3. Emit a root `__init__.py` (with `root_init_module_docstring`), `__version__.py`, and one empty `__init__.py` per first-level subpackage directory.

`python::sql_model::generate` follows the same skeleton but with a **two-phase plan**:

1. `plan::build_plan(schemas)` is a pure pass: it walks the schema tree and builds an immutable `SqlPlan { tables, junctions }`. Junction (many-to-many) detection lives entirely here.
2. `renderer::render_table` / `render_many` / `render_init` consume the plan, never the schemas. This keeps the renderer trivial to test against a hand-rolled `SqlPlan`.

## Templates

- Embedded at build time with `include_str!` (see `env::load_embedded`). `--templates-dir` calls `env.set_loader(path_loader(dir))` instead, so the env resolves template names against a directory.
- The pydantic templates are **byte-identical to upstream `data-model-code-generator`** so we can re-vendor with a clean `cp`. That ties the generator's per-field context shape to those templates — don't drift the field names lightly.
- The Rust templates we own outright. Every generated artifact goes through Jinja — including the small ones (`Enum.jinja2` covers both plain `enum Typ { Angebot, Ausschreibung, … }` and the single-variant discriminator shape via a `single_variant` bool; `DefaultImpl.jinja2` covers both the `impl Default for X { … }` and `// Default impl omitted: field \`bad\`` shapes; `CargoToml.jinja2` renders the generated crate's manifest). No raw-string builders.
- `env::make_environment` installs an `unknown_method_callback` so the pydantic templates' `.items()` / `.dict()` calls on map values resolve to the `items` filter (the vendored Jinja2 templates use Python-style method syntax).
- `imports.rs` provides language-neutral helpers shared by both `python::imports::ImportBlock` and `rust::imports::UseBlock`: `group_named_by_module` (Named variants → BTreeMap<module, BTreeSet<name>>) and `stitch_nonempty_blocks` (filter empty blocks, join with separator). Per-language Sibling rendering stays in each renderer because the path syntaxes diverge.

## Naming helpers (`naming.rs`, `layout.rs`, `python/mod.rs`)

- `module_file_name(["bo", "Angebot"])` → `"angebot"`.
- `to_snake_case("APIVersion")` → `"api_version"`. Acronyms collapse correctly (`"URL"` → `"url"`).
- `python_attr_name("_id")` → `"id_"` — Pydantic v2 forbids leading-underscore attrs, but `id` would shadow the builtin, so we append `_`.
- `sanitize_member_name("2-01-7-001")` → `"_2_01_7_001"` — non-`[A-Za-z0-9_]` becomes `_`, digit-starters get prefixed.

These functions are pure — no IO, no globals. Test them with plain assertions.

## Error handling

`Error` is `thiserror`-derived (`Io`, `TemplateRender`, `TemplateNotFound`, `SchemaLookup`, `Schema`, `UnclassifiableProperty`, `UnsupportedSchemaShape`). Don't introduce `anyhow` here — the CLI does the final-line printing.

## Adding a new output type

See `AGENTS.md` §6 for the rules. The shortest path:

1. Add a feature flag in `Cargo.toml`.
2. Create `src/<language>/<flavour>/` with a `pub fn generate` orchestrator. Reuse `naming.rs`, `layout.rs`, `refs.rs`, and `imports.rs`, as well as `env::make_environment`. If you find yourself copying logic from `python/pydantic.rs`, `python/sql_model/`, `rust/plain/`, or `rust/crate_/`, lift it into the language's `mod.rs` or a new shared module instead.
3. Place templates under `src/templates/<language>/<flavour>/` and wire them up in `env::load_embedded` behind the new feature flag.
4. Add the new flavour to `GenerateFlavour` in `bo4e-cli/src/cli/generate.rs` and dispatch it there.
5. Add an integration test under `tests/`. The existing `parity_*.rs` and `integration_*.rs` files are good templates.

## Tests

- `tests/integration_pydantic.rs`, `tests/integration_sql_model.rs`, `tests/integration_rust_plain.rs`, `tests/integration_rust_crate.rs` — end-to-end generation against a fixture set in `tests/fixtures/`.
- `tests/parity_pydantic.rs`, `tests/parity_sql_model.rs` — compare output against a golden directory to catch unintended drift.
- `tests/compile_rust_crate.rs` — generates a `rust-crate` flavour output then shells out to `cargo build` against it, so a syntactically-invalid emit fails CI directly.
- `tests/skeleton.rs` — sanity test for the shared per-language scaffolding (root init, version file, subpackage inits).
- Unit tests live inline with each module; `env.rs` carries template-load smoke tests for every embedded template.
