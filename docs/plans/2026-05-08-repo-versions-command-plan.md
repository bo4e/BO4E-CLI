# `bo4e repo versions` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the Python `bo4e repo versions` command to Rust, including a small Console refactor that splits info (stdout) from warn/error (stderr).

**Architecture:** A pure `filter_tags` function in a new `repo/filter.rs` module decides which tags pass; `io/git.rs` shells out to git for raw input; `io/github.rs` adds an async `release_exists`; `cli/repo.rs` glues them together with a hand-rolled 3-column table renderer.

**Tech Stack:** Rust 2024, clap (derive), tokio (existing runtime helper), octocrab, chrono, regex, lazy_static, std::process::Command for git shell-outs, console crate for terminal styling.

---

## Pre-flight: ensure clean working tree

- [ ] **Confirm starting state**

```bash
cd /repos/bo4e-cli && git status
cargo build 2>&1 | tail -3
cargo test 2>&1 | tail -3
```

Expected: clean working tree, build with 0 warnings, 108 tests passing.

---

## Task 1: Split `Console::print` into info / warn / error channels

**Files:**
- Modify: `src/console/console.rs`

The current `Console::print` always uses `eprintln!`. We replace it with three methods: `print_info` (stdout, level-gated), `print_warn` (stderr, always), `print_error` (stderr, always). The old `print` is removed; macros in Task 2 are updated to call `print_info`.

- [ ] **Step 1: Read the current file**

```bash
cat src/console/console.rs
```

Confirm `Console` has `level: Level`, `highlighter: RwLock<Highlighter>`, and a single `print(level, msg)` method.

- [ ] **Step 2: Replace `print` with three new methods**

Replace the body of `impl Console` (keeping `new`, `would_emit`, `add_schema_names` unchanged):

```rust
impl Console {
    pub fn new(level: Level) -> Self {
        Self {
            level,
            highlighter: RwLock::new(Highlighter::default()),
        }
    }

    /// Returns `true` iff a message of the given level would be emitted by this console.
    pub fn would_emit(&self, message_level: Level) -> bool {
        message_level <= self.level
    }

    /// Emit an informational message to stdout iff `message_level <= self.level`,
    /// after applying the highlighter.
    pub fn print_info(&self, message_level: Level, msg: &str) {
        if !self.would_emit(message_level) {
            return;
        }
        let highlighted = self.highlighter.read().unwrap().apply(msg);
        println!("{highlighted}");
    }

    /// Emit a warning to stderr. Never suppressed (warnings are always shown).
    pub fn print_warn(&self, msg: &str) {
        let highlighted = self.highlighter.read().unwrap().apply(msg);
        eprintln!("{highlighted}");
    }

    /// Emit an error to stderr. Never suppressed.
    pub fn print_error(&self, msg: &str) {
        let highlighted = self.highlighter.read().unwrap().apply(msg);
        eprintln!("{highlighted}");
    }

    /// Register schema names for dynamic highlighting (call once after read_schemas).
    pub fn add_schema_names(&self, names: &[String]) {
        self.highlighter.write().unwrap().add_schema_names(names);
    }
}
```

The existing `test_emission_table` test still passes — it only exercises `would_emit`.

- [ ] **Step 3: Build to confirm `print` callers exist**

```bash
cargo build 2>&1 | grep "no method named .print"
```

Expected: `cprint!` macro at `src/console.rs:9` calls `.print(...)`. We update it in Task 2.

- [ ] **Step 4: Commit**

```bash
git add src/console/console.rs
git commit -m "$(cat <<'EOF'
refactor(console): split print into print_info (stdout) + print_warn/print_error (stderr)

Routes informational output to stdout; warnings and errors to stderr.
Macros are updated in the next commit.
EOF
)"
```

Note: build is intentionally broken between this commit and Task 2's commit. They could be one commit if you prefer to squash later — keeping them split is fine because the next task is the immediate follow-up.

---

## Task 2: Update macros + add `cwarn!` / `cerror!`

**Files:**
- Modify: `src/console.rs`

Update all four existing `cprint*!` macros to call `print_info`. Add `cwarn!` and `cerror!` macros that call `print_warn` / `print_error`.

- [ ] **Step 1: Replace `src/console.rs` with the new macro set**

```rust
pub mod console;
pub mod highlighter;
pub mod palette;
pub mod progress_bar;

/// Print a formatted info message at an explicit `Level`. Goes to stdout.
/// Emitted only if `level <= CONSOLE.level`.
#[macro_export]
macro_rules! cprint {
    ($level:expr, $($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print_info($level, &format!($($arg)*))
    };
}

/// Print a `Level::Quiet` info message. Emitted under every console level (including `--quiet`).
/// Goes to stdout.
#[macro_export]
macro_rules! cprint_quiet {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Quiet, $($arg)*)
    };
}

/// Print a `Level::Normal` info message. Default informational output. Goes to stdout.
#[macro_export]
macro_rules! cprint_normal {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Normal, $($arg)*)
    };
}

/// Print a `Level::Verbose` info message. Emitted only under `--verbose`. Goes to stdout.
#[macro_export]
macro_rules! cprint_verbose {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Verbose, $($arg)*)
    };
}

/// Print a warning to stderr. Always shown, regardless of `--quiet`.
#[macro_export]
macro_rules! cwarn {
    ($($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print_warn(&format!($($arg)*))
    };
}

/// Print an error to stderr. Always shown, regardless of `--quiet`.
#[macro_export]
macro_rules! cerror {
    ($($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print_error(&format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use crate::console::console::{Console, Level, CONSOLE};

    fn ensure_console_initialized() {
        let _ = CONSOLE.set(Console::new(Level::Verbose));
    }

    #[test]
    fn test_cprint_macros_compile_and_run() {
        ensure_console_initialized();
        crate::cprint!(Level::Normal, "hello {}", "world");
        crate::cprint_quiet!("forced");
        crate::cprint_normal!("default");
        crate::cprint_verbose!("detail {}", 42);
        crate::cwarn!("warn {}", "msg");
        crate::cerror!("error {}", "msg");
    }
}
```

- [ ] **Step 2: Build and run the macro test**

```bash
cargo test --lib console::tests::test_cprint_macros_compile_and_run
```

Expected: PASS.

- [ ] **Step 3: Run full test suite to confirm no regressions**

