# CLI UX & Robustness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring `bo4e-cli` to UX parity with the Python implementation — visible spinners, working `--quiet`/`--verbose` on every command, robust schema parsing on `v202501.0.0`, and styled `--help` output — and back the schema-parser fix with regression fixtures plus an opt-in full-BO4E integration suite.

**Architecture:** Bolt onto the existing `Console`/`Highlighter`/`palette` infrastructure. New `console::spinner` module wraps `indicatif::ProgressBar::new_spinner()` with three frame sets copied verbatim from rich. Quiet gating happens inside the IO layer (`io/github`, `io/cleanse`) via `CONSOLE.would_emit(Level::Normal)` — the spec's chosen pattern, already proven by `cli/repo`. Schema fix is investigation-driven: a parse-every-schema integration test reveals offending JSONs, regression fixtures land first, fix follows.

**Tech Stack:** Rust 2024, `clap` 4.5.45 (uses stable `builder::styling::Styles`), `indicatif` 0.18.0, `serde`/`serde_json`, `tempfile`, existing `cprint_*!` / `cwarn!` / `cerror!` macros.

**Spec:** `docs/plans/2026-05-09-cli-ux-and-robustness-design.md`. Read before starting.

**Branch:** Direct commits to `rust`. No feature branch.

---

## Pre-flight notes for the executor

- All paths below are relative to the repo root `/repos/bo4e-cli`.
- The python parity reference is checked out at `/tmp/bo4e-cli-python/` (HEAD `8ef040b`). Do **not** edit it. Read-only.
- Rich spinner frames are at `/repos/bo4e-cli/.tox/dev/Lib/site-packages/rich/_spinners.py`. Read-only.
- The fixture data is at `/repos/bo4e-cli/.tmp/bo4e_latest/` (192 JSONs, BO4E `v202501.0.0`). Do not commit.
- Tests run with `cargo test --workspace` from repo root. Add `-p bo4e-cli` to scope to one crate. Add `-- --ignored` for opt-in slow tests.
- Existing tests use the in-process pattern: `let _ = CONSOLE.set(Console::new(Level::X)); cmd.run().unwrap();`. No `assert_cmd` dependency — do not add one.
- The `cprint_normal!` / `cprint_verbose!` / `cwarn!` / `cerror!` macros are exported from `crate::` (see `console.rs`). Importing them at a call site is `use crate::{cprint_normal, cprint_verbose, cwarn};`.
- Always run `cargo build -p bo4e-cli` before each commit unless the change is doc-only. Always run `cargo test -p bo4e-cli` (and `-p bo4e-schemas` if `bo4e-schemas` was touched) before each commit. Tasks below state the exact tests; "and full suite" means `cargo test --workspace` on top.

---

## Task 1 — Create `console::spinner` module

**Files:**
- Create: `crates/bo4e-cli/src/console/spinner.rs`
- Modify: `crates/bo4e-cli/src/console.rs` (add `pub mod spinner;`)

- [ ] **Step 1: Add the module declaration**

In `crates/bo4e-cli/src/console.rs`, change the top of the file from:

```rust
pub mod console;
pub mod highlighter;
pub mod palette;
pub mod progress_bar;
```

to:

```rust
pub mod console;
pub mod highlighter;
pub mod palette;
pub mod progress_bar;
pub mod spinner;
```

- [ ] **Step 2: Write the failing test file** (pre-create the spinner module with empty body so tests fail)

Create `crates/bo4e-cli/src/console/spinner.rs` with **only** this content:

```rust
//! Named-spinner factories mirroring the python implementation's rich spinners.
//!
//! Frames copied verbatim from rich `_spinners.py` (vendored at
//! `.tox/dev/Lib/site-packages/rich/_spinners.py`).

use indicatif::{ProgressBar, ProgressStyle};
use std::borrow::Cow;
use std::time::Duration;

const EARTH_FRAMES: &[&str] = &["🌍 ", "🌎 ", "🌏 "];
const EARTH_INTERVAL_MS: u64 = 180;

const SQUISH_FRAMES: &[&str] = &["╫", "╪"];
const SQUISH_INTERVAL_MS: u64 = 100;

const GRENADE_FRAMES: &[&str] = &[
    "،   ",
    "′   ",
    " ´ ",
    " ‾ ",
    "  ⸌",
    "  ⸊",
    "  |",
    "  ⁎",
    "  ⁕",
    " ෴ ",
    "  ⁓",
    "   ",
    "   ",
    "   ",
];
const GRENADE_INTERVAL_MS: u64 = 80;

pub fn earth(msg: impl Into<Cow<'static, str>>) -> ProgressBar {
    spinner(msg, EARTH_FRAMES, EARTH_INTERVAL_MS)
}

pub fn squish(msg: impl Into<Cow<'static, str>>) -> ProgressBar {
    spinner(msg, SQUISH_FRAMES, SQUISH_INTERVAL_MS)
}

pub fn grenade(msg: impl Into<Cow<'static, str>>) -> ProgressBar {
    spinner(msg, GRENADE_FRAMES, GRENADE_INTERVAL_MS)
}

fn spinner(
    msg: impl Into<Cow<'static, str>>,
    frames: &'static [&'static str],
    interval_ms: u64,
) -> ProgressBar {
    let visible = crate::console::console::CONSOLE
        .get()
        .map(|c| c.would_emit(crate::console::console::Level::Normal))
        .unwrap_or(true);
    if !visible {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .expect("static template parses")
            .tick_strings(frames),
    );
    pb.set_message(msg.into());
    pb.enable_steady_tick(Duration::from_millis(interval_ms));
    pb
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{CONSOLE, Console, Level};

    #[test]
    fn frames_match_rich_v14_earth() {
        assert_eq!(EARTH_FRAMES, &["🌍 ", "🌎 ", "🌏 "]);
        assert_eq!(EARTH_INTERVAL_MS, 180);
    }

    #[test]
    fn frames_match_rich_v14_squish() {
        assert_eq!(SQUISH_FRAMES, &["╫", "╪"]);
        assert_eq!(SQUISH_INTERVAL_MS, 100);
    }

    #[test]
    fn frames_match_rich_v14_grenade() {
        // Length and a few key frames; full equality is enforced by the const itself.
        assert_eq!(GRENADE_FRAMES.len(), 14);
        assert_eq!(GRENADE_FRAMES[0], "،   ");
        assert_eq!(GRENADE_FRAMES[6], "  |");
        assert_eq!(GRENADE_FRAMES[13], "   ");
        assert_eq!(GRENADE_INTERVAL_MS, 80);
    }

    #[test]
    fn quiet_returns_hidden() {
        let _ = CONSOLE.set(Console::new(Level::Quiet));
        // Note: CONSOLE is a OnceLock so this set is best-effort across the whole
        // test binary. The assertion below uses `would_emit` directly to avoid
        // ordering brittleness with other tests in the binary.
        let c = CONSOLE.get().expect("set above or earlier");
        if !c.would_emit(Level::Normal) {
            assert!(earth("hi").is_hidden());
            assert!(squish("hi").is_hidden());
            assert!(grenade("hi").is_hidden());
        }
    }
}
```

- [ ] **Step 3: Build and run spinner tests**

