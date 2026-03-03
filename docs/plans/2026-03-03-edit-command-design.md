# Edit Command Design

**Date:** 2026-03-03
**Branch:** `rust`
**Scope:** `src/models/`, `src/io/`, `src/edit/`, `src/cli/`, `src/console/`

---

## Goal

Implement the `edit` subcommand in Rust, replicating the Python implementation on `main`. All
existing Python behaviour is preserved; the only intentional differences are startup speed and
compile-time safety (no `RefCell`, no runtime borrow checking).

---

## Data Model Changes

### `PrimitiveValue` (new, `src/models/json_schema.rs`)

```rust
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

`#[serde(untagged)]` makes it round-trip as bare JSON primitives. `Null` serializes as `null`.
Only primitive JSON values are accepted; arbitrary objects/arrays are rejected at the type level.

### `TypeBase` extension (`src/models/json_schema.rs`)

```rust
pub struct TypeBase {
    pub description: Option<String>,
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<PrimitiveValue>,
}
```

- `None` → field absent from JSON output (no `"default"` key emitted)
- `Some(PrimitiveValue::Null)` → `"default": null` emitted
- Distinction between "absent" and "explicitly null" is critical for `non_nullable` and
  `set_default_version`.

### `Config` / `AdditionalModel` fix (`src/models/config.rs`)

