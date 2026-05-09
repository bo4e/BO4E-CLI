# CLI UX & Robustness Design

**Status:** Approved (brainstorm 2026-05-09).

**Branch:** Direct commits to `rust`. A draft PR `rust → main` may be opened for ongoing review; no separate feature branch.

**Parity reference:** `/tmp/bo4e-cli-python/src/bo4e_cli/` is checked out as a git worktree of the upstream Python implementation. Spinner names, verbose-message sites, and console behaviour are mirrored from there.

## Goal

Bring the Rust CLI to UX parity with the Python implementation along four axes — visible progress feedback, respect for `--quiet`/`--verbose`, robust schema parsing on the latest BO4E (`v202501.0.0`), and styled `--help` output — and lock the bug-fix in with regression tests against the full BO4E schema set.

## Non-Goals

- Reworking the existing `Console`/`Highlighter`/`palette` infrastructure. The work bolts onto what's there.
- New subcommands or new generator types.
- A nightly CI job for the opt-in full-BO4E suite. Recommended but out of scope for this spec.
- Pixel-exact match between help-text colours and the runtime `palette` constants. Help-text colours stay in clap's stable 16-colour `AnsiColor` API; the runtime `Highlighter` is unchanged.
- Replacing `indicatif` with a different progress library.

---

## 1. Spinners

### 1.1 New module `crates/bo4e-cli/src/console/spinner.rs`

Three named factory functions, one per rich spinner used by the python CLI. Each returns an `indicatif::ProgressBar` already configured with template, tick frames, tick interval, and `enable_steady_tick`.

| Factory | Rich spinner | Frames | Interval |
| --- | --- | --- | --- |
| `earth(msg)` | `earth` | `🌍 ` `🌎 ` `🌏 ` | 180 ms |
| `squish(msg)` | `squish` | `╫` `╪` | 100 ms |
| `grenade(msg)` | `grenade` | `،   ` `′   ` ` ´ ` ` ‾ ` `  ⸌` `  ⸊` `  \|` `  ⁎` `  ⁕` ` ෴ ` `  ⁓` `   ` `   ` `   ` | 80 ms |

The full grenade frame list is copied verbatim from `/repos/bo4e-cli/.tox/dev/Lib/site-packages/rich/_spinners.py`. A unit test asserts the constants match (string equality), so any future rich version bump that changed frames would surface as a test failure to triage.

Sketch:

```rust
use indicatif::{ProgressBar, ProgressStyle};
use std::borrow::Cow;
use std::time::Duration;

const EARTH_FRAMES:   &[&str] = &["🌍 ", "🌎 ", "🌏 "];
const SQUISH_FRAMES:  &[&str] = &["╫", "╪"];
const GRENADE_FRAMES: &[&str] = &[/* 14 entries verbatim */];

pub fn earth(msg: impl Into<Cow<'static, str>>) -> ProgressBar {
    spinner(msg, EARTH_FRAMES, 180)
}
pub fn squish(msg: impl Into<Cow<'static, str>>) -> ProgressBar {
    spinner(msg, SQUISH_FRAMES, 100)
}
pub fn grenade(msg: impl Into<Cow<'static, str>>) -> ProgressBar {
    spinner(msg, GRENADE_FRAMES, 80)
}

fn spinner(msg: impl Into<Cow<'static, str>>, frames: &'static [&'static str], ms: u64) -> ProgressBar {
    if !crate::console::console::CONSOLE
        .get()
        .map(|c| c.would_emit(crate::console::console::Level::Normal))
        .unwrap_or(true)
    {
        return ProgressBar::hidden();
    }
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .unwrap()
            .tick_strings(frames),
    );
    pb.set_message(msg.into());
    pb.enable_steady_tick(Duration::from_millis(ms));
    pb
}
```

### 1.2 Sites where spinners get added

Mechanical mapping from the python `with CONSOLE.status("...", spinner=...):` blocks:

| Rust call site | Spinner | Message |
| --- | --- | --- |
| `io/github::resolve_latest_version` | `earth` | `"Querying GitHub for latest version"` |
| `io/github::get_target_commitish_from_tag` | `earth` | `"Querying GitHub tree"` |
| `io/github::_get_schemas_from_github_recursive` (the listing phase, before the counted download bar) | `earth` | `"Querying GitHub tree"` |
| `io/cleanse::clear_dir_if_needed` | `grenade` | `format!("Clearing directory {}", directory.display())` |
| `cli/diff` (when wired): "Comparing JSON-schemas" | `squish` | `"Comparing JSON-schemas..."` |
| `cli/generate` (when wired): "Parsing schemas into Python classes" | `squish` | `"Parsing schemas into Python classes"` |
| `cli/generate`: "Validating generated Python modules" | `squish` | `"Validating generated Python modules"` |
| `cli/generate` (sql-model only): "Parsing many-to-many relationships into Python classes" | `squish` | `"Parsing many-to-many relationships into Python classes"` |

