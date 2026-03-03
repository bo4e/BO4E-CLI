# Edit Command Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the `edit` subcommand in Rust, replicating all behaviour of the Python `edit` command on `main`.

**Architecture:** Approach A — one Rust module per transformation, mirroring the Python layout. A `OnceLock<Console>` singleton owns verbose state and the regex-based highlighter; `cprint!` / `cprint_verbose!` macros are the only call-site API. The `edit` pipeline runs fully synchronously (no Tokio).

**Tech Stack:** `clap` (CLI), `serde` / `serde_json` (JSON), `regex` / `serde_regex` (pattern matching), `walkdir` (directory traversal), `console` crate (ANSI styles), `std::sync::{OnceLock, RwLock}` (global singleton).

**Design doc:** `docs/plans/2026-03-03-edit-command-design.md`

---

## Task 1: `PrimitiveValue` enum + `TypeBase.default`

**Files:**
- Modify: `src/models/json_schema.rs`

### Step 1: Write failing test

Add inside the existing `#[cfg(test)]` block in `src/models/json_schema.rs`:

```rust
#[test]
fn test_primitive_value_roundtrip() {
    use serde_json;
    let cases: &[(&str, PrimitiveValue)] = &[
        ("null", PrimitiveValue::Null),
        ("true", PrimitiveValue::Bool(true)),
        ("42", PrimitiveValue::Integer(42)),
        ("3.14", PrimitiveValue::Float(3.14)),
        ("\"hello\"", PrimitiveValue::String("hello".into())),
    ];
    for (json, expected) in cases {
        let v: PrimitiveValue = serde_json::from_str(json).unwrap();
        assert_eq!(v, expected);
        let back = serde_json::to_string(&expected).unwrap();
        assert_eq!(back, *json);
    }
}

#[test]
fn test_typebase_default_absent_not_emitted() {
    let base = TypeBase { description: None, title: None, default: None };
    let json = serde_json::to_string(&base).unwrap();
    assert!(!json.contains("default"), "absent default must not appear in JSON");
}

#[test]
fn test_typebase_default_null_emitted() {
    let base = TypeBase { description: None, title: None, default: Some(PrimitiveValue::Null) };
    let json = serde_json::to_string(&base).unwrap();
    assert!(json.contains("\"default\":null"));
}

#[test]
fn test_typebase_default_string_roundtrip() {
    let base = TypeBase {
        description: None,
        title: None,
        default: Some(PrimitiveValue::String("v202401.1.0".into())),
    };
    let json = serde_json::to_string(&base).unwrap();
    let back: TypeBase = serde_json::from_str(&json).unwrap();
    assert_eq!(back.default, base.default);
}
```

### Step 2: Run to confirm compile error

```
cargo test test_primitive_value_roundtrip
```

Expected: compile error — `PrimitiveValue` not defined, `TypeBase` missing `default` field.

### Step 3: Add `PrimitiveValue` and extend `TypeBase`

In `src/models/json_schema.rs`, add **before** the `TypeBase` struct:

```rust
/// A primitive JSON value used for schema `default` fields.
/// Only null, bool, integer, float, and string are permitted.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum PrimitiveValue {
    Null,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
}
```

Extend `TypeBase`:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeBase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<PrimitiveValue>,
}
```

Remove `Eq` from `TypeBase` derive (since `PrimitiveValue` containing `f64` cannot be `Eq`).
Also remove `Eq` from any types that derive it through `TypeBase` if the compiler complains —
follow the error chain and drop `Eq` where needed (keep `PartialEq`).

### Step 4: Run tests

```
cargo test models::json_schema::tests
```

Expected: all existing tests still pass; four new tests pass.

### Step 5: Commit

```
git add src/models/json_schema.rs
git commit -m "feat(models): add PrimitiveValue enum and TypeBase.default field"
```

---

## Task 2: Fix `Config` to support `$ref` in `additional_fields` and `additional_models`

**Files:**
- Modify: `src/models/config.rs`

### Step 1: Write failing test

Add `#[cfg(test)]` block at the bottom of `src/models/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_deserializes_additional_field_ref() {
        let json = r#"{
            "additionalFields": [
                { "$ref": "./some_fields.json" }
            ]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.additional_fields.len(), 1);
        assert!(matches!(
            &config.additional_fields[0],
            AdditionalFieldOrRef::Reference(r) if r.path == "./some_fields.json"
        ));
    }

    #[test]
    fn test_config_deserializes_concrete_additional_field() {
        let json = r#"{
            "additionalFields": [
                {
                    "pattern": "bo\\..*",
                    "fieldName": "foo",
                    "fieldDef": { "type": "string" }
                }
            ]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(matches!(&config.additional_fields[0], AdditionalFieldOrRef::Field(_)));
    }
}
```

### Step 2: Run to confirm failure

```
cargo test models::config::tests
```

Expected: compile error — `AdditionalFieldOrRef` not defined.

### Step 3: Implement

Replace `src/models/config.rs` content:

```rust
use crate::models::json_schema::{SchemaRootObject, SchemaRootStrEnum, SchemaType};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// A `{ "$ref": "path" }` pointer used inside config fields.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SchemaRef {
    #[serde(rename = "$ref")]
    pub path: String,
}

/// An entry in `additionalFields` — either a concrete definition or a file reference.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum AdditionalFieldOrRef {
    Field(AdditionalField),
    Reference(SchemaRef),
}

/// A field that is added to the schema.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalField {
    #[serde(with = "serde_regex")]
    pub pattern: Regex,
    pub field_name: String,
    pub field_def: SchemaType,
}

impl PartialEq for AdditionalField {
    fn eq(&self, other: &Self) -> bool {
        self.pattern.as_str() == other.pattern.as_str()
            && self.field_name == other.field_name
            && self.field_def == other.field_def
    }
}
impl Eq for AdditionalField {}

/// An enum item that is added to the schema.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalEnumItem {
    #[serde(with = "serde_regex")]
    pub pattern: Regex,
    pub items: Vec<String>,
}

impl PartialEq for AdditionalEnumItem {
    fn eq(&self, other: &Self) -> bool {
        self.pattern.as_str() == other.pattern.as_str() && self.items == other.items
    }
}
impl Eq for AdditionalEnumItem {}

/// The inline schema or a file reference for an additional model.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum SchemaRootTypeOrReference {
    SchemaRootObject(SchemaRootObject),
    SchemaRootStrEnum(SchemaRootStrEnum),
    Reference(SchemaRef),
}

/// A model that is added to the schema.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdditionalModel {
    pub module: String, // "bo", "com", or "enum"
    pub schema: SchemaRootTypeOrReference,
}

/// The config file model.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default, with = "serde_regex")]
    pub non_nullable_fields: Vec<Regex>,
    #[serde(default)]
    pub additional_fields: Vec<AdditionalFieldOrRef>,
    #[serde(default)]
    pub additional_enum_items: Vec<AdditionalEnumItem>,
    #[serde(default)]
    pub additional_models: Vec<AdditionalModel>,
}
```

### Step 4: Run tests

```
cargo test models::config::tests
cargo test models::json_schema::tests
```

Expected: all pass. Fix any compile errors from removed `Eq` on `SchemaRootObject` etc.
caused by `PrimitiveValue` containing `f64` — drop `Eq` derive and implement `PartialEq` manually
where necessary, or use `#[allow(clippy::derive_partial_eq_without_eq)]`.

### Step 5: Commit

```
git add src/models/config.rs
git commit -m "feat(models): add AdditionalFieldOrRef and SchemaRef for config $ref support"
```

---

## Task 3: Global `Console` singleton + `cprint!` macros

**Files:**
- Modify: `src/console/console.rs`
- Modify: `src/console.rs` (make submodules public, add macro re-exports)
- Modify: `src/cli/base.rs` (add `--verbose` global arg)
- Modify: `src/main.rs` (initialize CONSOLE before dispatch)

### Step 1: Implement `Console` in `src/console/console.rs`

