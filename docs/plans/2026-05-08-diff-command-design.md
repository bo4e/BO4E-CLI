# Diff Command Design

**Date:** 2026-05-08
**Branch:** `rust`
**Scope:** `src/diff/`, `src/io/{changes,matrix}.rs`, `src/cli/diff.rs`, `src/models/{matrix,version}.rs`, `src/console/console.rs`

---

## Goal

Implement the `bo4e diff` subcommand group in Rust, replicating the Python implementation on `main`. Three sub-subcommands: `diff schemas`, `diff matrix`, `diff version-bump`. All Python behaviour and JSON output formats are preserved bit-for-bit so existing diff files remain consumable.

Out of scope for this design (tracked separately): `bo4e generate`, `bo4e repo versions`, shell autocompletion.

---

## Architectural Notes

**No Python-style generators.** Python's `_diff_*` functions return `Iterable[Change]` via `yield`. Stable Rust does not have generators, so the diff functions take a `&mut Vec<Change>` (the "collector") and push into it. Each `yield change` becomes `out.push(change)`; each `yield from sub_iter` becomes a recursive call passing the same collector. The two places where Python materializes via `list(...)` to inspect before yielding (`_diff_schema_differing_types`, `_diff_any_of_or_all_of_schemas`) become recursive calls that return a fresh `Vec<Change>` for inspection.

**Mirror the Python module layout 1:1.** Matches the `pull` and `edit` precedent. Smallest cognitive distance from the Python source map.

**No graph library.** The Python `diff/matrix.py` uses `networkx` to validate the diff files form a single linear chain. Each version key has at most one outgoing and one incoming edge — the structure is a linked list, not a general graph. Hand-roll the validation in ~50 LOC.

---

## Module Map

```
src/cli/diff.rs           – clap subcommand group
src/diff.rs               – module root: pub mod {diff, filters, matrix, version}
src/diff/diff.rs          – schema comparison (port of diff/diff.py)
src/diff/filters.rs       – is_change_critical, has_critical
src/diff/matrix.rs        – linear-chain validation + matrix generation
src/diff/version.rs       – check_version_bump
src/io/changes.rs         – read/write diff JSON files
src/io/matrix.rs          – write CSV/JSON for compatibility matrix
```

Registrations:

- `src/cli.rs` — add `pub mod diff;`
- `src/cli/base.rs` — add `Diff(Diff)` variant to `SubcommandsLevel1`
- `src/io.rs` — add `pub mod changes; pub mod matrix;`
- `src/diff.rs` (new) — module root re-exporting four submodules
- `src/main.rs` — add `mod diff;`

---

## Model Adjustments

### `src/models/matrix.rs`