```bash
cargo test 2>&1 | tail -3
```

Expected: 108 passed (or 109 if you count the augmented compile test) — no failures.

- [ ] **Step 4: Commit**

```bash
git add src/console.rs
git commit -m "$(cat <<'EOF'
feat(console): add cwarn!/cerror! macros, route cprint* to stdout

cprint*! macros now write to stdout via print_info.
cwarn!/cerror! write to stderr and are never suppressed.
EOF
)"
```

---

## Task 3: Migrate existing "Warning:" call sites to `cwarn!`

**Files:**
- Modify: `src/cli/edit.rs:85`
- Modify: `src/edit/add.rs:19`, `:43`, `:83`, `:99`
- Modify: `src/edit/non_nullable.rs:106`, `:151`

Seven existing call sites use `cprint_normal!("Warning: ...")` — under the project channel rule these are warnings (stderr), not info (stdout). Migrate them to `cwarn!` and drop the `"Warning: "` prefix (the macro name conveys the kind).

- [ ] **Step 1: Find every `cprint_normal!` that starts with `"Warning:`**

```bash
grep -rn 'cprint_normal!("Warning' src/
```

Expected output: 7 lines across `src/cli/edit.rs`, `src/edit/add.rs`, `src/edit/non_nullable.rs`.

- [ ] **Step 2: Update `src/cli/edit.rs`**

Find:
```rust
cprint_normal!("Warning: could not parse schema for version stamping: {}", e);
```
Replace with:
```rust
cwarn!("could not parse schema for version stamping: {}", e);
```

Update the import at the top of the file: `use crate::{cprint_normal, cprint_verbose};` becomes `use crate::{cprint_normal, cprint_verbose, cwarn};`.

- [ ] **Step 3: Update `src/edit/add.rs`**

Update the import: `use crate::{cprint_normal, cprint_verbose};` becomes `use crate::{cprint_normal, cprint_verbose, cwarn};`.

Replace all 4 occurrences:

| Before | After |
| --- | --- |
| `cprint_normal!("Warning: could not parse schema '{}': {}", module_path, e);` | `cwarn!("could not parse schema '{}': {}", module_path, e);` |
| `cprint_normal!("Warning: pattern '{}' did not match any schemas", field.pattern);` | `cwarn!("pattern '{}' did not match any schemas", field.pattern);` |
| `cprint_normal!("Warning: could not parse schema '{}': {}", module_path, e);` (second occurrence) | `cwarn!("could not parse schema '{}': {}", module_path, e);` |
| `cprint_normal!("Warning: pattern '{}' did not match any schemas", item.pattern);` | `cwarn!("pattern '{}' did not match any schemas", item.pattern);` |

- [ ] **Step 4: Update `src/edit/non_nullable.rs`**

Update the import: `use crate::{cprint_normal, cprint_verbose};` becomes `use crate::{cprint_normal, cprint_verbose, cwarn};`.

Replace:

| Before | After |
| --- | --- |
| `cprint_normal!("Warning: could not parse schema '{}': {}", module_path, e);` | `cwarn!("could not parse schema '{}': {}", module_path, e);` |
| `cprint_normal!("Warning: non-nullable pattern '{}' did not match any fields", pattern);` | `cwarn!("non-nullable pattern '{}' did not match any fields", pattern);` |

- [ ] **Step 5: Verify no `cprint_normal!("Warning` remain**

```bash
grep -rn 'cprint_normal!("Warning' src/
```

Expected: no output.

- [ ] **Step 6: Build + test**

```bash
cargo build 2>&1 | tail -3
cargo test 2>&1 | tail -3
```

Expected: clean build, 108+ tests passing.

- [ ] **Step 7: Commit**

```bash
git add src/cli/edit.rs src/edit/add.rs src/edit/non_nullable.rs
git commit -m "$(cat <<'EOF'
refactor: migrate 'Warning:' info-prefixed prints to cwarn! (stderr)

These messages are warnings, not info — they belong on stderr per the
project channel rule. Drops the redundant 'Warning:' prefix since the
cwarn! macro name already conveys the kind.
EOF
)"
```

---

## Task 4: Add `RefKind` enum to `models/git.rs`

**Files:**
- Modify: `src/models/git.rs`

- [ ] **Step 1: Read the current file**

```bash
cat src/models/git.rs
```

Expected: a single `Reference` enum with `#[allow(dead_code)]`.

- [ ] **Step 2: Add `RefKind` next to `Reference`**

Replace the file contents with:

```rust
#[allow(dead_code)]
pub enum Reference {
    Branch(String),
    Tag(String),
    Commit(String),
    Head,
}

/// Lightweight classification of a git reference, returned by `io::git::get_ref`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefKind {
    Tag,
    Branch,
    Commit,
}
```

- [ ] **Step 3: Build to confirm it compiles**

```bash
cargo build 2>&1 | tail -3
```

Expected: clean (no new warnings — `RefKind` is used by `io::git::get_ref` in Task 10).

- [ ] **Step 4: Commit**

```bash
git add src/models/git.rs
git commit -m "feat(models): add RefKind enum for git reference classification"
```

---

## Task 5: Create `repo/` module + `FilterOptions` struct

**Files:**
- Create: `src/repo.rs`
- Create: `src/repo/filter.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Create the module declaration file**

Create `src/repo.rs`:

```rust
pub mod filter;
```

- [ ] **Step 2: Create the filter file with the struct + empty function**

Create `src/repo/filter.rs`:

```rust
use crate::models::version::Version;

pub struct FilterOptions {
    /// Number of versions to return. `0` means "all since `threshold`".
    pub n: u32,
    /// Drop release-candidate versions.
    pub exclude_candidates: bool,
    /// In each functional group, keep only the newest technical version.
    pub exclude_technical_bumps: bool,
    /// Drop the first input element (set when the user-supplied ref is itself a tag).
    pub skip_first: bool,
    /// Stop iteration when this version is reached. Used only when `n == 0`.
    pub threshold: Version,
}