Read the existing file first (currently near-empty). Replace its content:

```rust
use crate::console::highlighter::Highlighter;
use std::sync::{OnceLock, RwLock};

pub static CONSOLE: OnceLock<Console> = OnceLock::new();

pub struct Console {
    pub verbose: bool,
    highlighter: RwLock<Highlighter>,
}

impl Console {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            highlighter: RwLock::new(Highlighter::default()),
        }
    }

    /// Print a message (always shown), applying the highlighter.
    pub fn print(&self, msg: &str) {
        let highlighted = self.highlighter.read().unwrap().apply(msg);
        eprintln!("{}", highlighted);
    }

    /// Print a message only when verbose mode is enabled.
    pub fn print_verbose(&self, msg: &str) {
        if self.verbose {
            self.print(msg);
        }
    }

    /// Register schema names for dynamic highlighting (called once after read_schemas).
    pub fn add_schema_names(&self, names: &[String]) {
        self.highlighter.write().unwrap().add_schema_names(names);
    }
}
```

### Step 2: Stub out `Highlighter::default()` and `Highlighter::apply()`

In `src/console/highlighter.rs`, replace the existing WIP content with a minimal but correct
implementation that compiles. Full regex patterns come in Task 4; for now:

```rust
use console::Style;
use regex::Regex;

pub struct Highlighter {
    rules: Vec<(Regex, Style)>,
}

impl Default for Highlighter {
    fn default() -> Self {
        Self { rules: Vec::new() }
    }
}

impl Highlighter {
    pub fn apply(&self, text: &str) -> String {
        // Placeholder: return text unchanged until Task 4 fills the rules.
        text.to_string()
    }

    pub fn add_schema_names(&mut self, names: &[String]) {
        // Placeholder: implemented in Task 4.
        let _ = names;
    }
}
```

### Step 3: Add `cprint!` and `cprint_verbose!` macros

Add to `src/console.rs` (keeping existing module declarations, making console public):

```rust
pub mod console;
pub mod highlighter;
pub mod palette;
pub mod progress_bar;

/// Print a formatted message through the global CONSOLE (always shown).
#[macro_export]
macro_rules! cprint {
    ($($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print(&format!($($arg)*))
    };
}

/// Print a formatted message only when verbose mode is active.
#[macro_export]
macro_rules! cprint_verbose {
    ($($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized — call CONSOLE.set() in main() before dispatch")
            .print_verbose(&format!($($arg)*))
    };
}
```

### Step 4: Add `--verbose` global flag to `Cli` and initialize `CONSOLE` in `main`

In `src/cli/base.rs`, add the verbose field to `Cli`:

```rust
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Enable verbose output.
    #[arg(global = true, short = 'v', long, default_value_t = false)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<SubcommandsLevel1>,
}
```

In `src/main.rs`:

```rust
use crate::cli::base::Executable;
use crate::console::console::{Console, CONSOLE};
mod cli;
mod console;
mod edit;
mod io;
mod models;
mod utils;

use clap::Parser;

fn main() -> Result<(), String> {
    let cli = cli::base::Cli::parse();
    CONSOLE
        .set(Console::new(cli.verbose))
        .map_err(|_| "CONSOLE already initialized".to_string())?;
    cli.run()
}
```

### Step 5: Verify it compiles and the macro works in `pull.rs`

Add a temporary `cprint!("Pull started");` line at the top of `Pull::run()` in
`src/cli/pull.rs`, then run:

```
cargo build
```

Expected: compiles cleanly. Remove the temporary line after confirming.

### Step 6: Commit

```
git add src/console/console.rs src/console/highlighter.rs src/console.rs src/cli/base.rs src/main.rs
git commit -m "feat(console): add global Console singleton and cprint!/cprint_verbose! macros"
```

---

## Task 4: Complete the `Highlighter` with BO4E regex patterns

**Files:**
- Modify: `src/console/highlighter.rs`

The Python `BO4EHighlighter` applies patterns in priority order. Replicate them here using the
`console` crate's `Style` for ANSI output. Colors are defined in `src/console/palette.rs`.

### Step 1: Write a test

Add `#[cfg(test)]` block in `src/console/highlighter.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_bo4e_keyword() {
        let h = Highlighter::default();
        let result = h.apply("Processing BO4E schema");
        // The word BO4E should be wrapped in ANSI escape codes
        assert!(result.contains("BO4E"), "original text preserved");
        assert!(result.contains('\x1b'), "ANSI codes present");
    }

    #[test]
    fn test_highlight_version() {
        let h = Highlighter::default();
        let result = h.apply("Version v202401.1.0-rc1 found");
        assert!(result.contains('\x1b'), "version should be highlighted");
    }

    #[test]
    fn test_add_schema_names_highlighted() {
        let mut h = Highlighter::default();
        h.add_schema_names(&["Angebot".to_string()]);
        let result = h.apply("Processing Angebot schema");
        assert!(result.contains('\x1b'), "schema name should be highlighted");
    }

    #[test]
    fn test_plain_text_unchanged_structure() {
        let h = Highlighter::default();
        let result = h.apply("no special content");
        // No ANSI sequences — text is returned as-is
        assert_eq!(result, "no special content");
    }
}
```

### Step 2: Run to confirm current failure

```
cargo test console::highlighter::tests
```

Expected: `test_highlight_bo4e_keyword` and `test_highlight_version` fail (no ANSI codes yet).

### Step 3: Implement `Highlighter` with full rules

Replace `src/console/highlighter.rs`:

```rust
use crate::console::palette;
use console::Style;
use regex::Regex;

/// A single highlight rule: a named-capture regex and the style to apply to each capture group.
struct Rule {
    regex: Regex,
    /// Styles indexed by capture group name. Matches with no named group use the whole match.
    group_styles: Vec<(&'static str, Style)>,
}

pub struct Highlighter {
    rules: Vec<Rule>,
}

impl Default for Highlighter {
    fn default() -> Self {
        let mut h = Self { rules: Vec::new() };
        h.add_static_rules();
        h
    }
}

impl Highlighter {
    fn add_static_rules(&mut self) {
        // Priority order: lower index = applied first (can be overwritten by later rules).
        // Python uses numeric priority; here order of push matches ascending priority.

        // bo / com / enum word highlighting (low priority)
        self.push_rule(
            r"\b(?P<bo>bo|BO|Bo)|(?P<com>com|COM|Com)|(?P<enum>enum|ENUM|Enum)\b",
            &[
                ("bo",   palette::BO),
                ("com",  palette::COM),
                ("enum", palette::ENUM),
            ],
        );

        // JSON keyword
        self.push_rule(r"\b(?P<json>JSON|json)\b", &[("json", palette::SUB_ACCENT)]);

        // BO4E brand (high priority — overwrites plain "bo"/"4e" matches)
        self.push_rule(
            r"\b(?P<bo4e_bo>BO)(?P<bo4e_4e>4E)\b",
            &[
                ("bo4e_bo", palette::MAIN_ACCENT),
                ("bo4e_4e", palette::MAIN),
            ],
        );

        // Version strings
        self.push_rule(
            r"(?P<version>v?\d{6}\.\d+\.\d+(?:-rc\d*)?(?:\+dev\w+)?)",
            &[("version", palette::SUB_ACCENT)],
        );

        // File paths (Unix-style and Windows-style)
        self.push_rule(
            r"(?P<path>(?:/[\w.\-]+)+/?|[a-zA-Z]:(?:\\[\w.\-]+)*\\?)",
            &[("path", palette::SUB)],
        );
    }

    fn push_rule(&mut self, pattern: &str, groups: &[(&'static str, &'static str)]) {
        let regex = Regex::new(pattern).expect("static highlighter regex is valid");
        let group_styles = groups
            .iter()
            .map(|(name, color)| (*name, Style::new().color256(0).fg(parse_hex(color))))
            .collect();
        self.rules.push(Rule { regex, group_styles });
    }

    /// Apply all highlight rules to `text`, returning an ANSI-styled string.
    pub fn apply(&self, text: &str) -> String {
        // Build a list of (start, end, Style) spans, then render.
        // Later rules overwrite spans from earlier rules at the same position.
        let len = text.len();
        // Each byte position holds the style index (0 = unstyled).
        // Simple approach: collect non-overlapping spans per rule, render left-to-right.
        let mut spans: Vec<(usize, usize, Style)> = Vec::new();

        for rule in &self.rules {
            for caps in rule.regex.captures_iter(text) {
                for (group_name, style) in &rule.group_styles {
                    if let Some(m) = caps.name(group_name) {
                        spans.push((m.start(), m.end(), style.clone()));
                    }
                }
            }
        }

        if spans.is_empty() {
            return text.to_string();
        }

        // Sort spans: later-added (higher priority) rules win on overlap.
        // Render spans in text order; skip overlapping ranges already styled.
        spans.sort_by_key(|(start, _, _)| *start);

        let mut result = String::with_capacity(text.len() * 2);
        let mut cursor = 0usize;
        // Deduplicate overlapping spans (last writer wins via sort stability + dedup).
        let mut active: Vec<(usize, usize, Style)> = Vec::new();
        for span in spans {
            if span.0 >= cursor {
                active.push(span);
            }
        }

        for (start, end, style) in active {
            if start > cursor {
                result.push_str(&text[cursor..start]);
            }
            result.push_str(&style.apply_to(&text[start..end]).to_string());
            cursor = end;
        }
        if cursor < len {
            result.push_str(&text[cursor..]);
        }
        result
    }

    /// Register schema class names as highlighted terms (called once after read_schemas).
    pub fn add_schema_names(&mut self, names: &[String]) {
        if names.is_empty() {
            return;
        }
        let pattern = names
            .iter()
            .map(|n| regex::escape(n))
            .collect::<Vec<_>>()
            .join("|");
        let full_pattern = format!(r"\b(?P<schema_name>{})\b", pattern);
        self.push_rule(&full_pattern, &[("schema_name", palette::MAIN_ACCENT)]);
    }
}

fn parse_hex(hex: &str) -> console::Color {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    console::Color::TrueColor { r, g, b }
}
```