The current Rust enum `CompatibilitySymbol` flattens emoji and text variants (`Unchanged` / `ReprUnchanged` / …). The Python has two parallel `StrEnum`s (`CompatibilitySymbol` for emoji, `CompatibilityText` for text) selected at build time via a `use_emotes: bool`. Replace the flat enum with two parallel enums and a unifying tagged container:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilitySymbol {
    ChangeNone,         // 🟢
    ChangeNonCritical,  // 🟡
    ChangeCritical,     // 🔴
    NonExistent,        // -
    Added,              // ➕
    Removed,            // ➖
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityText {
    ChangeNone,         // "none"
    ChangeNonCritical,  // "non-critical"
    ChangeCritical,     // "critical"
    NonExistent,        // "-"
    Added,              // "added"
    Removed,            // "removed"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compatibility {
    Symbol(CompatibilitySymbol),
    Text(CompatibilityText),
}
```

Implement `Display`, `Serialize`, `Deserialize` for both inner enums via single string-roundtrip lookup tables (the existing `bimap`-based pattern). `Compatibility` `(de)serialize`s untagged: try emoji set first, fall back to text set.

The current `BiMap` collapses both sets and produces lossy serialization (writing emojis but parsing both). The new shape preserves the `use_emotes` choice through round-trips.

`CompatibilityMatrix.root` changes from `HashMap<String, Vec<…>>` to `IndexMap<String, Vec<…>>` so insertion order survives serialization (modules are inserted sorted by lowercased path tuple, matching Python's `sorted(...)` order).

New dependency: `indexmap = { version = "2", features = ["serde"] }`.

### `src/models/version.rs`

Visibility:

- `bumped_major`, `bumped_functional`, `bumped_technical`, `bumped_candidate`, `is_release_candidate` → `pub`.
- `is_dirty` → `pub`.

New accessor on `DirtyVersion`:

```rust
impl DirtyVersion {
    /// Borrow the semantic version, discarding dirt metadata.
    pub fn version(&self) -> &Version { &self.version }
}
```

Cross-type comparison `Version` ↔ `DirtyVersion` (deliberately *not* `DirtyVersion` ↔ `DirtyVersion`, which would be ill-defined: comparing two untagged commits requires consulting git history):

```rust
impl PartialEq<DirtyVersion> for Version {
    fn eq(&self, other: &DirtyVersion) -> bool {
        *self == other.version && !other.is_dirty()
    }
}
impl PartialOrd<DirtyVersion> for Version {
    fn partial_cmp(&self, other: &DirtyVersion) -> Option<Ordering> {
        match self.cmp(&other.version) {
            Ordering::Equal if other.is_dirty() => Some(Ordering::Less),
            ord => Some(ord),
        }
    }
}
impl PartialEq<Version>  for DirtyVersion { fn eq(&self, o: &Version) -> bool { o == self } }
impl PartialOrd<Version> for DirtyVersion {
    fn partial_cmp(&self, o: &Version) -> Option<Ordering> { o.partial_cmp(self).map(Ordering::reverse) }
}
```

Semantics: at equal semantic version, a dirty version sorts *strictly newer* than a clean one. These impls are added for completeness; `check_version_bump` itself does not use them (see below).

### `src/console/console.rs`

The three modes (quiet, normal, verbose) form a logging-level system. Each message has a level, the console has a level, and a message is emitted iff `message_level <= console_level`:

| Console | Message | Emitted |
|---|---|---|
| Quiet   | Quiet   | yes |
| Quiet   | Normal  | no  |
| Quiet   | Verbose | no  |
| Normal  | Quiet   | yes |
| Normal  | Normal  | yes |
| Normal  | Verbose | no  |
| Verbose | Quiet   | yes |
| Verbose | Normal  | yes |
| Verbose | Verbose | yes |

```rust
/// Importance of a console message. Lower discriminants are more important —
/// a message is emitted iff its level is `<=` the console's level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    pub fn new(level: Level) -> Self;
    /// Emit `msg` iff `level <= self.level`.
    pub fn print(&self, level: Level, msg: &str);
}
```

Single macro replaces both existing ones:

```rust
#[macro_export]
macro_rules! cprint {
    ($level:expr, $($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized")
            .print($level, &format!($($arg)*))
    };
}
```

Call sites:

```rust
cprint!(Level::Quiet,   "Critical surfacing info");   // was print_force
cprint!(Level::Normal,  "Done.");                      // was cprint!
cprint!(Level::Verbose, "Inspecting field {}", name);  // was cprint_verbose!
```

Migration: existing `cprint!(...)` invocations become `cprint!(Level::Normal, ...)`, existing `cprint_verbose!(...)` become `cprint!(Level::Verbose, ...)`. The old `cprint_verbose!` macro is removed. Touched files outside diff scope: `src/cli/{edit,pull}.rs`, `src/edit/{add,non_nullable,update_refs}.rs` — straight find-and-replace.

CLI: a global `--quiet`/`-q` flag is declared on the root `Cli` struct alongside `--verbose`/`-v`, with `conflicts_with = "verbose"`. `main()` resolves the pair into a single `Level` (`(verbose, quiet) ↦ Level::Verbose | Level::Quiet | Level::Normal`) and passes it to `Console::new`.

---

## `src/io/changes.rs`

```rust
pub fn read_changes_from_diff_files(paths: &[PathBuf]) -> Result<Vec<Changes>, String>;
pub fn write_changes(changes: &Changes, file_path: &Path) -> Result<(), String>;
```

`read_changes_from_diff_files`:

1. For each path: error if missing.
2. Read UTF-8 file content.
3. `serde_json::from_str` into `Changes`.
4. Collect all into `Vec<Changes>` (Python returns an iterator; Rust caller wants ownership of all parsed items, so collect eagerly — call sites are the matrix command which needs to inspect them all anyway).

`write_changes`:

1. Create parent directories.
2. Serialize with `serde_json::to_string_pretty` (indent 2, matching Python `model_dump_json(indent=2)`).
3. Write UTF-8.

---

## `src/io/matrix.rs`

```rust
pub fn write_compatibility_matrix_csv(
    output: &Path,
    matrix: &CompatibilityMatrix,
    versions: &[String],
) -> Result<(), String>;

pub fn write_compatibility_matrix_json(
    output: &Path,
    matrix: &CompatibilityMatrix,
) -> Result<(), String>;
```

CSV uses delimiter `,`, line terminator `\n`, escape char `/`. Header: `("", "{v0} ↦ {v1}", "↦ {v2}", "↦ {v3}", …)`. Rows are `(module, …entries.compatibility.to_string())`.

New dependency: `csv = "1"`.

JSON output is `serde_json::to_string_pretty` of the matrix.

---

## `src/diff/diff.rs`

Public surface — single function:

```rust
pub fn diff_schemas(old: &Schemas, new: &Schemas) -> Changes;
```

Implementation skeleton (collector pattern, all helpers `pub(super)` so siblings can unit-test them via `super::`):

```rust
fn diff_type_base(
    old: &TypeBase, new: &TypeBase,
    old_trace: &str, new_trace: &str,
    out: &mut Vec<Change>,
);

fn diff_enum_schemas(
    old: &StrEnumSchema, new: &StrEnumSchema,
    old_trace: &str, new_trace: &str,
    out: &mut Vec<Change>,
);

fn diff_object_schemas(
    old: &ObjectSchema, new: &ObjectSchema,
    old_trace: &str, new_trace: &str,
    out: &mut Vec<Change>,
);

fn diff_ref_schemas(...);
fn diff_array_schemas(...);
fn diff_string_schemas(...);

fn diff_any_of_schemas(
    old: &AnyOfSchema, new: &AnyOfSchema,
    old_trace: &str, new_trace: &str,
    out: &mut Vec<Change>,
);

fn diff_all_of_schemas(
    old: &AllOfSchema, new: &AllOfSchema,
    old_trace: &str, new_trace: &str,
    out: &mut Vec<Change>,
);

// Shared internal helper called by the two above. The `kind` discriminant
// chooses which Change variants (FieldAnyOf*Type* vs FieldAllOf*Type*) to emit
// and which JSON key (`any_of` vs `all_of`) to embed in the trace strings.
fn diff_variant_list(
    old: &[SchemaType], new: &[SchemaType],
    old_trace: &str, new_trace: &str,
    kind: VariantKind,
    out: &mut Vec<Change>,
);

enum VariantKind { AnyOf, AllOf }

fn diff_schema_differing_types(...) -> ();   // pushes into out

fn diff_schema_type(
    old: &SchemaType, new: &SchemaType,
    old_trace: &str, new_trace: &str,
    out: &mut Vec<Change>,
);

fn diff_root_schemas(...);
```

Two helpers materialize a local `Vec<Change>` (Python `list(...)`) and inspect before extending the outer `out`:

- `diff_schema_differing_types` for the Object↔Array cardinality probe.
- `diff_variant_list` for variant pairing — for each (old_variant, new_variant) pair, recursively diff into a fresh `Vec<Change>`. If `has_critical(&sub) == false` (no critical sub-changes), the variants are deemed equal — extend `out` with the (non-critical) sub-changes and mark the new variant matched. Otherwise treat the old variant as removed. After all old variants are processed, any unmatched new variant indices are emitted as `*TypeAdded`.

Key detail from Python: `_diff_type_base` ignores `description` differences that disappear after substituting `{__gh_version__}` for the `REGEX_VERSION` pattern — needed because version strings appear in autogenerated docstrings. Rust uses `regex::Regex::replace_all` with the same pattern.

Another detail: `_diff_type_base` skips `default` changes when the title is `" Version"` (with leading space) on either side — autogenerated `_version` defaults change with each release.

Trace strings use plain `String` formatting with `/` separators, mirroring `PurePosixPath(*module)`. No `Path`/`PathBuf` because forward-slash semantics are wanted regardless of OS.

`Schemas` set ops needed (added in the same change to `models/schema_meta.rs`). The crate stores schemas as `Rc<RefCell<Schema>>`, so the iterators yield those wrappers and call sites `.borrow()` to access the inner schema:

```rust
impl Schemas {
    /// Schemas in `self` whose module is not present in `other`.
    pub fn module_difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a Rc<RefCell<Schema>>>;

    /// Schemas whose module is present in both `self` and `other` (returning self's value).
    pub fn module_intersection<'a>(
        &'a self,
        other: &'a Self,
    ) -> impl Iterator<Item = &'a Rc<RefCell<Schema>>>;
}
```

Both implementations use the existing `modules()` set membership for filtering; no new index needed.

---

## `src/diff/filters.rs`

```rust
pub fn is_change_critical(change: &Change) -> bool;

