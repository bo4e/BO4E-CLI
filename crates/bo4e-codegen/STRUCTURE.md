# STRUCTURE.md — `bo4e-codegen`

Generates source code from a `bo4e_schemas::Schemas` collection. Today the only output family is Python (`python-pydantic`, `python-sql-model`); the architecture is built to grow.

## Purpose

- Take typed BO4E schemas in, write a complete language package out.
- Be small and self-contained: templates are embedded into the binary via `include_str!`, but a `--templates-dir` override is supported for ad-hoc tweaks.
- Surface a single, narrow public API (`generate`, `OutputType`, `Options`, `Error`) that `bo4e-cli` consumes.

## Layout

```
src/
├── lib.rs           # `generate(schemas, OutputType, output_dir, &Options) -> Result<Vec<PathBuf>, Error>`
├── output_type.rs   # `OutputType` enum (variants gated by Cargo features)
├── env.rs           # MiniJinja Environment builder; embedded + disk template loaders
├── error.rs         # `Error` (thiserror)
├── naming.rs        # Pure naming helpers: module_file_name, to_snake_case
├── python/
│   ├── mod.rs       # Cross-flavour Python helpers (PYTHON_RESERVED, python_attr_name,
│   │                #   sanitize_enum_member_name, module_paths, first_level_subdirs,
│   │                #   write_empty_subdir_inits, root_init_module_docstring)
│   ├── imports.rs   # ImportBlock — renders the per-file `from … import …` header
│   ├── types.rs     # JSON-Schema → Python type-hint mapping, default formatting, refs
│   ├── pydantic.rs  # `generate_pydantic` — per-class context + Jinja render
│   └── sql_model/
│       ├── mod.rs      # `generate_sql_model` orchestrator
│       ├── plan.rs     # Pure two-phase planner: schemas → `SqlPlan` { tables, junctions }
│       └── renderer.rs # Consumes `SqlPlan`, renders tables, many.py, __init__
└── templates/
    └── python/
        ├── pydantic/  # BaseModel, Enum, __init__ (vendored from data-model-code-generator)
        └── sql_model/ # BaseModel, Enum, Config, ManyLinks, __init__
```

Tests live in `tests/` (integration: `integration_pydantic.rs`, `integration_sql_model.rs`, `parity_*.rs`, `skeleton.rs`) and inline in each module.

## Public API

```rust
pub fn generate(
    schemas: &Schemas,
    output_type: OutputType,
    output_dir: &Path,
    options: &Options,
) -> Result<Vec<PathBuf>, Error>;

pub struct Options<'a> {
    pub clear_output: bool,
    pub templates_dir: Option<&'a Path>,
}

pub enum OutputType {            // variants behind cfg(feature = …)
    PythonPydantic,
    PythonSqlModel,
}
```

`generate` clears or creates the output dir, builds a MiniJinja environment, and dispatches on `OutputType`. The returned `Vec<PathBuf>` is every file written — the CLI uses this list for logging.

## Feature flags

```
[features]
default            = ["python"]
python             = ["python-pydantic", "python-sql-model"]
python-pydantic    = []
python-sql-model   = []
```

`OutputType` variants and template `include_str!` calls are `#[cfg(feature = …)]`-gated. With all Python features off, `OutputType` is empty and the match in `lib.rs` becomes an `unreachable!()` — that's intentional (it keeps the file well-formed without forcing a default).

## How a generator runs (pydantic example)

1. `lib::generate` clears the output dir and calls `python::pydantic::generate_pydantic`.
2. `generate_pydantic` iterates over `schemas`. For each schema:
   - Compute `(out_dir, file_name, depth)` via `python::module_paths`.
   - Build a per-class context struct (`PydanticField` / `EnumMember`) that mirrors the vendored `BaseModel.jinja2` / `Enum.jinja2` shape.
   - Render with MiniJinja; prepend an `ImportBlock` (the vendored template doesn't emit a pydantic import header — see the file-level docstring in `python/pydantic.rs` for the deliberate workarounds).
3. Emit a root `__init__.py` (with `root_init_module_docstring`), `__version__.py`, and one empty `__init__.py` per first-level subpackage directory.

`python-sql-model` follows the same skeleton but with a **two-phase plan**:

1. `plan::build_plan(schemas)` is a pure pass: it walks the schema tree and builds an immutable `SqlPlan { tables, junctions }`. Junction (many-to-many) detection lives entirely here.
2. `renderer::render_table` / `render_many` / `render_init` consume the plan, never the schemas. This keeps the renderer trivial to test against a hand-rolled `SqlPlan`.

## Templates

- Embedded at build time with `include_str!` (see `env::load_embedded`). `--templates-dir` calls `env.set_loader(path_loader(dir))` instead, so the env resolves template names against a directory.
- The pydantic templates are **byte-identical to upstream `data-model-code-generator`** so we can re-vendor with a clean `cp`. That ties the generator's per-field context shape to those templates — don't drift the field names lightly.
- `env::make_environment` installs an `unknown_method_callback` so the templates' `.items()` / `.dict()` calls on map values resolve to the `items` filter (the vendored Jinja2 templates use Python-style method syntax).

## Naming helpers (`naming.rs`, `python/mod.rs`)

- `module_file_name(["bo", "Angebot"])` → `"angebot"`.
- `to_snake_case("APIVersion")` → `"api_version"`. Acronyms collapse correctly (`"URL"` → `"url"`).
- `python_attr_name("_id")` → `"id_"` — Pydantic v2 forbids leading-underscore attrs, but `id` would shadow the builtin, so we append `_`.
- `sanitize_enum_member_name("2-01-7-001")` → `"_2_01_7_001"` — non-`[A-Za-z0-9_]` becomes `_`, digit-starters get prefixed.

These functions are pure — no IO, no globals. Test them with plain assertions.

## Error handling

`Error` is `thiserror`-derived (`Io`, `TemplateRender`, `TemplateNotFound`, `SchemaLookup`, `OutputTypeNotCompiledIn`, `Schema`, `UnclassifiableProperty`). Don't introduce `anyhow` here — the CLI does the final-line printing.

## Adding a new output type

See `AGENTS.md` §6 for the rules. The shortest path:

1. Add a feature flag in `Cargo.toml`.
2. Add a variant to `OutputType` gated by it.
3. Create `src/<language>/<flavour>/` with a `generate_<flavour>` orchestrator. Reuse `naming.rs`, the `python::module_paths` / `first_level_subdirs` helpers, and `env::make_environment`. If you find yourself copying logic from `pydantic.rs` or `sql_model/`, lift it into `mod.rs` or a new shared module instead.
4. Place templates under `src/templates/<language>/<flavour>/` and wire them up in `env::load_embedded` behind the new feature flag.
5. Add an integration test under `tests/`. The existing `parity_*.rs` and `integration_*.rs` files are good templates.

## Tests

- `tests/integration_pydantic.rs`, `tests/integration_sql_model.rs` — end-to-end generation against a fixture set in `tests/fixtures/`.
- `tests/parity_pydantic.rs`, `tests/parity_sql_model.rs` — compare output against a golden directory to catch unintended drift.
- `tests/skeleton.rs` — sanity test for the shared per-language scaffolding (root init, version file, subpackage inits).
- Unit tests live inline with each module.