**Note:** `palette::*` constants are `&'static str` hex strings. `parse_hex` converts them to
`console::Color::TrueColor`. `Style::new().fg(color)` applies the foreground color.

### Step 4: Run tests

```
cargo test console::highlighter::tests
```

Expected: all four tests pass.

### Step 5: Commit

```
git add src/console/highlighter.rs
git commit -m "feat(console): implement regex-based Highlighter with BO4E color scheme"
```

---

## Task 5: `read_schemas` + `walkdir` dependency

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/io/schemas.rs`

### Step 1: Add `walkdir` to `Cargo.toml`

```toml
walkdir = "2.5"
```

### Step 2: Write failing test

Add to the bottom of `src/io/schemas.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_test_dir() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".version"), "v202401.1.0").unwrap();
        let bo_dir = dir.path().join("bo");
        fs::create_dir_all(&bo_dir).unwrap();
        fs::write(
            bo_dir.join("Angebot.json"),
            r#"{"type":"object","title":"Angebot","properties":{},"required":[],"additionalProperties":false}"#,
        ).unwrap();
        let enum_dir = dir.path().join("enum");
        fs::create_dir_all(&enum_dir).unwrap();
        fs::write(
            enum_dir.join("Typ.json"),
            r#"{"type":"string","title":"Typ","enum":["A","B"]}"#,
        ).unwrap();
        dir
    }

    #[test]
    fn test_read_schemas_finds_all_json_files() {
        let dir = make_test_dir();
        let schemas = read_schemas(dir.path()).unwrap();
        assert_eq!(schemas.schemas().len(), 2);
        assert!(schemas.get_by_name("Angebot").is_some());
        assert!(schemas.get_by_name("Typ").is_some());
    }

    #[test]
    fn test_read_schemas_derives_module_path() {
        let dir = make_test_dir();
        let schemas = read_schemas(dir.path()).unwrap();
        let angebot = schemas.get_by_name("Angebot").unwrap();
        assert_eq!(angebot.borrow().module(), &["bo", "Angebot"]);
    }

    #[test]
    fn test_read_schemas_skips_version_file() {
        let dir = make_test_dir();
        let schemas = read_schemas(dir.path()).unwrap();
        // .version is not a .json file so it should not be included
        assert!(schemas.get_by_name(".version").is_none());
    }
}
```

**Note:** `tempfile` crate is needed. Add it to `Cargo.toml` under `[dev-dependencies]`:
```toml
tempfile = "3"
```

### Step 3: Run to confirm failure

```
cargo test io::schemas::tests::test_read_schemas_finds_all_json_files
```

Expected: compile error — `read_schemas` not defined.

### Step 4: Implement `read_schemas`

Uncomment and complete the function in `src/io/schemas.rs`:

```rust
use walkdir::WalkDir;
use crate::models::schema_meta::{Schema, Schemas};
use std::rc::Rc;
use std::cell::RefCell;

pub fn read_schemas(input_dir: &std::path::Path) -> Result<Schemas, String> {
    let version = read_version_file(input_dir)?;
    let mut schemas = Schemas::new(version);

    for entry in WalkDir::new(input_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file()
                && e.path().extension().map_or(false, |ext| ext == "json")
                && !e.file_name().to_string_lossy().starts_with('.')
        })
    {
        let relative_path = entry
            .path()
            .strip_prefix(input_dir)
            .map_err(|e| format!("Failed to strip prefix: {}", e))?
            .with_extension(""); // remove .json

        let module: Vec<String> = relative_path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();

        let schema_text = std::fs::read_to_string(entry.path())
            .map_err(|e| format!("Failed to read {}: {}", entry.path().display(), e))?;

        let mut schema = Schema::new(module, None)?;
        schema.load_schema(schema_text);
        schemas.add_schema(Rc::new(RefCell::new(schema)))?;
    }

    Ok(schemas)
}
```

### Step 5: Run tests

```
cargo test io::schemas::tests
```

Expected: all three new tests pass; existing `write_schemas` logic unaffected.

### Step 6: Commit

```
git add Cargo.toml src/io/schemas.rs
git commit -m "feat(io): implement read_schemas with walkdir"
```

---

## Task 6: `src/io/config.rs` — `load_config` and `get_additional_schemas`

**Files:**
- Create: `src/io/config.rs`
- Modify: `src/io.rs`

### Step 1: Register the new module

Add to `src/io.rs`:

```rust
pub mod config;
```

### Step 2: Write failing tests

Create `src/io/config.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_config_dir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn test_load_config_empty() {
        let dir = make_config_dir();
        let cfg_path = dir.path().join("config.json");
        fs::write(&cfg_path, "{}").unwrap();
        let config = load_config(&cfg_path).unwrap();
        assert!(config.additional_fields.is_empty());
        assert!(config.non_nullable_fields.is_empty());
    }

    #[test]
    fn test_load_config_resolves_additional_field_ref() {
        let dir = make_config_dir();
        // The referenced file contains a single AdditionalField
        let field_json = r#"{"pattern":"bo\\..*","fieldName":"myField","fieldDef":{"type":"string"}}"#;
        fs::write(dir.path().join("field.json"), field_json).unwrap();

        let config_json = r#"{"additionalFields":[{"$ref":"./field.json"}]}"#;
        let cfg_path = dir.path().join("config.json");
        fs::write(&cfg_path, config_json).unwrap();

        let config = load_config(&cfg_path).unwrap();
        assert_eq!(config.additional_fields.len(), 1);
        assert_eq!(config.additional_fields[0].field_name, "myField");
    }

    #[test]
    fn test_load_config_resolves_ref_to_list() {
        let dir = make_config_dir();
        let fields_json = r#"[
            {"pattern":"bo\\..*","fieldName":"f1","fieldDef":{"type":"string"}},
            {"pattern":"com\\..*","fieldName":"f2","fieldDef":{"type":"integer"}}
        ]"#;
        fs::write(dir.path().join("fields.json"), fields_json).unwrap();

        let config_json = r#"{"additionalFields":[{"$ref":"./fields.json"}]}"#;
        let cfg_path = dir.path().join("config.json");
        fs::write(&cfg_path, config_json).unwrap();

        let config = load_config(&cfg_path).unwrap();
        assert_eq!(config.additional_fields.len(), 2);
    }
}
```

### Step 3: Run to confirm failure

```
cargo test io::config::tests
```

Expected: compile error — module and functions not yet defined.

### Step 4: Implement

```rust
use crate::models::config::{AdditionalField, AdditionalFieldOrRef, AdditionalModel,
                             Config, SchemaRootTypeOrReference};