`AdditionalModel.schema` currently uses a raw `String` for the reference variant. Replace with a
proper inline `$ref` struct:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SchemaRef {
    #[serde(rename = "$ref")]
    pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum SchemaRootTypeOrReference {
    SchemaRootObject(SchemaRootObject),
    SchemaRootStrEnum(SchemaRootStrEnum),
    Reference(SchemaRef),
}
```

`Config.additional_fields` is extended similarly:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum AdditionalFieldOrRef {
    Field(AdditionalField),
    Reference(SchemaRef),
}

pub struct Config {
    pub non_nullable_fields: Vec<Regex>,
    pub additional_fields: Vec<AdditionalFieldOrRef>,  // changed
    pub additional_enum_items: Vec<AdditionalEnumItem>,
    pub additional_models: Vec<AdditionalModel>,
}
```

References are resolved at load time in `io/config.rs`; callers always receive `Vec<AdditionalField>`.

---

## `src/io/config.rs` (new)

```rust
pub fn load_config(path: &Path) -> Result<Config, String>
pub fn get_additional_schemas(config: &Config, config_path: &Path) -> Result<Vec<Schema>, String>
```

**`load_config`:**
1. Read and deserialize the JSON config.
2. Iterate `additional_fields`; for each `AdditionalFieldOrRef::Reference(r)`:
   - Resolve the path relative to the config file's parent directory.
   - Deserialize as `AdditionalField` or `Vec<AdditionalField>`.
   - Replace the reference entry with the resolved items.
3. Return the resolved `Config` (all fields are now concrete `AdditionalField` values, but the
   type is kept as `Config` with the internal `Vec<AdditionalField>` after resolution — or use a
   separate resolved config type if cleaner).

**`get_additional_schemas`:**
1. Iterate `config.additional_models`.
2. If `schema` is a `Reference`, read the referenced file and deserialize as `SchemaRootType`.
3. Validate `title` is non-empty (error otherwise — needed to derive the class name).
4. Construct `Schema::new(vec![module, title], None)` and call `schema.load_schema(text)`.
5. Return `Vec<Schema>`.

---

## `src/io/schemas.rs` — `read_schemas`

Add `walkdir` to `Cargo.toml`. Implement:

```rust
pub fn read_schemas(input_dir: &Path) -> Result<Schemas, String>
```

1. `read_version_file(input_dir)` → `version`.
2. Walk recursively with `walkdir`, keep `*.json` files, skip dotfiles (names starting with `.`).
3. For each file: strip `input_dir` prefix → relative path → strip `.json` → module path
   (`["bo", "Angebot"]` from `bo/Angebot.json`).
4. `Schema::new(module, None)` + `schema.load_schema(fs::read_to_string(path)?)`.
5. `schemas.add_schema(Rc::new(RefCell::new(schema)))`.

---

## `src/edit/update_refs.rs` — implement stub

`update_reference(reference, current_module, namespace) -> Result<(), String>`:

1. Try `REF_ONLINE_REGEX`:
   - Validate captured `version` == `schemas.version` (error on mismatch).
   - Build `reference_module_path` from `sub_path` + `model`.
2. Else try `REF_DEFS_REGEX`:
   - Look up `model` in `namespace`; error if not found.
   - `reference_module_path = namespace[model].clone()`.
3. Else: `cprint_verbose!` warning, return `Ok(())` unchanged.
4. Compute relative ref:
   - Walk `zip(reference_module_path, current_module)` until they diverge at index `i`.
   - `relative_ref = "../".repeat(current_module.len() - i - 1) + joined_remaining + ".json#"`.
   - Set `reference.r#ref = relative_ref`.

---

## `src/edit/non_nullable.rs` (new)

```rust
pub fn field_to_non_nullable(
    schema: &mut SchemaRootObject,
    field_name: &str,
) -> Result<(), String>
```

1. Get the property; assert it is `SchemaType::AnyOf`.
2. Find and remove the `SchemaType::NullSchema` variant from `any_of`.
3. If `default` is `Some(PrimitiveValue::Null)`, set `default = None` and add `field_name` to
   `required` if not already present.
4. If `any_of.len() == 1`, flatten: take the single remaining `SchemaType`, copy `title`,
   `description`, `default` from the `AnyOfSchema.base` onto its `base`, replace the property
   entry with the unwrapped type.

```rust
pub fn transform_all_non_nullable_fields(
    patterns: &[Regex],
    schemas: &mut Schemas,
    verbose: bool,
) -> Result<(), String>
```

1. Collect all `(field_path, field_name, schema_module)` triples up-front (avoids repeated
   re-iteration).
2. For each pattern: fullmatch against `field_path`, call `field_to_non_nullable` on matches.
3. `cprint!` match count summary; `cprint_verbose!` per-match lines; `cprint!` warning on zero
   matches.

---

## `src/edit/add.rs` (new)

```rust
pub fn transform_all_additional_fields(
    fields: &[AdditionalField],
    schemas: &mut Schemas,
    verbose: bool,
)
```

For each `AdditionalField`:
- Fullmatch pattern against module path of each `SchemaRootObject`.
- Insert `field_def` into `properties` under `field_name`.
- If `field_def.base.default` is `None` and `field_name` not in `required`, append to `required`.
- `cprint_verbose!` per-match; `cprint!` match count / warning.

```rust
pub fn transform_all_additional_enum_items(
    items: &[AdditionalEnumItem],
    schemas: &mut Schemas,
    verbose: bool,
)
```

Same structure for `SchemaRootStrEnum`, extending `enum_values`.

---

## `src/console/` — Global Console

### `src/console/console.rs`

```rust
pub static CONSOLE: OnceLock<Console> = OnceLock::new();

pub struct Console {
    verbose: bool,
    highlighter: RwLock<Highlighter>,
}

impl Console {
    pub fn new(verbose: bool) -> Self { ... }
    pub fn print(&self, msg: &str) { ... }           // always prints
    pub fn print_verbose(&self, msg: &str) { ... }   // only if verbose
    pub fn add_schema_names(&self, names: &[&str]) { ... } // write-locks highlighter
}
```

`RwLock<Highlighter>` — write lock taken once after `read_schemas`; thereafter all access is
read-only. No contention in practice (single-threaded edit pipeline).

### Macros (`src/console/mod.rs` or a dedicated `src/macros.rs`)

```rust
#[macro_export]
macro_rules! cprint {
    ($($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized")
            .print(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! cprint_verbose {
    ($($arg:tt)*) => {
        $crate::console::console::CONSOLE
            .get()
            .expect("CONSOLE not initialized")
            .print_verbose(&format!($($arg)*))
    };
}
```

### `src/console/highlighter.rs` (complete existing WIP)

Regex patterns matching Python's `BO4EHighlighter`:
- `BO4E` / `4E` (high priority)
- `bo` / `com` / `enum` (word boundary)
- Version strings (`v202401.1.0-rc1` pattern)
- Windows/Unix file paths
- Dynamically added schema names (via `add_schema_names`)

### `src/console/palette.rs`

Named `Style` constants matching the Python theme (`bo4e.bo`, `bo4e.version`, `bo4e.field`, etc.).

---

## `src/cli/edit.rs` (new)

```rust
#[derive(Args)]
pub struct Edit {
    #[arg(short = 'i', long = "input", required = true)]
    pub input_dir: PathBuf,

    #[arg(short = 'o', long = "output", required = true)]
    pub output_dir: PathBuf,

    #[arg(short = 'c', long = "config")]
    pub config_file: Option<PathBuf>,

    #[arg(long, default_value_t = true)]   // --no-default-version to disable
    pub set_default_version: bool,

    #[arg(long, default_value_t = true)]   // --no-clear-output to disable
    pub clear_output: bool,
}
```

`--verbose` is declared globally on the root CLI struct with `#[arg(global = true)]` and stored
in the root struct; `main()` initializes `CONSOLE` with it before dispatching.

**`run()` sequence:**

1. `clear_dir_if_needed(&output_dir, clear_output)`
2. `read_schemas(&input_dir)` → `schemas`
3. If config:
   - `load_config(&config_file)` → `config`
   - `get_additional_schemas(&config, &config_file)` → add to `schemas`
   - `CONSOLE.get().unwrap().add_schema_names(...)` (dynamic highlighting)
   - `cprint!("Added all additional models")`
   - `transform_all_additional_fields(...)`; `cprint!("Added all additional fields")`
   - `transform_all_non_nullable_fields(...)`; `cprint!("Transformed all non nullable fields")`
   - `transform_all_additional_enum_items(...)`; `cprint!("Added all additional enum items")`
4. If `set_default_version`: iterate schemas, if `SchemaRootObject` and `"_version"` in
   `properties`, set `properties["_version"].base.default = Some(PrimitiveValue::String(version))`
5. `update_references_all(&mut schemas)` (always, same as `pull`)
6. `write_schemas(&schemas, &output_dir)`

---

## Tests

| File | Tests |
|---|---|
| `models/json_schema.rs` | Round-trip `TypeBase.default` for null, string, number; absent default not emitted |
| `edit/update_refs.rs` | Online ref → relative; `$defs` ref → relative; unknown ref unchanged; version mismatch error |
| `edit/non_nullable.rs` | Removes null variant; removes null default + adds to required; flattens single-variant AnyOf |
| `edit/add.rs` | Pattern match inserts field; no-default → added to required; enum items extended |
| `cli/edit.rs` | Integration: invoke with test input dir + config, assert output files and schema structure |