/// Pure filter over an already-sorted (descending) list of candidate versions.
///
/// `is_release` is invoked for each candidate that survives all other rules.
/// Returning `Ok(false)` skips the candidate; returning `Err` aborts the whole
/// filter and propagates the error to the caller.
pub fn filter_tags(
    candidates: &[Version],
    opts: &FilterOptions,
    mut is_release: impl FnMut(&Version) -> Result<bool, String>,
) -> Result<Vec<Version>, String> {
    let _ = (candidates, opts, &mut is_release);
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::version::Version;

    fn v(s: &str) -> Version {
        s.parse().unwrap()
    }

    fn opts(n: u32) -> FilterOptions {
        FilterOptions {
            n,
            exclude_candidates: false,
            exclude_technical_bumps: false,
            skip_first: false,
            threshold: v("v202401.0.0"),
        }
    }

    #[test]
    fn test_empty_input_returns_empty() {
        let out = filter_tags(&[], &opts(0), |_| Ok(true)).unwrap();
        assert!(out.is_empty());
    }
}
```

- [ ] **Step 3: Register the module in `main.rs`**

Add `mod repo;` next to the existing `mod` declarations:

```rust
mod cli;
mod console;
mod diff;
mod edit;
mod io;
mod models;
mod repo;
mod utils;
```

- [ ] **Step 4: Run the (passing) test**

```bash
cargo test --lib repo::filter::tests::test_empty_input_returns_empty
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/repo.rs src/repo/filter.rs src/main.rs
git commit -m "feat(repo): scaffold filter module with FilterOptions"
```

---

## Task 6: `filter_tags` — stop rules (TDD)

**Files:**
- Modify: `src/repo/filter.rs`

Implement the two stop rules: `n > 0 && out.len() >= n`, and `n == 0 && current == threshold`.

- [ ] **Step 1: Write three failing tests**

Append to the `tests` module in `src/repo/filter.rs`:

```rust
#[test]
fn test_n_positive_stops_after_n_yields() {
    let cands = vec![v("v202401.5.0"), v("v202401.4.0"), v("v202401.3.0"), v("v202401.2.0")];
    let out = filter_tags(&cands, &opts(2), |_| Ok(true)).unwrap();
    assert_eq!(out, vec![v("v202401.5.0"), v("v202401.4.0")]);
}

#[test]
fn test_n_zero_stops_at_threshold() {
    let cands = vec![v("v202401.2.0"), v("v202401.1.0"), v("v202401.0.0"), v("v202400.9.0")];
    let out = filter_tags(&cands, &opts(0), |_| Ok(true)).unwrap();
    assert_eq!(out, vec![v("v202401.2.0"), v("v202401.1.0")]);
}

#[test]
fn test_n_zero_no_threshold_returns_all() {
    let cands = vec![v("v202401.2.0"), v("v202401.1.0")];
    let out = filter_tags(&cands, &opts(0), |_| Ok(true)).unwrap();
    assert_eq!(out, vec![v("v202401.2.0"), v("v202401.1.0")]);
}
```

- [ ] **Step 2: Run the tests — expect failures**

```bash
cargo test --lib repo::filter::tests
```

Expected: 3 failures (the new tests; the empty test still passes).

- [ ] **Step 3: Implement `filter_tags`**

Replace the body of `filter_tags` with:

```rust
pub fn filter_tags(
    candidates: &[Version],
    opts: &FilterOptions,
    mut is_release: impl FnMut(&Version) -> Result<bool, String>,
) -> Result<Vec<Version>, String> {
    let mut out: Vec<Version> = Vec::new();
    for v in candidates.iter() {
        if opts.n > 0 && out.len() as u32 >= opts.n {
            break;
        }
        if opts.n == 0 && *v == opts.threshold {
            break;
        }
        if !is_release(v)? {
            continue;
        }
        out.push(v.clone());
    }
    Ok(out)
}
```

(The skip rules go in Task 7.)

- [ ] **Step 4: Run tests — expect pass**

```bash
cargo test --lib repo::filter::tests
```

Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add src/repo/filter.rs
git commit -m "feat(repo): filter_tags stop rules (n cap and threshold)"
```

---

## Task 7: `filter_tags` — skip rules (TDD)

**Files:**
- Modify: `src/repo/filter.rs`

Add the three skip rules: `exclude_candidates`, `exclude_technical_bumps` (against last yielded), `skip_first` (by input index).

- [ ] **Step 1: Write four failing tests**

Append to the `tests` module:

```rust
#[test]
fn test_exclude_candidates_drops_rcs_only() {
    let cands = vec![v("v202401.3.0"), v("v202401.2.0-rc1"), v("v202401.2.0")];
    let mut o = opts(0);
    o.exclude_candidates = true;
    let out = filter_tags(&cands, &o, |_| Ok(true)).unwrap();
    assert_eq!(out, vec![v("v202401.3.0"), v("v202401.2.0")]);
}

#[test]
fn test_exclude_technical_bumps_keeps_newest_per_group() {
    // Three technical bumps under v202401.2.x, plus a different functional group above.
    let cands = vec![
        v("v202401.3.0"),
        v("v202401.2.5"),
        v("v202401.2.4"),
        v("v202401.2.3"),
        v("v202401.1.0"),
    ];
    let mut o = opts(0);
    o.exclude_technical_bumps = true;
    let out = filter_tags(&cands, &o, |_| Ok(true)).unwrap();
    // Newest of each functional group: v202401.3.0, v202401.2.5, v202401.1.0
    assert_eq!(
        out,
        vec![v("v202401.3.0"), v("v202401.2.5"), v("v202401.1.0")]
    );
}

#[test]
fn test_skip_first_drops_index_zero() {
    let cands = vec![v("v202401.3.0"), v("v202401.2.0"), v("v202401.1.0")];
    let mut o = opts(0);
    o.skip_first = true;
    let out = filter_tags(&cands, &o, |_| Ok(true)).unwrap();
    assert_eq!(out, vec![v("v202401.2.0"), v("v202401.1.0")]);
}

#[test]
fn test_skip_first_is_by_input_index_not_post_filter() {
    // Index 0 is an RC. With both flags on, it gets dropped by skip_first OR
    // by exclude_candidates — but we still only drop one element.
    let cands = vec![v("v202401.3.0-rc1"), v("v202401.3.0"), v("v202401.2.0")];
    let mut o = opts(0);
    o.skip_first = true;
    o.exclude_candidates = true;
    let out = filter_tags(&cands, &o, |_| Ok(true)).unwrap();
    assert_eq!(out, vec![v("v202401.3.0"), v("v202401.2.0")]);
}
```

- [ ] **Step 2: Run — expect failures**