use crate::models::json_schema::SchemaRootType;
use crate::models::schema_meta::Schema;
use std::path::Path;

/// Resolved config — `additional_fields` contains only concrete `AdditionalField` values.
pub struct ResolvedConfig {
    pub non_nullable_fields: Vec<regex::Regex>,
    pub additional_fields: Vec<AdditionalField>,
    pub additional_enum_items: Vec<crate::models::config::AdditionalEnumItem>,
    pub additional_models: Vec<AdditionalModel>,
}

/// Load and fully resolve the config file at `path`.
/// Any `{ "$ref": "..." }` entries in `additionalFields` are replaced with the referenced
/// `AdditionalField` or `Vec<AdditionalField>` loaded from the referenced path.
pub fn load_config(path: &Path) -> Result<ResolvedConfig, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    let raw: Config = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse config: {}", e))?;

    let config_dir = path.parent().unwrap_or(Path::new("."));
    let mut additional_fields: Vec<AdditionalField> = Vec::new();

    for entry in raw.additional_fields {
        match entry {
            AdditionalFieldOrRef::Field(f) => additional_fields.push(f),
            AdditionalFieldOrRef::Reference(r) => {
                let ref_path = if Path::new(&r.path).is_absolute() {
                    std::path::PathBuf::from(&r.path)
                } else {
                    config_dir.join(&r.path)
                };
                let ref_text = std::fs::read_to_string(&ref_path)
                    .map_err(|e| format!("Failed to read referenced field file {:?}: {}", ref_path, e))?;
                // Try as a list first, then as a single object.
                if let Ok(list) = serde_json::from_str::<Vec<AdditionalField>>(&ref_text) {
                    additional_fields.extend(list);
                } else {
                    let single: AdditionalField = serde_json::from_str(&ref_text)
                        .map_err(|e| format!("Failed to parse referenced field file {:?}: {}", ref_path, e))?;
                    additional_fields.push(single);
                }
            }
        }
    }

    Ok(ResolvedConfig {
        non_nullable_fields: raw.non_nullable_fields,
        additional_fields,
        additional_enum_items: raw.additional_enum_items,
        additional_models: raw.additional_models,
    })
}

