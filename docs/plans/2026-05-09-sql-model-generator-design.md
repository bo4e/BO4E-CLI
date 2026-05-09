# sql-model Generator Design

**Status:** Approved (brainstorm 2026-05-09).

**Branch:** All work commits directly to `rust`. A draft PR `rust → main` may be opened for ongoing review; no separate feature branch.

**Parity reference:** `/tmp/bo4e-cli-python/src/bo4e_cli/generate/python/sql_parser.py` and `custom_templates/{BaseModel,Config,Enum,ManyLinks}.jinja2`. This generator must produce a drop-in replacement for the Python SQLModel output: same module/file/class/field/foreign-key/junction-table names, same `Field(...)` and `Relationship(...)` argument shapes.

## Goal

Add the `python-sql-model` output type to `bo4e-codegen` so `bo4e generate -t python-sql-model -i <schemas> -o <out>` writes a Python package of SQLModel ORM classes that mirror the BO4E business objects. The output is importable at the same paths and exposes the same attribute surface as the current Python implementation.

## Non-Goals

- Generating database migrations or DDL.
- Supporting SQL dialects other than the SQLAlchemy-default (the upstream Python implementation does not).
- Resolving circular references between BOs that exist in the schema graph today; same limitations as the Python implementation (see "Out of Scope" below).
- Re-running the existing `bo4e-schemas` JSON Schema parser fixes; serde-deserialisation gaps that the Python implementation tolerates are dealt with case-by-case if they block a fixture.

## Pre-flight Cleanup

Before any sql-model code is written, drop the unused `python-pydantic-v1` flavour and rename the surviving `python-pydantic-v2` to plain `python-pydantic`. We never implemented v1 and have no consumer asking for it; carrying both names doubles the surface area for the sql-model addition.

This cleanup is mechanical, no behaviour change beyond removing v1 mentions and renaming v2 identifiers.

### Drop `python-pydantic-v1`

| Location | Change |
| --- | --- |
| `crates/bo4e-codegen/Cargo.toml` | Remove `python-pydantic-v1 = []` line; remove `"python-pydantic-v1"` from the `python` umbrella feature. |
| `crates/bo4e-cli/Cargo.toml` | Remove `python-pydantic-v1 = ["bo4e-codegen/python-pydantic-v1"]` line. |
| `crates/bo4e-codegen/src/output_type.rs` | Remove the `#[cfg(feature = "python-pydantic-v1")] PythonPydanticV1` enum variant and matching arm in `as_str`. |
| `crates/bo4e-codegen/src/lib.rs` | Remove the `#[cfg(feature = "python-pydantic-v1")]` arms from `generate(...)` and from the `mod python` `cfg(any(...))` gate. |
| `README.md` | Remove `python-pydantic-v1` from the supported-languages list, the `--output-type` flag-table allowed values, and the `cargo install --features` example list. |

### Rename `python-pydantic-v2` → `python-pydantic`

Mechanical find/replace across the workspace, scoped exactly to the locations below. The Python world has no v3 on the roadmap; the `-v2` suffix carries no future-proofing value and clutters the CLI.

