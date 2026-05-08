# Diff Command Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the `bo4e diff` subcommand group (`schemas`, `matrix`, `version-bump`) in Rust, replicating Python `main` behaviour bit-for-bit.

**Architecture:** Mirror the Python module layout 1:1: `src/diff/{diff,filters,matrix,version}.rs` plus `src/io/{changes,matrix}.rs` and `src/cli/diff.rs`. Diff functions take a `&mut Vec<Change>` collector instead of Python generators. A `Level`-based `Console` (Quiet=0, Normal=1, Verbose=2) replaces the old `verbose: bool`. A hand-rolled linear-chain validator replaces `networkx`.

**Tech Stack:** `clap` (CLI), `serde`/`serde_json` (JSON), `csv = "1"` (matrix CSV), `indexmap = "2"` (order-preserving map), `regex` (version-string substitution in description diffs).

**Design doc:** `docs/plans/2026-05-08-diff-command-design.md`

---

## Task 1: Add `csv` and `indexmap` dependencies

**Files:**
- Modify: `Cargo.toml`

### Step 1: Add deps

In `Cargo.toml`, append to `[dependencies]`:

```toml
csv = "1"
indexmap = { version = "2", features = ["serde"] }
```

### Step 2: Verify build

```
cargo check
```

Expected: clean build, no errors, just a note about new transitive deps.

### Step 3: Commit

```
git add Cargo.toml Cargo.lock
git commit -m "build: add csv and indexmap dependencies for diff command"
```

---

## Task 2: Make version helpers `pub`, add `DirtyVersion::version()` accessor

**Files:**
- Modify: `src/models/version.rs`

### Step 1: Write failing test

Append at the bottom of `src/models/version.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirty_version_accessor_returns_inner_version() {
        let dv: DirtyVersion = "v202401.0.1+gabc.d20260101".parse().unwrap();
        let v = dv.version();
        assert_eq!(v.to_string(), "v202401.0.1");
    }

    #[test]
    fn test_bumped_helpers_are_callable_publicly() {
        let a: Version = "v202401.0.1".parse().unwrap();
        let b: Version = "v202401.0.2".parse().unwrap();
        let c: Version = "v202401.1.0".parse().unwrap();
        let d: Version = "v202402.0.0".parse().unwrap();
        assert!(b.bumped_technical(&a));
        assert!(c.bumped_functional(&a));
        assert!(d.bumped_major(&a));
        assert!(!a.is_release_candidate());
    }

    #[test]
    fn test_dirty_version_is_dirty_public() {
        let clean: DirtyVersion = "v202401.0.1".parse().unwrap();
        let with_commit: DirtyVersion = "v202401.0.1+gabcdef".parse().unwrap();
        assert!(!clean.is_dirty());
        assert!(with_commit.is_dirty());
    }
}
```

### Step 2: Run to confirm failure

```
cargo test models::version::tests
```

Expected: compile error — `version`, `bumped_*`, `is_release_candidate`, `is_dirty` are private.

### Step 3: Make helpers `pub` and add accessor

Edit `src/models/version.rs`:

- Change the `impl Version { ... }` block: `fn is_release_candidate` → `pub fn is_release_candidate`; same for `bumped_major`, `bumped_functional`, `bumped_technical`, `bumped_candidate`.
- Change the `impl DirtyVersion { ... }` block: `fn is_dirty` → `pub fn is_dirty`. Add a new method:

```rust
impl DirtyVersion {
    /// Borrow the semantic version, discarding dirt metadata.
    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn is_dirty(&self) -> bool {
        self.commit_part.is_some() || self.dirty_worktree_date.is_some()
    }
}
```

### Step 4: Run tests

```
cargo test models::version::tests
```

Expected: 3 new tests pass.

### Step 5: Commit

```
git add src/models/version.rs
git commit -m "feat(models/version): expose bumped/is_release_candidate/is_dirty + add DirtyVersion::version()"
```

---

## Task 3: Cross-type comparison `Version` ↔ `DirtyVersion`

**Files:**
- Modify: `src/models/version.rs`

### Step 1: Write failing test

Append to the existing `mod tests` in `src/models/version.rs`:

```rust
    #[test]
    fn test_version_eq_clean_dirty_at_same_semver() {
        let v: Version = "v202401.0.1".parse().unwrap();
        let dv_clean: DirtyVersion = "v202401.0.1".parse().unwrap();
        let dv_dirty: DirtyVersion = "v202401.0.1+gabc".parse().unwrap();
        assert!(v == dv_clean);
        assert!(!(v == dv_dirty)); // dirty is strictly newer at same semver
    }

    #[test]
    fn test_version_lt_dirty_at_same_semver() {
        let v: Version = "v202401.0.1".parse().unwrap();
        let dv_dirty: DirtyVersion = "v202401.0.1+gabc".parse().unwrap();
        assert!(v < dv_dirty);
        assert!(dv_dirty > v);
    }

    #[test]
    fn test_version_cmp_dirty_when_semver_differs() {
        let v_old: Version = "v202401.0.1".parse().unwrap();
        let dv_new: DirtyVersion = "v202401.0.2".parse().unwrap();
        assert!(v_old < dv_new);
        let v_new: Version = "v202401.0.2".parse().unwrap();
        let dv_old: DirtyVersion = "v202401.0.1+gabc".parse().unwrap();
        assert!(v_new > dv_old);
    }

    #[test]
    fn test_dirty_eq_clean_symmetric() {
        let v: Version = "v202401.0.1".parse().unwrap();
        let dv: DirtyVersion = "v202401.0.1".parse().unwrap();
        assert!(dv == v);
    }
```

### Step 2: Run to confirm failure

```
cargo test models::version::tests::test_version_eq_clean_dirty_at_same_semver
```

Expected: compile error — no `PartialEq<DirtyVersion>` for `Version`.

### Step 3: Implement cross-type comparisons

Append after the existing `impl DirtyVersion { ... }` in `src/models/version.rs`:

```rust
impl PartialEq<DirtyVersion> for Version {
    /// `Version == DirtyVersion` iff same semver and the dirty side is clean.
    fn eq(&self, other: &DirtyVersion) -> bool {
        *self == other.version && !other.is_dirty()
    }
}

impl PartialOrd<DirtyVersion> for Version {
    /// At equal semver, a dirty `DirtyVersion` is strictly newer than a clean `Version`.
    fn partial_cmp(&self, other: &DirtyVersion) -> Option<Ordering> {
        match self.cmp(&other.version) {
            Ordering::Equal if other.is_dirty() => Some(Ordering::Less),
            ord => Some(ord),
        }
    }
}

impl PartialEq<Version> for DirtyVersion {
    fn eq(&self, o: &Version) -> bool {
        o == self
    }
}

impl PartialOrd<Version> for DirtyVersion {
    fn partial_cmp(&self, o: &Version) -> Option<Ordering> {
        o.partial_cmp(self).map(Ordering::reverse)
    }
}
```

### Step 4: Run tests

```
cargo test models::version::tests
```

Expected: all 4 cross-type tests pass alongside the previous tests.

### Step 5: Commit

```
git add src/models/version.rs
git commit -m "feat(models/version): cross-type PartialEq/PartialOrd between Version and DirtyVersion"
```

---

## Task 4: Redesign `models/matrix.rs` — split symbol/text enums and switch to `IndexMap`

**Files:**
- Modify: `src/models/matrix.rs`

### Step 1: Write failing test

Replace the bottom of `src/models/matrix.rs` (after the existing structs) with a new `#[cfg(test)] mod tests` block:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::version::DirtyVersion;

    #[test]
    fn test_compatibility_symbol_roundtrip_emoji() {
        for sym in [
            CompatibilitySymbol::ChangeNone,
            CompatibilitySymbol::ChangeNonCritical,
            CompatibilitySymbol::ChangeCritical,
            CompatibilitySymbol::NonExistent,
            CompatibilitySymbol::Added,
            CompatibilitySymbol::Removed,
        ] {
            let s = serde_json::to_string(&sym).unwrap();
            let back: CompatibilitySymbol = serde_json::from_str(&s).unwrap();
            assert_eq!(sym, back);
        }
    }

    #[test]
    fn test_compatibility_text_roundtrip() {
        for t in [
            CompatibilityText::ChangeNone,
            CompatibilityText::ChangeNonCritical,
            CompatibilityText::ChangeCritical,
            CompatibilityText::NonExistent,
            CompatibilityText::Added,
            CompatibilityText::Removed,
        ] {
            let s = serde_json::to_string(&t).unwrap();
            let back: CompatibilityText = serde_json::from_str(&s).unwrap();
            assert_eq!(t, back);
        }
    }

    #[test]
    fn test_compatibility_serializes_emoji_then_text() {
        let c_emoji = Compatibility::Symbol(CompatibilitySymbol::Added);
        assert_eq!(serde_json::to_string(&c_emoji).unwrap(), "\"\u{2795}\"");

        let c_text = Compatibility::Text(CompatibilityText::Added);
        assert_eq!(serde_json::to_string(&c_text).unwrap(), "\"added\"");
    }

    #[test]
    fn test_compatibility_deserialize_tries_emoji_first() {
        let parsed: Compatibility = serde_json::from_str("\"\u{2795}\"").unwrap();
        assert!(matches!(parsed, Compatibility::Symbol(CompatibilitySymbol::Added)));

        let parsed_text: Compatibility = serde_json::from_str("\"added\"").unwrap();
        assert!(matches!(parsed_text, Compatibility::Text(CompatibilityText::Added)));
    }

    #[test]
    fn test_compatibility_matrix_preserves_module_order() {
        let v: DirtyVersion = "v202401.0.1".parse().unwrap();
        let entry = CompatibilityMatrixEntry {
            previous_version: v.clone(),
            next_version: v.clone(),
            compatibility: Compatibility::Symbol(CompatibilitySymbol::ChangeNone),
        };
        let mut m = CompatibilityMatrix { root: IndexMap::new() };
        m.root.insert("bo.Angebot".to_string(), vec![entry.clone()]);
        m.root.insert("com.Adresse".to_string(), vec![entry]);

        let json = serde_json::to_string(&m).unwrap();
        let pos_bo = json.find("bo.Angebot").unwrap();
        let pos_com = json.find("com.Adresse").unwrap();
        assert!(pos_bo < pos_com, "module insertion order must survive serialization");
    }
}
```

### Step 2: Run to confirm failure

```
cargo test models::matrix::tests
```

Expected: compile error — `CompatibilityText`, `Compatibility`, `IndexMap` not in scope; `CompatibilitySymbol` variants renamed.

### Step 3: Replace `models/matrix.rs`

Replace the *entire* file `src/models/matrix.rs` with:

```rust
use crate::models::version::DirtyVersion;
use bimap::BiMap;
use indexmap::IndexMap;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display};

lazy_static! {
    pub static ref COMPATIBILITY_SYMBOLS: BiMap<CompatibilitySymbol, String> = BiMap::from_iter([
        (CompatibilitySymbol::ChangeNone,        "\u{1F7E2}".to_string()), // 🟢
        (CompatibilitySymbol::ChangeNonCritical, "\u{1F7E1}".to_string()), // 🟡
        (CompatibilitySymbol::ChangeCritical,    "\u{1F534}".to_string()), // 🔴
        (CompatibilitySymbol::NonExistent,       "-".to_string()),
        (CompatibilitySymbol::Added,             "\u{2795}".to_string()),  // ➕
        (CompatibilitySymbol::Removed,           "\u{2796}".to_string()),  // ➖
    ]);

    pub static ref COMPATIBILITY_TEXTS: BiMap<CompatibilityText, String> = BiMap::from_iter([
        (CompatibilityText::ChangeNone,        "none".to_string()),
        (CompatibilityText::ChangeNonCritical, "non-critical".to_string()),
        (CompatibilityText::ChangeCritical,    "critical".to_string()),
        (CompatibilityText::NonExistent,       "non-existent".to_string()),
        (CompatibilityText::Added,             "added".to_string()),
        (CompatibilityText::Removed,           "removed".to_string()),
    ]);
}

/// Emoji rendering of a compatibility cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompatibilitySymbol {
    ChangeNone,
    ChangeNonCritical,
    ChangeCritical,
    NonExistent,
    Added,
    Removed,
}

impl Display for CompatibilitySymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            COMPATIBILITY_SYMBOLS.get_by_left(self).ok_or(std::fmt::Error)?
        )
    }
}