Each spinner is dropped (`drop(pb)` or scope exit) once its work completes. The existing **counted** download bar in `_execute_futures_with_progress_bar` stays — counted bars beat spinners when total is known.

### 1.3 Quiet & verbose interaction

- `Level::Quiet` → factories return `ProgressBar::hidden()`. Same call-site code, no output.
- `Level::Normal` → spinners visible.
- `Level::Verbose` → spinners visible **plus** verbose `cprint_verbose!` lines (see §2.2). Spinner lines and verbose lines coexist; `indicatif` integrates with stdout via `eprintln!` interleaving so they don't tear.
- Non-TTY (piped output, CI without TTY) → `indicatif` auto-detects and renders nothing. No `--no-spin` flag needed; matches python (rich does the same).

### 1.4 Tests

- `console::spinner::tests::frames_match_rich_v14` — three string-equality assertions against the constants, with a comment pointing at `.tox/dev/.../rich/_spinners.py` for the source of truth.
- `console::spinner::tests::quiet_returns_hidden` — set `CONSOLE` to `Level::Quiet`, call each factory, assert `pb.is_hidden()`.

---

## 2. Quiet/verbose for `pull` (and command audit)

### 2.1 Thread `Console` through `pull` and the github IO layer

`Pull::run` currently hardcodes `enable_output: true`. Replace the boolean parameter chain (`get_schemas_from_github(version, token, enable_output)` → `_execute_futures_with_progress_bar(futures, enable_output)`) with calls to the global `CONSOLE`:

| Today | After |
| --- | --- |
| `get_schemas_from_github(&version, token, true)` | `get_schemas_from_github(&version, token)` |
| `_execute_futures_with_progress_bar(futures, enable_output)` | `_execute_futures_with_progress_bar(futures)` — internally calls `CONSOLE.would_emit(Level::Normal)` to gate the bar |
| `Pull::run` does not touch `CONSOLE` | `Pull::run` unchanged at the call site (the IO layer self-gates), but covered by the audit in §2.3 |

The `pull` integration tests (if any exist; if not, see §3.2 for fixture-backed coverage) must run under `CONSOLE` set to `Level::Quiet` and assert no progress bar is rendered. Use `indicatif::ProgressDrawTarget::hidden()` if the test needs to inspect bar state.

### 2.2 Map python `show_only_on_verbose=True` sites to `cprint_verbose!`

The python implementation calls `CONSOLE.print(..., show_only_on_verbose=True)` at the sites listed in §2.2.1. Each Rust equivalent gets a `cprint_verbose!(...)` call at the same logical point. Sites the Rust port doesn't yet have (e.g. `io/git`, `io/config`) are out of scope for this spec — those are commands not yet ported.

#### 2.2.1 In-scope verbose sites

| Python site | Rust equivalent | Verbose message |
| --- | --- | --- |
| `io/github.py:104` (per-file decode in tree walk) | `io/github::_get_schemas_from_github_recursive`, inside the `futures.push` future, before `Schema::new` | `format!("Fetched schema {}", file_path)` |
| `io/github.py:130` (per-API-call) | `io/github::get_target_commitish_from_tag`, after the await | `format!("Resolved tag {} → commitish {}", version_tag, commitish)` |
| `io/cleanse.py:16` | `io/cleanse::clear_dir_if_needed`, the does-not-exist branch | `format!("Directory {} does not exist, nothing to clear.", directory.display())` |
| `io/cleanse.py:27` | `io/cleanse::clear_dir_if_needed`, the cleared branch | `format!("Cleared directory {} ({} entries removed)", directory.display(), n)` |
| `edit/add.py:60,66,69,87,93,96` (pattern-match counts, per-field skip reasons) | `cli/edit.rs` add-related branches (silent `if matched`-style sites) | Mirror python wording: `format!("Pattern '{}' matched {} fields", pattern, matches)`, etc. |
| `edit/update_refs.py:40,53,63,74` (per-ref matched/updated/skipped) | `cli/edit.rs` update-refs-related branches | Mirror python wording: `format!("Matched online reference: {}", field.ref)`, `format!("Updated reference {} to: {}", field.ref, relative_ref)`, etc. |
| `diff/version.py:31,33,39` (changes JSON dump, bump-needed messages) | `cli/diff.rs` — wherever the bump decision is made | Mirror python wording. |