| Location | Change |
| --- | --- |
| `crates/bo4e-codegen/Cargo.toml` | Rename feature `python-pydantic-v2` → `python-pydantic`. Update the `python` umbrella feature accordingly. |
| `crates/bo4e-cli/Cargo.toml` | Rename feature, including the path-dependency reference. |
| `crates/bo4e-codegen/src/output_type.rs` | `PythonPydanticV2` → `PythonPydantic`; `value(name = "python-pydantic-v2")` → `value(name = "python-pydantic")`; `as_str` returns `"python-pydantic"`. |
| `crates/bo4e-codegen/src/lib.rs` | `OutputType::PythonPydanticV2` arm → `OutputType::PythonPydantic`; `python::pydantic_v2::generate_pydantic_v2` → `python::pydantic::generate_pydantic`. |
| `crates/bo4e-codegen/src/python/mod.rs` | `pub(crate) mod pydantic_v2;` → `pub(crate) mod pydantic;` with matching `cfg`. |
| `crates/bo4e-codegen/src/python/pydantic_v2.rs` | Rename file → `pydantic.rs`; rename module-level function `generate_pydantic_v2` → `generate_pydantic`. |
| `crates/bo4e-codegen/src/templates/python/pydantic_v2/` | Rename directory → `pydantic/`. |
| `crates/bo4e-codegen/src/env.rs` | Update all `include_str!("templates/python/pydantic_v2/...")` to `templates/python/pydantic/...`; update template-name keys from `"python/pydantic_v2/..."` to `"python/pydantic/..."`. Update test names and fixture paths. |
| `crates/bo4e-codegen/src/python/types.rs` | Rename `map_pydantic_v2` → `map_pydantic`. |
| `crates/bo4e-codegen/src/python/pydantic.rs` | Update internal `use` statements (`map_pydantic_v2` → `map_pydantic`). |
| `crates/bo4e-codegen/tests/integration_pydantic_v2.rs` | Rename file → `integration_pydantic.rs`; update inner doc/comments referencing `python-pydantic-v2`. |
| `crates/bo4e-codegen/tests/parity_pydantic_v2.rs` | Rename file → `parity_pydantic.rs`; update inner references. |
| `crates/bo4e-codegen/tests/skeleton.rs` | Replace any literal `"python-pydantic-v2"` / `PythonPydanticV2` mentions. |
| `crates/bo4e-cli/tests/generate_smoke.rs` | Replace `python-pydantic-v2` arg literal with `python-pydantic`. |
| `README.md` | Replace remaining `python-pydantic-v2` mentions (there are 4: example command, flag-table allowed values, `cargo install --features` line, available-features sentence) with `python-pydantic`. |

The two existing design and plan docs (`docs/plans/2026-05-08-generate-command-{design,plan}.md`) are historical records of the work that *was* done under the v1/v2 names; they are **not** rewritten. The new spec (this file) and its successor plan use the post-cleanup names from the start.

### Cleanup verification

After the rename:

- `cargo build --workspace` and `cargo test --workspace` pass.
- `cargo install --path crates/bo4e-cli --no-default-features --features python-pydantic` succeeds and the resulting binary's `--help` shows `--output-type` accepting only `python-pydantic`.
- `cargo install --path crates/bo4e-cli` (default features) shows `--output-type` accepting `python-pydantic` and (once sql-model lands) `python-sql-model`.
- `grep -r 'pydantic[_-]v[12]'` over the workspace returns zero hits outside the historical docs in `docs/plans/`.

## CLI Surface

No new CLI flags. The cleanup makes the existing flag accept `python-pydantic` instead of `python-pydantic-v2`; this design adds `python-sql-model` to the accepted set:

| Flag | Short | Purpose |
| --- | --- | --- |
| `--output-type` | `-t` | Now `python-pydantic` or `python-sql-model` (gated by Cargo feature). |

All other generate flags (`--input`, `--output`, `--no-clear-output`, `--templates-dir`) are unchanged.

## Workspace Structure

After this design lands, `bo4e-codegen` looks like:

```text
crates/bo4e-codegen/
├── Cargo.toml                              # features: python (umbrella), python-pydantic, python-sql-model
├── src/
│   ├── lib.rs                              # generate() match arm for PythonSqlModel calls into sql_model::
│   ├── env.rs                              # include_str! templates for both pydantic + sql_model
│   ├── error.rs
│   ├── naming.rs
│   ├── output_type.rs                      # OutputType { PythonPydantic, PythonSqlModel }
│   ├── python/
│   │   ├── mod.rs                          # exposes pydantic + sql_model submodules behind cfg
│   │   ├── imports.rs                      # shared import-block builder
│   │   ├── types.rs                        # map_pydantic (shared by both flavours; see "Why a shared mapper")
│   │   ├── pydantic.rs                     # post-cleanup rename of pydantic_v2.rs
│   │   └── sql_model/
│   │       ├── mod.rs                      # generate_sql_model() — orchestration, file I/O, render
│   │       └── plan.rs                     # SqlPlan, build_plan(), and types it carries
│   └── templates/python/
│       ├── pydantic/                       # post-cleanup rename
│       │   ├── BaseModel.jinja2
│       │   ├── Enum.jinja2
│       │   └── __init__.jinja2
│       └── sql_model/
│           ├── BaseModel.jinja2            # vendored byte-identical from python custom_templates/
│           ├── Config.jinja2               # ditto
│           ├── Enum.jinja2                 # ditto
│           ├── ManyLinks.jinja2            # ditto
│           └── __init__.jinja2             # authored — same purpose as pydantic/__init__.jinja2
└── tests/
    ├── fixtures/
    │   ├── bo4e_min/                       # existing 3-schema fixture (Angebot, Adresse, Typ)
    │   └── bo4e_sql_min/                   # new fixture (see "Test fixture")
    ├── integration_pydantic.rs             # post-cleanup rename; gains assertion against bo4e_sql_min
    ├── integration_sql_model.rs            # new
    ├── parity_pydantic.rs                  # post-cleanup rename
    ├── parity_sql_model.rs                 # new
    └── skeleton.rs
```

