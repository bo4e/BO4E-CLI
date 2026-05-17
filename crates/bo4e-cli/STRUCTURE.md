# STRUCTURE.md — `bo4e-cli`

The user-facing crate. Ships the `bo4e` binary, but also a library facade so integration tests can drive subcommands without spawning a process.

## Purpose

- Parse CLI args (clap, with a custom help renderer that runs through the BO4E highlighter).
- Implement every subcommand (`pull`, `edit`, `diff`, `generate`, `repo`) behind a single `Executable` trait.
- Provide all the surrounding plumbing the commands need: console output, GitHub IO, git wrappers, config loading, schema-edit transforms.

## Layout

```
src/
├── main.rs            # binary entrypoint; thin shim around `Cli::try_parse` + `Executable::run`
├── lib.rs             # library facade re-exporting every module + `test_lock::CWD_LOCK`
├── cli.rs             # `pub mod cli` wrapper (file pair pattern used throughout)
├── cli/
│   ├── base.rs        # `Cli`, `SubcommandsLevel1`, the `Executable` trait, help styling
│   ├── pull.rs        # `bo4e pull` (GitHub fetch + offline-rewrite refs)
│   ├── edit.rs        # `bo4e edit` (run config-driven schema transforms)
│   ├── generate.rs    # `bo4e generate` (delegate to bo4e-codegen)
│   ├── diff.rs        # `bo4e diff schemas | matrix | version-bump`
│   └── repo.rs        # `bo4e repo versions` (BO4E-python tag listing)
├── console.rs / console/
│   ├── console.rs     # `CONSOLE: OnceLock<Console>`, Level, print_info / print_warn / print_error
│   ├── highlighter.rs # rule-based colouriser (schemas, versions, paths, BO4E, help text)
│   ├── mark.rs        # MarkStyle + sentinel chars used by macros to wrap pre-styled spans
│   ├── palette.rs     # the BO4E colour palette
│   ├── progress_bar.rs# `new_progress_bar` / `finish_progress_bar` (indicatif wrapper)
│   └── spinner.rs     # spinner helpers for long-running calls
├── edit.rs / edit/
│   ├── add.rs         # add_field / add_model / add_enum_item transforms
│   ├── non_nullable.rs# strip the nullable wrapper for regex-matched fields
│   └── update_refs.rs # rewrite GitHub `$ref` URLs into relative offline refs
├── diff.rs / diff/
│   ├── diff.rs        # core schema diff walker (allOf/anyOf/array/object/refs/enum/string)
│   ├── filters.rs     # `has_critical` + change-class predicates
│   ├── matrix.rs      # chain N diffs into a compatibility matrix (CSV/JSON)
│   └── version.rs     # `check_version_bump` → Technical | Functional | Major
├── io.rs / io/
│   ├── github.rs      # octocrab-based fetch; token detection (`gh auth token` / env / regex)
│   ├── git.rs         # shell-out helpers (`git clone`, `git log`, …)
│   ├── config.rs      # load + resolve the edit config (incl. `$ref`-style inclusion)
│   ├── cleanse.rs     # `clear_dir_if_needed` (prompt + wipe) — used by pull/edit/generate
│   ├── changes.rs     # read / write a `Changes` JSON diff file
│   └── matrix.rs      # write_compatibility_matrix_{csv,json}
├── models.rs / models/
│   ├── cli.rs         # `Token` + `get_token_as_string` (CLI-shared types)
│   ├── config.rs      # serde models for the edit config file
│   ├── changes.rs     # `Change`, `ChangeType`, `ChangeValue`, `Changes`
│   ├── git.rs         # `Reference` (git refspec parsing)
│   ├── graph.rs       # `GraphIR`, `Node`, `Field`, `Edge`, `Cardinality` (on-disk graph IR)
│   └── matrix.rs      # `CompatibilityMatrix`, `MatrixCell`, …
├── repo.rs / repo/
│   └── filter.rs      # `FilterOptions` + `filter_tags` for `bo4e repo versions`
└── utils.rs / utils/
    └── tokio.rs       # `get_runtime` — single-threaded runtime used by `pull` and `github`
```

The `foo.rs` + `foo/` pair is consistent: `foo.rs` is a `pub mod foo;` stub so the directory's modules show up.

## Entry points and control flow

```rust
fn main() -> Result<(), String> {
    let args = Cli::try_parse()?;
    CONSOLE.set(Console::new(level_from(args)))?;
    args.run()        // dispatches into SubcommandsLevel1::run → <each subcommand>::run
}
```

Every subcommand implements `cli::base::Executable::run`. `Cli::try_parse` failures are rendered through the BO4E highlighter before exit (so `--help` matches the styling of normal output).