/// Returns true if any change in the iterator is critical.
pub fn has_critical<'a, I: IntoIterator<Item = &'a Change>>(changes: I) -> bool;
```

`is_change_critical` matches Python's exact set:

```
FieldRemoved, FieldTypeChanged, FieldCardinalityChanged,
FieldReferenceChanged, FieldStringFormatChanged,
FieldAnyOfTypeAdded, FieldAnyOfTypeRemoved,
FieldAllOfTypeAdded, FieldAllOfTypeRemoved,
ClassRemoved, EnumValueRemoved
```

`has_critical` replaces Python's `any(filter_non_crit(...))` (the Python name is misleading: `filter_non_crit` keeps critical changes).

---

## `src/diff/matrix.rs`

```rust
pub struct VersionChain {
    pub nodes: Vec<ChainNode>,   // ordered start → end, length n+1
    pub edges: Vec<ChainEdge>,   // edges[i] connects nodes[i] → nodes[i+1], length n
}
pub struct ChainNode {
    pub version_key: String,     // e.g. "v202401.0.1"
    pub schemas: Schemas,
}
pub struct ChainEdge {
    pub changes: Changes,
}

pub fn build_chain(diffs: Vec<Changes>) -> Result<VersionChain, String>;

pub fn create_compatibility_matrix(
    chain: &VersionChain,
    use_emotes: bool,
) -> CompatibilityMatrix;
```

`build_chain`:

1. For each `Changes`: derive `(old_key, new_key) = (changes.old_version().to_string(), changes.new_version().to_string())`.
2. Maintain `nodes: HashMap<String, Schemas>`. On insert, if the key already exists with different `Schemas`, error: `"Node {key} already exists with different attributes"`.
3. Maintain `out: HashMap<String, usize>` (key → diff index providing the outgoing edge) and `in_set: HashSet<String>`. Reject duplicate outgoing or duplicate incoming → error.
4. `start = key in nodes \ in_set`; require exactly one. Likewise `end = key in nodes \ keys(out)`; require exactly one.
5. Walk: `cursor = start`; collect `nodes_ordered` and `edges_ordered`. For each step: `next_idx = out[cursor]`; push `diffs[next_idx]` as edge, advance cursor to `diffs[next_idx].new_version().to_string()`. Stop when cursor = end.
6. Validate `edges_ordered.len() == diffs.len()` (otherwise the input was disconnected — extra edges remain unvisited) → error.

`create_compatibility_matrix`:

1. Collect all module paths across all `chain.nodes[*].schemas` into `BTreeSet<Vec<String>>`.
2. Convert to `Vec<Vec<String>>` and sort by `module.iter().map(|p| p.to_lowercase()).collect::<Vec<_>>()` (matches Python's tuple-of-lowercased-parts sort key).
3. For each module:
   - `let class_path_str = format!("/{}#", module.join("/"));`
   - For each consecutive `(node_a, edge, node_b)`:
     - Filter `edge.changes.changes` by `c.old_trace.starts_with(&class_path_str) || c.new_trace.starts_with(&class_path_str)`.
     - Compute `compatibility = determine_symbol(&filtered, &node_b.schemas, &module, use_emotes)`.
     - Push entry with `previous_version = node_a.schemas.version.clone()`, `next_version = node_b.schemas.version.clone()`, `compatibility`.
   - Insert `(module.join("."), entries)` into the `IndexMap`.

`determine_symbol`:

- 1 change of `ClassRemoved` → `Removed`
- 1 change of `ClassAdded` → `Added`
- module not in `node_b.schemas.modules` → `NonExistent`
- 0 changes → `ChangeNone`
- `has_critical(&changes)` → `ChangeCritical`
- otherwise → `ChangeNonCritical`

The `assert all(...)` from Python (no `CLASS_*` in non-trivial change list) becomes a `debug_assert!` — invariant maintained by `diff_schemas`, kept in tests but not in release.

---

## `src/diff/version.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionBumpKind {
    Technical,
    Functional,
    Major,
}