### Why a shared type mapper

`map_pydantic_v2` (renamed to `map_pydantic` by the cleanup) emits exactly the type expressions sql-model needs at the SQLModel-class level: `str | None`, `list[Decimal]`, `Adresse | None` and so on. The sql-model pre-pass in `sql_parser.py` does *not* re-derive type strings — it accepts whatever datamodel-code-generator emits. We mirror that: the Rust sql-model generator calls into `map_pydantic` for every field that survives the pre-pass unaltered, and constructs its own annotation strings only for the special cases (id columns, foreign-key columns, relationship attributes, junction-link attributes, sa_column wrappers).

### Why a two-file `sql_model` module

`pydantic.rs` is one ~600-line file because the pydantic flavour has no schema-rewriting pre-pass — the type mapper, import collector, and per-class render are tightly coupled. The sql-model flavour adds an orthogonal concern (the pre-pass that decides which fields become foreign-keys, which become relationships, which spawn junction tables). Splitting that concern into `plan.rs` lets the pre-pass be unit-tested in isolation, and keeps `mod.rs` (the orchestrator) below the size where it stops fitting in a single mental model.

## Feature Flags

```toml
# crates/bo4e-codegen/Cargo.toml
[features]
default            = ["python"]
python             = ["python-pydantic", "python-sql-model"]
python-pydantic    = []
python-sql-model   = []
```

Same gating philosophy as the pre-cleanup setup: a feature compiled out removes the `OutputType` variant entirely, so the CLI's clap parser only accepts compiled-in values. `cargo install bo4e-cli --no-default-features --features python-sql-model` builds an sql-model-only binary.

## `bo4e-codegen` Public API

No change. `OutputType::PythonSqlModel` becomes a callable variant; the `generate(...)` signature and the `Options` / `Error` types stay as they are.

## SqlPlan Data Model

The pre-pass walks all schemas and produces an immutable `SqlPlan`. The render pass consumes the plan and writes files. Plan and render are decoupled so the plan can be unit-tested without touching disk or templates.