## Console

`CONSOLE` is a `OnceLock<Console>` set once in `main`. Three macros wrap the common cases:

| Macro / call            | Goes to | Suppressed when     |
| ----------------------- | ------- | ------------------- |
| `cprint_quiet!`         | stdout  | never               |
| `cprint_normal!`        | stdout  | `--quiet`           |
| `cprint_verbose!`       | stdout  | not `--verbose`     |
| `cwarn!` / `cerror!`    | stderr  | never               |

Routing rule: info → stdout, warnings/errors → stderr. Non-quiet mode is not designed to be parsed — under `--quiet` only essential machine-readable output is printed.

The `Highlighter` is rule-based with a sentinel mark protocol: callers wrap pre-classified spans (schema names, file paths, BO4E versions) in invisible sentinel chars so the highlighter can colour them without re-parsing.

## Library facade and `CWD_LOCK`

`lib.rs` re-exports every module so integration tests in `tests/` can call into subcommands directly. It also defines:

```rust
#[cfg(test)]
pub(crate) mod test_lock {
    use std::sync::Mutex;
    pub(crate) static CWD_LOCK: Mutex<()> = Mutex::new(());
}
```

Tests that mutate `std::env::set_current_dir` (any test that runs `bo4e repo versions` against a fixture repo, for example) **must** take this lock for their duration. Cargo runs tests in parallel by default and unprotected `set_current_dir` calls race silently. Note: only the lib's own unit tests can reach `test_lock`; integration tests get the same protection by serialising on the lock visibly in their setup.

## Subcommand notes

- **`pull`** — uses octocrab via a single-threaded tokio runtime (`utils::tokio::get_runtime`). Resolves `latest` against the BO4E-Schemas GitHub repo. After downloading, rewrites GitHub `$ref` URLs to relative offline paths through `edit::update_refs::update_references_all`. Token resolution order: env → `gh auth token` → none.
- **`edit`** — reads schemas, resolves the config (incl. `$ref` includes via `io::config::load_config`), applies `add` / `non_nullable` / `update_refs` transforms in a fixed order, brands the output `DirtyVersion` with today's `.d<YYYYMMDD>` suffix.
- **`diff`** — has three subcommands: `schemas` (produce a JSON `Changes` diff), `matrix` (chain N diffs into CSV/JSON), `version-bump` (classify a diff as Technical / Functional / Major, with `--major-bump-allowed` gating).
- **`generate`** — dispatches to the per-flavour `pub fn generate` in `bo4e-codegen`. The `Generate` struct holds `common: GenerateCommon` (shared flags: `--no-clear-output`, `--templates-dir`, input/output dirs) and `flavour: GenerateFlavour` (subcommands: `PythonPydantic`, `PythonSqlModel`, `RustPlain`, `RustCrate(RustCrateArgs)`). Feature-gating ensures only compiled-in flavours appear in `--help`.
- **`repo versions`** — shells out to `git log` from a BO4E-python checkout, parses tags through `models::git::Reference`, filters with `repo::filter::FilterOptions`. CI uses this to discover release tags.

## Error handling

The boundary is `Executable::run -> Result<(), String>`. Internally, fallible IO uses `std::io::Error` / `String`; library crates surface their own `Error` types (`bo4e_codegen::Error`, schema-side `String` for now). Print human-facing errors through `cerror!` so they pick up styling.

## Integration tests

Under `tests/`:

- `full_bo4e.rs` — drives `pull` → `edit` → `generate` end-to-end against a fixture.
- `generate_smoke.rs` — minimal generate run on a tiny schema set.
- `quiet_verbose.rs` — verifies `--quiet` / `--verbose` routing and stream destinations.
- `regression_schema_parse.rs` — pins specific JSON-Schema parsing edge cases.

These all link against the library facade in `lib.rs`. Use `tempfile::tempdir()` for IO and take `CWD_LOCK` if you change the process cwd.

## When extending

- **New subcommand**: add a struct in `cli/<name>.rs`, implement `Executable`, register in `SubcommandsLevel1` in `cli/base.rs`, add help styling tests in `cli/base.rs::tests` mirroring `each_subcommand_help_contains_ansi`.
- **New edit transform**: add a file under `src/edit/`, expose a `transform_all_*` entry point, and call it from `cli/edit.rs` in the canonical order. Update `models/config.rs` if the config shape grows.
- **New diff metric / change kind**: extend `models/changes.rs` and the walker in `diff/diff.rs`. Keep `diff/filters.rs::has_critical` exhaustive.
- **New CLI-wide flag**: add to `cli/base.rs::Cli` with `#[arg(global = true, …)]`. If it changes the console behaviour, plumb it through `Console::new`.