```bash
cargo test --lib repo::filter::tests
```

Expected: 4 new failures.

- [ ] **Step 3: Implement skip rules**

Replace `filter_tags` with:

```rust
pub fn filter_tags(
    candidates: &[Version],
    opts: &FilterOptions,
    mut is_release: impl FnMut(&Version) -> Result<bool, String>,
) -> Result<Vec<Version>, String> {
    let mut out: Vec<Version> = Vec::new();
    let mut last_yielded: Option<Version> = None;
    for (i, v) in candidates.iter().enumerate() {
        if opts.n > 0 && out.len() as u32 >= opts.n {
            break;
        }
        if opts.n == 0 && *v == opts.threshold {
            break;
        }
        if opts.exclude_candidates && v.is_release_candidate() {
            continue;
        }
        if opts.exclude_technical_bumps {
            if let Some(prev) = &last_yielded {
                if prev.bumped_technical(v) {
                    continue;
                }
            }
        }
        if i == 0 && opts.skip_first {
            continue;
        }
        if !is_release(v)? {
            continue;
        }
        out.push(v.clone());
        last_yielded = out.last().cloned();
    }
    Ok(out)
}
```

- [ ] **Step 4: Run — expect pass**

```bash
cargo test --lib repo::filter::tests
```

Expected: 8 passed.

- [ ] **Step 5: Commit**

```bash
git add src/repo/filter.rs
git commit -m "feat(repo): filter_tags skip rules (RCs, technical bumps, first)"
```

---

## Task 8: `filter_tags` — `is_release` semantics + combination test

**Files:**
- Modify: `src/repo/filter.rs`

The implementation already calls `is_release`; this task pins the semantics with tests.

- [ ] **Step 1: Add three tests**

Append to the `tests` module:

```rust
#[test]
fn test_is_release_false_skips_version() {
    let cands = vec![v("v202401.3.0"), v("v202401.2.0"), v("v202401.1.0")];
    let out = filter_tags(&cands, &opts(0), |x| Ok(*x != v("v202401.2.0"))).unwrap();
    assert_eq!(out, vec![v("v202401.3.0"), v("v202401.1.0")]);
}

#[test]
fn test_is_release_err_aborts() {
    let cands = vec![v("v202401.3.0"), v("v202401.2.0")];
    let result = filter_tags(&cands, &opts(0), |_| Err("network".to_string()));
    assert_eq!(result, Err("network".to_string()));
}

#[test]
fn test_combination_n_with_skip_rules() {
    // n=3, exclude_candidates, exclude_technical_bumps, skip_first
    // Input (descending):  rc, 3.5, 3.0, 2.0, 1.5, 1.0
    // skip_first: drops rc (index 0)
    // exclude_candidates: would also drop rc (no double-count)
    // exclude_technical_bumps: from each functional group, keep newest:
    //   3.x → 3.5, 2.x → 2.0, 1.x → 1.5
    // n=3 cap: stop after 3 yields → 3.5, 2.0, 1.5
    let cands = vec![
        v("v202401.4.0-rc1"),
        v("v202401.3.5"),
        v("v202401.3.0"),
        v("v202401.2.0"),
        v("v202401.1.5"),
        v("v202401.1.0"),
    ];
    let mut o = opts(3);
    o.exclude_candidates = true;
    o.exclude_technical_bumps = true;
    o.skip_first = true;
    let out = filter_tags(&cands, &o, |_| Ok(true)).unwrap();
    assert_eq!(
        out,
        vec![v("v202401.3.5"), v("v202401.2.0"), v("v202401.1.5")]
    );
}
```

- [ ] **Step 2: Run — expect pass on first try**

```bash
cargo test --lib repo::filter::tests
```

Expected: 11 passed (these tests exercise existing behavior, not new code).

- [ ] **Step 3: Commit**

```bash
git add src/repo/filter.rs
git commit -m "test(repo): pin is_release semantics + filter combination"
```

---

## Task 9: Implement `tags_merged` in `io/git.rs`

**Files:**
- Modify: `src/io/git.rs`

Shell out to `git tag --merged <ref> --sort=-version:refname --sort=-creatordate`. Test against a real temp git fixture.

- [ ] **Step 1: Add the test**

Append a `tests` module to `src/io/git.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    /// Initialize a git repo with 3 tagged commits.
    /// Returns the tempdir guard (drop = cleanup).
    fn make_git_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let run = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(p)
                .output()
                .expect("git invocation failed");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@t.t"]);
        run(&["config", "user.name", "t"]);
        run(&["commit", "--allow-empty", "-m", "c1", "-q"]);
        run(&["tag", "v202401.0.1"]);
        run(&["commit", "--allow-empty", "-m", "c2", "-q"]);
        run(&["tag", "v202401.0.2"]);
        run(&["commit", "--allow-empty", "-m", "c3", "-q"]);
        run(&["tag", "v202401.1.0"]);
        run(&["tag", "not-a-version"]);
        dir
    }

    #[test]
    fn test_tags_merged_returns_descending_version_order() {
        let dir = make_git_repo();
        let _g = std::env::set_current_dir(dir.path()).unwrap();
        let tags = tags_merged("HEAD").unwrap();
        // Descending: 1.0 first, then 0.2, 0.1, then non-version tag.
        assert_eq!(tags[0], "v202401.1.0");
        assert!(tags.contains(&"v202401.0.2".to_string()));
        assert!(tags.contains(&"v202401.0.1".to_string()));
        assert!(tags.contains(&"not-a-version".to_string()));
    }
}
```

Note: the test uses `std::env::set_current_dir` because the existing `git` invocations don't pass a working directory. If you'd rather pass `--git-dir`/`--work-tree`, refactor — but the simpler approach matches the existing scaffolding which uses default cwd.

- [ ] **Step 2: Add `tags_merged` (still failing)**

Add this function to `src/io/git.rs`. Only remove `#[allow(dead_code)]` from items this task makes live (`check_success` and `tags_merged` itself); `clone_repo` and other still-unused items keep their per-item `#[allow(dead_code)]`:

```rust
pub fn tags_merged(reference: &str) -> io::Result<Vec<String>> {
    let output = Command::new("git")
        .args([
            "tag",
            "--merged",
            reference,
            "--sort=-version:refname",
            "--sort=-creatordate",
        ])
        .output()?;
    check_success(&output, "Failed to list merged tags.")?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}
```