```rust
// crates/bo4e-codegen/src/python/sql_model/plan.rs

use bo4e_schemas::Schemas;
use std::collections::BTreeMap;

pub(crate) struct SqlPlan {
    /// All BO/COM/enum tables, keyed by their module path (e.g. `["bo", "Angebot"]`,
    /// matching `bo4e_schemas::models::schema_meta::Schema::module`). Enums become
    /// Python `StrEnum` modules (no `table=True`) but live in this map so the
    /// renderer can iterate uniformly.
    pub(crate) tables: BTreeMap<Vec<String>, TablePlan>,
    /// All M:N junction tables that need to land in `<output>/many.py`.
    pub(crate) junctions: Vec<JunctionTable>,
}

pub(crate) struct TablePlan {
    /// Same module-path key as in `SqlPlan.tables` (e.g. `["bo", "Angebot"]`).
    pub(crate) module: Vec<String>,
    pub(crate) class_name: String,
    pub(crate) is_enum: bool,
    /// Description (the schema-level `description`), used for the class docstring.
    pub(crate) description: Option<String>,
    /// The fields, in insertion order from the JSON Schema. The pre-pass has
    /// already split 1:1 references into a (ForeignKey, Relationship) pair.
    pub(crate) sql_fields: Vec<SqlField>,
}

pub(crate) enum SqlField {
    /// Plain scalar: `str`, `int`, `Decimal`, `datetime`, etc.
    /// Renders as `name: type_ = Field(default=...)`.
    Scalar {
        name: String,
        type_: String,        // e.g. "str | None", "Decimal", produced via map_pydantic
        nullable: bool,
        default: Option<String>,  // already-quoted Python default expression
        title: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>_id: UUID = Field(default=None, foreign_key="adresse.id")`.
    /// Emitted alongside its sibling `Relationship` entry.
    ForeignKey {
        name: String,             // "<field>_id"
        target_class: String,     // "Adresse"
        target_table: String,     // "adresse"
        nullable: bool,
        ondelete: Option<String>, // "SET NULL" when nullable, else None
    },
    /// `<name>: Adresse | None = Relationship(sa_relationship_kwargs={...})`.
    /// For 1:1 references; sibling of a ForeignKey above.
    Relationship {
        name: String,
        target_class: String,
        owner_class: String,      // for sa_relationship_kwargs.foreign_keys
        fk_field_name: String,    // "<field>_id"
        nullable: bool,
        docstring: Option<String>,
    },
    /// `<name>: list[Adresse] = Relationship(link_model=AngebotAdressenLink)`.
    /// For M:N references; the junction class is appended to `SqlPlan.junctions`.
    ManyRelationship {
        name: String,
        target_class: String,
        link_class: String,       // "AngebotAdressenLink"
        nullable: bool,
        docstring: Option<String>,
    },
    /// `<name>: Typ | None = Field(sa_column=Column(Enum(Typ, name="typ")))`.
    EnumColumn {
        name: String,
        enum_class: String,
        is_list: bool,
        nullable: bool,
        default: Option<String>,  // "Typ.ANGEBOT" or None
        title: Option<String>,
        docstring: Option<String>,
    },
    /// `<name>: list[Decimal] = Field(sa_column=Column(ARRAY(Numeric)))`.
    ScalarArray {
        name: String,
        py_inner: String,         // "Decimal", "str", ...
        sa_inner: &'static str,   // "Numeric", "String", ...
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

pub(crate) struct JunctionTable {
    /// `AngebotAdressenLink` — class name and table name (lower-cased).
    pub(crate) class_name: String,
    /// Owning side (the BO containing the `list[Reference]` field).
    pub(crate) owner_class: String,
    pub(crate) owner_table: String,        // "angebot"
    pub(crate) owner_id_field: String,     // "angebot_id"
    /// Referenced side (the BO appearing inside the list).
    pub(crate) target_class: String,
    pub(crate) target_table: String,       // "adresse"
    pub(crate) target_id_field: String,    // "adresse_id"
    /// Source field name on the owner (`adressen` for Angebot.adressen).
    /// Stored for diagnostic / docstring purposes only.
    pub(crate) source_field: String,
}

pub(crate) fn build_plan(schemas: &Schemas) -> SqlPlan;
```

### Pre-pass invariants

The pre-pass mirrors `adapt_parse_for_sql_model` in `sql_parser.py`. Key behaviours we replicate:

1. **`_id` field is dropped**, replaced with a synthetic `id: UUID = Field(default_factory=uuid4, primary_key=True, alias="_id")`. If the schema has no `_id`, we still synthesise one — every BO/COM table must have a primary key.
2. **1:1 reference (`{"$ref": "../com/Adresse.json#"}` or its `Optional[...]` form)** becomes two `SqlField`s: a `ForeignKey { name: "adresse_id", target_table: "adresse", nullable: <true if originally Optional, else false>, ondelete: <Some("SET NULL") if nullable else None> }` and a `Relationship { name: "adresse", target_class: "Adresse", fk_field_name: "adresse_id" }`. The order in the rendered class is FK first, then Relationship — this matches the Python output ordering.
3. **M:N reference (`list[Reference]` or its `Optional` form)** becomes a single `ManyRelationship` on the owner, plus a `JunctionTable` entry pushed onto `SqlPlan.junctions`. Junction class name is `{Owner}{PascalCase(field_name)}Link` (e.g. `Angebot` × `adressen` → `AngebotAdressenLink`). Both junction-table FKs use `primary_key=True` and `ondelete="CASCADE"`.
4. **Enum reference** becomes an `EnumColumn` (variants for nullable, list, default). The enum *target* schema produces a separate `TablePlan { is_enum: true, ... }` that the renderer turns into a Python `StrEnum` file with no `table=True`.
5. **`Any` and `list[Any]`** become `AnyColumn { is_array }` rendered as `Field(sa_column=Column(PickleType, nullable=...))` or `Column(ARRAY(PickleType), nullable=...)`.
6. **`list[<scalar>]`** becomes `ScalarArray` with the SQLAlchemy type lookup table (mirroring `SCHEMA_TYPE_AS_SQLALCHEMY_TYPE` in `sql_parser.py`):

   | JSON Schema type | SA inner |
   | --- | --- |
   | string | `String` |
   | integer | `Integer` |
   | number | `Float` |
   | boolean | `Boolean` |
   | (decimal — string with `format: decimal`) | `Numeric` |

   Unsupported inner types (e.g. `list[Reference]` doesn't reach here — it goes through case 3) cause `Error::SchemaLookup { … }`.
7. **Everything else** (plain scalars, `datetime`, `Decimal`, `Optional[<scalar>]`) flows through the existing `map_pydantic` type mapper unchanged and lands as a `Scalar` SqlField.

`build_plan` is **pure**: it takes `&Schemas` and returns `SqlPlan` with no side effects. This is what makes the unit tests fast.

## Render Orchestration

```rust
// crates/bo4e-codegen/src/python/sql_model/mod.rs

use crate::error::Error;
use bo4e_schemas::Schemas;
use minijinja::Environment;
use std::path::{Path, PathBuf};

pub(crate) fn generate_sql_model(
    schemas: &Schemas,
    output_dir: &Path,
    env: &Environment<'_>,
) -> Result<Vec<PathBuf>, Error>;
```

Behaviour:

1. Build the plan: `let plan = plan::build_plan(schemas);`.
2. For each `(path, table)` in `plan.tables`:
   - Compute output file path (`output_dir/<lower>/<file>.py`) and ensure parent dirs exist (matches the existing `pydantic` flavour).
   - Build the import block (the per-`SqlField` variants advertise which imports they need; an internal `ImportBlock` aggregates and dedupes).
   - Render the appropriate template:
     - `is_enum: true` → `python/sql_model/Enum.jinja2`.
     - `is_enum: false` → `python/sql_model/BaseModel.jinja2` with `SQL=true`-style context (the vendored `BaseModel.jinja2` switches on `SQL` to emit `, table=True` and the SQL-specific import block).
   - Write the rendered text to the file.
3. If `plan.junctions` is non-empty, render `python/sql_model/ManyLinks.jinja2` with `links=plan.junctions` and write to `output_dir/many.py` (single file at the package root, matching the Python implementation).
4. Render `python/sql_model/__init__.jinja2` and write `output_dir/__init__.py` re-exporting all generated classes (same content shape as the pydantic flavour's `__init__.py`).
5. Render `__version__.py` (re-uses the pydantic flavour's helper — `__version__.py` is template-independent, just `__version__: str = "<DirtyVersion>"`).
6. Per-subpackage `__init__.py` files (`<output>/bo/__init__.py`, `<output>/com/__init__.py`, `<output>/enum/__init__.py`) — empty, same as the pydantic flavour.
7. Return the list of written file paths.

The single `many.py` at the root (instead of one file per junction or one per subpackage) matches the Python output exactly. Junction classes referenced from within `<output>/bo/angebot.py` use `from ..many import AngebotAdressenLink`.

## Vendored Templates

The four templates `BaseModel.jinja2`, `Config.jinja2`, `Enum.jinja2`, `ManyLinks.jinja2` come from `/tmp/bo4e-cli-python/src/bo4e_cli/generate/python/custom_templates/`. They are vendored byte-identical into `crates/bo4e-codegen/src/templates/python/sql_model/` and registered via `include_str!` in `env.rs`.

The pydantic flavour got away with a subset (no `Config.jinja2`, no `ManyLinks.jinja2`) because its renderer never sets the `config` or `SQL` context keys — the Jinja `{%- if config %}` and `{%- if SQL %}` branches stay dormant. The sql-model flavour exercises both branches, so all four templates ship.

`__init__.jinja2` for the sql-model flavour is authored fresh (same as we did for pydantic), not vendored — the Python implementation has nothing equivalent because it generates `__init__.py` content from a Python helper, not a template.

### MiniJinja syntax compatibility

The vendored templates use `dict.items()` (`{%- for field_name, value in config.dict(exclude_unset=True).items() %}` in Config.jinja2). MiniJinja supports this when the value is a Rust `BTreeMap`/`HashMap` serialised through serde — confirmed against the pydantic implementation. No template edits are anticipated, but if any specific construct fails to render the implementation plan will surface it per template.

## Test Fixture

`crates/bo4e-codegen/tests/fixtures/bo4e_sql_min/` is a minimal synthesised fixture — *not* a copy from the upstream BO4E schemas — that exercises every distinct `SqlField` variant exactly once. Inventory:

- `bo/Angebot.json` — owner BO with: a primary-key `_id`, a `_typ` enum reference (default `"ANGEBOT"`), a 1:1 nullable reference (`adresse: Optional[Adresse]`), an M:N reference (`adressen: list[Adresse]`), a plain scalar (`angebotsnummer: Optional[str]`), a nullable scalar default (`angebotsdatum: Optional[datetime]`), an `Any` field, and a `list[Decimal]` field.
- `com/Adresse.json` — referenced COM with one scalar field (`ort: str`).
- `enum/Typ.json` — a `StrEnum` with two members.
- `.version` — `202401.4.0`.

`build_plan(&schemas)` over this fixture produces every `SqlField` variant + a single `JunctionTable`, which makes it the smallest input that achieves full plan-coverage.

The same fixture is also pointed at by `integration_pydantic.rs` — the user's instruction is that more semantical coverage is better. The pydantic generator should successfully render this richer fixture too (the M:N field becomes a plain `list[Adresse] | None`; the `Any` field becomes `Any | None`; the `list[Decimal]` becomes `list[Decimal] | None`). If the pydantic generator surfaces gaps in the existing implementation when fed this fixture, those are real defects to fix as part of the cleanup or out-of-scope follow-ups (decided per defect).

## CLI Wiring

Single-line change in `crates/bo4e-codegen/src/lib.rs`: replace the placeholder

```rust
#[cfg(feature = "python-sql-model")]
OutputType::PythonSqlModel => Err(Error::OutputTypeNotCompiledIn(output_type.as_str())),
```

with

```rust
#[cfg(feature = "python-sql-model")]
OutputType::PythonSqlModel => {
    python::sql_model::generate_sql_model(schemas, output_dir, &env)?;
    Ok(())
}
```

No other CLI-layer change. The `Generate` struct in `crates/bo4e-cli/src/cli/generate.rs` already accepts the `PythonSqlModel` `ValueEnum` variant once the feature is compiled in.

## Drop-in Parity Contract

The output of `bo4e generate -t python-sql-model` must be importable at the same paths and expose the same surface as the Python implementation's output. Specifically:

### Module layout

- `bo/Angebot.json` → `<output>/bo/angebot.py` containing `class Angebot(SQLModel, table=True)`.
- `com/Adresse.json` → `<output>/com/adresse.py` containing `class Adresse(SQLModel, table=True)`.
- `enum/Typ.json` → `<output>/enum/typ.py` containing `class Typ(StrEnum)` (no `table=True`).
- `<output>/many.py` containing all junction classes (e.g. `class AngebotAdressenLink(SQLModel, table=True)`).
- `<output>/__init__.py` re-exporting all generated classes and the junction classes.
- `<output>/__version__.py`.
- Per-subpackage `__init__.py` (empty).

### Class internals

- Class name = JSON `title`.
- Field names = snake_case of JSON property names; original key preserved via `Field(alias=<original>)` when they differ (`_id`, `_typ`, `_version` are common cases).
- Primary key: `id: uuid_pkg.UUID = Field(default_factory=uuid_pkg.uuid4, primary_key=True, alias="_id", title="<title>")`. The Python implementation imports `uuid as uuid_pkg`; we match this exactly.
- Foreign-key column for a 1:1 reference to `Adresse`:
  - Required: `adresse_id: uuid_pkg.UUID = Field(..., foreign_key="adresse.id")`
  - Optional: `adresse_id: uuid_pkg.UUID | None = Field(default=None, foreign_key="adresse.id", ondelete="SET NULL")`
- Relationship attribute for the same reference:
  - `adresse: Adresse | None = Relationship(sa_relationship_kwargs={"foreign_keys": ["Angebot.adresse_id"]})`
- M:N relationship attribute:
  - `adressen: list[Adresse] = Relationship(link_model=AngebotAdressenLink)`
  - `Optional[list[Adresse]]` becomes `adressen: list[Adresse] | None = Relationship(link_model=AngebotAdressenLink)`
- Enum column (nullable example): `_typ: Typ | None = Field(default=Typ.ANGEBOT, alias="_typ", ...)`
- Scalar array: `werte: list[Decimal] = Field(sa_column=Column(ARRAY(Numeric)))`
- `Any` column: `extras: Any | None = Field(sa_column=Column(PickleType, nullable=True))`

### Junction class shape

```python
class AngebotAdressenLink(SQLModel, table=True):
    """
    class linking m-n relation of tables Angebot and Adresse for field adressen.
    """
    angebot_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key="angebot.id", ondelete="CASCADE")
    """Id linking to Angebot."""
    adresse_id: uuid_pkg.UUID = Field(..., primary_key=True, foreign_key="adresse.id", ondelete="CASCADE")
    """Id linking to Adresse."""