/// Load additional schemas declared in the config.
/// Returns a `Vec<Schema>` ready to be added to `Schemas`.
pub fn get_additional_schemas(
    models: &[AdditionalModel],
    config_path: &Path,
) -> Result<Vec<Schema>, String> {
    let config_dir = config_path.parent().unwrap_or(Path::new("."));
    let mut result = Vec::new();

    for model in models {
        let (schema_root, _ref_path) = match &model.schema {
            SchemaRootTypeOrReference::SchemaRootObject(o) => {
                (SchemaRootType::Object(o.clone()), None)
            }
            SchemaRootTypeOrReference::SchemaRootStrEnum(e) => {
                (SchemaRootType::StrEnum(e.clone()), None)
            }
            SchemaRootTypeOrReference::Reference(r) => {
                let ref_path = if Path::new(&r.path).is_absolute() {
                    std::path::PathBuf::from(&r.path)
                } else {
                    config_dir.join(&r.path)
                };
                let text = std::fs::read_to_string(&ref_path)
                    .map_err(|e| format!("Failed to read schema ref {:?}: {}", ref_path, e))?;
                let parsed: SchemaRootType = serde_json::from_str(&text)
                    .map_err(|e| format!("Failed to parse schema ref {:?}: {}", ref_path, e))?;
                (parsed, Some(ref_path))
            }
        };

        let title = match &schema_root {
            SchemaRootType::Object(o) => o.object.base.title.clone(),
            SchemaRootType::StrEnum(e) => e.str_enum.base.title.clone(),
        }
        .ok_or_else(|| "Config error: title is required for additional models".to_string())?;

        if title.is_empty() {
            return Err("Config error: title must be non-empty for additional models".to_string());
        }

        let module = vec![model.module.clone(), title.clone()];
        let schema_text = serde_json::to_string(&schema_root)
            .map_err(|e| format!("Failed to serialize additional model: {}", e))?;
        let mut schema = Schema::new(module, None)?;
        schema.load_schema(schema_text);
        result.push(schema);
    }

    Ok(result)
}
```

### Step 5: Run tests

```
cargo test io::config::tests
```

Expected: all three tests pass.

### Step 6: Commit

```
git add src/io/config.rs src/io.rs
git commit -m "feat(io): add load_config and get_additional_schemas"
```

---

## Task 7: Implement `update_reference`

**Files:**
- Modify: `src/edit/update_refs.rs`

The function signature and call site are already in place. The stub just returns `Ok(())`.

### Step 1: Write failing tests

Add to `#[cfg(test)]` in `src/edit/update_refs.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::json_schema::ReferenceSchema;
    use std::collections::HashMap;

    fn make_ref(r: &str) -> ReferenceSchema {
        ReferenceSchema { base: Default::default(), r#ref: r.to_string() }
    }

    fn namespace(entries: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        entries.iter().map(|(k, v)| {
            (k.to_string(), v.iter().map(|s| s.to_string()).collect())
        }).collect()
    }

    #[test]
    fn test_online_ref_rewritten_to_relative() {
        let mut r = make_ref(
            "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0/\
             src/bo4e_schemas/bo/Angebot.json"
        );
        let module = vec!["com".to_string(), "Adresse".to_string()];
        let ns = namespace(&[("Angebot", &["bo", "Angebot"])]);
        update_reference(&mut r, &module, &ns, "v202401.1.0").unwrap();
        assert_eq!(r.r#ref, "../bo/Angebot.json#");
    }

    #[test]
    fn test_defs_ref_rewritten_to_relative() {
        let mut r = make_ref("#/$defs/Angebot");
        let module = vec!["com".to_string(), "Adresse".to_string()];
        let ns = namespace(&[("Angebot", &["bo", "Angebot"])]);
        update_reference(&mut r, &module, &ns, "v202401.1.0").unwrap();
        assert_eq!(r.r#ref, "../bo/Angebot.json#");
    }

    #[test]
    fn test_unknown_ref_unchanged() {
        let mut r = make_ref("../already/relative.json#");
        let module = vec!["bo".to_string(), "Foo".to_string()];
        let ns = HashMap::new();
        update_reference(&mut r, &module, &ns, "v202401.1.0").unwrap();
        assert_eq!(r.r#ref, "../already/relative.json#");
    }

    #[test]
    fn test_version_mismatch_is_error() {
        let mut r = make_ref(
            "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.0.0/\
             src/bo4e_schemas/bo/Angebot.json"
        );
        let module = vec!["bo".to_string(), "Foo".to_string()];
        let ns = HashMap::new();
        let result = update_reference(&mut r, &module, &ns, "v202401.1.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Version mismatch"));
    }

    #[test]
    fn test_same_module_becomes_hash() {
        // Reference to a schema in the same directory → relative_ref is just "#"
        let mut r = make_ref(
            "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0/\
             src/bo4e_schemas/bo/Angebot.json"
        );
        let module = vec!["bo".to_string(), "Angebot".to_string()];
        let ns = namespace(&[("Angebot", &["bo", "Angebot"])]);
        update_reference(&mut r, &module, &ns, "v202401.1.0").unwrap();
        // Same module path → self-reference → "#"
        assert_eq!(r.r#ref, "#");
    }
}
```

### Step 2: Run to confirm failures

```
cargo test edit::update_refs::tests
```

Expected: tests compile but fail (the stub returns `Ok(())` without modifying `r.r#ref`).

### Step 3: Update function signature and implement

The current private `update_reference` takes `(reference, current_module, namespace)` and gets
`version` from... nowhere (it needs the version for the mismatch check). Add `version: &str` as
a parameter. Update the call site in `update_references_single` to pass `schemas.version.to_string()`.

```rust
fn update_reference(
    reference: &mut ReferenceSchema,
    current_module: &[String],
    namespace: &HashMap<String, Vec<String>>,
    version: &str,
) -> Result<(), String> {
    let reference_module_path: Vec<String>;

    if let Some(caps) = REF_ONLINE_REGEX.captures(&reference.r#ref) {
        let ref_version = caps.name("version").unwrap().as_str();
        if ref_version != version {
            return Err(format!(
                "Version mismatch: '{}' does not match '{}' for reference '{}'",
                ref_version, version, reference.r#ref
            ));
        }
        let sub_path = caps.name("sub_path").map_or("", |m| m.as_str());
        let model = caps.name("model").unwrap().as_str();
        reference_module_path = sub_path
            .split('/')
            .filter(|s| !s.is_empty())
            .chain(std::iter::once(model))
            .map(String::from)
            .collect();
    } else if let Some(caps) = REF_DEFS_REGEX.captures(&reference.r#ref) {
        let model = caps.name("model").unwrap().as_str();
        reference_module_path = namespace
            .get(model)
            .cloned()
            .ok_or_else(|| format!("Could not find schema '{}' in namespace", model))?;
    } else {
        cprint_verbose!("Reference unchanged. Could not parse reference: {}", reference.r#ref);
        return Ok(());
    }

    // Find the divergence point between reference_module_path and current_module.
    let diverge = reference_module_path
        .iter()
        .zip(current_module.iter())
        .position(|(a, b)| a != b)
        .unwrap_or(reference_module_path.len().min(current_module.len()));

    let relative_ref = if diverge == reference_module_path.len()
        && diverge == current_module.len()
    {
        // Identical module paths — self-reference.
        "#".to_string()
    } else {
        let up = current_module.len().saturating_sub(diverge + 1);
        let remaining = reference_module_path[diverge..].join("/");
        format!("{}{}.json#", "../".repeat(up), remaining)
    };

    cprint_verbose!("Updated reference {} to: {}", reference.r#ref, relative_ref);
    reference.r#ref = relative_ref;
    Ok(())
}
```

Update `update_references_single` to pass the version:

```rust
fn update_references_single(
    schema: &mut Schema,
    namespace: &HashMap<String, Vec<String>>,
    version: &str,
) -> Result<(), String> {
    let module: Vec<String> = schema.module().iter().cloned().collect();
    let visitable: &mut dyn Visitable = schema.schema_mut()?;
    cntrl_to_result(
        visitable.try_visit_all_mut::<ReferenceSchema, String>(&mut |reference| {
            result_to_cntrl(update_reference(reference, &module, namespace, version))
        }),
    )
}

pub fn update_references_all(schemas: &mut Schemas) -> Result<(), String> {
    let namespace = schemas.modules_by_name();
    let version = schemas.version.to_string();
    for schema in schemas.iter_mut() {
        update_references_single(schema.borrow_mut().deref_mut(), &namespace, &version)?;
    }
    Ok(())
}
```

### Step 4: Run tests

```
cargo test edit::update_refs::tests
```

Expected: all five tests pass.

### Step 5: Commit

```
git add src/edit/update_refs.rs
git commit -m "feat(edit): implement update_reference with online, defs, and unknown ref handling"
```

---

## Task 8: `src/edit/non_nullable.rs`

**Files:**
- Create: `src/edit/non_nullable.rs`
- Modify: `src/edit.rs`

### Step 1: Register module

Add to `src/edit.rs`:

```rust
pub mod non_nullable;
pub mod update_refs;
```

### Step 2: Write failing tests

Create `src/edit/non_nullable.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::json_schema::*;
    use std::collections::BTreeMap;

    fn make_nullable_object(field_name: &str) -> SchemaRootObject {
        SchemaRootObject {
            base: Default::default(),
            object: ObjectSchema {
                base: Default::default(),
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: BTreeMap::from([(
                    field_name.to_string(),
                    SchemaType::AnyOf(AnyOfSchema {
                        base: TypeBase {
                            default: Some(PrimitiveValue::Null),
                            ..Default::default()
                        },
                        any_of: vec![
                            SchemaType::StringSchema(Default::default()),
                            SchemaType::NullSchema(Default::default()),
                        ],
                    }),
                )]),
                required: vec![],
            },
        }
    }

    #[test]
    fn test_field_to_non_nullable_removes_null_variant() {
        let mut schema = make_nullable_object("name");
        field_to_non_nullable(&mut schema, "name").unwrap();
        let prop = schema.object.properties.get("name").unwrap();
        // Should be a bare StringSchema now (flattened)
        assert!(matches!(prop, SchemaType::StringSchema(_)));
    }

    #[test]
    fn test_field_to_non_nullable_removes_null_default() {
        let mut schema = make_nullable_object("name");
        field_to_non_nullable(&mut schema, "name").unwrap();
        // field moved to required since null default was removed
        assert!(schema.object.required.contains(&"name".to_string()));
    }

    #[test]
    fn test_field_to_non_nullable_keeps_non_null_default() {
        let field_name = "count";
        let mut schema = SchemaRootObject {
            base: Default::default(),
            object: ObjectSchema {
                base: Default::default(),
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: BTreeMap::from([(
                    field_name.to_string(),
                    SchemaType::AnyOf(AnyOfSchema {
                        base: TypeBase {
                            default: Some(PrimitiveValue::Integer(0)),
                            ..Default::default()
                        },
                        any_of: vec![
                            SchemaType::IntegerSchema(Default::default()),
                            SchemaType::NullSchema(Default::default()),
                        ],
                    }),
                )]),
                required: vec![],
            },
        };
        field_to_non_nullable(&mut schema, field_name).unwrap();
        let prop = schema.object.properties.get(field_name).unwrap();
        // AnyOf flattened to IntegerSchema; default 0 preserved
        if let SchemaType::IntegerSchema(s) = prop {
            assert_eq!(s.base.default, Some(PrimitiveValue::Integer(0)));
        } else {
            panic!("Expected IntegerSchema, got {:?}", prop);
        }
        // Not added to required because default is still present
        assert!(!schema.object.required.contains(&field_name.to_string()));
    }

    #[test]
    fn test_field_to_non_nullable_preserves_multi_variant() {
        // AnyOf with 3 variants — removing null leaves 2, should not flatten.
        let field_name = "value";
        let mut schema = SchemaRootObject {
            base: Default::default(),
            object: ObjectSchema {
                base: Default::default(),
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: BTreeMap::from([(
                    field_name.to_string(),
                    SchemaType::AnyOf(AnyOfSchema {
                        base: TypeBase {
                            default: Some(PrimitiveValue::Null),
                            ..Default::default()
                        },
                        any_of: vec![
                            SchemaType::StringSchema(Default::default()),
                            SchemaType::IntegerSchema(Default::default()),
                            SchemaType::NullSchema(Default::default()),
                        ],
                    }),
                )]),
                required: vec![],
            },
        };
        field_to_non_nullable(&mut schema, field_name).unwrap();
        let prop = schema.object.properties.get(field_name).unwrap();
        // Still AnyOf (two remaining variants), not flattened
        assert!(matches!(prop, SchemaType::AnyOf(_)));
    }
}
```

### Step 3: Run to confirm failure

```
cargo test edit::non_nullable::tests
```

Expected: compile error — `field_to_non_nullable` not defined.

### Step 4: Implement

```rust
use crate::models::json_schema::{
    AnyOfSchema, NullSchema, PrimitiveValue, SchemaRootObject, SchemaType, TypeBase,
};

/// Remove the `null` variant from a nullable `AnyOf` property.
///
/// Preconditions (returns `Err` if violated):
/// - Property exists in `schema.properties`.
/// - Property is `SchemaType::AnyOf`.
/// - `AnyOf` contains at least one `SchemaType::NullSchema` variant.
pub fn field_to_non_nullable(
    schema: &mut SchemaRootObject,
    field_name: &str,
) -> Result<(), String> {
    let prop = schema
        .object
        .properties
        .get_mut(field_name)
        .ok_or_else(|| format!("Field '{}' not found", field_name))?;

    let any_of_schema = match prop {
        SchemaType::AnyOf(a) => a,
        other => {
            return Err(format!(
                "Expected AnyOf for field '{}', got {:?}",
                field_name, other
            ))
        }
    };

    // Remove the first NullSchema variant.
    let null_pos = any_of_schema
        .any_of
        .iter()
        .position(|v| matches!(v, SchemaType::NullSchema(_)))
        .ok_or_else(|| format!("Field '{}' AnyOf contains no NullSchema", field_name))?;
    any_of_schema.any_of.remove(null_pos);

    // If the default was explicitly null, remove it and add field to required.
    let had_null_default = any_of_schema.base.default == Some(PrimitiveValue::Null);
    if had_null_default {
        any_of_schema.base.default = None;
        if !schema.object.required.contains(&field_name.to_string()) {
            schema.object.required.push(field_name.to_string());
        }
    }

    // Flatten to single type when only one variant remains.
    if any_of_schema.any_of.len() == 1 {
        let inherited_base = any_of_schema.base.clone();
        let inner = any_of_schema.any_of.remove(0);
        // Copy title, description, default from AnyOf base onto the inner type's base.
        let new_prop = apply_base_to_schema_type(inner, inherited_base);
        *schema.object.properties.get_mut(field_name).unwrap() = new_prop;
    }

    Ok(())
}

fn apply_base_to_schema_type(mut schema_type: SchemaType, base: TypeBase) -> SchemaType {
    let inner_base = match &mut schema_type {
        SchemaType::StringSchema(s) => &mut s.base,
        SchemaType::IntegerSchema(s) => &mut s.base,
        SchemaType::NumberSchema(s) => &mut s.base,
        SchemaType::BooleanSchema(s) => &mut s.base,
        SchemaType::AnySchema(s) => &mut s.base,
        SchemaType::NullSchema(s) => &mut s.base,
        SchemaType::DecimalSchema(s) => &mut s.base,
        SchemaType::ConstantSchema(s) => &mut s.base,
        SchemaType::ReferenceSchema(s) => &mut s.base,
        SchemaType::ArraySchema(s) => &mut s.base,
        SchemaType::AnyOf(s) => &mut s.base,
        SchemaType::AllOf(s) => &mut s.base,
        SchemaType::Object(s) => &mut s.base,
        SchemaType::StrEnum(s) => &mut s.base,
    };
    if inner_base.title.is_none() { inner_base.title = base.title; }
    if inner_base.description.is_none() { inner_base.description = base.description; }
    if inner_base.default.is_none() { inner_base.default = base.default; }
    schema_type
}

/// Apply non-nullable patterns to all matching fields in all schemas.
pub fn transform_all_non_nullable_fields(
    patterns: &[regex::Regex],
    schemas: &mut crate::models::schema_meta::Schemas,
) -> Result<(), String> {
    use crate::cprint;
    use crate::cprint_verbose;

    // Collect (field_path, field_name, module) triples up-front.
    // field_path = "bo.Angebot.fieldName"
    let mut triples: Vec<(String, String, Vec<String>)> = Vec::new();
    for schema_rc in schemas.iter() {
        let schema = schema_rc.borrow();
        // Only SchemaRootObject schemas have properties.
        // We use get_serialized_schema + re-parse lazily — instead, check schema field.
        // Since schema may be lazily parsed, we skip it if not yet parsed.
        // To get the parsed schema, we need a mutable borrow — handled below per pattern.
        let module = schema.module().to_vec();
        drop(schema);

        // Try to get the schema; if it fails (not loaded), skip.
        let mut schema_mut = schema_rc.borrow_mut();
        let root = match schema_mut.schema_mut() {
            Ok(r) => r,
            Err(_) => continue,
        };
        if let crate::models::json_schema::SchemaRootType::Object(obj) = root {
            let prefix = module.join(".");
            for field_name in obj.object.properties.keys() {
                let field_path = format!("{}.{}", prefix, field_name);
                triples.push((field_path, field_name.clone(), module.clone()));
            }
        }
    }

    for pattern in patterns {
        let mut matches = 0usize;
        for (field_path, field_name, module) in &triples {
            if pattern.is_match(field_path) {
                let schema_rc = schemas
                    .get_by_module(module)
                    .ok_or_else(|| format!("Schema not found for module {:?}", module))?;
                let mut schema = schema_rc.borrow_mut();
                let root = schema.schema_mut()?;
                if let crate::models::json_schema::SchemaRootType::Object(obj) = root {
                    // Check preconditions before calling field_to_non_nullable
                    let prop = obj.object.properties.get(field_name);
                    let is_anyof_with_null = prop.map_or(false, |p| {
                        matches!(p, SchemaType::AnyOf(a) if
                            a.any_of.iter().any(|v| matches!(v, SchemaType::NullSchema(_)))
                            && a.base.default.is_some()
                        )
                    });
                    if is_anyof_with_null {
                        field_to_non_nullable(obj, field_name)?;
                        matches += 1;
                        cprint_verbose!("Applied pattern '{}' to field {}", pattern, field_path);
                    }
                }
            }
        }
        if matches == 0 {
            cprint!("Warning: Pattern '{}' did not match any fields", pattern);
        } else {
            cprint!("Pattern '{}' matched {} fields", pattern, matches);
        }
    }

    Ok(())
}
```

**Note:** `use crate::models::json_schema::SchemaType;` is needed at the top of the file.

### Step 5: Run tests

```
cargo test edit::non_nullable::tests
```

Expected: all four tests pass.

### Step 6: Commit

```
git add src/edit/non_nullable.rs src/edit.rs
git commit -m "feat(edit): add non_nullable transformation"
```

---

## Task 9: `src/edit/add.rs`

**Files:**
- Create: `src/edit/add.rs`
- Modify: `src/edit.rs`

### Step 1: Register module

Add to `src/edit.rs`:

```rust
pub mod add;
```

### Step 2: Write failing tests

Create `src/edit/add.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::config::{AdditionalEnumItem, AdditionalField};
    use crate::models::json_schema::*;
    use crate::models::schema_meta::{Schema, Schemas};
    use crate::models::version::DirtyVersion;
    use regex::Regex;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;
    use std::str::FromStr;

    fn make_schemas() -> Schemas {
        let version = DirtyVersion::from_str("v202401.1.0").unwrap();
        let mut schemas = Schemas::new(version);

        let obj = SchemaRootType::Object(SchemaRootObject {
            base: Default::default(),
            object: ObjectSchema {
                base: TypeBase { title: Some("Foo".into()), ..Default::default() },
                r#type: LiteralTypeObject::Object,
                additional_properties: false,
                properties: BTreeMap::new(),
                required: vec![],
            },
        });
        let schema_text = serde_json::to_string(&obj).unwrap();
        let mut schema = Schema::new(vec!["bo".into(), "Foo".into()], None).unwrap();
        schema.load_schema(schema_text);
        schemas.add_schema(Rc::new(RefCell::new(schema))).unwrap();

        let str_enum = SchemaRootType::StrEnum(SchemaRootStrEnum {
            base: Default::default(),
            str_enum: StrEnumSchema {
                base: TypeBase { title: Some("Bar".into()), ..Default::default() },
                r#type: LiteralTypeString::String,
                enum_values: vec!["X".into()],
            },
        });
        let enum_text = serde_json::to_string(&str_enum).unwrap();
        let mut schema2 = Schema::new(vec!["enum".into(), "Bar".into()], None).unwrap();
        schema2.load_schema(enum_text);
        schemas.add_schema(Rc::new(RefCell::new(schema2))).unwrap();

        schemas
    }

    #[test]
    fn test_transform_additional_fields_inserts_field() {
        let mut schemas = make_schemas();
        let field = AdditionalField {
            pattern: Regex::new(r"bo\.Foo").unwrap(),
            field_name: "myField".into(),
            field_def: SchemaType::StringSchema(Default::default()),
        };
        transform_all_additional_fields(&[field], &mut schemas);
        let schema_rc = schemas.get_by_name("Foo").unwrap();
        let mut schema = schema_rc.borrow_mut();
        let root = schema.schema_mut().unwrap();
        if let SchemaRootType::Object(obj) = root {
            assert!(obj.object.properties.contains_key("myField"));
        } else {
            panic!("Expected Object schema");
        }
    }

    #[test]
    fn test_transform_additional_fields_adds_to_required_when_no_default() {
        let mut schemas = make_schemas();
        let field = AdditionalField {
            pattern: Regex::new(r"bo\.Foo").unwrap(),
            field_name: "req".into(),
            field_def: SchemaType::StringSchema(Default::default()), // no default
        };
        transform_all_additional_fields(&[field], &mut schemas);
        let schema_rc = schemas.get_by_name("Foo").unwrap();
        let mut schema = schema_rc.borrow_mut();
        let root = schema.schema_mut().unwrap();
        if let SchemaRootType::Object(obj) = root {
            assert!(obj.object.required.contains(&"req".to_string()));
        }
    }

    #[test]
    fn test_transform_additional_fields_no_required_when_has_default() {
        let mut schemas = make_schemas();
        let mut string_schema = StringSchema::default();
        string_schema.base.default = Some(crate::models::json_schema::PrimitiveValue::String("x".into()));
        let field = AdditionalField {
            pattern: Regex::new(r"bo\.Foo").unwrap(),
            field_name: "opt".into(),
            field_def: SchemaType::StringSchema(string_schema),
        };
        transform_all_additional_fields(&[field], &mut schemas);
        let schema_rc = schemas.get_by_name("Foo").unwrap();
        let mut schema = schema_rc.borrow_mut();
        let root = schema.schema_mut().unwrap();
        if let SchemaRootType::Object(obj) = root {
            assert!(!obj.object.required.contains(&"opt".to_string()));
        }
    }

    #[test]
    fn test_transform_additional_enum_items_extends_enum() {
        let mut schemas = make_schemas();
        let item = AdditionalEnumItem {
            pattern: Regex::new(r"enum\.Bar").unwrap(),
            items: vec!["Y".into(), "Z".into()],
        };
        transform_all_additional_enum_items(&[item], &mut schemas);
        let schema_rc = schemas.get_by_name("Bar").unwrap();
        let mut schema = schema_rc.borrow_mut();
        let root = schema.schema_mut().unwrap();
        if let SchemaRootType::StrEnum(e) = root {
            assert!(e.str_enum.enum_values.contains(&"Y".to_string()));
            assert!(e.str_enum.enum_values.contains(&"Z".to_string()));
        }
    }
}
```

### Step 3: Run to confirm failure

```
cargo test edit::add::tests
```

Expected: compile error — functions not defined.

### Step 4: Implement

```rust
use crate::models::config::{AdditionalEnumItem, AdditionalField};
use crate::models::json_schema::{PrimitiveValue, SchemaRootType, SchemaType};
use crate::models::schema_meta::Schemas;
use crate::{cprint, cprint_verbose};

/// Insert additional fields into matching `SchemaRootObject` schemas.
pub fn transform_all_additional_fields(fields: &[AdditionalField], schemas: &mut Schemas) {
    for field in fields {
        let mut matches = 0usize;
        for schema_rc in schemas.iter() {
            let module_path = schema_rc.borrow().module().join(".");
            if !field.pattern.is_match(&module_path) {
                continue;
            }
            let mut schema = schema_rc.borrow_mut();
            let root = match schema.schema_mut() {
                Ok(r) => r,
                Err(_) => continue,
            };
            if let SchemaRootType::Object(obj) = root {
                obj.object
                    .properties
                    .insert(field.field_name.clone(), field.field_def.clone());

                // Add to required unless field_def has an explicit default.
                let has_default = field_def_has_default(&field.field_def);
                if !has_default && !obj.object.required.contains(&field.field_name) {
                    obj.object.required.push(field.field_name.clone());
                }

                matches += 1;
                cprint_verbose!(
                    "Applied pattern '{}' to schema {}. Added field '{}'",
                    field.pattern,
                    module_path,
                    field.field_name
                );
            }
        }
        if matches == 0 {
            cprint!(
                "Warning: Pattern '{}' did not match any schemas",
                field.pattern
            );
        } else {
            cprint!(
                "Pattern '{}' matched {} schemas",
                field.pattern,
                matches
            );
        }
    }
}

fn field_def_has_default(schema_type: &SchemaType) -> bool {
    let base = match schema_type {
        SchemaType::StringSchema(s) => &s.base,
        SchemaType::IntegerSchema(s) => &s.base,
        SchemaType::NumberSchema(s) => &s.base,
        SchemaType::BooleanSchema(s) => &s.base,
        SchemaType::AnySchema(s) => &s.base,
        SchemaType::NullSchema(s) => &s.base,
        SchemaType::DecimalSchema(s) => &s.base,
        SchemaType::ConstantSchema(s) => &s.base,
        SchemaType::ReferenceSchema(s) => &s.base,
        SchemaType::ArraySchema(s) => &s.base,
        SchemaType::AnyOf(s) => &s.base,
        SchemaType::AllOf(s) => &s.base,
        SchemaType::Object(s) => &s.base,
        SchemaType::StrEnum(s) => &s.base,
    };
    base.default.is_some()
}

/// Extend enum values in matching `SchemaRootStrEnum` schemas.
pub fn transform_all_additional_enum_items(items: &[AdditionalEnumItem], schemas: &mut Schemas) {
    for item in items {
        let mut matches = 0usize;
        for schema_rc in schemas.iter() {
            let module_path = schema_rc.borrow().module().join(".");
            if !item.pattern.is_match(&module_path) {
                continue;
            }
            let mut schema = schema_rc.borrow_mut();
            let root = match schema.schema_mut() {
                Ok(r) => r,
                Err(_) => continue,
            };
            if let SchemaRootType::StrEnum(e) = root {
                e.str_enum.enum_values.extend(item.items.iter().cloned());
                matches += 1;
                cprint_verbose!(
                    "Applied pattern '{}' to schema {}. Added enum items: {:?}",
                    item.pattern,
                    module_path,
                    item.items
                );
            }
        }
        if matches == 0 {
            cprint!(
                "Warning: Pattern '{}' did not match any schemas",
                item.pattern
            );
        } else {
            cprint!("Pattern '{}' matched {} schemas", item.pattern, matches);
        }
    }
}
```

### Step 5: Run tests

```
cargo test edit::add::tests
```

Expected: all four tests pass.

### Step 6: Commit

```
git add src/edit/add.rs src/edit.rs
git commit -m "feat(edit): add additional fields and enum items transformations"
```

---

## Task 10: `src/cli/edit.rs` and integration test

**Files:**
- Create: `src/cli/edit.rs`
- Modify: `src/cli.rs`
- Modify: `src/cli/base.rs`

### Step 1: Register module and subcommand

Add to `src/cli.rs`:

```rust
pub mod base;
pub mod edit;
mod pull;
```

Add `Edit` variant to `SubcommandsLevel1` in `src/cli/base.rs`:

```rust
use crate::cli::edit::Edit;

#[derive(Subcommand)]
pub enum SubcommandsLevel1 {
    Pull(Pull),
    Edit(Edit),
}

impl Executable for SubcommandsLevel1 {
    fn run(&self) -> Result<(), String> {
        match self {
            SubcommandsLevel1::Pull(c) => c.run(),
            SubcommandsLevel1::Edit(c) => c.run(),
        }
    }
}
```

### Step 2: Write integration test

Create `src/cli/edit.rs` with an integration test block first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::console::console::{Console, CONSOLE};
    use std::fs;
    use std::sync::OnceLock;
    use tempfile::TempDir;

    fn init_console() {
        // May already be initialized in a previous test — that's fine.
        let _ = CONSOLE.set(Console::new(false));
    }

    fn make_input_dir() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".version"), "v202401.1.0").unwrap();

        let bo = dir.path().join("bo");
        fs::create_dir_all(&bo).unwrap();
        fs::write(
            bo.join("Angebot.json"),
            r#"{"type":"object","title":"Angebot","properties":{},"required":[],"additionalProperties":false}"#,
        ).unwrap();

        let enums = dir.path().join("enum");
        fs::create_dir_all(&enums).unwrap();
        fs::write(
            enums.join("Typ.json"),
            r#"{"type":"string","title":"Typ","enum":["A"]}"#,
        ).unwrap();
        dir
    }

    fn make_config(dir: &TempDir) -> std::path::PathBuf {
        let cfg = r#"{
            "additionalFields": [
                { "pattern": "bo\\.Angebot", "fieldName": "foo", "fieldDef": { "type": "string" } }
            ],
            "additionalEnumItems": [
                { "pattern": "enum\\.Typ", "items": ["B", "C"] }
            ]
        }"#;
        let path = dir.path().join("config.json");
        fs::write(&path, cfg).unwrap();
        path
    }

    #[test]
    fn test_edit_without_config_copies_schemas() {
        init_console();
        let input = make_input_dir();
        let output = tempfile::tempdir().unwrap();

        let cmd = Edit {
            input_dir: input.path().to_path_buf(),
            output_dir: output.path().to_path_buf(),
            config_file: None,
            no_default_version: true,
            no_clear_output: false,
        };
        cmd.run().unwrap();

        assert!(output.path().join("bo/Angebot.json").exists());
        assert!(output.path().join("enum/Typ.json").exists());
        assert!(output.path().join(".version").exists());
    }

    #[test]
    fn test_edit_with_config_applies_transformations() {
        init_console();
        let input = make_input_dir();
        let output = tempfile::tempdir().unwrap();
        let cfg_dir = tempfile::tempdir().unwrap();
        let cfg_path = make_config(&cfg_dir);

        let cmd = Edit {
            input_dir: input.path().to_path_buf(),
            output_dir: output.path().to_path_buf(),
            config_file: Some(cfg_path),
            no_default_version: true,
            no_clear_output: false,
        };
        cmd.run().unwrap();

        // Angebot should have the "foo" field
        let angebot_text = fs::read_to_string(output.path().join("bo/Angebot.json")).unwrap();
        assert!(angebot_text.contains("\"foo\""));

        // Typ should have the added enum items
        let typ_text = fs::read_to_string(output.path().join("enum/Typ.json")).unwrap();
        assert!(typ_text.contains("\"B\""));
        assert!(typ_text.contains("\"C\""));
    }
}
```

### Step 3: Run to confirm failure

```
cargo test cli::edit::tests
```

Expected: compile error — `Edit` struct not defined.

### Step 4: Implement `Edit`

```rust
use crate::cli::base::Executable;
use crate::console::console::CONSOLE;
use crate::edit::add::{transform_all_additional_enum_items, transform_all_additional_fields};
use crate::edit::non_nullable::transform_all_non_nullable_fields;
use crate::edit::update_refs::update_references_all;
use crate::io::cleanse::clear_dir_if_needed;
use crate::io::config::{get_additional_schemas, load_config};
use crate::io::schemas::{read_schemas, write_schemas};
use crate::models::json_schema::{PrimitiveValue, SchemaRootType};
use crate::{cprint, cprint_verbose};
use clap::Args;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// Edit JSON-schemas in the input directory and save results to the output directory.
///
/// If no configuration file is provided, schemas are copied unchanged (with references updated).
#[derive(Args)]
pub struct Edit {
    /// The directory to read the JSON-schemas from.
    #[arg(short = 'i', long = "input", required = true, value_name = "INPUT_DIRECTORY")]
    pub input_dir: PathBuf,