pub fn check_version_bump(
    changes: &Changes,
    major_bump_allowed: bool,
) -> Result<VersionBumpKind, String>;
```

Both versions must be clean (`Version`, not `DirtyVersion`) — a dirty diff file is not a valid baseline for bump validation. Conversion via existing `TryFrom<&DirtyVersion> for Version`:

```rust
let v_old: Version = changes.old_version().try_into()
    .map_err(|e: String| format!("Old version of diff is dirty and cannot serve as a baseline: {e}"))?;
let v_new: Version = changes.new_version().try_into()
    .map_err(|e: String| format!("New version of diff is dirty and cannot be validated: {e}"))?;

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
if !functional && v_new.bumped_functional(&v_old) {
    return Err("Functional bump detected but no functional changes found.".into());
}
if functional && !v_new.bumped_functional(&v_old) {
    return Err("Technical bump detected but functional changes found.".into());
}
Ok(if functional { VersionBumpKind::Functional } else { VersionBumpKind::Technical })
```

Verbose-only side effects: `cprint!(Level::Verbose, ...)` the diff JSON (without `old_schemas`/`new_schemas`) and the "Functional / Technical release bump is needed" line, before the bump-direction checks.

---

## `src/cli/diff.rs`

```rust
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
    pub input_dir_base: PathBuf,
    pub input_dir_comp: PathBuf,
    #[arg(short = 'o', long = "output", required = true)]
    pub output_file: PathBuf,
}