```

This template is `ManyLinks.jinja2` rendered verbatim — the field names, default sentinel `...`, `primary_key`, `foreign_key`, and `ondelete` arguments come directly from the upstream template.

### Imports

Stitched on before writing each file (mirrors the pydantic flavour's approach for the same reason — `BaseModel.jinja2` only handles its own SQL-specific import block):

```python
import uuid as uuid_pkg
from typing import Any
from sqlalchemy import ARRAY, Column, Enum, ForeignKey, Numeric, PickleType
from sqlmodel import Field, Relationship, SQLModel
from ..com.adresse import Adresse
from ..enum.typ import Typ
from ..many import AngebotAdressenLink
```

Order: stdlib (`uuid`), `typing`, `sqlalchemy` (alphabetised within), `sqlmodel`, then relative imports (alphabetised by `from` path). This deterministic ordering is asserted by an integration test.

### Non-promises (same as the pydantic flavour)

- Whitespace and blank-line counts.
- Comment text not derived directly from a schema field's `description`.
- Order of `Field(...)` keyword arguments beyond what `ManyLinks.jinja2` and the Python `build_field_definition` helper produce (we match Python's order: `default`, `title`, `alias`, then alphabetised remainder).

## Testing Strategy

Three layers, each catching a distinct failure class.

### 1. Unit tests in `bo4e-codegen` (Rust-only)

In `src/python/sql_model/plan.rs`:

- **`build_plan` over `bo4e_sql_min`** asserts that `plan.tables` contains the expected `TablePlan`s, that `Angebot.sql_fields` contains a `ForeignKey { name: "adresse_id" }` immediately followed by a `Relationship { name: "adresse" }`, that the `ManyRelationship { name: "adressen" }` exists with `link_class: "AngebotAdressenLink"`, and that `plan.junctions` has one entry with `class_name: "AngebotAdressenLink", owner_table: "angebot", target_table: "adresse"`.
- **Each `SqlField` variant** is independently asserted from the same fixture so a regression points at a specific case.
- **Synthetic edge-case tests** for: missing `_id` in source schema (synth id is added), enum default value passes through (`default: Some("Typ.ANGEBOT".into())`), nullable vs required toggles `ondelete`.

In `src/python/sql_model/mod.rs`:

- **Per-template render smoke tests** against a synthetic small `SqlPlan` (no fixture I/O): `BaseModel.jinja2` with a `Relationship` field renders containing `"= Relationship("`; `ManyLinks.jinja2` with a junction renders containing the FK string and the docstring `"""Id linking to Angebot."""`.

### 2. Integration tests (require `python3` in CI)

`crates/bo4e-codegen/tests/integration_sql_model.rs`:

- Run `generate(schemas, OutputType::PythonSqlModel, tempdir, &Options::default())` against `bo4e_sql_min`.
- Assert directory tree: `<out>/bo/angebot.py`, `<out>/com/adresse.py`, `<out>/enum/typ.py`, `<out>/many.py`, `<out>/__init__.py`, `<out>/__version__.py`, plus the per-subpackage `__init__.py` files.
- Shell out to `python3 -c "import ast; ..."` and parse `<out>/bo/angebot.py`; assert:
  - Class `Angebot(SQLModel, table=True)` exists.
  - `id`, `adresse_id`, `adresse`, `adressen`, `_typ`, `angebotsnummer`, `angebotsdatum`, `extras`, `werte` are all present as field annotations.
  - `from ..com.adresse import Adresse` import is present.
  - `from ..many import AngebotAdressenLink` import is present.
- Parse `<out>/many.py`; assert `AngebotAdressenLink(SQLModel, table=True)` with both FK fields.
- Skipped locally if `python3` is unavailable; gated on its presence in CI.

The existing `integration_pydantic.rs` test gains an additional `generate_into_tmp(OutputType::PythonPydantic, "bo4e_sql_min")` call asserting the richer fixture renders without error and produces the M:N field as `list[Adresse] | None`.

### 3. End-to-end parity test (require `python3`)

`crates/bo4e-codegen/tests/parity_sql_model.rs` — same shape as `parity_pydantic.rs`:

- Vendor a small slice of the upstream `bo4e_rel_refs` schemas under `tests/fixtures/bo4e_sql_parity/` (a follow-up if time permits — the implementation plan can stub this and proceed without parity until the upstream Python image is available in CI).
- Run our generator and the Python generator into separate tempdirs.
- Walk Python `ast` of every `.py` file pair; assert class names, base classes, field annotations, `Field(...)` aliases, `Relationship(...)` link models match.

### Explicitly *not* doing

- Asserting byte-identity against Python output.
- Generating a real database and inserting test rows (out of scope — the test would catch SQLAlchemy-runtime bugs but not the Python static-import shape that the project actually cares about).

## Out of Scope

These items are deliberate non-goals for this design. They become candidates for follow-up plans if and when there's a concrete consumer asking.

- **Circular references between BOs.** The Python implementation uses string-based forward references (`"Angebot"`) when SQLAlchemy needs them. We replicate the exact same behaviour, no more. Any new circular-reference graph that breaks today's Python output is *also* broken today; fixing it is a separate, schema-side concern.
- **`bo4e-schemas` serde-deserialisation gaps.** If the sql-model fixture surfaces a JSON-Schema construct the existing Rust schema parser can't deserialise, the implementation plan flags it; resolving it is filed as a separate task on the schema crate, not this generator.
- **`ConstantSchema` TODO.** The current pydantic generator has a known TODO around constant-valued schema fragments. The sql-model generator inherits the same gap and does not attempt to close it.
- **Migration generation, DDL emission, alembic integration.** The output is plain Python ORM classes; downstream tooling consumes them.
- **Configurable enum table name strategy.** The Python implementation hard-codes `Enum(<Cls>, name="<lower>")`. We match that exactly.

## Migration Plan

The full task decomposition lives in the implementation plan that follows this design. The phases at a glance:

1. **Pre-flight cleanup** — drop v1, rename v2 → pydantic. Single commit per logical step (drop v1; rename feature; rename module/file; rename templates dir; rename test files; update README). All workspace tests stay green throughout.
2. **`SqlPlan` data model + `build_plan`** with unit tests against the synthetic `bo4e_sql_min` fixture. No file I/O, no templates yet.
3. **Vendor sql-model templates** into `templates/python/sql_model/`, register in `env.rs`. Add per-template render smoke tests.
4. **Render orchestration** `generate_sql_model` with import-block aggregation. Wire the lib.rs match arm.
5. **Integration test** against `bo4e_sql_min` (skip-if-python3-missing).
6. **Cross-fixture coverage**: feed `bo4e_sql_min` to `integration_pydantic.rs`; address any gaps.
7. **Parity test** stub (full fixture wiring is a follow-up if upstream Python image is unavailable in CI).
8. **README** — document `python-sql-model` in the same sections that pydantic occupies post-cleanup.

## Open Questions / Risks

- **Python `ast.dump()` stability across Python versions.** Same risk as the pydantic parity test. Mitigated by the same approach: pin or normalise.
- **Junction-class collisions when two BOs both reference the same target via the same field name.** The `{Owner}{PascalCase(field_name)}Link` naming is collision-free *given a unique owner* but two owners using the same field name (`adressen`) and target (`Adresse`) would each get their own junction (`AngebotAdressenLink`, `VertragAdressenLink`) — distinct, no collision. Documented for future schema authors.
- **Self-referencing M:N (e.g. an `Angebot` with `verwandte_angebote: list[Angebot]`)** is not present in current schemas. If it appears, both endpoints of the junction are `angebot.id` — the Python implementation handles this; we replicate the behaviour but won't have fixture coverage until a real schema needs it.
- **`map_pydantic` rename is unconditional.** Even if a future v3-pydantic flavour appears, the type-mapping function will still be named `map_pydantic`, and a new flavour-specific mapper would be added alongside it. Documented so we don't get confused by the lack of a version suffix.

## References

- Python implementation: `/tmp/bo4e-cli-python/src/bo4e_cli/generate/python/sql_parser.py` and `custom_templates/`.
- Existing Rust patterns: `crates/bo4e-codegen/src/python/pydantic_v2.rs` (becomes `pydantic.rs` post-cleanup); `crates/bo4e-codegen/src/python/imports.rs`; `crates/bo4e-codegen/src/env.rs`.
- Prior design: `docs/plans/2026-05-08-generate-command-design.md` (overall architecture, template engine choice, drop-in parity contract).
- MiniJinja docs: https://docs.rs/minijinja/.
