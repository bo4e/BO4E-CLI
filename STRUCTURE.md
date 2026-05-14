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

- **Shared, language-neutral helpers** live at the top of `bo4e-codegen/src/` (`naming.rs`, `layout.rs`, `refs.rs`, `imports.rs`). Per-language modules (`python/`, future `rust/`) own only the bits that actually differ — type-string mapping, import-block rendering, language-specific reserved words.
- **Schemas are the source of truth.** Everything downstream (edit, diff, generate) operates on a `Schemas` collection loaded from disk. The schema directory carries a `.version` file that captures the upstream BO4E version (plain or "dirty").
- **Per-flavour `pub fn generate`.** `bo4e-codegen` exposes one public function per flavour
  under language-named modules (`python::pydantic`, `python::sql_model`, `rust::plain`,
  `rust::crate_`). The CLI's subcommand enum (`GenerateFlavour` in `cli/generate.rs`)
  is the only runtime dispatcher; the library has no equivalent.
- **Templates over hand-written emission.** Generators build a context struct (Python flavour matters: pydantic's per-field context mirrors what the vendored `data-model-code-generator` `BaseModel.jinja2` expects) and render embedded MiniJinja templates. The CLI exposes `--templates-dir` so users can override individual templates without rebuilding the binary.
- **Strict schema invariant: `required ⇔ no default`.** Every object property must satisfy: the property is in the schema's `required` array *if and only if* it has **no** declared `default`. Both violations are rejected at generate time (`Error::InconsistentSchema`):
  - *required + default declared* — the default is unreachable because the JSON key is always present.
  - *optional + no default* — the JSON key may be absent and the runtime has no fallback; the generator refuses to invent one.

  Schema-literal defaults (any `PrimitiveValue`: `null`, bool, integer, float, string) drive the generated field's default expression directly. Structural defaults that the target language *requires* to express optionality — `= None` for pydantic `Optional[T]`, `#[serde(default)]` for Rust `Option<T>` — are emitted on top of (not in place of) the schema literal. The renderer never invents the literal itself; if you want one, put it in the schema.

- **Schemas are editable, no special-cased field names.** Every rendering decision is driven by the schema's *shape*, never by a field's name. `_version`, `_typ`, `_id` are not special — they're whatever the schema says they are. `bo4e edit` is free to add/remove/rename fields or flip their `required`/`default` shape, and the generated code follows. If a renderer ever needs a per-name carve-out, that's a sign the schema needs a more precise type, not the codegen needs a heuristic.

- **AllOf and AnyOf are restricted.** The type mappers accept only:
  - `allOf` with **exactly one** element (used as a single-item wrapper). Multi-element `allOf` (intersection) is rejected as `UnsupportedSchemaShape`.
  - `anyOf` with **one** non-null branch and **one** `null` branch — the Optional/nullable pattern. Anything else (real unions, multiple non-null branches, missing null branch) is rejected.

  Real unions and intersections would require sum-type emission with discriminators; until BO4E uses them, the generator refuses early rather than producing surprising output.

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