`check_success` is already in the file — drop its `#[allow(dead_code)]` since it's now used by `tags_merged`.

- [ ] **Step 3: Run the test**

```bash
cargo test --lib io::git::tests::test_tags_merged_returns_descending_version_order
```

Expected: PASS.

- [ ] **Step 4: Run all tests to confirm no regressions**

```bash
cargo test 2>&1 | tail -3
```

- [ ] **Step 5: Commit**

```bash
git add src/io/git.rs
git commit -m "feat(io/git): implement tags_merged (real-git fixture test)"
```

---

## Task 10: Implement `get_ref` with HEAD fallback

**Files:**
- Modify: `src/io/git.rs`

Make `is_version_tag`, `is_branch`, `is_commit_hash` public. Add `get_ref` that classifies a value as `(RefKind, resolved_string)`, falling back to current HEAD with an info message if the value matches none of the three. Drop `#[allow(dead_code)]` from items now in the public path.

- [ ] **Step 1: Add the test**

Append to the `tests` module in `src/io/git.rs`:

```rust
#[test]
fn test_get_ref_classifies_tag_branch_and_falls_back_to_head() {
    let dir = make_git_repo();
    let _g = std::env::set_current_dir(dir.path()).unwrap();

    // Existing console for the fallback's info message.
    use crate::console::console::{Console, Level, CONSOLE};
    let _ = CONSOLE.set(Console::new(Level::Quiet));

    let (kind, value) = get_ref("v202401.0.1").unwrap();
    assert_eq!(kind, crate::models::git::RefKind::Tag);
    assert_eq!(value, "v202401.0.1");

    let (kind, value) = get_ref("main").unwrap();
    assert_eq!(kind, crate::models::git::RefKind::Branch);
    assert_eq!(value, "main");

    // Unknown value → fallback to HEAD's commit SHA.
    let (kind, value) = get_ref("definitely-not-a-ref").unwrap();
    assert_eq!(kind, crate::models::git::RefKind::Commit);
    assert_eq!(value.len(), 40); // full SHA from `git rev-parse HEAD`
}
```

- [ ] **Step 2: Make is_* helpers pub and remove their dead_code allows**

In `src/io/git.rs`, change:

```rust
#[allow(dead_code)]
fn is_version_tag(value: &str) -> io::Result<bool> { ... }

#[allow(dead_code)]
fn is_branch(value: &str) -> io::Result<bool> { ... }

#[allow(dead_code)]
fn is_commit_hash(value: &str) -> io::Result<bool> { ... }

#[allow(dead_code)]
fn get_branches_containing_commit(...) // keep this allow — get_ref doesn't need it but is_commit_hash does

#[allow(dead_code)]
fn get_commit_sha(branch_or_tag: &str) -> io::Result<String> { ... }
```

to:

```rust
pub fn is_version_tag(value: &str) -> io::Result<bool> { ... }
pub fn is_branch(value: &str) -> io::Result<bool> { ... }
pub fn is_commit_hash(value: &str) -> io::Result<bool> { ... }
// get_branches_containing_commit stays as a private helper of is_commit_hash; keep its #[allow]
pub fn get_commit_sha(branch_or_tag: &str) -> io::Result<String> { ... }
```

(The `get_commit_date` function gets its `#[allow]` removed in Task 11.)

- [ ] **Step 3: Add `get_ref`**

Add to `src/io/git.rs`, near the bottom but before the `tests` module:

```rust
use crate::cprint_normal;
use crate::models::git::RefKind;

pub fn get_ref(value: &str) -> io::Result<(RefKind, String)> {
    if is_version_tag(value)? {
        return Ok((RefKind::Tag, value.to_string()));
    }
    if is_branch(value)? {
        return Ok((RefKind::Branch, value.to_string()));
    }
    if is_commit_hash(value)? {
        return Ok((RefKind::Commit, value.to_string()));
    }
    if value == "HEAD" {
        return Ok((RefKind::Commit, get_commit_sha("HEAD")?));
    }
    let cur = get_commit_sha("HEAD")?;
    cprint_normal!("'{value}' is not a tag, branch, or commit; falling back to HEAD ({cur}).");
    Ok((RefKind::Commit, cur))
}
```

- [ ] **Step 4: Run the test**

```bash
cargo test --lib io::git::tests::test_get_ref_classifies_tag_branch_and_falls_back_to_head
```

Expected: PASS.

- [ ] **Step 5: Build to confirm no warnings**

```bash
cargo build 2>&1 | tail -3
```

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add src/io/git.rs
git commit -m "feat(io/git): add get_ref with HEAD fallback; expose is_* helpers"
```

---

## Task 11: Implement `get_last_n_tags` body + finalize commit helpers

**Files:**
- Modify: `src/io/git.rs`

Replace the stubbed body of `get_last_n_tags` with a real implementation that calls `tags_merged` + parses to `Version`s + delegates to `filter_tags`. Make `get_commit_date` public; drop its `#[allow]`. Replace the existing function signature to accept structured options.

- [ ] **Step 1: Define the options struct + new signature**

In `src/io/git.rs`, replace the entire stubbed `get_last_n_tags` block (and its `#[allow]`) with:

```rust
use crate::cwarn;
use crate::models::version::Version;
use crate::repo::filter::{FilterOptions, filter_tags};
use std::str::FromStr;

pub struct GetLastNTagsOpts<'a, F>
where
    F: FnMut(&Version) -> Result<bool, String>,
{
    pub n: u32,
    pub reference: &'a str,
    pub exclude_candidates: bool,
    pub exclude_technical_bumps: bool,
    pub skip_first: bool,
    pub is_release: F,
}

pub fn get_last_n_tags<F>(opts: GetLastNTagsOpts<'_, F>) -> Result<Vec<Version>, String>
where
    F: FnMut(&Version) -> Result<bool, String>,
{
    let raw = tags_merged(opts.reference).map_err(|e| e.to_string())?;

    let mut candidates: Vec<Version> = Vec::with_capacity(raw.len());
    for tag in raw {
        match Version::from_str(&tag) {
            Ok(v) => candidates.push(v),
            Err(_) => cwarn!("skipping unparseable tag '{tag}'"),
        }
    }

    let filter_opts = FilterOptions {
        n: opts.n,
        exclude_candidates: opts.exclude_candidates,
        exclude_technical_bumps: opts.exclude_technical_bumps,
        skip_first: opts.skip_first,
        threshold: Version::from_str("v202401.0.0").expect("hardcoded threshold parses"),
    };

    filter_tags(&candidates, &filter_opts, opts.is_release)
}
```