    /// The directory to save the edited JSON-schemas to.
    #[arg(short = 'o', long = "output", required = true, value_name = "OUTPUT_DIRECTORY")]
    pub output_dir: PathBuf,

    /// The configuration file for editing the schemas.
    #[arg(short = 'c', long = "config", value_name = "CONFIG_FILE")]
    pub config_file: Option<PathBuf>,

    /// Skip automatically setting the `_version` field default.
    #[arg(long, default_value_t = false)]
    pub no_default_version: bool,

    /// Skip clearing the output directory before saving.
    #[arg(long, default_value_t = false)]
    pub no_clear_output: bool,
}

impl Executable for Edit {
    fn run(&self) -> Result<(), String> {
        clear_dir_if_needed(&self.output_dir, !self.no_clear_output)
            .map_err(|e| e.to_string())?;

        let mut schemas = read_schemas(&self.input_dir)?;

        if let Some(config_path) = &self.config_file {
            let config = load_config(config_path)?;

            // Additional models
            let extra = get_additional_schemas(&config.additional_models, config_path)?;
            for schema in extra {
                schemas.add_schema(Rc::new(RefCell::new(schema)))?;
            }
            // Register schema names for highlighting
            let names: Vec<String> = schemas
                .schemas()
                .iter()
                .map(|s| s.borrow().name().to_string())
                .collect();
            CONSOLE
                .get()
                .expect("CONSOLE not initialized")
                .add_schema_names(&names);
            cprint!("Added all additional models");

            transform_all_additional_fields(&config.additional_fields, &mut schemas);
            cprint!("Added all additional fields");

            transform_all_non_nullable_fields(&config.non_nullable_fields, &mut schemas)?;
            cprint!("Transformed all non nullable fields");

            transform_all_additional_enum_items(&config.additional_enum_items, &mut schemas);
            cprint!("Added all additional enum items");
        }

        if !self.no_default_version {
            let version_str = schemas.version.to_string().trim_start_matches('v').to_string();
            for schema_rc in schemas.iter() {
                let mut schema = schema_rc.borrow_mut();
                let root = match schema.schema_mut() {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                if let SchemaRootType::Object(obj) = root {
                    if let Some(version_prop) = obj.object.properties.get_mut("_version") {
                        let base = match version_prop {
                            crate::models::json_schema::SchemaType::StringSchema(s) => &mut s.base,
                            crate::models::json_schema::SchemaType::AnyOf(s) => &mut s.base,
                            _ => continue,
                        };
                        base.default = Some(PrimitiveValue::String(version_str.clone()));
                    }
                }
            }
            cprint!("Set default versions to v{}", version_str);
        }

        update_references_all(&mut schemas)?;

        write_schemas(&schemas, &self.output_dir).map_err(|e| e.to_string())
    }
}
```

### Step 5: Run tests

```
cargo test cli::edit::tests
cargo test
```

Expected: all tests pass; `cargo test` succeeds with no failures.

### Step 6: Manual smoke test

Build and try:

```
cargo build
./target/debug/bo4e-cli edit --help
./target/debug/bo4e-cli edit -i <some_schemas_dir> -o /tmp/out
./target/debug/bo4e-cli -v edit -i <some_schemas_dir> -o /tmp/out
```

Expected: help text printed; command runs; verbose flag accepted on either side of the subcommand.

### Step 7: Commit

```
git add src/cli/edit.rs src/cli/base.rs src/cli.rs
git commit -m "feat(cli): add edit subcommand with --verbose global flag"
```

---

## Done

All tasks complete. Run the full test suite one final time:

```
cargo test
```

Expected: all tests pass, zero warnings on logic code.
