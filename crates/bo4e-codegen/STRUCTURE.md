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

pub struct GenerateOutput {
    pub written: Vec<PathBuf>,
    pub diagnostics: Vec<String>,
}

// Per-flavour entry points (each behind its Cargo feature):
pub fn python::pydantic::generate(schemas, output_dir, &Options) -> Result<GenerateOutput, Error>;
pub fn python::sql_model::generate(schemas, output_dir, &Options) -> Result<GenerateOutput, Error>;
pub fn rust::plain::generate(schemas, output_dir, &Options) -> Result<GenerateOutput, Error>;
pub fn rust::crate_::generate(schemas, output_dir, &Options, &RustCrateOptions) -> Result<GenerateOutput, Error>;
```

Each per-flavour `generate` clears or creates the output dir, builds a MiniJinja environment, and writes all files. The returned `GenerateOutput.written` is every file written — the CLI uses this list for logging; `diagnostics` carries info-level per-file decision strings surfaced via `--verbose`. The CLI's subcommand enum (`GenerateFlavour` in `cli/generate.rs`) is the only runtime dispatcher; the library has no equivalent.

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

`Error` is `thiserror`-derived (`Io`, `TemplateRender`, `TemplateNotFound`, `SchemaLookup`, `Schema`, `UnclassifiableProperty`, `UnsupportedSchemaShape`, `InconsistentSchema`, `InvalidOption`). Don't introduce `anyhow` here — the CLI does the final-line printing.

## Schema invariants (validated at generate time)

Validation is decoupled from rendering. **`bo4e_codegen::validate::all_schemas(&Schemas)`** is the single public entry point: each per-flavour `generate()` calls it once at the top, before any file is written. The validator has access to the full `Schemas` collection so cross-schema checks (`$ref` resolution) happen here rather than being deferred to a renderer.

Enforced invariants (every violation raises `Error::InconsistentSchema`):

1. **`required` references `properties`.** Every name in `required` must have a matching entry in `properties` — otherwise the generated type omits the field silently, breaking the schema contract.
2. **`required ⇔ no default`** (the strict required/default invariant):
   - `required + default declared` — the default is unreachable.
   - `optional + no default` — the runtime has no fallback when the JSON key is absent.
3. **Default value matches the schema type.** A `string` property only accepts a `String` default; `integer` only `Integer`; `decimal` accepts `Integer`/`Float`/`String` (string must parse as a decimal); `boolean` only `Bool`; `Any`/`Object` only `Null`; `Array` accepts **no** default. A nullable `anyOf:[T, null]` accepts `T`'s kinds plus `Null`.
4. **Typed-format defaults are parse-checked.** `date`, `date-time`, `time`, `uuid` string defaults must parse as that format at generate time, so the renderer can emit typed constructors (`chrono::NaiveDate::from_ymd_opt(…)`, `uuid::uuid!(…)`, etc.) whose `unwrap()` paths can never fail at runtime.
5. **`$ref` defaults are resolved.** `null` defaults pass for any `$ref` target. Non-null defaults are only valid when the target resolves to a `StrEnum` and the string is one of the enum's declared members. `$ref` to an object schema with a non-null default is rejected.
6. **Property name shape.** Names must be `[A-Za-z_][A-Za-z0-9_]*` (camelCase or snake_case identifier shape); anything else can't round-trip through `to_snake_case` → `rust_field_name` / `python_attr_name`.
7. **Pure `type: null` rejected.** A property whose schema *is* `NullSchema` (rather than appearing as a branch of `anyOf:[T, null]`) has no use in BO4E.

**AllOf / AnyOf shape restrictions** are enforced symmetrically in both `rust/types.rs::map_rust` *and* `python/types.rs::map_pydantic`. Both return `Result<MappedType, UnsupportedShape>` and the orchestrators convert the error to `Error::UnsupportedSchemaShape`:
- `allOf` must have **exactly one** element. Multi-element `allOf` (intersection) is rejected.
- `anyOf` must be the `Optional` pattern: **one** non-null branch plus **one** `null` branch. Real unions and `anyOf` without a `null` branch are rejected.

No renderer special-cases field names. `_version`, `_typ`, `_id` are mapped purely from their schema shape across **all** flavours (pydantic, sql_model, rust-plain, rust-crate); `bo4e edit` changes flow through to the generated output.

## Rust path layout

All Rust output paths are computed up-front via **`bo4e_codegen::rust::path_segments(&[String])`** and **`bo4e_codegen::rust::module_paths(output_dir, module)`**:

- Every segment is lowercased.
- Every `enum` segment (at any depth) is rewritten to `enums` (`enum` is a Rust keyword and `pub mod enum;` would not compile).

The rewrite is applied recursively, in a single place, so per-schema file paths, `pub mod X;` declarations in `mod.rs`, sibling `use` imports, and root re-exports all agree by construction — no post-write disk walk to rename directories, no risk of drift between the on-disk location and the import path.

## Layout: root + arbitrary depth

`crate::layout::ModuleTree::from_schemas(schemas)` builds a tree view of where every schema lives in the output. Each tree node carries its direct leaves and its child sub-directory names; orchestrators walk the tree to emit `mod.rs` / `__init__.py` at every level.

- **Root-level schemas** (e.g. BO4E's `ZusatzAttribut.json`) live at the output root. The root `mod.rs` (or `lib.rs` in `rust-crate`) declares them via `pub mod <leaf>;` and re-exports them via `pub use <leaf>::<Class>;`. The pydantic root `__init__.py` re-exports them via `from .<leaf> import <Class>`.
- **Arbitrary depth** is supported: `foo/bar/Baz.json` produces a `mod.rs` / `__init__.py` at every intermediate level, with each level declaring its immediate children.
- The `enum/` directory is renamed to `enums/` in Rust output (`enum` is a keyword); the rename applies recursively to any `enum` segment at any depth. Python keeps `enum/` as-is since it's not a Python keyword.

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