#[derive(Args)]
pub struct DiffMatrixArgs {
    /// One or more diff JSON files. Order does not matter.
    #[arg(required = true)]
    pub input_diff_files: Vec<PathBuf>,
    #[arg(short = 'o', long = "output", required = true)]
    pub output_file: PathBuf,
    #[arg(short = 't', long = "output-type", default_value = "csv")]
    pub output_type: MatrixOutputType,
    #[arg(long = "use-emotes", default_value_t = false)]
    pub use_emotes: bool,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum MatrixOutputType { Json, Csv }

#[derive(Args)]
pub struct VersionBumpArgs {
    pub diff_file: PathBuf,
    /// Reject major version bumps.
    #[arg(long = "no-major", action = clap::ArgAction::SetFalse)]
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
```

`run_schemas`:

1. `let old = read_schemas(&a.input_dir_base)?;`
2. `let new = read_schemas(&a.input_dir_comp)?;`
3. `cprint!(Level::Normal, "Comparing JSON-schemas...");`
4. `let changes = diff_schemas(&old, &new);`
5. `cprint!(Level::Normal, "Compared JSON-schemas.");`
6. `write_changes(&changes, &a.output_file)?;`
7. `cprint!(Level::Normal, "Saved Diff to file: {}", a.output_file.display());`

`run_matrix`:

1. `let diffs = read_changes_from_diff_files(&a.input_diff_files)?;`
2. `let chain = build_chain(diffs)?;`
3. `let matrix = create_compatibility_matrix(&chain, a.use_emotes);`
4. `let path: Vec<String> = chain.nodes.iter().map(|n| n.version_key.clone()).collect();`
5. Match on `output_type`: CSV → `write_compatibility_matrix_csv(&a.output_file, &matrix, &path)`; JSON → `write_compatibility_matrix_json(&a.output_file, &matrix)`.

`run_version_bump`:

1. `let mut diffs = read_changes_from_diff_files(std::slice::from_ref(&a.diff_file))?;`
2. `let changes = diffs.pop().ok_or("Empty diff file list")?;`
3. `let kind = check_version_bump(&changes, a.major_bump_allowed)?;`
4. `cprint!(Level::Normal, "Valid {:?} version bump.", kind);`

`--quiet` and `--verbose` are global flags on `Cli`, mutually exclusive, and resolved into a single `Level` passed into `Console::new` from `main`. The run functions don't read them directly.

---

## Error Handling

All `Result` types use `String` errors, matching the existing crate convention. `main()` already converts a non-zero `Result<(), String>` into a non-zero exit code with the message printed.

`diff/diff.rs` and `diff/filters.rs` are pure logic and never error. Everything else (`io/`, `diff/matrix.rs::build_chain`, `diff/version.rs`, CLI runners) returns `Result<_, String>`.

---

## Tests

Following the established TDD convention: tests live in the same file under `#[cfg(test)]`, drive each task with a failing test first.

| Module | Tests |
|---|---|
| `models/version.rs` | `version_eq_dirty_when_clean_and_equal_semantically`; `version_lt_dirty_at_same_semantic_version`; `version_eq_clean_dirty`; `dirty_gt_clean_at_same_semantic_version`; `dirty_lt_clean_when_older` |
| `models/matrix.rs` | `roundtrip_compatibility_symbol_emoji`; `roundtrip_compatibility_text`; `compatibility_serializes_emoji_first_then_text` |
| `console/console.rs` | Full 3×3 emission table: for each `(console_level, message_level)` pair assert that emission matches the table above |
| `io/changes.rs` | `roundtrip_write_then_read_preserves_changes`; `read_missing_file_errors` |
| `io/matrix.rs` | `csv_golden_3_versions_3_modules`; `json_roundtrip_preserves_module_order` |
| `diff/filters.rs` | `is_change_critical_full_table` (one expected boolean per `ChangeType` variant); `has_critical_finds_one_in_mixed`; `has_critical_returns_false_for_only_non_critical` |
| `diff/diff.rs` | `class_added_class_removed`; `field_added_field_removed`; `field_default_changed`; `description_change_with_version_substitution_is_ignored`; `version_field_default_change_is_ignored`; `field_title_changed`; `field_type_changed_unrelated_types`; `field_cardinality_changed_object_to_array`; `field_string_format_changed`; `field_reference_changed`; `enum_value_added_removed`; `any_of_variant_added_removed`; `any_of_pairs_with_non_critical_inner_change`; `all_of_variant_added` |
| `diff/matrix.rs` | `build_chain_orders_three_unsorted_diffs`; `build_chain_rejects_two_starts`; `build_chain_rejects_two_ends`; `build_chain_rejects_disconnected_input`; `build_chain_rejects_duplicate_edge`; `build_chain_rejects_node_attribute_mismatch`; `create_compatibility_matrix_emits_removed_added_existent_unchanged_critical_noncritical`; `module_order_in_matrix_is_lowercased_sorted` |
| `diff/version.rs` | `errors_when_old_version_is_dirty`; `errors_when_new_version_is_dirty`; `errors_when_new_not_newer_than_old`; `major_bump_allowed_returns_major`; `major_bump_disallowed_returns_err`; `functional_bump_no_changes_errors`; `technical_bump_with_changes_errors`; `valid_technical_bump_returns_technical`; `valid_functional_bump_returns_functional` |
| `cli/diff.rs` | `integration_diff_schemas_writes_expected_changes`; `integration_diff_matrix_csv_writes_expected_table`; `integration_diff_version_bump_succeeds_on_valid_bump`; `integration_diff_version_bump_errors_on_dirty_baseline` |

Test fixtures are built in code (no JSON golden files outside the test source) — each test constructs a minimal `Schemas` / `Changes` value inline.

---

## New Dependencies

| Crate | Purpose |
|---|---|
| `csv = "1"` | CSV writing for `diff matrix` |
| `indexmap = { version = "2", features = ["serde"] }` | Order-preserving map for `CompatibilityMatrix` |

No removed dependencies.