The exact line numbers above are pinned to the python source as of `/tmp/bo4e-cli-python` HEAD `8ef040b` so a future python-side change is detectable as drift.

#### 2.2.2 Existing `cprint_verbose!` macro

The `cprint_verbose!` / `cprint_normal!` / `cwarn!` macros already exist (used by `cli/edit.rs`). No new macros, no new infrastructure.

### 2.3 Per-command audit

A single audit task at the end of this section. For each subcommand (`pull`, `edit`, `diff`, `generate`, `repo`), manually run:

| Command | `--quiet` expectation | `--verbose` expectation |
| --- | --- | --- |
| `pull -t latest -o /tmp/x` | No spinners, no progress bar, no info lines. Errors and warnings still on stderr. | Per-API-call lines, per-file fetched lines, plus normal output. |
| `edit -i in -o out` | Spinners suppressed; only result. | Per-pattern match counts, per-ref decisions. |
| `diff -i a -o b -t out` | Suppressed. | Changes JSON, bump-needed line. |
| `generate -i in -o out -t python-pydantic` | Suppressed. | Per-class generated lines. |
| `repo versions` | Already implemented; no regression. | (No verbose sites — confirm.) |

The audit produces a one-line note per command in the implementation plan ("PASS" / "FIX: …"). Each FIX becomes a follow-up task or a fold-in to the relevant section above.

### 2.4 Tests

- One per-command integration test under `crates/bo4e-cli/tests/quiet_verbose.rs` exercising the three levels via `assert_cmd` or by setting `CONSOLE` directly and capturing stdout/stderr. Each test runs in <1s by using a minimal in-memory or tiny disk fixture (no network).
- The `.tmp/bo4e_latest/` fixture (§3.2) is too large for these tests — they use the existing `bo4e_min` / `bo4e_sql_min` fixtures.

---

## 3. Schema parsing bug + test fixtures

### 3.1 The bug

`bo4e generate` (and `edit`, `diff`) on `v202501.0.0` fails with:

```
Error: "schema model error: Failed to parse schema: data did not match any variant of untagged enum SchemaRootType"
```

Root cause is unknown until investigation. Two known-likely shapes:

- **Missing variant** — upstream BO4E added a schema shape (e.g. a new `oneOf` form, a const enum, a tuple type) that `SchemaRootType` doesn't model.
- **Untagged-enum ordering quirk** — `SchemaRootType` has `#[serde(untagged)]`; if a more-permissive variant appears earlier in the enum than a more-specific one, valid JSONs get mis-matched. Prior incident: `ReferenceSchema` with `#[serde(default)]` matching `{}` before `AnySchema`.

The fix shape is **not** pre-decided in this spec — it depends on what the investigation finds. The spec **does** lock in:

- A failing test must land **before** any fix code (TDD).
- The fix must keep `bo4e_sql_min` and `bo4e_min` fixtures passing (no regression).
- The fix must keep all 192 schemas in `.tmp/bo4e_latest/` parsing successfully (forward parity).

### 3.2 Fixture strategy (hybrid)

#### 3.2.1 Committed regression fixtures

Path: `crates/bo4e-cli/tests/fixtures/regressions/`

One JSON file per problematic schema identified during §3.1 investigation. Each gets a paired `#[test]` in `crates/bo4e-cli/tests/regression_schema_parse.rs`:

```rust
#[test]
fn parses_<short_descriptive_name>() {
    let raw = include_str!("fixtures/regressions/<filename>.json");
    let parsed: bo4e_schemas::SchemaRootType = serde_json::from_str(raw)
        .expect("regression: schema must parse");
    // optional: shape assertions on `parsed` to lock the variant
}
```

Naming: `<bo4e_class_name>_<short_problem_tag>.json` (e.g. `marktteilnehmer_string_const_default.json`). Filenames double as test-name suffixes — `parses_marktteilnehmer_string_const_default`.

#### 3.2.2 Opt-in full-BO4E suite

Path: `crates/bo4e-cli/tests/full_bo4e.rs`