impl Serialize for CompatibilitySymbol {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(COMPATIBILITY_SYMBOLS.get_by_left(self).ok_or_else(|| {
            serde::ser::Error::custom(format!("Unknown compatibility symbol: {:?}", self))
        })?)
    }
}

impl<'de> Deserialize<'de> for CompatibilitySymbol {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        COMPATIBILITY_SYMBOLS
            .get_by_right(&s)
            .copied()
            .ok_or_else(|| serde::de::Error::custom(format!("Unknown compatibility symbol: {}", s)))
    }
}

/// Plain-text rendering of a compatibility cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompatibilityText {
    ChangeNone,
    ChangeNonCritical,
    ChangeCritical,
    NonExistent,
    Added,
    Removed,
}

impl Display for CompatibilityText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            COMPATIBILITY_TEXTS.get_by_left(self).ok_or(std::fmt::Error)?
        )
    }
}

impl Serialize for CompatibilityText {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(COMPATIBILITY_TEXTS.get_by_left(self).ok_or_else(|| {
            serde::ser::Error::custom(format!("Unknown compatibility text: {:?}", self))
        })?)
    }
}

impl<'de> Deserialize<'de> for CompatibilityText {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        COMPATIBILITY_TEXTS
            .get_by_right(&s)
            .copied()
            .ok_or_else(|| serde::de::Error::custom(format!("Unknown compatibility text: {}", s)))
    }
}

/// Either an emoji or a textual rendering. (De)serializes untagged: emoji first, text second.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Compatibility {
    Symbol(CompatibilitySymbol),
    Text(CompatibilityText),
}

impl Display for Compatibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Compatibility::Symbol(s) => s.fmt(f),
            Compatibility::Text(t)   => t.fmt(f),
        }
    }
}

/// A single entry of the compatibility matrix: one (prev → next) cell for one module.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityMatrixEntry {
    pub previous_version: DirtyVersion,
    pub next_version: DirtyVersion,
    pub compatibility: Compatibility,
}

/// Module name → row of (prev, next, compatibility) entries. `IndexMap` preserves insertion order.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityMatrix {
    #[serde(flatten, default)]
    pub root: IndexMap<String, Vec<CompatibilityMatrixEntry>>,
}

// (existing #[cfg(test)] block from Step 1 stays at the bottom)
```

### Step 4: Run tests

```
cargo test models::matrix::tests
```

Expected: all 5 tests pass.

### Step 5: Commit

```
git add src/models/matrix.rs Cargo.toml
git commit -m "refactor(models/matrix): split CompatibilitySymbol/Text + Compatibility wrapper, IndexMap for ordering"
```

---

## Task 5: Redesign `console/console.rs` — `Level`-based emission

**Files:**
- Modify: `src/console/console.rs`

### Step 1: Write failing test

Replace the existing `#[cfg(test)] mod tests` in `src/console/console.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_ordering() {
        assert!(Level::Quiet < Level::Normal);
        assert!(Level::Normal < Level::Verbose);
    }

    /// Full 3×3 emission table from the design doc.
    #[test]
    fn test_emission_table() {
        // (console_level, message_level, expected_emission)
        let cases: &[(Level, Level, bool)] = &[
            (Level::Quiet,   Level::Quiet,   true),
            (Level::Quiet,   Level::Normal,  false),
            (Level::Quiet,   Level::Verbose, false),
            (Level::Normal,  Level::Quiet,   true),
            (Level::Normal,  Level::Normal,  true),
            (Level::Normal,  Level::Verbose, false),
            (Level::Verbose, Level::Quiet,   true),
            (Level::Verbose, Level::Normal,  true),
            (Level::Verbose, Level::Verbose, true),
        ];
        for (cl, ml, expected) in cases {
            let c = Console::new(*cl);
            assert_eq!(
                c.would_emit(*ml),
                *expected,
                "console={:?} message={:?}",
                cl, ml
            );
        }
    }
}
```

### Step 2: Run to confirm failure

```
cargo test console::console::tests
```

Expected: compile error — `Level` not defined; `Console::new` signature mismatch; `would_emit` not present.

### Step 3: Replace `console/console.rs`

Replace the entire file `src/console/console.rs` with:

```rust
use crate::console::highlighter::Highlighter;
use std::sync::{OnceLock, RwLock};

pub static CONSOLE: OnceLock<Console> = OnceLock::new();

/// Importance of a console message. Lower discriminants are more important —
/// a message is emitted iff its level is `<=` the console's level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Level {
    /// Must always be surfaced, including under `--quiet`.
    Quiet = 0,
    /// Default informational output.
    Normal = 1,
    /// Detail emitted only under `--verbose`.
    Verbose = 2,
}

pub struct Console {
    level: Level,
    highlighter: RwLock<Highlighter>,
}

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

    /// Emit `msg` iff `message_level <= self.level`, after applying the highlighter.
    pub fn print(&self, message_level: Level, msg: &str) {
        if !self.would_emit(message_level) {
            return;
        }
        let highlighted = self.highlighter.read().unwrap().apply(msg);
        eprintln!("{}", highlighted); // NOTE: stderr keeps stdout clean for piping
    }

    /// Register schema names for dynamic highlighting (call once after read_schemas).
    pub fn add_schema_names(&self, names: &[String]) {
        self.highlighter.write().unwrap().add_schema_names(names);
    }
}