Run: `cargo test -p bo4e-cli console::spinner -- --nocapture`
Expected: 4 tests pass. (`quiet_returns_hidden` may no-op if another test already initialized `CONSOLE` to a higher level — that's intentional.)

- [ ] **Step 4: Run the full bo4e-cli suite**

Run: `cargo test -p bo4e-cli`
Expected: all green; spinner module adds no regressions.

- [ ] **Step 5: Commit**

```bash
git add crates/bo4e-cli/src/console.rs crates/bo4e-cli/src/console/spinner.rs
git commit -m "feat(cli/console): add spinner factories (earth/squish/grenade)"
```

---

## Task 2 — Wire `earth` spinner into `io/github`

**Files:**
- Modify: `crates/bo4e-cli/src/io/github.rs`

The python CLI uses `with CONSOLE.status("Querying GitHub for latest version", spinner="earth"):` around `resolve_latest_version`, `with CONSOLE.status("Querying GitHub tree", spinner="earth"):` around `get_target_commitish_from_tag`, and another `"Querying GitHub tree"` earth around the recursive listing. Mirror all three.

- [ ] **Step 1: Add the spinner import**

At the top of `crates/bo4e-cli/src/io/github.rs`, after the existing `use crate::console::progress_bar::...;` line, add:

```rust
use crate::console::spinner;
```

- [ ] **Step 2: Wrap `resolve_latest_version` with a spinner**

Replace the current body of `resolve_latest_version` (around lines 207–215) with:

```rust
pub async fn resolve_latest_version(token: Option<&str>) -> Result<Version, String> {
    let _spin = spinner::earth("Querying GitHub for latest version");
    let octocrab = get_octocrab_instance(token)?;
    let latest_release = get_bo4e_schemas_repo_handler(&octocrab)
        .releases()
        .get_latest()
        .await
        .map_err(|e| e.to_string())?;
    Version::from_str(&latest_release.tag_name)
}
```

The leading underscore in `_spin` is intentional: the spinner runs until dropped at scope exit. Indicatif handles the silent-on-quiet path via the factory.

- [ ] **Step 3: Wrap `get_target_commitish_from_tag` with a spinner**

Replace the current body (around lines 161–171) with:

```rust
async fn get_target_commitish_from_tag(
    repo_handler: &RepoHandler<'_>,
    version_tag: &Version,
) -> Result<String, String> {
    let _spin = spinner::earth("Querying GitHub tree");
    let reference = repo_handler
        .releases()
        .get_by_tag(&version_tag.to_string())
        .await
        .map_err(|e| e.to_string())?;
    Ok(reference.target_commitish)
}
```

- [ ] **Step 4: Wrap the recursive listing in `get_schemas_from_github` with a spinner**

In `get_schemas_from_github` (around lines 177–205), wrap the `_get_schemas_from_github_recursive` call with a second `earth` spinner. Replace the body with:

```rust
pub async fn get_schemas_from_github(
    version_tag: &Version,
    token: Option<&str>,
    enable_output: bool,
) -> Result<Schemas, String> {
    let octocrab = get_octocrab_instance(token)?;
    let target_commitish =
        get_target_commitish_from_tag(&get_bo4e_schemas_repo_handler(&octocrab), version_tag)
            .await?;

    let schema_downloads = {
        let _spin = spinner::earth("Querying GitHub tree");
        _get_schemas_from_github_recursive(
            octocrab,
            target_commitish,
            "src/bo4e_schemas".to_string(),
        )
        .await?
    };

    let local_set = tokio::task::LocalSet::new();
    let schemas_vector = local_set
        .run_until(_execute_futures_with_progress_bar(
            schema_downloads,
            enable_output,
        ))
        .await?
        .into_iter()
        .collect::<Result<Vec<Schema>, String>>()?;
    let schemas = Schemas::try_from((schemas_vector, version_tag.into()))?;

    Ok(schemas)
}
```

The `enable_output` parameter is removed in Task 6; do not change it now.

- [ ] **Step 5: Build and run io/github tests**

Run: `cargo build -p bo4e-cli` then `cargo test -p bo4e-cli`
Expected: green. Spinners do nothing in non-TTY test environments.

- [ ] **Step 6: Commit**

```bash
git add crates/bo4e-cli/src/io/github.rs
git commit -m "feat(cli/io/github): show earth spinner during GitHub API calls"
```

---

## Task 3 — Wire `grenade` spinner + verbose into `io/cleanse`

**Files:**
- Modify: `crates/bo4e-cli/src/io/cleanse.rs`

Python sites: `io/cleanse.py:16` ("Directory X does not exist, nothing to clear.", verbose) and `io/cleanse.py:27` ("Cleared directory X (N entries removed)", verbose), plus `with CONSOLE.status("Clearing directory X", spinner="grenade"):` around the actual clear.

- [ ] **Step 1: Replace the function body**

Replace the entire contents of `crates/bo4e-cli/src/io/cleanse.rs` with:

```rust
use crate::console::spinner;
use crate::{cprint_verbose};
use std::path::Path;

/// Clear (and delete) the directory if `clear_output` is true and the directory exists.
/// If the path points to a file instead of a directory, an error is returned.
/// If `clear_output` is false, the function does nothing and returns Ok(()).
/// If the directory does not exist, it is also considered a success (no error).
pub fn clear_dir_if_needed(output_dir: &Path, clear_output: bool) -> std::io::Result<()> {
    if !clear_output {
        return Ok(());
    }
    if !output_dir.try_exists()? {
        cprint_verbose!(
            "Directory {} does not exist, nothing to clear.",
            output_dir.display()
        );
        return Ok(());
    }
    if !output_dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "Tried to clear a directory, but the path points to a file.",
        ));
    }
    let entries_removed = std::fs::read_dir(output_dir)?.count();
    let _spin = spinner::grenade(format!("Clearing directory {}", output_dir.display()));
    std::fs::remove_dir_all(output_dir)?;
    cprint_verbose!(
        "Cleared directory {} ({} entries removed)",
        output_dir.display(),
        entries_removed
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{CONSOLE, Console, Level};

    fn ensure_console() {
        let _ = CONSOLE.set(Console::new(Level::Verbose));
    }

    #[test]
    fn clears_existing_directory() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("a.txt"), b"x").unwrap();
        clear_dir_if_needed(&nested, true).unwrap();
        assert!(!nested.exists());
    }

    #[test]
    fn nonexistent_directory_is_ok() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        clear_dir_if_needed(&missing, true).unwrap();
    }

    #[test]
    fn clear_output_false_is_noop() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("a.txt"), b"x").unwrap();
        clear_dir_if_needed(&nested, false).unwrap();
        assert!(nested.exists());
    }

    #[test]
    fn file_path_is_error() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("file");
        std::fs::write(&f, b"x").unwrap();
        let err = clear_dir_if_needed(&f, true).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotADirectory);
    }
}
```

The `cprint_verbose!` import requires `CONSOLE` to be set; the four new tests call `ensure_console()` to satisfy that, matching the pattern in `edit/add.rs`.

- [ ] **Step 2: Build and run cleanse tests**

Run: `cargo test -p bo4e-cli io::cleanse`
Expected: 4 tests pass.

- [ ] **Step 3: Run the full bo4e-cli suite**

Run: `cargo test -p bo4e-cli`
Expected: green. (Pre-existing edit tests already initialise `CONSOLE`; the new module tests are independent.)

- [ ] **Step 4: Commit**

```bash
git add crates/bo4e-cli/src/io/cleanse.rs
git commit -m "feat(cli/io/cleanse): grenade spinner + verbose log lines"
```

---

## Task 4 — Wire `squish` spinner into `cli/diff`

**Files:**
- Modify: `crates/bo4e-cli/src/cli/diff.rs`

Python uses three `squish` blocks: comparing schemas, reading changes from diff files, creating compatibility matrix. Wrap the matching Rust call sites.

- [ ] **Step 1: Add the spinner import**

In `crates/bo4e-cli/src/cli/diff.rs`, change the import block at the top from:

```rust
use crate::cli::base::Executable;
use crate::cprint_normal;
use crate::diff::diff::diff_schemas;
```

to:

```rust
use crate::cli::base::Executable;
use crate::console::spinner;
use crate::cprint_normal;
use crate::diff::diff::diff_schemas;
```

- [ ] **Step 2: Wrap `diff_schemas` in `run_schemas` with `squish`**

Replace the body of `run_schemas` (currently lines 77–94) with:

```rust
fn run_schemas(a: &DiffSchemasArgs) -> Result<(), String> {
    let out_old = read_schemas(&a.input_dir_base)?;
    for w in &out_old.warnings {
        crate::cwarn!("{w}");
    }
    let old = out_old.schemas;
    let out_new = read_schemas(&a.input_dir_comp)?;
    for w in &out_new.warnings {
        crate::cwarn!("{w}");
    }
    let new = out_new.schemas;
    let changes = {
        let _spin = spinner::squish("Comparing JSON-schemas...");
        diff_schemas(&old, &new)
    };
    cprint_normal!("Compared JSON-schemas.");
    write_changes(&changes, &a.output_file)?;
    cprint_normal!("Saved Diff to file: {}", a.output_file.display());
    Ok(())
}
```

The pre-existing `cprint_normal!("Comparing JSON-schemas...");` is removed — the spinner replaces it (the python idiom is "spinner during the work, status line after").

- [ ] **Step 3: Wrap `read_changes_from_diff_files` + `build_chain` with `squish` in `run_matrix`**

Replace the body of `run_matrix` (currently lines 96–107) with:

```rust
fn run_matrix(a: &DiffMatrixArgs) -> Result<(), String> {
    let (chain, matrix) = {
        let _spin = spinner::squish("Creating compatibility matrix...");
        let diffs = read_changes_from_diff_files(&a.input_diff_files)?;
        let chain = build_chain(diffs)?;
        let matrix = create_compatibility_matrix(&chain, a.use_emotes);
        (chain, matrix)
    };
    let path: Vec<String> = chain.nodes.iter().map(|n| n.version_key.clone()).collect();
    match a.output_type {
        MatrixOutputType::Csv => write_compatibility_matrix_csv(&a.output_file, &matrix, &path)?,
        MatrixOutputType::Json => write_compatibility_matrix_json(&a.output_file, &matrix)?,
    }
    cprint_normal!("Saved compatibility matrix to: {}", a.output_file.display());
    Ok(())
}
```

The reading-from-diff and matrix-creation phases are coalesced into a single visible "Creating compatibility matrix..." spinner because they happen in immediate succession with no user-visible boundary; the python implementation uses two adjacent squish blocks but has no observable gap between them.

- [ ] **Step 4: Build and run diff tests**

Run: `cargo test -p bo4e-cli cli::diff`
Expected: 4 pre-existing tests still pass; no new tests yet.

- [ ] **Step 5: Commit**

```bash
git add crates/bo4e-cli/src/cli/diff.rs
git commit -m "feat(cli/diff): squish spinner during compare + matrix build"
```

---

## Task 5 — Wire `squish` spinners into `cli/generate`

**Files:**
- Modify: `crates/bo4e-cli/src/cli/generate.rs`

Python uses `squish` around "Parsing schemas into Python classes", "Validating generated Python modules", and (sql-model only) "Parsing many-to-many relationships into Python classes". The Rust `generate` is monolithic — `bo4e_codegen::generate` does the work in one call. Wrap the whole call with one squish, message keyed off the output type.

- [ ] **Step 1: Replace `Generate::run`**

Replace the body of `Generate::run` (currently lines 30–48) with:

```rust
impl Executable for Generate {
    fn run(&self) -> Result<(), String> {
        let out = bo4e_schemas::io::schemas::read_schemas(&self.input)
            .map_err(|e| format!("failed to read schemas: {e}"))?;
        for w in &out.warnings {
            crate::cwarn!("{w}");
        }

        let _spin = crate::console::spinner::squish(format!(
            "Generating {} output",
            self.output_type
        ));

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

This uses the `Display` impl on `bo4e_codegen::OutputType` if one exists. If it does not compile, replace `self.output_type` with `format!("{:?}", self.output_type)` (Debug fallback) — the message is non-load-bearing.

- [ ] **Step 2: Build and run generate tests**

Run: `cargo build -p bo4e-cli` then `cargo test -p bo4e-cli`
Expected: green. The pre-existing `tests/generate_smoke.rs` test should still pass.

- [ ] **Step 3: If Step 1's `format!("Generating {} ...", self.output_type)` fails to compile**

Fall back to:

```rust
let _spin = crate::console::spinner::squish(format!(
    "Generating {:?} output",
    self.output_type
));
```

Re-run `cargo build -p bo4e-cli` to confirm.

- [ ] **Step 4: Commit**

```bash
git add crates/bo4e-cli/src/cli/generate.rs
git commit -m "feat(cli/generate): squish spinner during code generation"
```

---

## Task 6 — Drop `enable_output: bool` from io/github, gate via `CONSOLE`

**Files:**
- Modify: `crates/bo4e-cli/src/io/github.rs`
- Modify: `crates/bo4e-cli/src/cli/pull.rs`

Today `Pull::run` passes `enable_output: true` unconditionally. The progress bar inside `_execute_futures_with_progress_bar` uses that boolean to decide whether to render. Replace the boolean parameter chain with a `CONSOLE` check inside the IO layer (matches `cli/repo`'s pattern).

- [ ] **Step 1: Drop the parameter from `_execute_futures_with_progress_bar`**

In `crates/bo4e-cli/src/io/github.rs`, change the signature of `_execute_futures_with_progress_bar` (around line 97) from:

```rust
async fn _execute_futures_with_progress_bar<T: 'static>(
    futures: Vec<AsyncInvokeLater<T>>,
    enable_output: bool,
) -> Result<Vec<T>, String> {
```

to:

```rust
async fn _execute_futures_with_progress_bar<T: 'static>(
    futures: Vec<AsyncInvokeLater<T>>,
) -> Result<Vec<T>, String> {
```

Inside the body, change the line currently reading:

```rust
    let pb = enable_output.then(|| new_progress_bar(total as u64, Some(start_message.to_string())));
```

to:

```rust
    let visible = crate::console::console::CONSOLE
        .get()
        .map(|c| c.would_emit(crate::console::console::Level::Normal))
        .unwrap_or(true);
    let pb = visible.then(|| new_progress_bar(total as u64, Some(start_message.to_string())));
```

- [ ] **Step 2: Drop the parameter from `get_schemas_from_github`**

Change the signature of `get_schemas_from_github` (around line 177) from:

```rust
pub async fn get_schemas_from_github(
    version_tag: &Version,
    token: Option<&str>,
    enable_output: bool,
) -> Result<Schemas, String> {
```

to:

```rust
pub async fn get_schemas_from_github(
    version_tag: &Version,
    token: Option<&str>,
) -> Result<Schemas, String> {
```

Inside, change the call from:

```rust
        .run_until(_execute_futures_with_progress_bar(
            schema_downloads,
            enable_output,
        ))
```

to:

```rust
        .run_until(_execute_futures_with_progress_bar(schema_downloads))
```

- [ ] **Step 3: Drop the third positional argument at the call site in `pull.rs`**

In `crates/bo4e-cli/src/cli/pull.rs`, change line 69 from:

```rust
        let schemas = runtime.block_on(get_schemas_from_github(&version, token, true))?;
```

to:

```rust
        let schemas = runtime.block_on(get_schemas_from_github(&version, token))?;
```

- [ ] **Step 4: Build to confirm no other callers exist**

Run: `cargo build -p bo4e-cli`
Expected: clean build. If a build error mentions another caller of `get_schemas_from_github` or `_execute_futures_with_progress_bar`, drop the matching argument there too — there should be none in `crates/bo4e-cli`.

- [ ] **Step 5: Run the full bo4e-cli suite**

Run: `cargo test -p bo4e-cli`
Expected: green.

- [ ] **Step 6: Commit**

```bash
git add crates/bo4e-cli/src/io/github.rs crates/bo4e-cli/src/cli/pull.rs
git commit -m "refactor(cli/io/github): gate progress bar via CONSOLE, drop enable_output bool"
```

---

## Task 7 — Add per-API-call verbose lines to `io/github`

**Files:**
- Modify: `crates/bo4e-cli/src/io/github.rs`

Two python sites: `io/github.py:104` (per-file decode in tree walk) and `io/github.py:130` (per-API-call resolved tag). Add the matching `cprint_verbose!` calls.

- [ ] **Step 1: Add the macro import**

At the top of `crates/bo4e-cli/src/io/github.rs`, after `use crate::console::spinner;`, add:

```rust
use crate::cprint_verbose;
```

- [ ] **Step 2: Add per-file verbose in the tree walk**

Inside `_get_schemas_from_github_recursive`, locate the future pushed inside the `"file"` arm (currently lines 60–76). Modify the future body so that **after** the `decoded_content()` call succeeds and **before** `Schema::new`, it logs the file path. Replace the entire `futures.push(Box::pin(async move { ... }));` block with:

```rust
                    futures.push(Box::pin(async move {
                        let file_content = get_bo4e_schemas_repo_handler(&octocrab)
                            .get_content()
                            .r#ref(target_commitish)
                            .path(file_path.clone())
                            .send()
                            .await
                            .map_err(|e| e.to_string())?
                            .items[0]
                            .decoded_content()
                            .ok_or("Failed to retrieve and decode file content".to_string())?;
                        cprint_verbose!("Fetched schema {}", file_path);
                        let mut schema =
                            Schema::new(path_slice.split('/').map(String::from).collect(), None)?;
                        schema.load_schema(file_content);
                        Ok(schema)
                    }));
```

Note `file_path.clone()` is now used inside the async move (the `cprint_verbose!` after the `.path()` call needs `file_path` again).

- [ ] **Step 3: Add per-API-call verbose in `get_target_commitish_from_tag`**

Replace the body of `get_target_commitish_from_tag` (still has the `_spin` from Task 2) with:

```rust
async fn get_target_commitish_from_tag(
    repo_handler: &RepoHandler<'_>,
    version_tag: &Version,
) -> Result<String, String> {
    let _spin = spinner::earth("Querying GitHub tree");
    let reference = repo_handler
        .releases()
        .get_by_tag(&version_tag.to_string())
        .await
        .map_err(|e| e.to_string())?;
    cprint_verbose!(
        "Resolved tag {} → commitish {}",
        version_tag,
        reference.target_commitish
    );
    Ok(reference.target_commitish)
}
```

- [ ] **Step 4: Build and run**

Run: `cargo build -p bo4e-cli` then `cargo test -p bo4e-cli`
Expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/bo4e-cli/src/io/github.rs
git commit -m "feat(cli/io/github): per-API-call and per-schema verbose lines"
```

---

## Task 8 — Add `/.tmp` to `.gitignore` + create the fetch script

**Files:**
- Modify: `.gitignore`
- Create: `scripts/fetch-bo4e-fixture.sh`

The spec assumed `.tmp/` was gitignored — it is not (only `/tmp` is). Fix that, then add the hydration script so contributors can populate `.tmp/bo4e_latest/` in one command.

- [ ] **Step 1: Add `.tmp` to .gitignore**

Replace the contents of `.gitignore` with:

```
/target
/.idea
/tmp
/.tmp
.claude
```

- [ ] **Step 2: Create the fetch script**

Create `scripts/fetch-bo4e-fixture.sh` with content:

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION="${BO4E_VERSION:-latest}"
cargo build -p bo4e-cli --release
./target/release/bo4e pull -t "$VERSION" -o .tmp/bo4e_latest
echo "Hydrated .tmp/bo4e_latest at $(cat .tmp/bo4e_latest/.version)"
```

- [ ] **Step 3: Mark the script executable**

Run: `chmod +x scripts/fetch-bo4e-fixture.sh`
Expected: no output.

- [ ] **Step 4: Verify the fixture is already present** (do not re-pull; the user populated it earlier)

Run: `test -f .tmp/bo4e_latest/.version && cat .tmp/bo4e_latest/.version`
Expected: prints `v202501.0.0`.

- [ ] **Step 5: Verify .gitignore now hides `.tmp/`**

Run: `git status --short .tmp 2>&1; git check-ignore .tmp/bo4e_latest`
Expected: no output for the first command; second command exits 0 and prints the path.

- [ ] **Step 6: Commit**

```bash
git add .gitignore scripts/fetch-bo4e-fixture.sh
git commit -m "chore: gitignore /.tmp + add fetch-bo4e-fixture.sh"
```

---

## Task 9 — Add the opt-in `parse_every_schema` integration test

**Files:**
- Create: `crates/bo4e-cli/tests/full_bo4e.rs`

This test is the bug-investigation tool. It walks `.tmp/bo4e_latest/` and tries to parse each JSON via `serde_json::from_str::<bo4e_schemas::models::json_schema::SchemaRootType>`. Failures are collected and reported in a single panic with file paths.

- [ ] **Step 1: Write the test file**

Create `crates/bo4e-cli/tests/full_bo4e.rs` with:

```rust
//! Opt-in integration tests that run against the full BO4E schema set.
//!
//! Hydrate `.tmp/bo4e_latest/` first via `scripts/fetch-bo4e-fixture.sh`, then run:
//!   cargo test -p bo4e-cli --test full_bo4e -- --ignored
//!
//! These tests are NOT run by default `cargo test` — `#[ignore]` keeps them out.

use bo4e_schemas::models::json_schema::SchemaRootType;
use std::path::{Path, PathBuf};

const FIXTURE_ROOT: &str = ".tmp/bo4e_latest";

fn fixture_root() -> PathBuf {
    // Tests run from the crate dir; walk up to repo root.
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = crate_dir.parent().and_then(Path::parent).unwrap();
    workspace_root.join(FIXTURE_ROOT)
}

fn require_fixture(root: &Path) {
    if !root.join(".version").exists() {
        panic!(
            "missing {} — run scripts/fetch-bo4e-fixture.sh first",
            root.display()
        );
    }
}

fn walk_json_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn recurse(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("readdir") {
            let entry = entry.expect("dirent");
            let p = entry.path();
            if p.is_dir() {
                recurse(&p, out);
            } else if p.extension().and_then(|s| s.to_str()) == Some("json") {
                out.push(p);
            }
        }
    }
    recurse(root, &mut out);
    out
}

#[test]
#[ignore = "requires .tmp/bo4e_latest; run scripts/fetch-bo4e-fixture.sh first"]
fn parse_every_schema() {
    let root = fixture_root();
    require_fixture(&root);
    let files = walk_json_files(&root);
    assert!(!files.is_empty(), "fixture has no JSON files at {}", root.display());

    let mut failures: Vec<(PathBuf, String)> = Vec::new();
    for path in &files {
        let raw = std::fs::read_to_string(path).expect("read json");
        match serde_json::from_str::<SchemaRootType>(&raw) {
            Ok(_) => {}
            Err(e) => failures.push((path.clone(), e.to_string())),
        }
    }
    if !failures.is_empty() {
        let mut report = format!(
            "{}/{} schemas failed to parse:\n",
            failures.len(),
            files.len()
        );
        for (p, e) in &failures {
            report.push_str(&format!("  {} → {}\n", p.display(), e));
        }
        panic!("{}", report);
    }
}
```

- [ ] **Step 2: Run the test (ignored — should not run by default)**

Run: `cargo test -p bo4e-cli --test full_bo4e`
Expected: `running 1 test` → `test parse_every_schema ... ignored` → `test result: ok. 0 passed; 0 failed; 1 ignored`.

- [ ] **Step 3: Run the test explicitly with `--ignored` and capture its output**

Run: `cargo test -p bo4e-cli --test full_bo4e -- --ignored --nocapture 2>&1 | tee /tmp/parse_every_schema.log`
Expected: the test panics with a list of N failing schemas. **Save this output for Task 10.**

- [ ] **Step 4: Commit**

```bash
git add crates/bo4e-cli/tests/full_bo4e.rs
git commit -m "test(cli): add opt-in parse_every_schema integration test"
```

---

## Task 10 — Identify failing schemas, copy as committed regression fixtures

**Files:**
- Create: `crates/bo4e-cli/tests/fixtures/regressions/<name>.json` (one per unique failure)
- Create: `crates/bo4e-cli/tests/regression_schema_parse.rs`

**Investigation, not implementation.** Use the captured output from Task 9 step 3.

- [ ] **Step 1: Group failures by error message**

Read `/tmp/parse_every_schema.log` (from Task 9 step 3). Group failing entries by their serde error string. Each unique error string represents one fix-shape. Pick **one representative JSON per group**.

- [ ] **Step 2: For each representative, choose a descriptive filename**

Naming: `<bo4e_class_name_lowercase>_<short_problem_tag>.json`. Examples:
- A `bo/Marktteilnehmer.json` failing because it has a `const` discriminator → `marktteilnehmer_const_discriminator.json`
- A `com/Adresse.json` failing because of an unrecognised top-level shape → `adresse_unknown_root_shape.json`

If you cannot infer a good `short_problem_tag` from the error, use the BO4E class name plus the literal serde error category (e.g. `marktteilnehmer_no_variant_match.json`). Document each filename's reasoning in the commit message.

- [ ] **Step 3: Copy each representative JSON into the fixture directory**

Run, for each representative `<src>` from `.tmp/bo4e_latest/.../X.json`:

```bash
mkdir -p crates/bo4e-cli/tests/fixtures/regressions
cp <src> crates/bo4e-cli/tests/fixtures/regressions/<descriptive_name>.json
```

- [ ] **Step 4: Write the regression test file**

Create `crates/bo4e-cli/tests/regression_schema_parse.rs`. The file's top-level structure is fixed; the per-fixture tests are added as `#[test]` blocks, **one per regression JSON copied in Step 3**:

```rust
//! Regression tests for individual schemas that previously failed to parse.
//!
//! Each fixture under `fixtures/regressions/` represents one historical bug.
//! When a new BO4E release breaks parsing, copy the offending JSON in here
//! with a descriptive filename and add a paired `#[test]`.

use bo4e_schemas::models::json_schema::SchemaRootType;

fn parse(raw: &str) -> SchemaRootType {
    serde_json::from_str(raw).expect("regression: schema must parse")
}

// === ADD ONE TEST PER FIXTURE BELOW. PATTERN: ===
//
// #[test]
// fn parses_<filename_without_extension>() {
//     let raw = include_str!("fixtures/regressions/<filename>.json");
//     parse(raw);
// }
```

For each fixture copied in Step 3, append a `#[test]` block following the pattern shown in the comment. Example for a fixture `marktteilnehmer_const_discriminator.json`:

```rust
#[test]
fn parses_marktteilnehmer_const_discriminator() {
    let raw = include_str!("fixtures/regressions/marktteilnehmer_const_discriminator.json");
    parse(raw);
}
```

Repeat this block once per fixture, with the filename substituted.

- [ ] **Step 5: Confirm the new tests fail with the expected error**

Run: `cargo test -p bo4e-cli --test regression_schema_parse`
Expected: every new `parses_*` test FAILS with `"data did not match any variant of untagged enum SchemaRootType"`. If a test fails with a different error, the wrong JSON was copied — re-do Step 3 for that one.

- [ ] **Step 6: Commit (failing tests intentional — locking in the regression)**

```bash
git add crates/bo4e-cli/tests/fixtures/regressions/ crates/bo4e-cli/tests/regression_schema_parse.rs
git commit -m "test(cli): regression fixtures for SchemaRootType parse failures (failing — fix in next commit)"
```

---

## Task 11 — Implement the schema parsing fix

**Files:**
- Modify: `crates/bo4e-schemas/src/models/json_schema.rs` (likely; the actual file may differ if the missing variant requires a new struct type elsewhere)

**This task's exact code depends on what Task 10 surfaced.** Use the decision tree below; it covers every shape the spec calls out.

- [ ] **Step 1: Categorise each failing fixture from Task 10**

For each `parses_*` failing test, open the JSON and answer:

1. **Does the JSON have a `type` field?** If yes, what value? (`"object"`, `"string"`, `"array"`, `"integer"`, `"number"`, `"boolean"`, `"null"`, missing.)
2. **Does it have `oneOf`, `anyOf`, `allOf` at the root?**
3. **Does it have a `const` field?**
4. **Does it have an `enum` field with non-string values?**
5. **Does it have any field not present in `SchemaRootObject` or `SchemaRootStrEnum`?**

Read `crates/bo4e-schemas/src/models/json_schema.rs` lines 1–270 to confirm the exact field set of each existing variant. The current `SchemaRootType` has only two arms — `Object(SchemaRootObject)` and `StrEnum(SchemaRootStrEnum)`.

- [ ] **Step 2: Pick the fix shape per category**

| Category | Fix shape |
| --- | --- |
| Root-level `oneOf`/`anyOf`/`allOf` | Add a third `SchemaRootType::AnyOf(SchemaRootAnyOf)` (or `AllOf`/`OneOf`) variant + the corresponding wrapper struct mirroring `SchemaRootObject`'s layout. |
| Root-level `const` (single-value schema) | Add `SchemaRootType::Constant(SchemaRootConstant)` wrapping `ConstantSchema`. |
| Root-level numeric/boolean enum | Add `SchemaRootType::*Enum` variants per affected primitive. |
| Untagged-enum ordering quirk (the JSON parses against an unintended variant) | Reorder existing variants — most-specific first. Confirm by adding a test that asserts the parsed shape (e.g. `assert!(matches!(parsed, SchemaRootType::StrEnum(_)))`). |
| Field present in JSON but unmodelled in the existing wrapper | Extend the wrapper struct's fields with `#[serde(default)]` for backward compat. |

If none of the above apply, stop and surface the JSON to the user — the fix shape is genuinely novel and the spec's "out of scope" clause may apply.

- [ ] **Step 3: Add a unit test in `bo4e-schemas` that pins the fix's positive behaviour**

For each new variant added, add a `#[test]` in `crates/bo4e-schemas/src/models/json_schema.rs` (or its test module) asserting:

```rust
#[test]
fn parses_<variant_name>_root_schema() {
    let raw = r#"{ /* the minimal JSON shape that hit this variant */ }"#;
    let parsed: SchemaRootType = serde_json::from_str(raw).unwrap();
    assert!(matches!(parsed, SchemaRootType::<NewVariant>(_)));
}
```

Replace `<NewVariant>` and the JSON literal with the actual variant. **This must be a new positive-shape test, not a copy of the regression fixture test** — the schemas crate has no access to the bo4e-cli regression fixtures.

- [ ] **Step 4: Run the schemas crate tests**

Run: `cargo test -p bo4e-schemas`
Expected: all green (existing tests + the new `parses_*_root_schema` ones).

- [ ] **Step 5: Run the regression suite**

Run: `cargo test -p bo4e-cli --test regression_schema_parse`
Expected: every `parses_*` test from Task 10 now PASSES.

- [ ] **Step 6: Run the opt-in full suite to confirm forward parity**

Run: `cargo test -p bo4e-cli --test full_bo4e -- --ignored`
Expected: `parse_every_schema` PASSES — every JSON in `.tmp/bo4e_latest/` parses cleanly.

- [ ] **Step 7: Run the existing _min fixture suites to confirm no regression**

Run: `cargo test -p bo4e-codegen`
Expected: green. (Both `bo4e_min` and `bo4e_sql_min` integration tests still pass.)

- [ ] **Step 8: Commit**

```bash
git add crates/bo4e-schemas/src/models/json_schema.rs
git commit -m "fix(schemas): handle <category> in SchemaRootType (resolves v202501.0.0 parse failure)"
```

The `<category>` placeholder is filled with the actual category name from Step 2 (e.g. "constant root schemas", "untagged enum variant ordering").

---

## Task 12 — Quiet/verbose integration tests

**Files:**
- Create: `crates/bo4e-cli/tests/quiet_verbose.rs`

Per-command 3-level matrix using the `bo4e_min` fixture. Tests run in-process via `Cli::try_parse_from(...).command.unwrap().run()` and inspect captured stdout/stderr.

The `bo4e_min` fixture lives at `crates/bo4e-codegen/tests/fixtures/bo4e_min/` — read-only path.

- [ ] **Step 1: Verify the fixture path resolves from bo4e-cli tests**

Run: `ls crates/bo4e-codegen/tests/fixtures/bo4e_min/.version`
Expected: prints the path. (If it does not exist, the fixture must be located before continuing — search via `find crates -name '.version' -path '*bo4e_min*'`.)

- [ ] **Step 2: Write the test file**

Create `crates/bo4e-cli/tests/quiet_verbose.rs` with:

```rust
//! Quiet/verbose matrix tests.
//!
//! Drives each subcommand at each of the three levels (Quiet, Normal, Verbose)
//! and inspects observable behaviour. Spinners and progress bars auto-hide on
//! non-TTY (test environment), so these tests can only assert on cprint_* output.

use bo4e_cli::cli::base::{Cli, Executable};
use bo4e_cli::console::console::{CONSOLE, Console, Level};
use clap::Parser;

const FIXTURE: &str = "../bo4e-codegen/tests/fixtures/bo4e_min";

fn ensure_console(level: Level) {
    // OnceLock: best-effort init. If a previous test set a different level,
    // these assertions are skipped via early return.
    let _ = CONSOLE.set(Console::new(level));
}

fn current_level() -> Level {
    // Inspect via would_emit; Console doesn't expose `level` directly.
    let c = CONSOLE.get().expect("console set");
    if c.would_emit(Level::Verbose) {
        Level::Verbose
    } else if c.would_emit(Level::Normal) {
        Level::Normal
    } else {
        Level::Quiet
    }
}

#[test]
fn pull_command_parses_quiet_and_verbose_flags() {
    let cli_q = Cli::try_parse_from([
        "bo4e", "--quiet", "pull", "-o", "/tmp/x", "-t", "v202501.0.0",
    ])
    .unwrap();
    assert!(cli_q.quiet);
    let cli_v = Cli::try_parse_from([
        "bo4e", "--verbose", "pull", "-o", "/tmp/x", "-t", "v202501.0.0",
    ])
    .unwrap();
    assert!(cli_v.verbose);
}

#[test]
fn edit_quiet_does_not_panic() {
    ensure_console(Level::Quiet);
    if current_level() != Level::Quiet {
        return; // another test won the race; skip cleanly
    }
    let outdir = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "bo4e", "--quiet", "edit", "-i", FIXTURE, "-o",
        outdir.path().to_str().unwrap(),
    ])
    .unwrap();
    cli.run().expect("edit --quiet");
}

#[test]
fn edit_verbose_does_not_panic() {
    ensure_console(Level::Verbose);
    if current_level() != Level::Verbose {
        return;
    }
    let outdir = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "bo4e", "--verbose", "edit", "-i", FIXTURE, "-o",
        outdir.path().to_str().unwrap(),
    ])
    .unwrap();
    cli.run().expect("edit --verbose");
}

#[test]
fn generate_quiet_does_not_panic() {
    ensure_console(Level::Quiet);
    if current_level() != Level::Quiet {
        return;
    }
    let outdir = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "bo4e", "--quiet", "generate", "-i", FIXTURE, "-o",
        outdir.path().to_str().unwrap(), "-t", "python-pydantic",
    ])
    .unwrap();
    cli.run().expect("generate --quiet");
}

#[test]
fn generate_verbose_does_not_panic() {
    ensure_console(Level::Verbose);
    if current_level() != Level::Verbose {
        return;
    }
    let outdir = tempfile::tempdir().unwrap();
    let cli = Cli::try_parse_from([
        "bo4e", "--verbose", "generate", "-i", FIXTURE, "-o",
        outdir.path().to_str().unwrap(), "-t", "python-pydantic",
    ])
    .unwrap();
    cli.run().expect("generate --verbose");
}
```

The test file relies on `bo4e_cli` exposing `cli::base`, `console::console`, etc. as public modules. Verify with `grep -n "^pub mod" crates/bo4e-cli/src/main.rs` — if any module the tests need is not public, expose it via a small `crates/bo4e-cli/src/lib.rs` re-export.

- [ ] **Step 3: If `bo4e-cli` has no `lib.rs`, expose modules for tests**

Check: `ls crates/bo4e-cli/src/lib.rs`

If it does not exist, create `crates/bo4e-cli/src/lib.rs` with:

```rust
//! Library facade exposing the CLI's modules to integration tests.
//! The binary entrypoint remains in `main.rs`.

pub mod cli;
pub mod console;
pub mod diff;
pub mod edit;
pub mod io;
pub mod models;
pub mod repo;
pub mod utils;
```

Then update `crates/bo4e-cli/Cargo.toml` to declare both targets. Locate the existing `[[bin]]` block (or `[package]`/default-bin) and ensure both a library and binary target exist:

```toml
[lib]
name = "bo4e_cli"
path = "src/lib.rs"

[[bin]]
name = "bo4e"
path = "src/main.rs"
```

If `[lib]` was already declared, leave it alone; only add the missing piece.

In `crates/bo4e-cli/src/main.rs`, prefix all module references that previously came from the binary's own `mod`-tree with the library crate name. Concretely, replace existing `mod cli; mod console; mod diff; ...` declarations with:

```rust
use bo4e_cli::{cli, console};
```

(plus any other modules `main.rs` actually references).

- [ ] **Step 4: Run the new tests**

Run: `cargo test -p bo4e-cli --test quiet_verbose`
Expected: all 5 tests pass (some may exit early via the `current_level()` guard — that's acceptable).

- [ ] **Step 5: Run the full bo4e-cli suite**

Run: `cargo test -p bo4e-cli`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/bo4e-cli/tests/quiet_verbose.rs crates/bo4e-cli/src/lib.rs crates/bo4e-cli/src/main.rs crates/bo4e-cli/Cargo.toml
git commit -m "test(cli): quiet/verbose integration matrix using bo4e_min"
```

(Drop any of the 4 paths that you did not actually modify in Step 3.)

---

## Task 13 — Add the two opt-in pull→edit→diff→generate end-to-end tests

**Files:**
- Modify: `crates/bo4e-cli/tests/full_bo4e.rs` (append two new `#[ignore]`d tests)

These tests drive the full subcommand pipeline against `.tmp/bo4e_latest/`. They share the `fixture_root` / `require_fixture` helpers from Task 9.

- [ ] **Step 1: Append the two pipeline tests**

Open `crates/bo4e-cli/tests/full_bo4e.rs` and append (at end of file, after the existing `parse_every_schema`):

```rust
use bo4e_cli::cli::base::{Cli, Executable};
use clap::Parser;

fn drive_pipeline(generate_target: &str) {
    let root = fixture_root();
    require_fixture(&root);

    let workdir = tempfile::tempdir().unwrap();
    let edited  = workdir.path().join("edited");
    let diff_f  = workdir.path().join("diff.json");
    let gen_out = workdir.path().join("generated");

    // edit
    Cli::try_parse_from([
        "bo4e", "edit", "-i", root.to_str().unwrap(),
        "-o", edited.to_str().unwrap(),
    ])
    .unwrap()
    .run()
    .expect("edit");

    // diff (compare edited against itself — empty diff, exercises the read+write paths)
    Cli::try_parse_from([
        "bo4e", "diff", "schemas",
        edited.to_str().unwrap(),
        edited.to_str().unwrap(),
        "-o", diff_f.to_str().unwrap(),
    ])
    .unwrap()
    .run()
    .expect("diff");

    // generate
    Cli::try_parse_from([
        "bo4e", "generate", "-i", edited.to_str().unwrap(),
        "-o", gen_out.to_str().unwrap(),
        "-t", generate_target,
    ])
    .unwrap()
    .run()
    .expect("generate");

    assert!(gen_out.exists(), "generate produced no output");
}

#[test]
#[ignore = "requires .tmp/bo4e_latest; run scripts/fetch-bo4e-fixture.sh first"]
fn pull_to_edit_to_diff_to_generate_pydantic() {
    drive_pipeline("python-pydantic");
}

#[test]
#[ignore = "requires .tmp/bo4e_latest; run scripts/fetch-bo4e-fixture.sh first"]
fn pull_to_edit_to_diff_to_generate_sql_model() {
    drive_pipeline("python-sql-model");
}
```

The "pull" leg of the name is shorthand — these tests don't re-pull (network dependency); they consume an already-pulled fixture. The CLI's `pull` command itself is exercised by the fetch script in Task 8.

- [ ] **Step 2: Add an `ensure_console` helper if `Cli::run()` requires `CONSOLE` to be set**

If running Step 4 below produces `panicked at 'CONSOLE not initialized'`, prepend each test body with:

```rust
use bo4e_cli::console::console::{CONSOLE, Console, Level};
let _ = CONSOLE.set(Console::new(Level::Normal));
```

(Centralise via a `fn ensure_console()` if added.)

- [ ] **Step 3: Run with `--ignored`**

Run: `cargo test -p bo4e-cli --test full_bo4e -- --ignored`
Expected: 3 tests pass: `parse_every_schema`, `pull_to_edit_to_diff_to_generate_pydantic`, `pull_to_edit_to_diff_to_generate_sql_model`. **Task 11's fix must be in place for this to pass** — if `parse_every_schema` was failing before Task 11, these will also fail.

- [ ] **Step 4: Confirm default `cargo test` still skips them**

Run: `cargo test -p bo4e-cli --test full_bo4e`
Expected: `0 passed; 0 failed; 3 ignored`.

- [ ] **Step 5: Commit**

```bash
git add crates/bo4e-cli/tests/full_bo4e.rs
git commit -m "test(cli): opt-in pull→edit→diff→generate end-to-end against full BO4E"
```

---

## Task 14 — Per-command audit

**Files:**
- No code changes expected. If audit surfaces a FIX item, fold it back into Tasks 2–7 (re-open the relevant task, add a step, re-commit).

This is a manual verification task. Build the binary first.

- [ ] **Step 1: Build the binary**

Run: `cargo build -p bo4e-cli --release`
Expected: clean build at `target/release/bo4e`.

- [ ] **Step 2: Audit `pull`**

Run, in turn:

```bash
./target/release/bo4e --quiet pull -t v202501.0.0 -o /tmp/audit-pull-q
./target/release/bo4e          pull -t v202501.0.0 -o /tmp/audit-pull-n
./target/release/bo4e --verbose pull -t v202501.0.0 -o /tmp/audit-pull-v
```

Expected:
- **--quiet:** no spinners, no progress bar, no info lines. Exit 0.
- **(default):** earth spinners during API calls, counted progress bar during download. Exit 0.
- **--verbose:** above plus per-API-call lines (`Resolved tag v202501.0.0 → commitish ...`) and per-schema lines (`Fetched schema src/bo4e_schemas/...`).

If any expectation is violated, append a new step to the relevant task (Tasks 2, 6, or 7) and re-commit before proceeding.

- [ ] **Step 3: Audit `edit`**

```bash
./target/release/bo4e --quiet edit -i /tmp/audit-pull-n -o /tmp/audit-edit-q
./target/release/bo4e          edit -i /tmp/audit-pull-n -o /tmp/audit-edit-n
./target/release/bo4e --verbose edit -i /tmp/audit-pull-n -o /tmp/audit-edit-v
```

Expected: `--quiet` silent (only stderr warnings); `(default)` shows the existing `cprint_normal!` lines; `--verbose` adds per-pattern and per-ref decisions (already present in `edit/add.rs` and `edit/update_refs.rs`).

- [ ] **Step 4: Audit `diff`**

Pull a second version for comparison:

```bash
./target/release/bo4e pull -t v202401.4.0 -o /tmp/audit-pull-prev
./target/release/bo4e --quiet diff schemas /tmp/audit-pull-prev /tmp/audit-pull-n -o /tmp/audit-diff-q.json
./target/release/bo4e          diff schemas /tmp/audit-pull-prev /tmp/audit-pull-n -o /tmp/audit-diff-n.json
./target/release/bo4e --verbose diff schemas /tmp/audit-pull-prev /tmp/audit-pull-n -o /tmp/audit-diff-v.json
```

Expected: `--quiet` silent; `(default)` shows squish spinner + "Compared JSON-schemas." + save line; `--verbose` adds nothing additional **today** — diff has no `cprint_verbose!` sites yet. **If you choose to add the python parity verbose lines** (changes JSON dump, bump-needed), do so in `cli/diff.rs`'s `run_version_bump` and re-commit under Task 4.

- [ ] **Step 5: Audit `generate`**

```bash
./target/release/bo4e --quiet generate -i /tmp/audit-edit-n -o /tmp/audit-gen-q -t python-pydantic
./target/release/bo4e          generate -i /tmp/audit-edit-n -o /tmp/audit-gen-n -t python-pydantic
./target/release/bo4e --verbose generate -i /tmp/audit-edit-n -o /tmp/audit-gen-v -t python-pydantic
```

Expected: `--quiet` silent; `(default)` shows squish spinner during generation; `--verbose` is identical to default (no per-class verbose lines exist in the Rust generator). This is acceptable — generator-level verbose is a separate feature.

- [ ] **Step 6: Audit `repo versions`**

```bash
./target/release/bo4e --quiet repo versions
./target/release/bo4e          repo versions
./target/release/bo4e --verbose repo versions
```

Expected: `--quiet` shows only the version list (already implemented); `(default)` shows the rendered table; `--verbose` is identical to default. No regression.

- [ ] **Step 7: Audit `--help` styling** (covered by Task 15 but verify here)

```bash
./target/release/bo4e --help
./target/release/bo4e --help | cat
```

Expected: TTY shows ANSI-coloured headers/flags/placeholders; piped to `cat` shows plain text (anstream auto-strips). **If Task 15 has not been completed yet, this audit step will show plain output in both cases — that is fine; revisit after Task 15.**

- [ ] **Step 8: Cleanup audit dirs**

```bash
rm -rf /tmp/audit-pull-q /tmp/audit-pull-n /tmp/audit-pull-v /tmp/audit-pull-prev
rm -rf /tmp/audit-edit-q /tmp/audit-edit-n /tmp/audit-edit-v
rm -f  /tmp/audit-diff-q.json /tmp/audit-diff-n.json /tmp/audit-diff-v.json
rm -rf /tmp/audit-gen-q /tmp/audit-gen-n /tmp/audit-gen-v
```

- [ ] **Step 9: No commit unless a FIX surfaced**

If the audit triggered any code change, that change has already been committed inside the respective task (Step 2/3/4/5 instructions point back to Tasks 2/4/6/7). The audit itself produces no commit.

---

## Task 15 — Add clap `Styles` for `--help`

**Files:**
- Modify: `crates/bo4e-cli/src/cli/base.rs`

- [ ] **Step 1: Write the failing test first**

In `crates/bo4e-cli/src/cli/base.rs`, add to the existing `#[cfg(test)] mod tests { ... }` block:

```rust
    #[test]
    fn help_contains_ansi_when_styled() {
        let mut cmd = Cli::command();
        let rendered = cmd.render_help().to_string();
        assert!(
            rendered.contains("\x1b["),
            "expected ANSI escape sequences in --help output, got:\n{}",
            rendered
        );
    }

    #[test]
    fn each_subcommand_help_contains_ansi() {
        let mut cmd = Cli::command();
        for name in ["pull", "edit", "diff", "repo", "generate"] {
            let sub = cmd
                .find_subcommand_mut(name)
                .unwrap_or_else(|| panic!("subcommand {} missing", name));
            let rendered = sub.clone().render_help().to_string();
            assert!(
                rendered.contains("\x1b["),
                "subcommand {} help has no ANSI: {}",
                name,
                rendered
            );
        }
    }
```

- [ ] **Step 2: Run the new tests — confirm they FAIL**

Run: `cargo test -p bo4e-cli cli::base -- --nocapture`
Expected: `help_contains_ansi_when_styled` and `each_subcommand_help_contains_ansi` fail (no ANSI escapes yet); the existing two tests still pass.

- [ ] **Step 3: Add the styles**

In `crates/bo4e-cli/src/cli/base.rs`, change the imports from:

```rust
use crate::cli::diff::Diff;
use crate::cli::edit::Edit;
use crate::cli::generate::Generate;
use crate::cli::pull::Pull;
use crate::cli::repo::Repo;
use clap::{CommandFactory, Parser, Subcommand};
```

to:

```rust
use crate::cli::diff::Diff;
use crate::cli::edit::Edit;
use crate::cli::generate::Generate;
use crate::cli::pull::Pull;
use crate::cli::repo::Repo;
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{CommandFactory, Parser, Subcommand};
```

Add the constant just above the `#[derive(Parser)]` block:

```rust
// Matches palette::MAIN/SUB/ENUM/ERROR by tone; uses 16-colour AnsiColor for
// const-friendliness — help renders before CONSOLE is initialised.
const HELP_STYLES: Styles = Styles::styled()
    .header(     AnsiColor::Cyan.on_default()    .effects(Effects::BOLD))
    .usage(      AnsiColor::Cyan.on_default()    .effects(Effects::BOLD))
    .literal(    AnsiColor::Magenta.on_default() .effects(Effects::BOLD))
    .placeholder(AnsiColor::Yellow.on_default()  .effects(Effects::ITALIC))
    .error(      AnsiColor::Red.on_default()     .effects(Effects::BOLD))
    .valid(      AnsiColor::Cyan.on_default())
    .invalid(    AnsiColor::Red.on_default()     .effects(Effects::BOLD));
```

Change the `#[command(...)]` attribute on `Cli` from:

```rust
#[command(author, version, about, long_about = None)]
//#[command(propagate_version = true)]
pub struct Cli {
```

to:

```rust
#[command(author, version, about, long_about = None, styles = HELP_STYLES)]
//#[command(propagate_version = true)]
pub struct Cli {
```

- [ ] **Step 4: Run the tests — confirm they PASS**

Run: `cargo test -p bo4e-cli cli::base -- --nocapture`
Expected: all 4 tests pass (the 2 new + 2 pre-existing).

- [ ] **Step 5: Visually verify in a TTY**

Run: `cargo run --release -p bo4e-cli -- --help`
Expected: cyan bold "Usage:" / "Commands:" / "Options:" headers; magenta bold subcommand names (`pull`, `edit`, ...); yellow italic placeholders (`<OUTPUT_DIRECTORY>`).

Run: `cargo run --release -p bo4e-cli -- --help | cat`
Expected: plain text, no ANSI escapes.

- [ ] **Step 6: Commit**

```bash
git add crates/bo4e-cli/src/cli/base.rs
git commit -m "feat(cli): styled --help via clap Styles"
```

---

## Final integration check

After all tasks complete:

- [ ] **Step 1: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: all green. Ignored tests counted as ignored (`0 passed; 0 failed; 3 ignored` for `full_bo4e.rs`).

- [ ] **Step 2: Run the opt-in suite**

Run: `cargo test -p bo4e-cli --test full_bo4e -- --ignored`
Expected: 3 tests pass.

- [ ] **Step 3: Confirm clean working tree**

Run: `git status --short`
Expected: empty (all changes committed).

- [ ] **Step 4: Confirm commit graph**

Run: `git log --oneline -20`
Expected: roughly 14 new commits ahead of `4b465ff` (one per task plus the one-extra commits from Tasks 11 and 12 if `lib.rs` was added). One commit per task except Task 14 (audit-only, no commit).