Each test marked `#[ignore]`. Runs with `cargo test -- --ignored`. Reads from `.tmp/bo4e_latest/` (gitignored, hydrated by the script in §3.2.3). One test per command:

```rust
#[test]
#[ignore = "requires .tmp/bo4e_latest; run `scripts/fetch-bo4e-fixture.sh` first"]
fn pull_to_edit_to_diff_to_generate_pydantic() {
    let src = std::path::Path::new(".tmp/bo4e_latest");
    if !src.join(".version").exists() {
        panic!("missing .tmp/bo4e_latest fixture; run scripts/fetch-bo4e-fixture.sh");
    }
    // ... drive each subcommand against `src`, assert success
}

#[test]
#[ignore = "..."]
fn pull_to_edit_to_diff_to_generate_sql_model() { /* ... */ }

#[test]
#[ignore = "..."]
fn parse_every_schema() {
    // walk .tmp/bo4e_latest/**/*.json, parse each, fail with the offending path on error
}
```

Why `#[ignore]` instead of a Cargo feature: features pollute the dependency graph and require recompilation; `--ignored` is the idiomatic Rust knob for slow/conditional tests, requires no rebuild, and surfaces as a clear "0 passed; N filtered out" line in default `cargo test`.

#### 3.2.3 Hydration script

Path: `scripts/fetch-bo4e-fixture.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p bo4e-cli --release
./target/release/bo4e pull -t latest -o .tmp/bo4e_latest
echo "Hydrated .tmp/bo4e_latest at $(cat .tmp/bo4e_latest/.version)"
```

The script takes no arguments. To pin a specific version, edit the `-t` flag (or pass through `${BO4E_VERSION:-latest}` — single-line tweak). `.gitignore` already excludes `.tmp/`.

### 3.3 Plan-side investigation task

The implementation plan starts §3 with a single investigation task that lands the failing test from a representative offending JSON. Concretely:

1. Run the parse-every-schema test (already drafted in §3.2.2). Capture the failing paths and the inner serde error message for each.
2. For each unique failure shape, copy the offending JSON to `crates/bo4e-cli/tests/fixtures/regressions/` with a descriptive filename and add the paired `#[test]`.
3. Confirm the new tests fail with the same `"data did not match any variant of untagged enum SchemaRootType"` error against the current `SchemaRootType` definition.
4. Decide the fix shape (new variant, reorder, custom Deserialize, custom default-skipping ReferenceSchema) based on what the JSONs look like.
5. Implement the fix. All regression tests + both `_min` fixtures + the opt-in `parse_every_schema` integration test must pass.

---

## 4. Help styling via clap `Styles`

### 4.1 Wiring

Single change in `crates/bo4e-cli/src/cli/base.rs`:

```rust
use clap::builder::styling::{AnsiColor, Effects, Styles};

const HELP_STYLES: Styles = Styles::styled()
    .header(     AnsiColor::Cyan   .on_default().effects(Effects::BOLD))
    .usage(      AnsiColor::Cyan   .on_default().effects(Effects::BOLD))
    .literal(    AnsiColor::Magenta.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Yellow .on_default().effects(Effects::ITALIC))
    .error(      AnsiColor::Red    .on_default().effects(Effects::BOLD))
    .valid(      AnsiColor::Cyan   .on_default())
    .invalid(    AnsiColor::Red    .on_default().effects(Effects::BOLD));

#[derive(Parser)]
#[command(author, version, about, long_about = None, styles = HELP_STYLES)]
pub struct Cli { /* unchanged */ }
```

### 4.2 Colour mapping rationale

| clap slot | Colour | Tone match in `palette.rs` | Why |
| --- | --- | --- | --- |
| `header`, `usage` | Cyan + Bold | `MAIN` | Section anchors; same role as runtime "primary" output. |
| `literal` (subcommands, flags) | Magenta + Bold | `SUB` | Distinct from headers; matches the highlighter's flag/identifier tone. |
| `placeholder` (`<OUTPUT_DIRECTORY>`) | Yellow + Italic | `ENUM` | Italic separates "value goes here" from literal flag text. |
| `error` / `invalid` | Red + Bold | `ERROR` | Matches `eprintln!` warning/error styling. |
| `valid` | Cyan | (none — neutral) | Subdued positive hint. |

The 16-colour `AnsiColor` set is used (rather than the `Highlighter`-style RGB palette) because:

