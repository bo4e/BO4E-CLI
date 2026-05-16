# STRUCTURE.md — BO4E-CLI workspace

High-level map of the workspace. For day-to-day rules see `AGENTS.md`; for crate internals see each crate's own `STRUCTURE.md`.

## Workspace layout

```
.
├── Cargo.toml                 # workspace manifest, profile, cargo-dist config
├── cliff.toml                 # git-cliff CHANGELOG generation config
├── CHANGELOG.md               # generated, grouped per `## X.Y.Z` section
├── README.md                  # end-user docs
├── AGENTS.md                  # AI-agent playbook
├── STRUCTURE.md               # this file
├── .github/workflows/         # ci.yml, release-prepare.yml, release.yml
├── docs/plans/                # local-only design/plan notes (gitignored)
└── crates/
    ├── bo4e-schemas/          # schema model + IO (no codegen)
    ├── bo4e-codegen/          # template-driven Python generators
    └── bo4e-cli/              # CLI commands, console, IO glue
```

## Crate dependency graph

```
bo4e-schemas  ◀────────────┐
     ▲                     │
     │                     │
bo4e-codegen  ◀── bo4e-cli (binary + lib facade)
```

- `bo4e-schemas` has zero workspace deps; it owns the canonical schema types.
- `bo4e-codegen` depends on `bo4e-schemas` and exposes per-flavour `generate(...)` entry points.
- `bo4e-cli` (binary `bo4e`) wires everything together: parses CLI args, loads/edits schemas, dispatches to codegen, prints to the console.

## Crate one-liners

- **`bo4e-schemas`** — JSON-Schema model types, a `Visitable` tree-traversal trait (object-safe, closure-based), version parsing (`Version` / `DirtyVersion`), and on-disk read/write of a `.version`-anchored schema directory. See `crates/bo4e-schemas/STRUCTURE.md`.
- **`bo4e-codegen`** — Generator core. Owns an embedded MiniJinja `Environment`, templates under `src/templates/{python,rust}/`, naming helpers, and one `pub fn generate` per flavour (`python::pydantic`, `python::sql_model`, `rust::plain`, `rust::crate_`). See `crates/bo4e-codegen/STRUCTURE.md`.
- **`bo4e-cli`** — Subcommand modules (`pull`, `edit`, `diff`, `generate`, `repo`), the BO4E console (colour palette, highlighter, spinner/progress bar), GitHub / git / config IO, and the edit transforms. Also exposes a library facade so integration tests can drive internals. See `crates/bo4e-cli/STRUCTURE.md`.

## End-to-end data flow

```
pull       : GitHub (BO4E-Schemas)  ─▶ schemas dir (.json files + .version)
edit       : schemas dir + config   ─▶ edited schemas dir
generate   : schemas dir            ─▶ Python package (pydantic / sql-model) or Rust crate / module tree
diff       : two schemas dirs       ─▶ JSON diff file (Changes)
matrix     : N diff files           ─▶ CSV/JSON compatibility matrix
version-bump : a diff file          ─▶ technical | functional | major
repo       : a BO4E-python checkout ─▶ list of version tags (CI helper)
```

Every CLI command implements the `cli::base::Executable` trait. `main.rs` is a thin shim that parses args, initialises the `CONSOLE` singleton, then calls `Executable::run`.

## Key architectural decisions

- **Shared, language-neutral helpers** live at the top of `bo4e-codegen/src/` (`naming.rs`, `layout.rs`, `refs.rs`, `imports.rs`, `validate.rs`). Per-language modules (`python/`, `rust/`) own only the bits that actually differ — type-string mapping, import-block rendering, default-expression rendering, language-specific reserved words and path conventions.
- **Schemas are the source of truth.** Everything downstream (edit, diff, generate) operates on a `Schemas` collection loaded from disk. The schema directory carries a `.version` file that captures the upstream BO4E version (plain or "dirty").
- **Per-flavour `pub fn generate`.** `bo4e-codegen` exposes one public function per flavour
  under language-named modules (`python::pydantic`, `python::sql_model`, `rust::plain`,
  `rust::crate_`). The CLI's subcommand enum (`GenerateFlavour` in `cli/generate.rs`)
  is the only runtime dispatcher; the library has no equivalent.
- **Templates over hand-written emission.** Generators build a context struct (Python flavour matters: pydantic's per-field context mirrors what the vendored `data-model-code-generator` `BaseModel.jinja2` expects) and render embedded MiniJinja templates. The CLI exposes `--templates-dir` so users can override individual templates without rebuilding the binary.
- **Strict schema invariant: `required ⇔ no default`.** Every object property must satisfy: the property is in the schema's `required` array *if and only if* it has **no** declared `default`. Both violations are rejected at generate time (`Error::InconsistentSchema`):
  - *required + default declared* — the default is unreachable because the JSON key is always present.
  - *optional + no default* — the JSON key may be absent and the runtime has no fallback; the generator refuses to invent one.

- **Strict default-rendering matrix.** "Nullable" means the schema type is `null` or `anyOf:[…, null]`. The rendered type follows the schema's nullability **only** (no auto-`Option<T>` / `| None` widening for optional non-nullable fields). The default expression comes from the schema's `default` literal, **type-precisely** rendered: typed-format string defaults (`date`, `date-time`, `time`, `uuid`) and `Decimal` defaults emit typed constructors on both sides, not raw strings.

  | `required` | `nullable` | `default`  | Rust type   | Rust serde attrs                                       | Python type  | Python default              |
  | ---------- | ---------- | ---------- | ----------- | ------------------------------------------------------ | ------------ | --------------------------- |
  | ✓          | ✗          | ✗          | `T`         | —                                                      | `T`          | no default                  |
  | ✓          | ✓          | ✗          | `Option<T>` | —                                                      | `T \| None`  | no default                  |
  | ✗          | ✗          | literal `X`| `T`         | `default = "default_<field>"`                          | `T`          | `= X`                       |
  | ✗          | ✓          | `null`     | `Option<T>` | `default, skip_serializing_if = "Option::is_none"`     | `T \| None`  | `= None`                    |
  | ✗          | ✓          | literal `X`| `Option<T>` | `default = "default_<field>"`                          | `T \| None`  | `= X` (or `= EnumName.X`)   |

  The Rust side generates **per-field `default_<field>()` helper functions** for rows 3 and 5: serde's `#[serde(default = "…")]` syntax accepts only function paths, not literal values. The helper bodies use typed constructors keyed off the schema variant + format: `chrono::NaiveDate::from_ymd_opt(…)`, `chrono::NaiveTime::from_hms_nano_opt(…)`, `chrono::DateTime::parse_from_rfc3339(…)`, `uuid::uuid!(…)`, `rust_decimal_macros::dec!(…)`. The validator parse-checks the value at generate time, so `unwrap()` paths can never fail at runtime. Bare `#[serde(default)]` is used only where the language's `T::default()` already produces the schema's literal (row 4 for `Option<T>`-null, and `EnumName::default()` shapes on row 3 where the synthetic single-variant discriminator's Default matches). `skip_serializing_if = "Option::is_none"` is emitted on row 4 only — the schema's null default and serde's None coincide there, so serialised JSON cleanly omits the key. All other rows always serialise the field.

  The Python side expresses the matrix inline (`Field(default=…)` / `= …`) with the **same type precision**: `date(2024, 1, 15)`, `time(14, 30, 0, microsecond=N)`, `datetime.fromisoformat("…")`, `UUID("…")`, `Decimal("…")`. All five constructors return immutable values, so passing them as field defaults doesn't hit pydantic's mutable-default trap. No helper functions needed.

  **Single-variant discriminator narrowing** is symmetric too: when a property's schema is `ConstantSchema` or a single-member `StrEnum` (e.g. `_typ` with `{const: "ANGEBOT", type: "string", enum: ["ANGEBOT"]}`), Rust emits a synthetic single-variant enum (`pub enum FooTyp { Angebot }`) and Python pydantic emits `Literal["ANGEBOT"]`. The `sql_model` flavour can't carry `Literal[...]` as a column annotation (SQLModel's `table=True` inference raises `TypeError`), so it synthesises its own single-member `StrEnum` class (`class AngebotTyp(StrEnum): ANGEBOT = "ANGEBOT"`) inline above the table class and uses that as the column type — symmetric with the Rust enum. Multi-member enum `$ref`s are not narrowed.

- **Schemas are editable, no special-cased field names.** Every rendering decision is driven by the schema's *shape*, never by a field's name, across `pydantic`, `rust-plain`, and `rust-crate`. `_version`, `_typ`, `_id` are not special — they're whatever the schema says they are. `bo4e edit` is free to add/remove/rename fields or flip their `required`/`default` shape, and the generated code follows. If a renderer ever needs a per-name carve-out, that's a sign the schema needs a more precise type, not the codegen needs a heuristic.

  **`sql_model` carries the one documented exception**: SQLModel's `table=True` models require a primary key, and BO4E schemas carry no shape signal that names a PK column. The SQL plan therefore synthesizes a non-nullable `uuid_pkg.UUID` PK at the `_id` slot and drops the schema's own `_id` entry. See `bo4e-codegen/STRUCTURE.md` and the `plan::synth_id_field` comment for the rationale.

- **AllOf and AnyOf are restricted (Rust *and* Python).** Both type mappers (`rust::types::map_rust`, `python::types::map_pydantic`) accept only:
  - `allOf` with **exactly one** element (used as a single-item wrapper). Multi-element `allOf` (intersection) is rejected as `UnsupportedSchemaShape`.
  - `anyOf` with **one** non-null branch and **one** `null` branch — the Optional/nullable pattern. Anything else (real unions, multiple non-null branches, missing null branch) is rejected.

  **BO4E does not use multi-branch `anyOf` / multi-element `allOf` and is not planned to.** Real unions and intersections would require sum-type emission with discriminators; the generator refuses early rather than producing surprising output. Earlier versions of the Python mapper rendered these shapes as `A | B` / `A & B` approximations — that path is gone.

- **Layout: root + arbitrary depth.** Schemas live at any directory depth under the schema root. The root holds files like `ZusatzAttribut.json` (no subdirectory) plus subdirectory groupings like `bo/`, `com/`, `enum/`. The Rust generator emits a `mod.rs` at every directory level (including the output root, where it becomes `lib.rs` in the `rust-crate` flavour) so every schema is reachable. The pydantic generator writes an empty `__init__.py` at every nested directory and re-exports all classes from the root `__init__.py` — so `from <pkg> import <Class>` works for any depth. The `enum/` directory is renamed to `enums/` in Rust output because `enum` is a Rust keyword; the rewrite is recursive (any `enum` segment at any depth becomes `enums`) and is applied **at path-build time** via `bo4e_codegen::rust::path_segments` — there is no post-write disk walk to rename directories, so on-disk paths, `pub mod X;` declarations, and sibling `use` imports agree by construction.

- **Validation is a decoupled phase.** `bo4e_codegen::validate::all_schemas(&Schemas)` runs once at the top of each `generate()` — before any file is written — over the entire schema set. The validator has access to the full collection so cross-schema checks (`$ref` defaults must reference a real enum variant) happen here rather than being deferred to a renderer. A failed schema cannot produce a half-written output tree.

  Enforced invariants, each violation surfacing as `Error::InconsistentSchema { schema, property, reason }`:
  1. Every name in `required` is also declared in `properties`.
  2. Every property is in `required` iff it has *no* schema-declared default (the strict required/default matrix).
  3. Every property's default value's primitive kind is compatible with its declared schema type (a `string` property accepts only `String`; `integer` only `Integer`; `decimal` accepts `Integer`/`Float`/`String` and parses the string as a decimal; `boolean` only `Bool`; `Any`/`Object` only `Null`; `Array` accepts no default at all; an `anyOf:[T, null]` property accepts `T`'s kinds plus `Null`).
  4. Typed-format string defaults (`date`, `date-time`, `time`, `uuid`) parse as that format at generate time.
  5. `$ref` defaults: `null` is universally accepted; non-null defaults are only valid when the target resolves (through `Schemas`) to a `StrEnum` and the string is one of the enum's declared members.
  6. Inline `ConstantSchema` / `StrEnum` defaults match their declared values (the const value, or one of the enum members).
  7. Property names are legal identifier sources (`[A-Za-z_][A-Za-z0-9_]*`), and whose post-strip-leading-underscore form is non-empty and not all underscores.
  8. Distinct JSON property names within a schema produce distinct generated field identifiers — `_id` vs `id`, `fooBar` vs `foo_bar`, `type` vs `type_` are detected before any renderer runs.
  9. Pure `type: null` properties are rejected.

- **Closure-based, object-safe `Visitable`** for schema tree traversal. Avoids `RefCell`-style runtime borrow checking; supports early termination via `ControlFlow`. Used by the `edit` transforms and by `diff` walkers.
- **Console singleton with sentinel markup.** The CLI uses a `OnceLock<Console>` so every print site (including in libraries that the CLI calls into) renders through one highlighter. `--verbose` / `--quiet` filter by `Level`. Warnings/errors always go to stderr and are never suppressed (info goes to stdout).
- **CHANGELOG by git-cliff + cargo-dist.** `cliff.toml` parses conventional-commit subjects into release sections, then `cargo-dist`'s release workflow embeds the matching `## X.Y.Z` section in GitHub Releases. Keep heading shape stable.
- **Release pipeline = cargo-dist.** `workspace.metadata.dist` configures installers (shell, PowerShell, MSI) and cross-compile targets. `pr-run-mode = "plan"` means PRs only do a dry-run.

## CI and release flow

- `.github/workflows/ci.yml` — fmt, clippy, doc, and test on PRs + pushes to `main`. Tests fan out across macOS and Windows.
- `.github/workflows/release-prepare.yml` — opens a PR that bumps the workspace version and prepends a new CHANGELOG section.
- `.github/workflows/release.yml` — driven by cargo-dist; triggered by version tags. Builds binaries, generates installers, attaches them to the GitHub Release.

## Where to start when adding a new feature

| Goal                              | Start in                                                                 |
| --------------------------------- | ------------------------------------------------------------------------ |
| New CLI subcommand                | `crates/bo4e-cli/src/cli/`, add to `SubcommandsLevel1` in `cli/base.rs`. |
| New schema transform              | `crates/bo4e-cli/src/edit/`, drive it from `cli/edit.rs`.                |
| New diff metric                   | `crates/bo4e-cli/src/diff/`.                                             |
| New code-generation output type   | `crates/bo4e-codegen/` (see `AGENTS.md` §6 and the codegen STRUCTURE.md). |
| New schema-model type or visitor  | `crates/bo4e-schemas/src/models/` + `visitable.rs`.                      |