- [ ] **Step 2: Remove `#[allow(dead_code)]` from `get_commit_date`**

In `src/io/git.rs`, change:
```rust
#[allow(dead_code)]
fn get_commit_date(commit: &str) -> io::Result<String> {
```
to:
```rust
pub fn get_commit_date(commit: &str) -> io::Result<String> {
```

- [ ] **Step 3: Build**

```bash
cargo build 2>&1 | tail -10
```

Expected: clean. If you get unused-import warnings for `cwarn` or `cprint_normal` in tests, ignore (they're conditional).

- [ ] **Step 4: Add a basic integration test for `get_last_n_tags`**

Append to the `tests` module in `src/io/git.rs`:

```rust
#[test]
fn test_get_last_n_tags_returns_valid_versions_in_order() {
    let dir = make_git_repo();
    let _g = std::env::set_current_dir(dir.path()).unwrap();

    use crate::console::console::{Console, Level, CONSOLE};
    let _ = CONSOLE.set(Console::new(Level::Quiet));

    let opts = GetLastNTagsOpts {
        n: 0,
        reference: "HEAD",
        exclude_candidates: false,
        exclude_technical_bumps: false,
        skip_first: false,
        is_release: |_| Ok(true),
    };
    let out = get_last_n_tags(opts).unwrap();
    // The 'not-a-version' tag is dropped; 3 valid versions remain in descending order.
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].to_string(), "v202401.1.0");
    assert_eq!(out[1].to_string(), "v202401.0.2");
    assert_eq!(out[2].to_string(), "v202401.0.1");
}
```

- [ ] **Step 5: Run the test**

```bash
cargo test --lib io::git::tests::test_get_last_n_tags_returns_valid_versions_in_order
```

Expected: PASS.

- [ ] **Step 6: Run full suite**

```bash
cargo test 2>&1 | tail -3
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/io/git.rs
git commit -m "feat(io/git): implement get_last_n_tags via tags_merged + filter_tags"
```

---

## Task 12: Add `release_exists` to `io/github.rs`

**Files:**
- Modify: `src/io/github.rs`

Async helper that checks for a GitHub Release object (not just a pushed tag). 404 → `Ok(false)`; other errors → `Err`.

- [ ] **Step 1: Locate the right insertion point**

Open `src/io/github.rs`. Find `pub async fn resolve_latest_version`. We add `release_exists` immediately after.

- [ ] **Step 2: Add the function**

After `resolve_latest_version`, add:

```rust
/// Check if a GitHub *Release* exists in `bo4e/BO4E-Schemas` for the given version.
///
/// Note: a Release is more than a pushed tag. A tag without an associated Release
/// returns 404 from `releases().get_by_tag(...)` and is treated as `Ok(false)`.
pub async fn release_exists(version: &Version, token: Option<&str>) -> Result<bool, String> {
    let octocrab = get_octocrab_instance(token)?;
    match get_bo4e_schemas_repo_handler(&octocrab)
        .releases()
        .get_by_tag(&version.to_string())
        .await
    {
        Ok(_) => Ok(true),
        Err(octocrab::Error::GitHub { source, .. }) if source.status_code == http::StatusCode::NOT_FOUND => {
            Ok(false)
        }
        Err(octocrab::Error::GitHub { source, .. }) if source.status_code == http::StatusCode::FORBIDDEN => {
            Err(format!(
                "GitHub rate-limited the release-validation request ({}). \
                 Pass --token, set GITHUB_TOKEN, or use --no-validate-releases.",
                source.message
            ))
        }
        Err(e) => Err(e.to_string()),
    }
}
```

(`http::StatusCode` is already a transitive dep via octocrab and is in `Cargo.toml`.)

- [ ] **Step 3: Build**

```bash
cargo build 2>&1 | tail -5
```

Expected: clean. If `http` isn't directly imported, add `use http;` to the top of `src/io/github.rs` — but it's already a top-level dep so this should work via fully-qualified path.

If the build complains about `octocrab::Error::GitHub` variant fields, run `cargo doc --open -p octocrab` and adjust the pattern. The variants have changed across versions; the project pins 0.44.1.

- [ ] **Step 4: Add a compile-only smoke test**

Append to (or create) the `tests` module in `src/io/github.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time check that `release_exists` has the expected signature.
    /// We can't make a live network call in unit tests.
    #[test]
    fn test_release_exists_signature() {
        fn _assert_signature() -> impl std::future::Future<Output = Result<bool, String>> {
            let v: Version = "v202401.0.1".parse().unwrap();
            release_exists(&v, None)
        }
        let _ = _assert_signature; // silence unused
    }
}
```

- [ ] **Step 5: Run the smoke test**

```bash
cargo test --lib io::github::tests::test_release_exists_signature
```

Expected: PASS (compile only — never invokes the future).

- [ ] **Step 6: Commit**

```bash
git add src/io/github.rs
git commit -m "feat(io/github): add release_exists with 404 → Ok(false), 403 → guidance"
```

---

## Task 13: Build `cli/repo.rs` — clap surface, table renderer, run handler

**Files:**
- Create: `src/cli/repo.rs`
- Modify: `src/cli.rs`

The biggest task. Create the clap structure, the table renderer (hand-rolled, ~30 lines), and the `run_versions` handler that ties everything together.

- [ ] **Step 1: Create the file with clap structs only**

Create `src/cli/repo.rs`:

```rust
use crate::cli::base::Executable;
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct Repo {
    #[command(subcommand)]
    pub command: RepoSubcommand,
}

#[derive(Subcommand)]
pub enum RepoSubcommand {
    Versions(VersionsArgs),
}

/// Get the last n versions of the BO4E-python repository starting from the given reference.
///
/// This command must be executed from the root of a BO4E-python checkout.
#[derive(Args)]
pub struct VersionsArgs {
    /// Number of last versions to retrieve. 0 = all versions since v202401.0.0.
    #[arg(short = 'n', default_value_t = 0)]
    pub n: u32,

    /// Git reference to start from (tag, branch, commit, or "HEAD").
    /// Falls back to current HEAD if the value is none of those.
    #[arg(short = 'r', long = "ref", default_value = "main")]
    pub reference: String,

    /// Exclude release candidates from the output.
    #[arg(short = 'c', long, default_value_t = false)]
    pub exclude_candidates: bool,

    /// Exclude technical bumps; from each functional group, keep only the newest technical.
    #[arg(short = 't', long, default_value_t = false)]
    pub exclude_technical_bumps: bool,

    /// Show the full commit SHA. By default the SHA is truncated to 6 chars.
    #[arg(short = 's', long, default_value_t = false)]
    pub show_full_commit_sha: bool,

    /// Skip GitHub release validation (faster, fully offline).
    #[arg(long = "no-validate-releases", action = clap::ArgAction::SetFalse, default_value_t = true)]
    pub validate_releases: bool,

    /// GitHub token for the BO4E-Schemas release validation. Falls back to `gh auth token`.
    #[arg(long, env = "GITHUB_TOKEN")]
    pub token: Option<String>,
}

impl Executable for Repo {
    fn run(&self) -> Result<(), String> {
        match &self.command {
            RepoSubcommand::Versions(a) => run_versions(a),
        }
    }
}

fn run_versions(_args: &VersionsArgs) -> Result<(), String> {
    Err("not yet implemented".to_string())
}
```

Add `pub mod repo;` to `src/cli.rs` (next to the existing `pub mod pull;`, etc.).

- [ ] **Step 2: Confirm it compiles**

```bash
cargo build 2>&1 | tail -3
```

Expected: clean (the `Repo` variant isn't yet wired into `SubcommandsLevel1`, so it won't be reachable but it compiles).

- [ ] **Step 3: Add the table renderer**

Append to `src/cli/repo.rs`:

```rust
use console::{Style, style};

/// Hand-rolled 3-column table writer. Prints to stdout.
fn render_table(title: &str, rows: &[(String, String, String)]) {
    println!("{title}");

    if rows.is_empty() {
        println!("{}", style("(no versions found)").italic());
        return;
    }

    let headers = ("Version", "Commit SHA", "Commit date");
    let widths = (
        rows.iter().map(|r| r.0.len()).max().unwrap_or(0).max(headers.0.len()),
        rows.iter().map(|r| r.1.len()).max().unwrap_or(0).max(headers.1.len()),
        rows.iter().map(|r| r.2.len()).max().unwrap_or(0).max(headers.2.len()),
    );

    let bold = Style::new().bold();
    let dim = Style::new().dim();

    println!(
        "{}  {}  {}",
        bold.apply_to(format!("{:<w$}", headers.0, w = widths.0)),
        bold.apply_to(format!("{:<w$}", headers.1, w = widths.1)),
        bold.apply_to(format!("{:<w$}", headers.2, w = widths.2)),
    );

    for (i, (a, b, c)) in rows.iter().enumerate() {
        let s = if i % 2 == 1 { dim.clone() } else { Style::new() };
        println!(
            "{}  {}  {}",
            s.apply_to(format!("{:<w$}", a, w = widths.0)),
            s.apply_to(format!("{:<w$}", b, w = widths.1)),
            s.apply_to(format!("{:<w$}", c, w = widths.2)),
        );
    }
}
```

- [ ] **Step 4: Implement `run_versions`**

Replace the stubbed `run_versions` body:

```rust
use crate::cprint_normal;
use crate::io::git::{GetLastNTagsOpts, get_commit_date, get_commit_sha, get_last_n_tags, get_ref};
use crate::io::github::release_exists;
use crate::models::cli::get_token_as_string;
use crate::models::cli::Token;
use crate::models::git::RefKind;
use crate::utils::tokio::get_runtime;

fn run_versions(args: &VersionsArgs) -> Result<(), String> {
    let (ref_kind, resolved_ref) = get_ref(&args.reference).map_err(|e| e.to_string())?;

    let ref_display = match ref_kind {
        RefKind::Tag => resolved_ref.clone(),
        RefKind::Branch => format!("latest commit on branch {resolved_ref}"),
        RefKind::Commit => {
            let short: String = resolved_ref.chars().take(6).collect();
            format!("commit {short}")
        }
    };

    let title = if args.n == 0 {
        format!("All versions between v202401.0.0 and {ref_display}")
    } else {
        format!("Last {} versions before {ref_display}", args.n)
    };

    let skip_first = matches!(ref_kind, RefKind::Tag);

    // Build the is_release closure. If validation is off, return Ok(true) cheaply.
    // If on, build a tokio runtime and block_on the async release_exists.
    let token: Option<String> = match (&args.token, args.validate_releases) {
        (Some(t), true) => Some(get_token_as_string(&Token::from(t.clone()))),
        _ => None,
    };

    let versions = if args.validate_releases {
        let runtime = get_runtime();
        let token_ref = token.as_deref();
        get_last_n_tags(GetLastNTagsOpts {
            n: args.n,
            reference: &resolved_ref,
            exclude_candidates: args.exclude_candidates,
            exclude_technical_bumps: args.exclude_technical_bumps,
            skip_first,
            is_release: |v| runtime.block_on(release_exists(v, token_ref)),
        })?
    } else {
        get_last_n_tags(GetLastNTagsOpts {
            n: args.n,
            reference: &resolved_ref,
            exclude_candidates: args.exclude_candidates,
            exclude_technical_bumps: args.exclude_technical_bumps,
            skip_first,
            is_release: |_| Ok(true),
        })?
    };

    if args.n > 0 && (versions.len() as u32) < args.n {
        crate::cwarn!(
            "fewer than {} tags found from this reference; got {}",
            args.n,
            versions.len()
        );
    }

    // Quiet mode: plain stdout, no metadata fetches.
    if !crate::console::console::CONSOLE
        .get()
        .expect("CONSOLE")
        .would_emit(crate::console::console::Level::Normal)
    {
        for v in &versions {
            println!("{v}");
        }
        return Ok(());
    }

    // Non-quiet: fetch commit metadata and render the table.
    let mut rows: Vec<(String, String, String)> = Vec::with_capacity(versions.len());
    for v in &versions {
        let sha = get_commit_sha(&v.to_string()).map_err(|e| e.to_string())?;
        let displayed_sha = if args.show_full_commit_sha {
            sha.clone()
        } else {
            sha.chars().take(6).collect()
        };
        let date = get_commit_date(&sha).map_err(|e| e.to_string())?;
        rows.push((v.to_string(), displayed_sha, date));
    }

    cprint_normal!(""); // blank line before the table for spacing
    render_table(&title, &rows);
    Ok(())
}
```

Note on `Token`/`get_token_as_string`: these come from `models/cli.rs` (used by `pull.rs`). Reuse them; the same `--token` ergonomic falls back to `gh auth token` inside `get_token_as_string` when given an empty/sentinel value.

If `Token::from(String)` doesn't exist as an `impl From<String> for Token`, replace `Token::from(t.clone())` with whatever the existing `pull.rs` constructor pattern is. Read `src/cli/pull.rs:1-50` and the `Token` API in `src/models/cli.rs` to mirror the working pattern there.

- [ ] **Step 5: Build**

```bash
cargo build 2>&1 | tail -10
```

Fix any errors. The most likely ones:
- `console` crate import: should be `use console::{Style, style};` (the crate is already a dep).
- `Token::from(...)` mismatch: read pull.rs and mirror exactly.
- `RefKind` not derived `Clone`/`Copy`: derive `Clone` if needed.

- [ ] **Step 6: Commit**

```bash
git add src/cli/repo.rs src/cli.rs
git commit -m "feat(cli/repo): add Repo command + Versions handler with hand-rolled table"
```

---

## Task 14: Wire Repo into the top-level CLI + end-to-end smoke test

**Files:**
- Modify: `src/cli/base.rs`
- Modify: `src/cli/repo.rs` (append test module)

- [ ] **Step 1: Register the Repo variant in `cli/base.rs`**

Edit `src/cli/base.rs`. Add `use crate::cli::repo::Repo;` near the top imports. Add `Repo(Repo)` to the `SubcommandsLevel1` enum and dispatch:

```rust
#[derive(Subcommand)]
pub enum SubcommandsLevel1 {
    Pull(Pull),
    Edit(Edit),
    Diff(Diff),
    Repo(Repo),
}

impl Executable for SubcommandsLevel1 {
    fn run(&self) -> Result<(), String> {
        match self {
            SubcommandsLevel1::Pull(pull) => pull.run(),
            SubcommandsLevel1::Edit(edit) => edit.run(),
            SubcommandsLevel1::Diff(diff) => diff.run(),
            SubcommandsLevel1::Repo(repo) => repo.run(),
        }
    }
}
```

- [ ] **Step 2: Build and try the help screens**

```bash
cargo build 2>&1 | tail -3
cargo run --release --quiet -- repo --help
cargo run --release --quiet -- repo versions --help
```

Expected: both help screens render with all 6 flags.

- [ ] **Step 3: Add an end-to-end smoke test**

Append to `src/cli/repo.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn make_git_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        let run = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(p)
                .output()
                .expect("git invocation failed");
            assert!(out.status.success(), "git {args:?} failed");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@t.t"]);
        run(&["config", "user.name", "t"]);
        run(&["commit", "--allow-empty", "-m", "c1", "-q"]);
        run(&["tag", "v202401.0.1"]);
        run(&["commit", "--allow-empty", "-m", "c2", "-q"]);
        run(&["tag", "v202401.0.2"]);
        run(&["commit", "--allow-empty", "-m", "c3", "-q"]);
        run(&["tag", "v202401.1.0"]);
        dir
    }

    fn ensure_console(level: crate::console::console::Level) {
        let _ = crate::console::console::CONSOLE.set(crate::console::console::Console::new(level));
    }

    #[test]
    fn test_run_versions_quiet_returns_versions_only() {
        let dir = make_git_repo();
        let _g = std::env::set_current_dir(dir.path()).unwrap();
        ensure_console(crate::console::console::Level::Quiet);

        let args = VersionsArgs {
            n: 0,
            reference: "HEAD".into(),
            exclude_candidates: false,
            exclude_technical_bumps: false,
            show_full_commit_sha: false,
            validate_releases: false,
            token: None,
        };
        run_versions(&args).expect("run_versions failed");
        // We can't capture stdout from inside the test runner without extra plumbing.
        // The assertion is implicit: no panic, no error.
    }

    #[test]
    fn test_run_versions_non_quiet_renders_table() {
        let dir = make_git_repo();
        let _g = std::env::set_current_dir(dir.path()).unwrap();
        ensure_console(crate::console::console::Level::Normal);

        let args = VersionsArgs {
            n: 0,
            reference: "HEAD".into(),
            exclude_candidates: false,
            exclude_technical_bumps: false,
            show_full_commit_sha: false,
            validate_releases: false,
            token: None,
        };
        run_versions(&args).expect("run_versions failed");
    }
}
```

(Stdout-capture in unit tests requires either `gag` crate or a process-level integration test. Both are heavier than this plan warrants. The smoke tests verify the function returns `Ok(())` on a real-git input; manual verification via `cargo run -- repo versions -q` covers the output shape.)

- [ ] **Step 4: Run the smoke tests**

```bash
cargo test --lib cli::repo::tests
```

Expected: 2 passed.

- [ ] **Step 5: Manual verification on a real BO4E-python checkout (if available)**

If you have a clone of `bo4e/BO4E-python`:

```bash
cd /path/to/BO4E-python
/repos/bo4e-cli/target/release/bo4e-cli repo versions -n 5 --no-validate-releases
/repos/bo4e-cli/target/release/bo4e-cli repo versions -n 5 --no-validate-releases -q
```

Expected: first command prints title + 3-column table to stdout; second prints just 5 version strings.

- [ ] **Step 6: Run full test suite**

```bash
cargo test 2>&1 | tail -3
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/cli/base.rs src/cli/repo.rs
git commit -m "feat(cli): wire Repo subcommand + end-to-end smoke test"
```

---

## Done

Final verification:

```bash
cargo build --release 2>&1 | tail -3   # 0 warnings expected
cargo test 2>&1 | tail -3                # all tests pass
cargo run --release -- repo versions --help
```

The `bo4e repo versions` command is now functional, the Console correctly routes info → stdout and warn/error → stderr across the codebase, and the previously-scaffolded `io/git.rs` module is fully live (no more `#[allow(dead_code)]` markers on the public path).