// (the #[cfg(test)] block from Step 1 lives below)
```

### Step 4: Run tests

```
cargo test console::console::tests
```

Expected: 2 tests pass.

### Step 5: Commit

```
git add src/console/console.rs
git commit -m "refactor(console): replace verbose flag with Level-based emission"
```

---

## Task 6: Redesign `cprint!` macros — parametrized + 3 wrappers

**Files:**
- Modify: `src/console.rs`

### Step 1: Write failing test

These macros are exercised at runtime by every command. Add a smoke test inside `src/console.rs` so the macros compile and dispatch correctly. Append at the bottom of the file:

```rust
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
    }
}
```

### Step 2: Run to confirm failure

```
cargo test console::tests::test_cprint_macros_compile_and_run
```

Expected: compile error — `cprint!` does not take a leading `Level` argument; `cprint_quiet!` / `cprint_normal!` not defined; existing `cprint!` calls inside `src/cli/edit.rs` and `src/edit/*` will also fail to compile (handled in Task 7).

### Step 3: Replace `src/console.rs`

Replace the *entire* file `src/console.rs` with:

```rust
pub mod console;
pub mod highlighter;
pub mod palette;
pub mod progress_bar;

/// Print a formatted message at an explicit `Level`. Emitted only if
/// `level <= CONSOLE.level`.
#[macro_export]
macro_rules! cprint {
    ($level:expr, $($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print($level, &format!($($arg)*))
    };
}

/// Print a `Level::Quiet` message. Emitted under every console level (including `--quiet`).
#[macro_export]
macro_rules! cprint_quiet {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Quiet, $($arg)*)
    };
}

/// Print a `Level::Normal` message. Default informational output.
#[macro_export]
macro_rules! cprint_normal {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Normal, $($arg)*)
    };
}

/// Print a `Level::Verbose` message. Emitted only under `--verbose`.
#[macro_export]
macro_rules! cprint_verbose {
    ($($arg:tt)*) => {
        $crate::cprint!($crate::console::console::Level::Verbose, $($arg)*)
    };
}
```

### Step 4: Run tests (will still fail — see Task 7)

```
cargo test console::tests::test_cprint_macros_compile_and_run --no-run
```

Expected: still fails because callers in `cli/edit.rs` and `edit/{add,non_nullable}.rs` use the old single-arg `cprint!`. Task 7 fixes that.

### Step 5: Commit

```
git add src/console.rs
git commit -m "refactor(console): cprint!(level, …) + cprint_{quiet,normal,verbose}! wrappers"
```

---

## Task 7: Migrate existing call sites `cprint!(...)` → `cprint_normal!(...)`

**Files:**
- Modify: `src/cli/edit.rs`
- Modify: `src/edit/add.rs`
- Modify: `src/edit/non_nullable.rs`

### Step 1: Write failing build check

```
cargo check
```

Expected: compile errors at every `cprint!("...")` call site (no leading `Level`). The list of such sites (from `grep -rn 'cprint!\(' src/` before this plan):

- `src/cli/edit.rs:66, 69, 72, 75, 85, 104`
- `src/edit/non_nullable.rs:106, 151, 153`
- `src/edit/add.rs:19, 43, 45, 83, 99, 101`

### Step 2: Replace each `cprint!` with `cprint_normal!`

In each of the three files, replace **every** `cprint!(` with `cprint_normal!(`. The arguments stay identical (the old macro had no level argument; the new `cprint_normal!` takes the same `(format, args...)` shape). Existing `cprint_verbose!(...)` invocations are untouched — their semantics survive (gated by the new `Level::Verbose` rule, identical observable effect).

Concretely, `cprint!("Added all additional models")` becomes `cprint_normal!("Added all additional models")`, and so on for all 15 occurrences listed above.

### Step 3: Verify build

```
cargo check
cargo test --no-run
```

Expected: clean build, no `cprint!(...)` (single-arg) call sites remain.

### Step 4: Run full test suite

```
cargo test
```

Expected: all existing tests still pass, plus the macro smoke test from Task 6.

### Step 5: Commit

```
git add src/cli/edit.rs src/edit/add.rs src/edit/non_nullable.rs
git commit -m "refactor(edit): migrate cprint! call sites to cprint_normal!"
```

---

## Task 8: Wire `--quiet` into `Cli` and resolve to `Level` in `main()`

**Files:**
- Modify: `src/cli/base.rs`
- Modify: `src/main.rs`

### Step 1: Write failing test

Append to `src/cli/base.rs` a `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_quiet_and_verbose_are_mutually_exclusive() {
        let result = Cli::try_parse_from(["bo4e", "--quiet", "--verbose", "edit",
            "-i", "in", "-o", "out"]);
        assert!(result.is_err(), "--quiet and --verbose must conflict");
    }

    #[test]
    fn test_quiet_flag_parses() {
        let cli = Cli::try_parse_from(["bo4e", "--quiet", "edit",
            "-i", "in", "-o", "out"]).unwrap();
        assert!(cli.quiet);
        assert!(!cli.verbose);
    }
}
```

### Step 2: Run to confirm failure

```
cargo test cli::base::tests
```

Expected: compile error — `Cli` has no field `quiet`.

### Step 3: Implement

Edit `src/cli/base.rs`. Replace the `Cli` struct with:

```rust
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
//#[command(propagate_version = true)]
pub struct Cli {
    /// Enable verbose output for all commands.
    #[arg(global = true, short = 'v', long, conflicts_with = "quiet")]
    pub verbose: bool,

    /// Suppress all non-essential output.
    #[arg(global = true, short = 'q', long)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Option<SubcommandsLevel1>,
}
```

Then update `src/main.rs`:

```rust
use crate::cli::base::Executable;
use crate::console::console::{Console, Level, CONSOLE};

mod cli;
mod console;
mod edit;
mod io;
mod models;
mod utils;

use clap::Parser;

fn main() -> Result<(), String> {
    let cli = cli::base::Cli::parse();
    let level = match (cli.verbose, cli.quiet) {
        (true, _) => Level::Verbose,
        (_, true) => Level::Quiet,
        _         => Level::Normal,
    };
    CONSOLE
        .set(Console::new(level))
        .map_err(|_| "CONSOLE already initialized".to_string())?;
    cli.run()
}
```

### Step 4: Run tests

```
cargo test cli::base::tests
cargo test
```

Expected: 2 new tests pass; all existing tests still pass.

### Step 5: Commit

```
git add src/cli/base.rs src/main.rs
git commit -m "feat(cli): add --quiet flag, resolve verbose/quiet into Level"
```

---

## Task 9: `Schemas::module_difference` and `Schemas::module_intersection`

**Files:**
- Modify: `src/models/schema_meta.rs`

### Step 1: Write failing test

Add (or extend) a `#[cfg(test)] mod tests` block at the bottom of `src/models/schema_meta.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::version::DirtyVersion;

    fn schema(module: &[&str]) -> Rc<RefCell<Schema>> {
        let m: Vec<String> = module.iter().map(|s| s.to_string()).collect();
        Rc::new(RefCell::new(Schema::new(m, None).unwrap()))
    }

    fn collection(modules: &[&[&str]]) -> Schemas {
        let v: DirtyVersion = "v202401.0.1".parse().unwrap();
        let mut s = Schemas::new(v);
        for m in modules {
            s.add_schema(schema(m)).unwrap();
        }
        s
    }

    #[test]
    fn test_module_difference_returns_only_unique_to_self() {
        let a = collection(&[&["bo", "Angebot"], &["com", "Adresse"]]);
        let b = collection(&[&["com", "Adresse"], &["enum", "Typ"]]);
        let only_a: Vec<Vec<String>> = a
            .module_difference(&b)
            .map(|s| s.borrow().module().to_vec())
            .collect();
        assert_eq!(only_a, vec![vec!["bo".to_string(), "Angebot".to_string()]]);
    }

    #[test]
    fn test_module_intersection_returns_self_values_in_both() {
        let a = collection(&[&["bo", "Angebot"], &["com", "Adresse"]]);
        let b = collection(&[&["com", "Adresse"], &["enum", "Typ"]]);
        let common: Vec<Vec<String>> = a
            .module_intersection(&b)
            .map(|s| s.borrow().module().to_vec())
            .collect();
        assert_eq!(common, vec![vec!["com".to_string(), "Adresse".to_string()]]);
    }
}
```

### Step 2: Run to confirm failure

```
cargo test models::schema_meta::tests
```

Expected: compile error — methods not defined.

### Step 3: Implement

Inside `impl Schemas { ... }` in `src/models/schema_meta.rs`, add:

```rust
    /// Schemas in `self` whose module is not present in `other`.
    pub fn module_difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a Rc<RefCell<Schema>>> {
        let other_modules = other.modules();
        self.schemas
            .iter()
            .filter(move |s| !other_modules.contains(s.borrow().module()))
    }

    /// Schemas whose module is present in both `self` and `other` (returns self's value).
    pub fn module_intersection<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a Rc<RefCell<Schema>>> {
        let other_modules = other.modules();
        self.schemas
            .iter()
            .filter(move |s| other_modules.contains(s.borrow().module()))
    }
```

Note: `modules()` returns `HashSet<&Vec<String>>`, and `s.borrow().module()` returns `&[String]`. `HashSet::contains` requires `Borrow` compatibility; if the compiler complains, change the filter to `other_modules.iter().any(|m| m.as_slice() == s.borrow().module())` — same semantics, no `Borrow` constraint needed.

### Step 4: Run tests

```
cargo test models::schema_meta::tests
```

Expected: 2 tests pass.

### Step 5: Commit

```
git add src/models/schema_meta.rs
git commit -m "feat(models/schema_meta): add module_difference and module_intersection iterators"
```

---

## Task 10: `src/io/changes.rs` — read/write diff JSON files

**Files:**
- Create: `src/io/changes.rs`
- Modify: `src/io.rs`

### Step 1: Write failing test

Create `src/io/changes.rs` with this content (test-first; production functions empty stubs that don't compile yet):

```rust
use crate::models::changes::Changes;
use std::path::{Path, PathBuf};

pub fn read_changes_from_diff_files(paths: &[PathBuf]) -> Result<Vec<Changes>, String> {
    todo!()
}

pub fn write_changes(changes: &Changes, file_path: &Path) -> Result<(), String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::changes::Changes;
    use crate::models::schema_meta::Schemas;
    use crate::models::version::DirtyVersion;

    fn empty_changes(old: &str, new: &str) -> Changes {
        let v_old: DirtyVersion = old.parse().unwrap();
        let v_new: DirtyVersion = new.parse().unwrap();
        Changes {
            old_schemas: Schemas::new(v_old),
            new_schemas: Schemas::new(v_new),
            changes: vec![],
        }
    }

    #[test]
    fn test_roundtrip_write_then_read_preserves_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("diff.json");
        let original = empty_changes("v202401.0.1", "v202401.0.2");

        write_changes(&original, &path).unwrap();
        let read_back = read_changes_from_diff_files(&[path]).unwrap();
        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].old_version().to_string(), "v202401.0.1");
        assert_eq!(read_back[0].new_version().to_string(), "v202401.0.2");
    }

    #[test]
    fn test_read_missing_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.json");
        let err = read_changes_from_diff_files(&[path]).unwrap_err();
        assert!(err.contains("nope.json") || err.to_lowercase().contains("not"));
    }

    #[test]
    fn test_write_creates_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/sub/diff.json");
        let c = empty_changes("v202401.0.1", "v202401.0.2");
        write_changes(&c, &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_pretty_indent_is_two_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("d.json");
        let c = empty_changes("v202401.0.1", "v202401.0.2");
        write_changes(&c, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        // serde_json::to_string_pretty uses two-space indent: lines must contain "  \"".
        assert!(text.contains("  \""), "expected two-space indent in pretty output");
    }
}
```

In `src/io.rs`, append:

```rust
pub mod changes;
```

### Step 2: Run to confirm failure

```
cargo test io::changes::tests
```

Expected: tests panic at `todo!()` — the stubs run.

### Step 3: Implement the production functions

Replace the two `todo!()` bodies in `src/io/changes.rs`:

```rust
pub fn read_changes_from_diff_files(paths: &[PathBuf]) -> Result<Vec<Changes>, String> {
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        if !p.exists() {
            return Err(format!("Diff file does not exist: {}", p.display()));
        }
        let text = std::fs::read_to_string(p)
            .map_err(|e| format!("Failed to read {}: {}", p.display(), e))?;
        let c: Changes = serde_json::from_str(&text)
            .map_err(|e| format!("Failed to parse {} as Changes: {}", p.display(), e))?;
        out.push(c);
    }
    Ok(out)
}

pub fn write_changes(changes: &Changes, file_path: &Path) -> Result<(), String> {
    if let Some(parent) = file_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
        }
    }
    let text = serde_json::to_string_pretty(changes)
        .map_err(|e| format!("Failed to serialize Changes: {}", e))?;
    std::fs::write(file_path, text)
        .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))
}
```

### Step 4: Run tests

```
cargo test io::changes::tests
```

Expected: 4 tests pass.

### Step 5: Commit

```
git add src/io/changes.rs src/io.rs
git commit -m "feat(io/changes): read/write diff JSON files"
```

---

## Task 11: `src/io/matrix.rs` — CSV and JSON writers for compatibility matrix

**Files:**
- Create: `src/io/matrix.rs`
- Modify: `src/io.rs`

### Step 1: Write failing test

Create `src/io/matrix.rs`:

```rust
use crate::models::matrix::CompatibilityMatrix;
use std::path::Path;

pub fn write_compatibility_matrix_csv(
    output: &Path,
    matrix: &CompatibilityMatrix,
    versions: &[String],
) -> Result<(), String> {
    todo!()
}

pub fn write_compatibility_matrix_json(
    output: &Path,
    matrix: &CompatibilityMatrix,
) -> Result<(), String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::matrix::{
        Compatibility, CompatibilityMatrix, CompatibilityMatrixEntry, CompatibilitySymbol,
        CompatibilityText,
    };
    use crate::models::version::DirtyVersion;
    use indexmap::IndexMap;

    fn dv(s: &str) -> DirtyVersion {
        s.parse().unwrap()
    }

    fn entry(prev: &str, next: &str, c: Compatibility) -> CompatibilityMatrixEntry {
        CompatibilityMatrixEntry {
            previous_version: dv(prev),
            next_version: dv(next),
            compatibility: c,
        }
    }

    #[test]
    fn test_csv_header_uses_arrow_between_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.csv");

        let mut root = IndexMap::new();
        root.insert(
            "bo.Angebot".to_string(),
            vec![
                entry("v202401.0.1", "v202401.0.2",
                    Compatibility::Text(CompatibilityText::ChangeNone)),
                entry("v202401.0.2", "v202401.1.0",
                    Compatibility::Text(CompatibilityText::ChangeNonCritical)),
            ],
        );
        let m = CompatibilityMatrix { root };
        let versions = vec![
            "v202401.0.1".to_string(),
            "v202401.0.2".to_string(),
            "v202401.1.0".to_string(),
        ];

        write_compatibility_matrix_csv(&path, &m, &versions).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let mut lines = text.lines();
        let header = lines.next().unwrap();
        assert_eq!(
            header,
            ",v202401.0.1 \u{21A6} v202401.0.2,\u{21A6} v202401.1.0"
        );
        let row = lines.next().unwrap();
        assert_eq!(row, "bo.Angebot,none,non-critical");
    }

    #[test]
    fn test_csv_emoji_row() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.csv");
        let mut root = IndexMap::new();
        root.insert(
            "bo.Angebot".to_string(),
            vec![entry("v202401.0.1", "v202401.0.2",
                Compatibility::Symbol(CompatibilitySymbol::ChangeCritical))],
        );
        let m = CompatibilityMatrix { root };
        let versions = vec!["v202401.0.1".to_string(), "v202401.0.2".to_string()];

        write_compatibility_matrix_csv(&path, &m, &versions).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\u{1F534}"), "expected red-circle emoji in csv");
    }

    #[test]
    fn test_json_roundtrip_preserves_module_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.json");
        let mut root = IndexMap::new();
        root.insert(
            "bo.Angebot".to_string(),
            vec![entry("v202401.0.1", "v202401.0.2",
                Compatibility::Text(CompatibilityText::ChangeNone))],
        );
        root.insert(
            "com.Adresse".to_string(),
            vec![entry("v202401.0.1", "v202401.0.2",
                Compatibility::Text(CompatibilityText::ChangeNone))],
        );
        let m = CompatibilityMatrix { root };

        write_compatibility_matrix_json(&path, &m).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let pos_bo = text.find("bo.Angebot").unwrap();
        let pos_com = text.find("com.Adresse").unwrap();
        assert!(pos_bo < pos_com);
    }
}
```

In `src/io.rs`, append:

```rust
pub mod matrix;
```

### Step 2: Run to confirm failure

```
cargo test io::matrix::tests
```

Expected: tests panic at `todo!()`.

### Step 3: Implement

Replace the two `todo!()` bodies in `src/io/matrix.rs`:

```rust
pub fn write_compatibility_matrix_csv(
    output: &Path,
    matrix: &CompatibilityMatrix,
    versions: &[String],
) -> Result<(), String> {
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
        }
    }
    if versions.len() < 2 {
        return Err("Need at least two versions to write a CSV matrix.".to_string());
    }
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b',')
        .terminator(csv::Terminator::Any(b'\n'))
        .escape(b'/')
        .from_path(output)
        .map_err(|e| format!("Failed to open {} for writing: {}", output.display(), e))?;

    // Header: ("", "v0 ↦ v1", "↦ v2", "↦ v3", …)
    let mut header: Vec<String> = Vec::with_capacity(versions.len());
    header.push(String::new());
    header.push(format!("{} \u{21A6} {}", versions[0], versions[1]));
    for v in &versions[2..] {
        header.push(format!("\u{21A6} {}", v));
    }
    wtr.write_record(&header)
        .map_err(|e| format!("CSV header write failed: {}", e))?;

    for (module_name, entries) in &matrix.root {
        let mut row: Vec<String> = Vec::with_capacity(entries.len() + 1);
        row.push(module_name.clone());
        for e in entries {
            row.push(e.compatibility.to_string());
        }
        wtr.write_record(&row)
            .map_err(|e| format!("CSV row write failed: {}", e))?;
    }

    wtr.flush()
        .map_err(|e| format!("CSV flush failed: {}", e))
}

pub fn write_compatibility_matrix_json(
    output: &Path,
    matrix: &CompatibilityMatrix,
) -> Result<(), String> {
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
        }
    }
    let text = serde_json::to_string_pretty(matrix)
        .map_err(|e| format!("Failed to serialize matrix: {}", e))?;
    std::fs::write(output, text)
        .map_err(|e| format!("Failed to write {}: {}", output.display(), e))
}
```

### Step 4: Run tests

```
cargo test io::matrix::tests
```

Expected: 3 tests pass.

### Step 5: Commit

```
git add src/io/matrix.rs src/io.rs
git commit -m "feat(io/matrix): CSV and JSON writers for compatibility matrix"
```

---

## Task 12: `src/diff.rs` module root + `src/diff/filters.rs`

**Files:**
- Create: `src/diff.rs`
- Create: `src/diff/filters.rs`
- Modify: `src/main.rs`

### Step 1: Write failing test

Create `src/diff/filters.rs` with the test up front and stub bodies:

```rust
use crate::models::changes::{Change, ChangeType};

pub fn is_change_critical(change: &Change) -> bool {
    todo!()
}

pub fn has_critical<'a, I: IntoIterator<Item = &'a Change>>(changes: I) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::changes::{Change, ChangeType};

    fn ch(t: ChangeType) -> Change {
        Change {
            r#type: t,
            old: None,
            new: None,
            old_trace: String::new(),
            new_trace: String::new(),
        }
    }

    #[test]
    fn test_is_change_critical_full_table() {
        let cases: &[(ChangeType, bool)] = &[
            (ChangeType::FieldAdded, false),
            (ChangeType::FieldRemoved, true),
            (ChangeType::FieldDefaultChanged, false),
            (ChangeType::FieldDescriptionChanged, false),
            (ChangeType::FieldTitleChanged, false),
            (ChangeType::FieldCardinalityChanged, true),
            (ChangeType::FieldReferenceChanged, true),
            (ChangeType::FieldStringFormatChanged, true),
            (ChangeType::FieldAnyOfTypeAdded, true),
            (ChangeType::FieldAnyOfTypeRemoved, true),
            (ChangeType::FieldAllOfTypeAdded, true),
            (ChangeType::FieldAllOfTypeRemoved, true),
            (ChangeType::FieldTypeChanged, true),
            (ChangeType::ClassAdded, false),
            (ChangeType::ClassRemoved, true),
            (ChangeType::ClassDescriptionChanged, false),
            (ChangeType::EnumValueAdded, false),
            (ChangeType::EnumValueRemoved, true),
        ];
        for (t, expected) in cases {
            assert_eq!(is_change_critical(&ch(t.clone())), *expected, "{:?}", t);
        }
    }

    #[test]
    fn test_has_critical_finds_one() {
        let v = vec![
            ch(ChangeType::FieldAdded),
            ch(ChangeType::FieldRemoved),
            ch(ChangeType::FieldDescriptionChanged),
        ];
        assert!(has_critical(&v));
    }

    #[test]
    fn test_has_critical_returns_false_for_only_non_critical() {
        let v = vec![
            ch(ChangeType::FieldAdded),
            ch(ChangeType::FieldDescriptionChanged),
        ];
        assert!(!has_critical(&v));
    }

    #[test]
    fn test_has_critical_empty_is_false() {
        let v: Vec<Change> = vec![];
        assert!(!has_critical(&v));
    }
}
```

Create `src/diff.rs`:

```rust
pub mod diff;
pub mod filters;
pub mod matrix;
pub mod version;
```

In `src/main.rs`, add `mod diff;` to the module declarations (alphabetical placement near `mod cli;`).

We must declare each submodule as a stub before `cargo check` will pass. Create *empty* files now:

```
src/diff/diff.rs    — // stub: implemented in Task 13
src/diff/matrix.rs  — // stub: implemented in Task 14
src/diff/version.rs — // stub: implemented in Task 15
```

### Step 2: Run to confirm failure

```
cargo test diff::filters::tests
```

Expected: tests panic at `todo!()`. Build itself succeeds because the diff submodules are valid (empty) Rust files.

### Step 3: Implement

Replace the two `todo!()` bodies in `src/diff/filters.rs`:

```rust
/// Set of change types that are considered breaking. Mirrors Python `_is_critical_change`.
pub fn is_change_critical(change: &Change) -> bool {
    matches!(
        change.r#type,
        ChangeType::FieldRemoved
            | ChangeType::FieldTypeChanged
            | ChangeType::FieldCardinalityChanged
            | ChangeType::FieldReferenceChanged
            | ChangeType::FieldStringFormatChanged
            | ChangeType::FieldAnyOfTypeAdded
            | ChangeType::FieldAnyOfTypeRemoved
            | ChangeType::FieldAllOfTypeAdded
            | ChangeType::FieldAllOfTypeRemoved
            | ChangeType::ClassRemoved
            | ChangeType::EnumValueRemoved
    )
}

/// Returns true iff any change in the iterator is critical.
pub fn has_critical<'a, I: IntoIterator<Item = &'a Change>>(changes: I) -> bool {
    changes.into_iter().any(is_change_critical)
}
```

### Step 4: Run tests

```
cargo test diff::filters::tests
```

Expected: 4 tests pass.

### Step 5: Commit

```
git add src/diff.rs src/diff/filters.rs src/diff/diff.rs src/diff/matrix.rs src/diff/version.rs src/main.rs
git commit -m "feat(diff/filters): is_change_critical + has_critical"
```

---

## Task 13: `src/diff/diff.rs` — schema comparison

**Files:**
- Modify: `src/diff/diff.rs`

This is the single largest task. We split it into multiple TDD cycles, each adding one helper and one test, all under one final commit.

### Step 1: Establish module skeleton + first failing test

Replace the contents of `src/diff/diff.rs` with:

```rust
use crate::diff::filters::has_critical;
use crate::models::changes::{Change, ChangeType, ChangeValue, Changes};
use crate::models::json_schema::{
    AllOfSchema, AnyOfSchema, ArraySchema, ObjectSchema, ReferenceSchema, SchemaRootType,
    SchemaType, StrEnumSchema, StringSchema, TypeBase,
};
use crate::models::schema_meta::Schemas;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    /// Matches the BO4E version string `vYYYYMM.f.t[-rcN]` anywhere in a description.
    /// Used to ignore version-only differences in autogenerated docstrings.
    static ref REGEX_VERSION_IN_DESC: Regex =
        Regex::new(r"v\d{6}\.\d+\.\d+(?:-rc\d*)?").unwrap();
}

const VERSION_DESC_PLACEHOLDER: &str = "{__gh_version__}";
const VERSION_TITLE_MARKER: &str = " Version"; // leading space — autogenerated _version field

#[derive(Debug, Clone, Copy)]
enum VariantKind { AnyOf, AllOf }

/// Compare two `Schemas` collections and return the list of changes between them.
pub fn diff_schemas(old: &Schemas, new: &Schemas) -> Changes {
    let mut out: Vec<Change> = Vec::new();
    diff_root_schemas(old, new, &mut out);
    Changes {
        old_schemas: old.clone(),
        new_schemas: new.clone(),
        changes: out,
    }
}

fn diff_root_schemas(_old: &Schemas, _new: &Schemas, _out: &mut Vec<Change>) {
    // implemented incrementally below
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::changes::ChangeType;
    use crate::models::json_schema::{
        LiteralTypeObject, LiteralTypeString, ObjectSchema, SchemaRootObject, SchemaRootStrEnum,
        SchemaRootType, SchemaType, StrEnumSchema, TypeBase,
    };
    use crate::models::schema_meta::{Schema, Schemas};
    use crate::models::version::DirtyVersion;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    fn empty_object_root(title: &str) -> SchemaRootType {
        SchemaRootType::Object(SchemaRootObject {
            object: ObjectSchema {
                base: TypeBase {
                    description: None,
                    title: Some(title.to_string()),
                    default: None,
                },
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: BTreeMap::new(),
                required: vec![],
            },
            // (other SchemaRootObject fields use Default; verify against
            // models/json_schema.rs at implementation time and adjust if needed)
        })
    }

    fn schema_with(module: &[&str], root: SchemaRootType) -> Rc<RefCell<Schema>> {
        let m: Vec<String> = module.iter().map(|s| s.to_string()).collect();
        Rc::new(RefCell::new(Schema::new(m, Some(root)).unwrap()))
    }

    fn collection(version: &str, items: Vec<Rc<RefCell<Schema>>>) -> Schemas {
        let v: DirtyVersion = version.parse().unwrap();
        let mut s = Schemas::new(v);
        for it in items {
            s.add_schema(it).unwrap();
        }
        s
    }

    #[test]
    fn test_class_added_when_module_only_in_new() {
        let old = collection("v202401.0.1", vec![]);
        let new = collection(
            "v202401.0.2",
            vec![schema_with(&["bo", "Angebot"], empty_object_root("Angebot"))],
        );
        let changes = diff_schemas(&old, &new);
        assert_eq!(changes.changes.len(), 1);
        assert_eq!(changes.changes[0].r#type, ChangeType::ClassAdded);
        assert_eq!(changes.changes[0].new_trace, "/bo/Angebot");
    }

    #[test]
    fn test_class_removed_when_module_only_in_old() {
        let old = collection(
            "v202401.0.1",
            vec![schema_with(&["bo", "Angebot"], empty_object_root("Angebot"))],
        );
        let new = collection("v202401.0.2", vec![]);
        let changes = diff_schemas(&old, &new);
        assert_eq!(changes.changes.len(), 1);
        assert_eq!(changes.changes[0].r#type, ChangeType::ClassRemoved);
        assert_eq!(changes.changes[0].old_trace, "/bo/Angebot");
    }
}
```

> **Note for the implementer:** the `SchemaRootObject` literal above may need extra fields (e.g., `definitions`) depending on the current shape of `models/json_schema.rs`. Read that file before each test fixture; add `..Default::default()` if `SchemaRootObject` derives `Default`, or fill in the missing fields literally. The same applies to every fixture below.

### Step 2: Run to confirm failure

```
cargo test diff::diff::tests::test_class_added_when_module_only_in_new
```

Expected: test fails — `diff_root_schemas` is empty so `changes` is empty.

### Step 3: Implement `diff_root_schemas` + class add/remove

Replace the empty `diff_root_schemas` stub:

```rust
fn diff_root_schemas(old: &Schemas, new: &Schemas, out: &mut Vec<Change>) {
    // Modules only in new → ClassAdded.
    for s in new.module_difference(old) {
        let module = s.borrow().module().to_vec();
        let trace = format!("/{}", module.join("/"));
        out.push(Change {
            r#type: ChangeType::ClassAdded,
            old: None,
            new: Some(ChangeValue::String(module.join("."))),
            old_trace: String::new(),
            new_trace: trace,
        });
    }

    // Modules only in old → ClassRemoved.
    for s in old.module_difference(new) {
        let module = s.borrow().module().to_vec();
        let trace = format!("/{}", module.join("/"));
        out.push(Change {
            r#type: ChangeType::ClassRemoved,
            old: Some(ChangeValue::String(module.join("."))),
            new: None,
            old_trace: trace,
            new_trace: String::new(),
        });
    }

    // Modules in both → recurse into root-level diff.
    for s_old in old.module_intersection(new) {
        let module = s_old.borrow().module().to_vec();
        let s_new = new.get_by_module(&module).expect("intersection guaranteed");
        let trace = format!("/{}", module.join("/"));

        // Borrow each schema mutably to materialize SchemaRootType from text if needed.
        let mut b_old = s_old.borrow_mut();
        let mut b_new = s_new.borrow_mut();
        let root_old = match b_old.schema_mut() {
            Ok(r) => r.clone(),
            Err(_) => continue,
        };
        let root_new = match b_new.schema_mut() {
            Ok(r) => r.clone(),
            Err(_) => continue,
        };
        diff_root_pair(&root_old, &root_new, &trace, &trace, out);
    }
}

fn diff_root_pair(
    old: &SchemaRootType,
    new: &SchemaRootType,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    match (old, new) {
        (SchemaRootType::Object(o), SchemaRootType::Object(n)) => {
            diff_object_schemas(&o.object, &n.object, old_trace, new_trace, out);
        }
        (SchemaRootType::StrEnum(o), SchemaRootType::StrEnum(n)) => {
            diff_enum_schemas(&o.string_enum, &n.string_enum, old_trace, new_trace, out);
            // ↑ field name on SchemaRootStrEnum may differ; adjust to match the actual struct.
        }
        // Object ↔ StrEnum at the root: model-level cardinality change.
        _ => {
            out.push(Change {
                r#type: ChangeType::FieldTypeChanged,
                old: None,
                new: None,
                old_trace: old_trace.to_string(),
                new_trace: new_trace.to_string(),
            });
        }
    }
}
```

> **Implementer note:** `SchemaRootObject` and `SchemaRootStrEnum` field names depend on `models/json_schema.rs`. Read that file when wiring the match — the `.object` / `.string_enum` accessors above are placeholders to be confirmed.

### Step 4: Re-run; expect both tests pass

```
cargo test diff::diff::tests::test_class_added_when_module_only_in_new
cargo test diff::diff::tests::test_class_removed_when_module_only_in_old
```

Expected: both pass.

### Step 5: Add `diff_type_base` + tests for description / title / default

Append to `src/diff/diff.rs` (above `mod tests`):

```rust
fn diff_type_base(
    old: &TypeBase,
    new: &TypeBase,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    // Description: ignore differences that disappear after substituting the version pattern.
    let desc_changed = match (&old.description, &new.description) {
        (Some(o), Some(n)) => {
            let o_norm = REGEX_VERSION_IN_DESC.replace_all(o, VERSION_DESC_PLACEHOLDER);
            let n_norm = REGEX_VERSION_IN_DESC.replace_all(n, VERSION_DESC_PLACEHOLDER);
            o_norm != n_norm
        }
        (None, None) => false,
        _ => true,
    };
    if desc_changed {
        out.push(Change {
            r#type: ChangeType::FieldDescriptionChanged,
            old: old.description.clone().map(ChangeValue::String),
            new: new.description.clone().map(ChangeValue::String),
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }

    // Title.
    if old.title != new.title {
        out.push(Change {
            r#type: ChangeType::FieldTitleChanged,
            old: old.title.clone().map(ChangeValue::String),
            new: new.title.clone().map(ChangeValue::String),
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }

    // Default — skip when title is exactly " Version" on either side
    // (autogenerated _version field whose default changes every release).
    let is_version_field = old.title.as_deref() == Some(VERSION_TITLE_MARKER)
        || new.title.as_deref() == Some(VERSION_TITLE_MARKER);
    if !is_version_field && old.default != new.default {
        out.push(Change {
            r#type: ChangeType::FieldDefaultChanged,
            old: None, // PrimitiveValue not directly representable in ChangeValue; design accepts None
            new: None,
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }
}
```

Add four tests inside `mod tests`:

```rust
    fn base(desc: Option<&str>, title: Option<&str>) -> TypeBase {
        TypeBase {
            description: desc.map(String::from),
            title: title.map(String::from),
            default: None,
        }
    }

    #[test]
    fn test_diff_type_base_emits_description_changed() {
        let mut out = vec![];
        diff_type_base(&base(Some("alpha"), None), &base(Some("beta"), None),
            "/x", "/x", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldDescriptionChanged);
    }

    #[test]
    fn test_diff_type_base_ignores_version_only_description_change() {
        let mut out = vec![];
        diff_type_base(
            &base(Some("Schema for v202401.0.1"), None),
            &base(Some("Schema for v202401.0.2"), None),
            "/x", "/x", &mut out,
        );
        assert_eq!(out.len(), 0);
    }

    #[test]
    fn test_diff_type_base_emits_title_changed() {
        let mut out = vec![];
        diff_type_base(&base(None, Some("A")), &base(None, Some("B")),
            "/x", "/x", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldTitleChanged);
    }

    #[test]
    fn test_diff_type_base_skips_version_field_default_change() {
        use crate::models::json_schema::PrimitiveValue;
        let mut a = base(None, Some(" Version"));
        let mut b = base(None, Some(" Version"));
        a.default = Some(PrimitiveValue::String("v202401.0.1".into()));
        b.default = Some(PrimitiveValue::String("v202401.0.2".into()));
        let mut out = vec![];
        diff_type_base(&a, &b, "/x", "/x", &mut out);
        assert_eq!(out.len(), 0);
    }
```

Run:

```
cargo test diff::diff::tests
```

Expected: 4 new tests pass alongside the 2 from earlier.

### Step 6: `diff_enum_schemas` (EnumValueAdded/Removed)

Append above `mod tests`:

```rust
fn diff_enum_schemas(
    old: &StrEnumSchema,
    new: &StrEnumSchema,
    old_trace: &str,
    new_trace: &str,
    out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);

    let old_set: std::collections::BTreeSet<&String> = old.enum_values.iter().collect();
    let new_set: std::collections::BTreeSet<&String> = new.enum_values.iter().collect();

    for v in new_set.difference(&old_set) {
        out.push(Change {
            r#type: ChangeType::EnumValueAdded,
            old: None,
            new: Some(ChangeValue::String((*v).clone())),
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }
    for v in old_set.difference(&new_set) {
        out.push(Change {
            r#type: ChangeType::EnumValueRemoved,
            old: Some(ChangeValue::String((*v).clone())),
            new: None,
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }
}
```

Test:

```rust
    fn enum_schema(values: &[&str]) -> StrEnumSchema {
        StrEnumSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeString::String,
            enum_values: values.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_enum_value_added_and_removed() {
        let mut out = vec![];
        diff_enum_schemas(
            &enum_schema(&["A", "B"]),
            &enum_schema(&["B", "C"]),
            "/x", "/x", &mut out,
        );
        let kinds: Vec<_> = out.iter().map(|c| c.r#type.clone()).collect();
        assert!(kinds.contains(&ChangeType::EnumValueAdded));
        assert!(kinds.contains(&ChangeType::EnumValueRemoved));
        assert_eq!(out.len(), 2);
    }
```

Run, confirm pass.

### Step 7: `diff_ref_schemas`, `diff_array_schemas`, `diff_string_schemas`

Append above `mod tests`:

```rust
fn diff_ref_schemas(
    old: &ReferenceSchema, new: &ReferenceSchema,
    old_trace: &str, new_trace: &str, out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);
    if old.r#ref != new.r#ref {
        out.push(Change {
            r#type: ChangeType::FieldReferenceChanged,
            old: Some(ChangeValue::String(old.r#ref.clone())),
            new: Some(ChangeValue::String(new.r#ref.clone())),
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }
}

fn diff_array_schemas(
    old: &ArraySchema, new: &ArraySchema,
    old_trace: &str, new_trace: &str, out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);
    diff_schema_type(
        &old.items, &new.items,
        &format!("{}/items", old_trace),
        &format!("{}/items", new_trace),
        out,
    );
}

fn diff_string_schemas(
    old: &StringSchema, new: &StringSchema,
    old_trace: &str, new_trace: &str, out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);
    if old.format != new.format {
        out.push(Change {
            r#type: ChangeType::FieldStringFormatChanged,
            old: old.format.as_ref().map(|f| ChangeValue::String(format!("{:?}", f))),
            new: new.format.as_ref().map(|f| ChangeValue::String(format!("{:?}", f))),
            old_trace: old_trace.to_string(),
            new_trace: new_trace.to_string(),
        });
    }
}
```

Tests:

```rust
    use crate::models::json_schema::{LiteralTypeArray, ReferenceSchema, StringSchema};

    #[test]
    fn test_ref_change_emits_field_reference_changed() {
        let r1 = ReferenceSchema { base: TypeBase::default(), r#ref: "#/A".into() };
        let r2 = ReferenceSchema { base: TypeBase::default(), r#ref: "#/B".into() };
        let mut out = vec![];
        diff_ref_schemas(&r1, &r2, "/x", "/x", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldReferenceChanged);
    }

    #[test]
    fn test_string_format_change() {
        use crate::models::json_schema::StringSchemaFormat;
        let mut a = StringSchema::default();
        let mut b = StringSchema::default();
        a.format = None;
        b.format = Some(StringSchemaFormat::DateTime);
        // ↑ verify the actual variant name in models/json_schema.rs
        let mut out = vec![];
        diff_string_schemas(&a, &b, "/x", "/x", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldStringFormatChanged);
    }
```

Run, confirm pass.

### Step 8: `diff_variant_list` + `diff_any_of_schemas` + `diff_all_of_schemas`

Append:

```rust
fn diff_any_of_schemas(
    old: &AnyOfSchema, new: &AnyOfSchema,
    old_trace: &str, new_trace: &str, out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);
    diff_variant_list(&old.any_of, &new.any_of, old_trace, new_trace, VariantKind::AnyOf, out);
}

fn diff_all_of_schemas(
    old: &AllOfSchema, new: &AllOfSchema,
    old_trace: &str, new_trace: &str, out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);
    diff_variant_list(&old.all_of, &new.all_of, old_trace, new_trace, VariantKind::AllOf, out);
}

fn diff_variant_list(
    old: &[SchemaType], new: &[SchemaType],
    old_trace: &str, new_trace: &str,
    kind: VariantKind, out: &mut Vec<Change>,
) {
    let key = match kind {
        VariantKind::AnyOf => "anyOf",
        VariantKind::AllOf => "allOf",
    };
    let added_t = match kind {
        VariantKind::AnyOf => ChangeType::FieldAnyOfTypeAdded,
        VariantKind::AllOf => ChangeType::FieldAllOfTypeAdded,
    };
    let removed_t = match kind {
        VariantKind::AnyOf => ChangeType::FieldAnyOfTypeRemoved,
        VariantKind::AllOf => ChangeType::FieldAllOfTypeRemoved,
    };

    let mut new_matched = vec![false; new.len()];
    for (oi, ov) in old.iter().enumerate() {
        let ot = format!("{}/{}/{}", old_trace, key, oi);
        let mut paired = false;
        for (ni, nv) in new.iter().enumerate() {
            if new_matched[ni] {
                continue;
            }
            let nt = format!("{}/{}/{}", new_trace, key, ni);
            let mut sub: Vec<Change> = Vec::new();
            diff_schema_type(ov, nv, &ot, &nt, &mut sub);
            if !has_critical(&sub) {
                out.extend(sub);
                new_matched[ni] = true;
                paired = true;
                break;
            }
        }
        if !paired {
            out.push(Change {
                r#type: removed_t.clone(),
                old: None, new: None,
                old_trace: ot, new_trace: new_trace.to_string(),
            });
        }
    }
    for (ni, matched) in new_matched.iter().enumerate() {
        if !matched {
            let nt = format!("{}/{}/{}", new_trace, key, ni);
            out.push(Change {
                r#type: added_t.clone(),
                old: None, new: None,
                old_trace: old_trace.to_string(),
                new_trace: nt,
            });
        }
    }
}
```

Tests:

```rust
    fn string_schema_t() -> SchemaType {
        SchemaType::StringSchema(StringSchema::default())
    }

    fn ref_t(r: &str) -> SchemaType {
        SchemaType::ReferenceSchema(ReferenceSchema {
            base: TypeBase::default(),
            r#ref: r.to_string(),
        })
    }

    #[test]
    fn test_any_of_variant_added_emits_field_any_of_type_added() {
        let old = AnyOfSchema { base: TypeBase::default(), any_of: vec![string_schema_t()] };
        let new = AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![string_schema_t(), ref_t("#/A")],
        };
        let mut out = vec![];
        diff_any_of_schemas(&old, &new, "/x", "/x", &mut out);
        let added: Vec<_> = out.iter()
            .filter(|c| c.r#type == ChangeType::FieldAnyOfTypeAdded).collect();
        assert_eq!(added.len(), 1);
    }

    #[test]
    fn test_any_of_variant_removed() {
        let old = AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![string_schema_t(), ref_t("#/A")],
        };
        let new = AnyOfSchema { base: TypeBase::default(), any_of: vec![string_schema_t()] };
        let mut out = vec![];
        diff_any_of_schemas(&old, &new, "/x", "/x", &mut out);
        let removed: Vec<_> = out.iter()
            .filter(|c| c.r#type == ChangeType::FieldAnyOfTypeRemoved).collect();
        assert_eq!(removed.len(), 1);
    }

    #[test]
    fn test_any_of_pairs_with_non_critical_inner_change() {
        // Same string variant on both sides but with a description difference (non-critical).
        let mut s_old = StringSchema::default();
        let mut s_new = StringSchema::default();
        s_old.base.description = Some("old".into());
        s_new.base.description = Some("new".into());
        let old = AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![SchemaType::StringSchema(s_old)],
        };
        let new = AnyOfSchema {
            base: TypeBase::default(),
            any_of: vec![SchemaType::StringSchema(s_new)],
        };
        let mut out = vec![];
        diff_any_of_schemas(&old, &new, "/x", "/x", &mut out);
        assert!(out.iter().all(|c| c.r#type != ChangeType::FieldAnyOfTypeAdded));
        assert!(out.iter().all(|c| c.r#type != ChangeType::FieldAnyOfTypeRemoved));
        assert!(out.iter().any(|c| c.r#type == ChangeType::FieldDescriptionChanged));
    }
```

Run, confirm pass.

### Step 9: `diff_schema_differing_types` + `diff_schema_type` dispatch

Append:

```rust
fn diff_schema_differing_types(
    old: &SchemaType, new: &SchemaType,
    old_trace: &str, new_trace: &str, out: &mut Vec<Change>,
) {
    // Object ↔ Array around the same logical field is a cardinality change,
    // not a type change: `Foo` vs `[Foo]`.
    let cardinality = matches!(
        (old, new),
        (SchemaType::Array(_), SchemaType::Object(_)) |
        (SchemaType::Object(_), SchemaType::Array(_)) |
        (SchemaType::Array(_), SchemaType::Reference(_)) |
        (SchemaType::Reference(_), SchemaType::Array(_))
    );
    let kind = if cardinality {
        ChangeType::FieldCardinalityChanged
    } else {
        ChangeType::FieldTypeChanged
    };
    out.push(Change {
        r#type: kind,
        old: Some(ChangeValue::Schema(old.clone())),
        new: Some(ChangeValue::Schema(new.clone())),
        old_trace: old_trace.to_string(),
        new_trace: new_trace.to_string(),
    });
}

fn diff_schema_type(
    old: &SchemaType, new: &SchemaType,
    old_trace: &str, new_trace: &str, out: &mut Vec<Change>,
) {
    use SchemaType::*;
    match (old, new) {
        (Object(o), Object(n)) => diff_object_schemas(o, n, old_trace, new_trace, out),
        (StrEnum(o), StrEnum(n)) => diff_enum_schemas(o, n, old_trace, new_trace, out),
        (Array(o), Array(n))   => diff_array_schemas(o, n, old_trace, new_trace, out),
        (AnyOf(o), AnyOf(n))   => diff_any_of_schemas(o, n, old_trace, new_trace, out),
        (AllOf(o), AllOf(n))   => diff_all_of_schemas(o, n, old_trace, new_trace, out),
        (StringSchema(o), StringSchema(n)) =>
            diff_string_schemas(o, n, old_trace, new_trace, out),
        (ReferenceSchema(o), ReferenceSchema(n)) =>
            diff_ref_schemas(o, n, old_trace, new_trace, out),
        // Other variants (Constant, Number, Decimal, Integer, Boolean, Null, Any) compare
        // by base only — fall through to base-only diff.
        _ if std::mem::discriminant(old) == std::mem::discriminant(new) => {
            // Same variant, no further structure to inspect — base diff is impossible without
            // a base accessor; safe to emit no change for these scalar variants.
        }
        _ => diff_schema_differing_types(old, new, old_trace, new_trace, out),
    }
}
```

> **Implementer note:** the SchemaType variant names (`Array`, `Reference`, `StringSchema`, `ReferenceSchema`, etc.) must match `models/json_schema.rs`. If the existing variant names differ, adjust both the dispatch and the cardinality match accordingly.

Tests:

```rust
    #[test]
    fn test_field_type_changed_unrelated_types() {
        use crate::models::json_schema::{NumberSchema};
        let old = SchemaType::StringSchema(StringSchema::default());
        let new = SchemaType::NumberSchema(NumberSchema::default());
        let mut out = vec![];
        diff_schema_type(&old, &new, "/x", "/x", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldTypeChanged);
    }

    #[test]
    fn test_field_cardinality_changed_object_to_array() {
        let obj = SchemaType::Object(ObjectSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeObject::Object,
            additional_properties: false,
            properties: BTreeMap::new(),
            required: vec![],
        });
        let arr = SchemaType::Array(ArraySchema {
            base: TypeBase::default(),
            r#type: LiteralTypeArray::Array,
            items: Box::new(SchemaType::StringSchema(StringSchema::default())),
        });
        let mut out = vec![];
        diff_schema_type(&obj, &arr, "/x", "/x", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].r#type, ChangeType::FieldCardinalityChanged);
    }
```

Run, confirm pass.

### Step 10: `diff_object_schemas` (FieldAdded / FieldRemoved + recurse)

Append:

```rust
fn diff_object_schemas(
    old: &ObjectSchema, new: &ObjectSchema,
    old_trace: &str, new_trace: &str, out: &mut Vec<Change>,
) {
    diff_type_base(&old.base, &new.base, old_trace, new_trace, out);

    for (name, schema_new) in &new.properties {
        if !old.properties.contains_key(name) {
            let nt = format!("{}/{}", new_trace, name);
            out.push(Change {
                r#type: ChangeType::FieldAdded,
                old: None,
                new: Some(ChangeValue::Schema(schema_new.clone())),
                old_trace: old_trace.to_string(),
                new_trace: nt,
            });
        }
    }
    for (name, schema_old) in &old.properties {
        if !new.properties.contains_key(name) {
            let ot = format!("{}/{}", old_trace, name);
            out.push(Change {
                r#type: ChangeType::FieldRemoved,
                old: Some(ChangeValue::Schema(schema_old.clone())),
                new: None,
                old_trace: ot,
                new_trace: new_trace.to_string(),
            });
        }
    }
    // Common keys → recurse.
    for (name, schema_old) in &old.properties {
        if let Some(schema_new) = new.properties.get(name) {
            diff_schema_type(
                schema_old, schema_new,
                &format!("{}/{}", old_trace, name),
                &format!("{}/{}", new_trace, name),
                out,
            );
        }
    }
}
```

Tests:

```rust
    fn obj(props: &[(&str, SchemaType)]) -> ObjectSchema {
        let mut p = BTreeMap::new();
        for (k, v) in props { p.insert(k.to_string(), v.clone()); }
        ObjectSchema {
            base: TypeBase::default(),
            r#type: LiteralTypeObject::Object,
            additional_properties: false,
            properties: p,
            required: vec![],
        }
    }

    #[test]
    fn test_object_field_added_and_removed() {
        let a = obj(&[("foo", string_schema_t())]);
        let b = obj(&[("bar", string_schema_t())]);
        let mut out = vec![];
        diff_object_schemas(&a, &b, "/x", "/x", &mut out);
        let kinds: Vec<_> = out.iter().map(|c| c.r#type.clone()).collect();
        assert!(kinds.contains(&ChangeType::FieldAdded));
        assert!(kinds.contains(&ChangeType::FieldRemoved));
    }

    #[test]
    fn test_object_field_default_changed_recurses() {
        use crate::models::json_schema::PrimitiveValue;
        let mut s_old = StringSchema::default();
        let mut s_new = StringSchema::default();
        s_old.base.default = Some(PrimitiveValue::String("a".into()));
        s_new.base.default = Some(PrimitiveValue::String("b".into()));
        let a = obj(&[("foo", SchemaType::StringSchema(s_old))]);
        let b = obj(&[("foo", SchemaType::StringSchema(s_new))]);
        let mut out = vec![];
        diff_object_schemas(&a, &b, "/x", "/x", &mut out);
        assert!(out.iter().any(|c| c.r#type == ChangeType::FieldDefaultChanged));
    }
```

Run, confirm pass.

### Step 11: Final integration check + commit

```
cargo test diff::diff::tests
cargo test
```

Expected: every test passes.

```
git add src/diff/diff.rs
git commit -m "feat(diff/diff): schema comparison via mutable Change collector"
```

---

## Task 14: `src/diff/matrix.rs` — linear-chain validation + matrix generation

**Files:**
- Modify: `src/diff/matrix.rs`

### Step 1: Write failing tests + skeleton

Replace `src/diff/matrix.rs` (currently a stub) with:

```rust
use crate::diff::filters::has_critical;
use crate::models::changes::{Change, ChangeType, Changes};
use crate::models::matrix::{
    Compatibility, CompatibilityMatrix, CompatibilityMatrixEntry, CompatibilitySymbol,
    CompatibilityText,
};
use crate::models::schema_meta::Schemas;
use indexmap::IndexMap;
use std::collections::{BTreeSet, HashMap, HashSet};

pub struct VersionChain {
    pub nodes: Vec<ChainNode>,   // ordered start → end, length n+1
    pub edges: Vec<ChainEdge>,   // edges[i] connects nodes[i] → nodes[i+1]
}

pub struct ChainNode {
    pub version_key: String,
    pub schemas: Schemas,
}

pub struct ChainEdge {
    pub changes: Changes,
}

pub fn build_chain(diffs: Vec<Changes>) -> Result<VersionChain, String> {
    todo!()
}

pub fn create_compatibility_matrix(
    chain: &VersionChain,
    use_emotes: bool,
) -> CompatibilityMatrix {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::changes::{Change, ChangeType, Changes};
    use crate::models::schema_meta::{Schema, Schemas};
    use crate::models::version::DirtyVersion;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn schema_from(module: &[&str]) -> Rc<RefCell<Schema>> {
        let m: Vec<String> = module.iter().map(|s| s.to_string()).collect();
        Rc::new(RefCell::new(Schema::new(m, None).unwrap()))
    }

    fn coll(version: &str, modules: &[&[&str]]) -> Schemas {
        let v: DirtyVersion = version.parse().unwrap();
        let mut s = Schemas::new(v);
        for m in modules { s.add_schema(schema_from(m)).unwrap(); }
        s
    }

    fn changes_between(old_v: &str, new_v: &str, items: Vec<Change>) -> Changes {
        Changes {
            old_schemas: coll(old_v, &[&["bo", "Angebot"]]),
            new_schemas: coll(new_v, &[&["bo", "Angebot"]]),
            changes: items,
        }
    }

    #[test]
    fn test_build_chain_orders_three_unsorted_diffs() {
        let d_ab = changes_between("v202401.0.1", "v202401.0.2", vec![]);
        let d_bc = changes_between("v202401.0.2", "v202401.1.0", vec![]);
        let d_cd = changes_between("v202401.1.0", "v202402.0.0", vec![]);
        // Provide unsorted.
        let chain = build_chain(vec![d_cd, d_ab, d_bc]).unwrap();
        let keys: Vec<_> = chain.nodes.iter().map(|n| n.version_key.clone()).collect();
        assert_eq!(keys, vec![
            "v202401.0.1".to_string(),
            "v202401.0.2".to_string(),
            "v202401.1.0".to_string(),
            "v202402.0.0".to_string(),
        ]);
        assert_eq!(chain.edges.len(), 3);
    }

    #[test]
    fn test_build_chain_rejects_two_starts() {
        let d1 = changes_between("v202401.0.1", "v202401.0.2", vec![]);
        let d2 = changes_between("v202401.1.0", "v202402.0.0", vec![]);
        let err = build_chain(vec![d1, d2]).unwrap_err();
        assert!(err.to_lowercase().contains("start") || err.to_lowercase().contains("disconnected"));
    }

    #[test]
    fn test_build_chain_rejects_duplicate_outgoing_edge() {
        let d1 = changes_between("v202401.0.1", "v202401.0.2", vec![]);
        let d2 = changes_between("v202401.0.1", "v202401.1.0", vec![]);
        let err = build_chain(vec![d1, d2]).unwrap_err();
        assert!(err.to_lowercase().contains("outgoing") || err.to_lowercase().contains("duplicate"));
    }

    #[test]
    fn test_build_chain_rejects_node_attribute_mismatch() {
        let d1 = Changes {
            old_schemas: coll("v202401.0.1", &[&["bo", "Angebot"]]),
            new_schemas: coll("v202401.0.2", &[&["bo", "Angebot"]]),
            changes: vec![],
        };
        let d2 = Changes {
            old_schemas: coll("v202401.0.2", &[&["enum", "Typ"]]), // different content for same version key
            new_schemas: coll("v202401.1.0", &[&["bo", "Angebot"]]),
            changes: vec![],
        };
        let err = build_chain(vec![d1, d2]).unwrap_err();
        assert!(err.to_lowercase().contains("different attributes"));
    }

    #[test]
    fn test_create_matrix_emits_unchanged_and_added() {
        let mut s_a = coll("v202401.0.1", &[&["bo", "Angebot"]]);
        let s_b = coll("v202401.0.2", &[&["bo", "Angebot"], &["enum", "Typ"]]);
        // ↑ cleaner: build via local helpers below.

        let class_added = Change {
            r#type: ChangeType::ClassAdded,
            old: None, new: None,
            old_trace: String::new(),
            new_trace: "/enum/Typ".to_string(),
        };
        let d = Changes {
            old_schemas: s_a.clone(),
            new_schemas: s_b.clone(),
            changes: vec![class_added],
        };
        let chain = build_chain(vec![d]).unwrap();
        let matrix = create_compatibility_matrix(&chain, false);

        let bo_row = matrix.root.get("bo.Angebot").unwrap();
        assert_eq!(bo_row.len(), 1);
        assert!(matches!(
            bo_row[0].compatibility,
            Compatibility::Text(CompatibilityText::ChangeNone)
        ));
        let enum_row = matrix.root.get("enum.Typ").unwrap();
        assert!(matches!(
            enum_row[0].compatibility,
            Compatibility::Text(CompatibilityText::Added)
        ));
    }
}
```

### Step 2: Run to confirm failure

```
cargo test diff::matrix::tests
```

Expected: tests panic at `todo!()`.

### Step 3: Implement `build_chain`

Replace the `todo!()` for `build_chain`:

```rust
pub fn build_chain(diffs: Vec<Changes>) -> Result<VersionChain, String> {
    if diffs.is_empty() {
        return Err("Cannot build a version chain from zero diffs.".to_string());
    }

    // Collect node schemas; reject any version key associated with two distinct Schemas.
    let mut nodes: HashMap<String, Schemas> = HashMap::new();
    let mut insert_node = |key: String, s: Schemas| -> Result<(), String> {
        if let Some(existing) = nodes.get(&key) {
            if existing != &s {
                return Err(format!("Node {} already exists with different attributes.", key));
            }
            return Ok(());
        }
        nodes.insert(key, s);
        Ok(())
    };

    let mut out_edge: HashMap<String, usize> = HashMap::new();
    let mut in_keys: HashSet<String> = HashSet::new();

    for (idx, d) in diffs.iter().enumerate() {
        let old_key = d.old_version().to_string();
        let new_key = d.new_version().to_string();
        insert_node(old_key.clone(), d.old_schemas.clone())?;
        insert_node(new_key.clone(), d.new_schemas.clone())?;
        if out_edge.insert(old_key.clone(), idx).is_some() {
            return Err(format!("Duplicate outgoing edge from version {}.", old_key));
        }
        if !in_keys.insert(new_key.clone()) {
            return Err(format!("Duplicate incoming edge to version {}.", new_key));
        }
    }

    // Identify start (in nodes \ in_keys) and end (in nodes \ keys(out)).
    let starts: Vec<&String> = nodes.keys().filter(|k| !in_keys.contains(*k)).collect();
    if starts.len() != 1 {
        return Err(format!(
            "Expected exactly one start node, found {}.",
            starts.len()
        ));
    }
    let start = starts[0].clone();

    let ends: Vec<&String> = nodes.keys().filter(|k| !out_edge.contains_key(*k)).collect();
    if ends.len() != 1 {
        return Err(format!("Expected exactly one end node, found {}.", ends.len()));
    }
    let end = ends[0].clone();

    // Walk the chain.
    let mut nodes_ordered: Vec<ChainNode> = Vec::new();
    let mut edges_ordered: Vec<ChainEdge> = Vec::new();
    let mut cursor = start.clone();
    nodes_ordered.push(ChainNode {
        version_key: cursor.clone(),
        schemas: nodes[&cursor].clone(),
    });

    while cursor != end {
        let next_idx = *out_edge.get(&cursor).ok_or_else(||
            format!("Disconnected chain: no outgoing edge from {}.", cursor))?;
        let edge_changes = diffs[next_idx].clone();
        let next_key = edge_changes.new_version().to_string();
        edges_ordered.push(ChainEdge { changes: edge_changes });
        cursor = next_key;
        nodes_ordered.push(ChainNode {
            version_key: cursor.clone(),
            schemas: nodes[&cursor].clone(),
        });
    }

    if edges_ordered.len() != diffs.len() {
        return Err("Disconnected chain: not all diffs are reachable from the start.".to_string());
    }

    Ok(VersionChain { nodes: nodes_ordered, edges: edges_ordered })
}
```

### Step 4: Run; first 4 tests should pass

```
cargo test diff::matrix::tests::test_build_chain
```

Expected: 4 build_chain tests pass.

### Step 5: Implement `create_compatibility_matrix`

Replace the `todo!()` for `create_compatibility_matrix`:

```rust
pub fn create_compatibility_matrix(
    chain: &VersionChain,
    use_emotes: bool,
) -> CompatibilityMatrix {
    // Collect every module across every node.
    let mut modules: BTreeSet<Vec<String>> = BTreeSet::new();
    for node in &chain.nodes {
        for m in node.schemas.modules() {
            modules.insert(m.clone());
        }
    }

    // Sort by lowercased path tuple, matching Python's `sorted(..., key=lambda m: tuple(p.lower()))`.
    let mut sorted: Vec<Vec<String>> = modules.into_iter().collect();
    sorted.sort_by_key(|m| m.iter().map(|p| p.to_lowercase()).collect::<Vec<_>>());

    let mut root: IndexMap<String, Vec<CompatibilityMatrixEntry>> = IndexMap::new();
    for module in &sorted {
        let class_path_str = format!("/{}#", module.join("/"));
        let mut entries: Vec<CompatibilityMatrixEntry> = Vec::with_capacity(chain.edges.len());

        for (i, edge) in chain.edges.iter().enumerate() {
            let node_a = &chain.nodes[i];
            let node_b = &chain.nodes[i + 1];

            let filtered: Vec<&Change> = edge
                .changes
                .changes
                .iter()
                .filter(|c| {
                    c.old_trace.starts_with(&class_path_str)
                        || c.new_trace.starts_with(&class_path_str)
                })
                .collect();

            let symbol = determine_compatibility(&filtered, &node_b.schemas, module, use_emotes);
            entries.push(CompatibilityMatrixEntry {
                previous_version: node_a.schemas.version.clone(),
                next_version: node_b.schemas.version.clone(),
                compatibility: symbol,
            });
        }

        root.insert(module.join("."), entries);
    }

    CompatibilityMatrix { root }
}

fn determine_compatibility(
    filtered: &[&Change],
    node_b: &Schemas,
    module: &[String],
    use_emotes: bool,
) -> Compatibility {
    let module_vec = module.to_vec();
    let exists_in_new = node_b.modules().contains(&module_vec);

    // Single-change short-circuits.
    if filtered.len() == 1 {
        match filtered[0].r#type {
            ChangeType::ClassRemoved => return wrap_symbol(use_emotes, Sym::Removed),
            ChangeType::ClassAdded   => return wrap_symbol(use_emotes, Sym::Added),
            _ => {}
        }
    }
    if !exists_in_new {
        return wrap_symbol(use_emotes, Sym::NonExistent);
    }
    if filtered.is_empty() {
        return wrap_symbol(use_emotes, Sym::ChangeNone);
    }

    // Invariant: by this point, no ClassAdded/ClassRemoved should remain in `filtered`.
    debug_assert!(
        filtered.iter().all(|c| !matches!(c.r#type, ChangeType::ClassAdded | ChangeType::ClassRemoved)),
        "ClassAdded/ClassRemoved must be the sole change in filtered list",
    );

    let owned: Vec<Change> = filtered.iter().map(|c| (*c).clone()).collect();
    if has_critical(&owned) {
        wrap_symbol(use_emotes, Sym::ChangeCritical)
    } else {
        wrap_symbol(use_emotes, Sym::ChangeNonCritical)
    }
}

#[derive(Copy, Clone)]
enum Sym { ChangeNone, ChangeNonCritical, ChangeCritical, NonExistent, Added, Removed }

fn wrap_symbol(use_emotes: bool, s: Sym) -> Compatibility {
    if use_emotes {
        Compatibility::Symbol(match s {
            Sym::ChangeNone        => CompatibilitySymbol::ChangeNone,
            Sym::ChangeNonCritical => CompatibilitySymbol::ChangeNonCritical,
            Sym::ChangeCritical    => CompatibilitySymbol::ChangeCritical,
            Sym::NonExistent       => CompatibilitySymbol::NonExistent,
            Sym::Added             => CompatibilitySymbol::Added,
            Sym::Removed           => CompatibilitySymbol::Removed,
        })
    } else {
        Compatibility::Text(match s {
            Sym::ChangeNone        => CompatibilityText::ChangeNone,
            Sym::ChangeNonCritical => CompatibilityText::ChangeNonCritical,
            Sym::ChangeCritical    => CompatibilityText::ChangeCritical,
            Sym::NonExistent       => CompatibilityText::NonExistent,
            Sym::Added             => CompatibilityText::Added,
            Sym::Removed           => CompatibilityText::Removed,
        })
    }
}
```

### Step 6: Run all matrix tests

```
cargo test diff::matrix::tests
```

Expected: all 5 tests pass.

### Step 7: Commit

```
git add src/diff/matrix.rs
git commit -m "feat(diff/matrix): linear-chain validation + compatibility matrix generation"
```

---

## Task 15: `src/diff/version.rs` — `check_version_bump`

**Files:**
- Modify: `src/diff/version.rs`

### Step 1: Write failing test + skeleton

Replace `src/diff/version.rs` with:

```rust
use crate::cprint_verbose;
use crate::models::changes::Changes;
use crate::models::version::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionBumpKind {
    Technical,
    Functional,
    Major,
}

pub fn check_version_bump(
    changes: &Changes,
    major_bump_allowed: bool,
) -> Result<VersionBumpKind, String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{Console, Level, CONSOLE};
    use crate::models::changes::{Change, ChangeType};
    use crate::models::schema_meta::Schemas;
    use crate::models::version::DirtyVersion;

    fn ensure_console() {
        let _ = CONSOLE.set(Console::new(Level::Normal));
    }

    fn changes(old: &str, new: &str, items: Vec<Change>) -> Changes {
        ensure_console();
        let v_old: DirtyVersion = old.parse().unwrap();
        let v_new: DirtyVersion = new.parse().unwrap();
        Changes {
            old_schemas: Schemas::new(v_old),
            new_schemas: Schemas::new(v_new),
            changes: items,
        }
    }

    fn ch(t: ChangeType) -> Change {
        Change { r#type: t, old: None, new: None,
            old_trace: String::new(), new_trace: String::new() }
    }

    #[test]
    fn test_errors_when_old_version_is_dirty() {
        let c = changes("v202401.0.1+gabc", "v202401.0.2", vec![]);
        let err = check_version_bump(&c, true).unwrap_err();
        assert!(err.to_lowercase().contains("dirty"));
    }

    #[test]
    fn test_errors_when_new_not_newer_than_old() {
        let c = changes("v202401.0.2", "v202401.0.1", vec![]);
        let err = check_version_bump(&c, true).unwrap_err();
        assert!(err.to_lowercase().contains("newer"));
    }

    #[test]
    fn test_major_bump_disallowed_returns_err() {
        let c = changes("v202401.0.1", "v202402.0.0", vec![]);
        let err = check_version_bump(&c, false).unwrap_err();
        assert!(err.to_lowercase().contains("major"));
    }

    #[test]
    fn test_major_bump_allowed_returns_major() {
        let c = changes("v202401.0.1", "v202402.0.0", vec![]);
        assert_eq!(check_version_bump(&c, true).unwrap(), VersionBumpKind::Major);
    }

    #[test]
    fn test_functional_bump_with_no_changes_errors() {
        let c = changes("v202401.0.1", "v202401.1.0", vec![]);
        let err = check_version_bump(&c, true).unwrap_err();
        assert!(err.to_lowercase().contains("functional"));
    }

    #[test]
    fn test_technical_bump_with_changes_errors() {
        let c = changes("v202401.0.1", "v202401.0.2",
            vec![ch(ChangeType::FieldAdded)]);
        let err = check_version_bump(&c, true).unwrap_err();
        assert!(err.to_lowercase().contains("technical"));
    }

    #[test]
    fn test_valid_technical_bump() {
        let c = changes("v202401.0.1", "v202401.0.2", vec![]);
        assert_eq!(check_version_bump(&c, true).unwrap(), VersionBumpKind::Technical);
    }

    #[test]
    fn test_valid_functional_bump() {
        let c = changes("v202401.0.1", "v202401.1.0",
            vec![ch(ChangeType::FieldAdded)]);
        assert_eq!(check_version_bump(&c, true).unwrap(), VersionBumpKind::Functional);
    }
}
```

### Step 2: Run to confirm failure

```
cargo test diff::version::tests
```

Expected: tests panic at `todo!()`.

### Step 3: Implement

Replace the `todo!()`:

```rust
pub fn check_version_bump(
    changes: &Changes,
    major_bump_allowed: bool,
) -> Result<VersionBumpKind, String> {
    let v_old: Version = changes.old_version().try_into()
        .map_err(|e: String| format!("Old version of diff is dirty and cannot serve as a baseline: {e}"))?;
    let v_new: Version = changes.new_version().try_into()
        .map_err(|e: String| format!("New version of diff is dirty and cannot be validated: {e}"))?;

    cprint_verbose!("Checking bump from {} to {}", v_old, v_new);

    if v_new <= v_old {
        return Err("The new version must be newer than the old version.".into());
    }

    if v_new.bumped_major(&v_old) {
        if !major_bump_allowed {
            return Err("Major version bump detected. Major bump is not allowed.".into());
        }
        return Ok(VersionBumpKind::Major);
    }

    let functional = !changes.changes.is_empty();
    let is_functional_bump = v_new.bumped_functional(&v_old);

    if functional && !is_functional_bump {
        return Err("Technical bump detected but functional changes found.".into());
    }
    if !functional && is_functional_bump {
        return Err("Functional bump detected but no functional changes found.".into());
    }

    Ok(if functional { VersionBumpKind::Functional } else { VersionBumpKind::Technical })
}
```

### Step 4: Run tests

```
cargo test diff::version::tests
```

Expected: all 8 tests pass.

### Step 5: Commit

```
git add src/diff/version.rs
git commit -m "feat(diff/version): check_version_bump with VersionBumpKind"
```

---

## Task 16: `src/cli/diff.rs` — clap subcommand group + runners

**Files:**
- Create: `src/cli/diff.rs`
- Modify: `src/cli.rs`
- Modify: `src/cli/base.rs`

### Step 1: Write failing test + skeleton

Create `src/cli/diff.rs`:

```rust
use crate::cli::base::Executable;
use crate::cprint_normal;
use crate::diff::diff::diff_schemas;
use crate::diff::matrix::{build_chain, create_compatibility_matrix};
use crate::diff::version::check_version_bump;
use crate::io::changes::{read_changes_from_diff_files, write_changes};
use crate::io::matrix::{write_compatibility_matrix_csv, write_compatibility_matrix_json};
use crate::io::schemas::read_schemas;
use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Args)]
pub struct Diff {
    #[command(subcommand)]
    pub command: DiffSubcommand,
}

#[derive(Subcommand)]
pub enum DiffSubcommand {
    Schemas(DiffSchemasArgs),
    Matrix(DiffMatrixArgs),
    VersionBump(VersionBumpArgs),
}

#[derive(Args)]
pub struct DiffSchemasArgs {
    /// Baseline directory of JSON schemas (the "old" side).
    pub input_dir_base: PathBuf,
    /// Directory of JSON schemas to compare against the baseline (the "new" side).
    pub input_dir_comp: PathBuf,
    /// Output diff JSON file.
    #[arg(short = 'o', long = "output", required = true)]
    pub output_file: PathBuf,
}

#[derive(Args)]
pub struct DiffMatrixArgs {
    /// One or more diff JSON files. Order does not matter.
    #[arg(required = true)]
    pub input_diff_files: Vec<PathBuf>,
    /// Output file path (CSV or JSON).
    #[arg(short = 'o', long = "output", required = true)]
    pub output_file: PathBuf,
    /// Output format.
    #[arg(short = 't', long = "output-type", default_value = "csv")]
    pub output_type: MatrixOutputType,
    /// Use emoji symbols instead of plain-text labels.
    #[arg(long = "use-emotes", default_value_t = false)]
    pub use_emotes: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum MatrixOutputType { Json, Csv }

#[derive(Args)]
pub struct VersionBumpArgs {
    /// Diff JSON file to validate.
    pub diff_file: PathBuf,
    /// Reject major version bumps.
    #[arg(long = "no-major", action = clap::ArgAction::SetFalse, default_value_t = true)]
    pub major_bump_allowed: bool,
}

impl Executable for Diff {
    fn run(&self) -> Result<(), String> {
        match &self.command {
            DiffSubcommand::Schemas(a)     => run_schemas(a),
            DiffSubcommand::Matrix(a)      => run_matrix(a),
            DiffSubcommand::VersionBump(a) => run_version_bump(a),
        }
    }
}

fn run_schemas(a: &DiffSchemasArgs) -> Result<(), String> {
    let old = read_schemas(&a.input_dir_base)?;
    let new = read_schemas(&a.input_dir_comp)?;
    cprint_normal!("Comparing JSON-schemas...");
    let changes = diff_schemas(&old, &new);
    cprint_normal!("Compared JSON-schemas.");
    write_changes(&changes, &a.output_file)?;
    cprint_normal!("Saved Diff to file: {}", a.output_file.display());
    Ok(())
}

fn run_matrix(a: &DiffMatrixArgs) -> Result<(), String> {
    let diffs = read_changes_from_diff_files(&a.input_diff_files)?;
    let chain = build_chain(diffs)?;
    let matrix = create_compatibility_matrix(&chain, a.use_emotes);
    let path: Vec<String> = chain.nodes.iter().map(|n| n.version_key.clone()).collect();
    match a.output_type {
        MatrixOutputType::Csv  => write_compatibility_matrix_csv(&a.output_file, &matrix, &path)?,
        MatrixOutputType::Json => write_compatibility_matrix_json(&a.output_file, &matrix)?,
    }
    cprint_normal!("Saved compatibility matrix to: {}", a.output_file.display());
    Ok(())
}

fn run_version_bump(a: &VersionBumpArgs) -> Result<(), String> {
    let mut diffs = read_changes_from_diff_files(std::slice::from_ref(&a.diff_file))?;
    let changes = diffs.pop().ok_or("Empty diff file list")?;
    let kind = check_version_bump(&changes, a.major_bump_allowed)?;
    cprint_normal!("Valid {:?} version bump.", kind);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{Console, Level, CONSOLE};
    use crate::models::changes::Changes;
    use crate::models::schema_meta::Schemas;
    use crate::models::version::DirtyVersion;
    use std::fs;

    fn ensure_console() {
        let _ = CONSOLE.set(Console::new(Level::Normal));
    }

    /// Build a minimal `Changes` and write it as a diff file. Useful for matrix and version-bump runners.
    fn write_diff(path: &std::path::Path, old_v: &str, new_v: &str) -> Changes {
        let v_old: DirtyVersion = old_v.parse().unwrap();
        let v_new: DirtyVersion = new_v.parse().unwrap();
        let c = Changes {
            old_schemas: Schemas::new(v_old),
            new_schemas: Schemas::new(v_new),
            changes: vec![],
        };
        write_changes(&c, path).unwrap();
        c
    }

    #[test]
    fn test_run_version_bump_succeeds_on_valid_technical_bump() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("d.json");
        write_diff(&p, "v202401.0.1", "v202401.0.2");
        let args = VersionBumpArgs { diff_file: p, major_bump_allowed: true };
        run_version_bump(&args).unwrap();
    }

    #[test]
    fn test_run_version_bump_errors_on_dirty_baseline() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("d.json");
        write_diff(&p, "v202401.0.1+gabc", "v202401.0.2");
        let args = VersionBumpArgs { diff_file: p, major_bump_allowed: true };
        assert!(run_version_bump(&args).is_err());
    }

    #[test]
    fn test_run_matrix_writes_csv() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let in_path = dir.path().join("d.json");
        write_diff(&in_path, "v202401.0.1", "v202401.0.2");
        let out_path = dir.path().join("m.csv");
        let args = DiffMatrixArgs {
            input_diff_files: vec![in_path],
            output_file: out_path.clone(),
            output_type: MatrixOutputType::Csv,
            use_emotes: false,
        };
        run_matrix(&args).unwrap();
        assert!(out_path.exists());
        let text = fs::read_to_string(&out_path).unwrap();
        assert!(text.contains("v202401.0.1"));
    }
}
```

### Step 2: Wire registrations

In `src/cli.rs`, add:

```rust
pub mod diff;
```

In `src/cli/base.rs`, change the imports/enum to register the subcommand:

```rust
use crate::cli::diff::Diff;
use crate::cli::edit::Edit;
use crate::cli::pull::Pull;

// ...

#[derive(Subcommand)]
pub enum SubcommandsLevel1 {
    Pull(Pull),
    Edit(Edit),
    Diff(Diff),
}

impl Executable for SubcommandsLevel1 {
    fn run(&self) -> Result<(), String> {
        match self {
            SubcommandsLevel1::Pull(pull) => pull.run(),
            SubcommandsLevel1::Edit(edit) => edit.run(),
            SubcommandsLevel1::Diff(diff) => diff.run(),
        }
    }
}
```

### Step 3: Run tests

```
cargo test cli::diff::tests
```

Expected: 3 integration tests pass.

### Step 4: Final full-suite check

```
cargo test
cargo build
```

Expected: every test passes; binary builds cleanly. Try once manually:

```
cargo run -- diff --help
cargo run -- diff schemas --help
cargo run -- diff matrix --help
cargo run -- diff version-bump --help
```

Expected: all four help screens render.

### Step 5: Commit

```
git add src/cli/diff.rs src/cli.rs src/cli/base.rs
git commit -m "feat(cli/diff): wire diff schemas/matrix/version-bump subcommand group"
```

---

## Task 17: Integration smoke — end-to-end `diff schemas → diff matrix → diff version-bump`

**Files:**
- Modify: `src/cli/diff.rs` (extend integration tests)

### Step 1: Write failing end-to-end test

Append to `mod tests` in `src/cli/diff.rs`:

```rust
    use crate::io::schemas::write_schemas;
    use crate::models::schema_meta::Schema;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn write_minimal_schema_dir(dir: &std::path::Path, version: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(".version"), version).unwrap();
        let bo = dir.join("bo");
        std::fs::create_dir_all(&bo).unwrap();
        std::fs::write(
            bo.join("Angebot.json"),
            r#"{"type":"object","title":"Angebot","properties":{},"required":[],"additionalProperties":false}"#,
        ).unwrap();
    }

    #[test]
    fn test_end_to_end_schemas_then_matrix_then_version_bump() {
        ensure_console();
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base");
        let comp = dir.path().join("comp");
        write_minimal_schema_dir(&base, "v202401.0.1");
        write_minimal_schema_dir(&comp, "v202401.0.2");

        let diff_file = dir.path().join("diff.json");
        run_schemas(&DiffSchemasArgs {
            input_dir_base: base,
            input_dir_comp: comp,
            output_file: diff_file.clone(),
        }).unwrap();
        assert!(diff_file.exists());

        let matrix_file = dir.path().join("m.csv");
        run_matrix(&DiffMatrixArgs {
            input_diff_files: vec![diff_file.clone()],
            output_file: matrix_file.clone(),
            output_type: MatrixOutputType::Csv,
            use_emotes: false,
        }).unwrap();
        assert!(matrix_file.exists());

        run_version_bump(&VersionBumpArgs {
            diff_file,
            major_bump_allowed: true,
        }).unwrap();
    }
```

### Step 2: Run

```
cargo test cli::diff::tests::test_end_to_end_schemas_then_matrix_then_version_bump
```

Expected: passes (no production code changes needed; this is exclusively a smoke test).

### Step 3: Final repository test pass

```
cargo test
```

Expected: every test in the crate passes.

### Step 4: Commit

```
git add src/cli/diff.rs
git commit -m "test(cli/diff): end-to-end schemas → matrix → version-bump smoke"
```

---

## Self-Review Checklist (already applied)

**Spec coverage:**

| Spec section | Task |
|---|---|
| `models/version.rs` visibility + accessor | Task 2 |
| `Version` ↔ `DirtyVersion` cross-type ord | Task 3 |
| `models/matrix.rs` enum split + IndexMap | Task 4 |
| `Console` Level redesign | Task 5 |
| `cprint!` macros + 3 wrappers | Task 6 |
| Migrate existing call sites | Task 7 |
| `--quiet` global flag + `main` resolution | Task 8 |
| `Schemas::module_difference / module_intersection` | Task 9 |
| `io/changes.rs` | Task 10 |
| `io/matrix.rs` | Task 11 |
| `diff.rs` module root + `diff/filters.rs` | Task 12 |
| `diff/diff.rs` schema comparison | Task 13 |
| `diff/matrix.rs` chain + matrix | Task 14 |
| `diff/version.rs` `check_version_bump` | Task 15 |
| `cli/diff.rs` subcommand group | Task 16 |
| End-to-end smoke | Task 17 |
| Add `csv`, `indexmap` deps | Task 1 |

**Type consistency:** `VariantKind` defined once in Task 13. `Sym` enum is a local helper in Task 14, deliberately distinct from public `CompatibilitySymbol`. `VersionBumpKind` defined in Task 15. `MatrixOutputType` and `Diff*Args` defined once in Task 16. `Level` enum defined in Task 5, used in Tasks 6–8.

**Implementer caveats inline (verify against current source):**
- `SchemaRootObject` / `SchemaRootStrEnum` field names (Task 13, Steps 1 & 3) — confirm from `models/json_schema.rs`.
- `SchemaType` variant names (`Object`, `StrEnum`, `Array`, `AnyOf`, `AllOf`, `StringSchema`, `ReferenceSchema`, `ConstantSchema`, `NumberSchema`, …) (Task 13, Step 9) — confirm from `models/json_schema.rs`.
- `StringSchemaFormat::DateTime` variant name (Task 13, Step 7) — confirm from `models/json_schema.rs`.
- `HashSet<&Vec<String>>::contains(&Vec<String>)` borrow shape in Task 9, Step 3 — fall back to `iter().any()` if the borrow check rejects.

These are explicit "look at the current file" notes, not placeholders — the surrounding code is fully specified.