1. `Styles::styled()` is `const`-friendly only with `AnsiColor` constants. Building dynamically requires moving wiring out of the derive attribute.
2. Help text renders before any other code runs (microseconds, before global `CONSOLE` is set). Const-folded styles avoid lazy-init machinery.
3. Help-text colours don't need pixel-exact palette match — the user sees runtime output and help text in different contexts.

A `// matches palette::MAIN/SUB/ENUM/ERROR by tone` comment documents the choice.

### 4.3 Terminal-dependence

Clap delegates rendering to `anstream`, which auto-detects TTY at runtime and strips ANSI escapes when stdout is piped or redirected. No code change needed; covered by the pipe test below. Override hooks (`NO_COLOR=1`, `CLICOLOR_FORCE=1`) work out of the box.

### 4.4 Tests

Path: `crates/bo4e-cli/src/cli/base.rs` (test module already present).

- `help_contains_ansi_when_styled` — call `Cli::command().render_help()`, convert to string, assert it contains `"\x1b["`.
- `subcommand_help_styled` — for each subcommand, call `Cli::command().find_subcommand(name).unwrap().clone().render_help()`, assert non-empty + contains `"\x1b["`.
- Pipe-strip behaviour is exercised manually in the audit (§2.3) — a unit test would have to swap `anstream`'s TTY detection, not worth the complexity.

---

## 5. File-by-file change inventory

| Path | Change |
| --- | --- |
| `crates/bo4e-cli/src/console/spinner.rs` | **New.** Three factory functions, three frame constants, frame-equality test, quiet-returns-hidden test. |
| `crates/bo4e-cli/src/console.rs` | Add `pub mod spinner;` alongside the existing `pub mod {console,highlighter,palette,progress_bar};` declarations. |
| `crates/bo4e-cli/src/io/github.rs` | Drop `enable_output: bool` from `get_schemas_from_github` and `_execute_futures_with_progress_bar`; gate progress bar via `CONSOLE.would_emit(Level::Normal)`; add `earth(...)` spinner around tag-resolve, tree-list, recursive-listing; add 2 `cprint_verbose!` calls (per-API-call, per-file decode). |
| `crates/bo4e-cli/src/io/cleanse.rs` | Add `grenade(...)` spinner around the clear loop; add 2 `cprint_verbose!` calls (does-not-exist, cleared). |
| `crates/bo4e-cli/src/cli/pull.rs` | Drop the third positional argument now removed from `get_schemas_from_github`. No other change needed (gating happens in IO layer). |
| `crates/bo4e-cli/src/cli/edit.rs` | Add `cprint_verbose!` calls at the sites in §2.2.1. |
| `crates/bo4e-cli/src/cli/diff.rs` | Add `squish(...)` spinners around the three diff phases; add 3 `cprint_verbose!` calls (changes JSON, bump-needed). |
| `crates/bo4e-cli/src/cli/generate.rs` | Add `squish(...)` spinners around schema-parsing/validating; for sql-model add the M:N spinner. |
| `crates/bo4e-cli/src/cli/base.rs` | Add `HELP_STYLES` const; add `styles = HELP_STYLES` to `#[command(...)]`; add 2 styling tests. |
| `crates/bo4e-cli/tests/fixtures/regressions/` | **New directory.** N JSON files identified by §3.3 investigation. |
| `crates/bo4e-cli/tests/regression_schema_parse.rs` | **New.** One `#[test]` per regression JSON. |
| `crates/bo4e-cli/tests/full_bo4e.rs` | **New.** Three `#[ignore]`d tests: `parse_every_schema`, `pull_to_edit_to_diff_to_generate_pydantic`, `pull_to_edit_to_diff_to_generate_sql_model`. |
| `crates/bo4e-cli/tests/quiet_verbose.rs` | **New.** Per-command 3-level matrix test using `_min` fixtures. |
| `scripts/fetch-bo4e-fixture.sh` | **New.** Hydration script. Mark executable. |
| `crates/bo4e-schemas/src/...` (likely `models/json_schema.rs`) | Schema-parsing fix per §3.3 — exact file decided at investigation time. |

---

## 6. Open decisions deferred to plan-time

- **Exact schema fix shape** (§3.3 step 4) — depends on what the investigation surfaces.
- **Whether spinner factories should accept a `Cow<'static, str>` vs `String`** — minor; pick whichever the indicatif API consumes most cleanly.
- **Whether `cli/diff` spinners cover all three python sites or only the user-visible one** — confirm during the audit (§2.3).

These are intentionally not pre-decided; the plan author decides at the relevant task.
