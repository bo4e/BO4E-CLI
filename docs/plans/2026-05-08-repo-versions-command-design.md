# `bo4e repo versions` Command Design

**Status:** approved (brainstorming)
**Author:** Leon Haffmans (with Claude)
**Date:** 2026-05-08

## Goal

Port the Python `bo4e repo versions` command to Rust. The command lists the last *n* version tags of a BO4E-python checkout, optionally filtered to drop release candidates, technical bumps, and tags that lack a corresponding GitHub release. Non-quiet mode renders a 3-column table (Version, Commit SHA, Commit date); quiet mode prints versions newline-separated for piping.

## Scope

- **In:** the `bo4e repo versions` subcommand and the supporting library code (`get_ref`, `release_exists`, the `get_last_n_tags` body, the pure filter, the table renderer).
- **In, prerequisite:** a small `Console` refactor that splits info (stdout) from warn/error (stderr). The current implementation routes everything through `eprintln!`, which violates the project-wide channel rule.
- **Out:** the `bo4e generate` command (separate Python sub-app, doesn't touch git, separate plan).

## Non-goals

- No new GitHub features beyond `release_exists`.
- No changes to existing `pull` / `edit` / `diff` behavior beyond the Console channel migration (those commands' existing `cprint_*` calls move from stderr to stdout where they are info-level).
- No multi-command `repo` sub-app at this stage; only `versions`. The clap structure leaves room to add siblings later.

## Architecture

```
src/
├── cli/
│   ├── base.rs              ← register Repo subcommand variant
│   └── repo.rs              ← NEW: clap surface, run handler, table renderer
├── io/
│   ├── git.rs               ← extend: implement get_last_n_tags body, add get_ref()
│   │                          and tags_merged(); make is_version_tag/is_branch/
│   │                          is_commit_hash public so get_ref can call them.
│   └── github.rs            ← extend: add release_exists()
├── repo/
│   └── filter.rs            ← NEW: pure filter_tags() + FilterOptions
├── models/
│   └── git.rs               ← extend: add RefKind enum
└── console/
    └── console.rs           ← refactor: split info vs warn/error routing
```

Three new files. Five existing files extended. No new crates: the table is hand-rolled using the already-present `console` crate for color and the standard library for layout.

The `repo/` top-level domain module mirrors `diff/`, keeping pure logic out of `io/` and `cli/`.

## Components

### `models/git.rs` — `RefKind` enum

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefKind { Tag, Branch, Commit }
```

The existing `Reference` enum stays as-is; `RefKind` is the lighter shape returned by `get_ref` (kind + resolved string).

### `repo/filter.rs` — pure filter

```rust
pub struct FilterOptions {
    pub n: u32,                  // 0 = all since threshold
    pub exclude_candidates: bool,
    pub exclude_technical_bumps: bool,
    pub skip_first: bool,        // true iff ref_type == Tag
    pub threshold: Version,      // hard floor; default v202401.0.0
}

pub fn filter_tags(
    candidates: &[Version],      // descending: candidates[0] is newest
    opts: &FilterOptions,
    is_release: impl FnMut(&Version) -> Result<bool, String>,
) -> Result<Vec<Version>, String>
```

The `is_release` closure is injected by the caller. Production passes a closure that calls `release_exists`; tests pass `|_| Ok(true)` or pre-canned answers. This is what makes the filter pure-yet-testable.

Algorithm (in this exact order — stop rules first, then skip rules):

```rust
pub fn filter_tags(
    candidates: &[Version],
    opts: &FilterOptions,
    mut is_release: impl FnMut(&Version) -> Result<bool, String>,
) -> Result<Vec<Version>, String> {
    let mut out = Vec::new();
    let mut last: Option<&Version> = None;       // last YIELDED version
    for (i, v) in candidates.iter().enumerate() {
        if opts.n > 0 && out.len() as u32 >= opts.n        { break; }
        if opts.n == 0 && *v == opts.threshold             { break; }
        if opts.exclude_candidates && v.is_release_candidate() { continue; }
        if opts.exclude_technical_bumps
            && let Some(prev) = last
            && prev.bumped_technical(v)                    { continue; }
        if i == 0 && opts.skip_first                       { continue; }
        if !is_release(v)?                                 { continue; }
        out.push(v.clone());
        last = out.last();
    }
    Ok(out)
}
```

Decisions baked in:
- **`last` tracks the last yielded version**, not the last seen. Matters for `exclude_technical_bumps`.
- **`skip_first` is by index in input**, not by index after filters. If the first input is also an RC and `--exclude-candidates` is on, it gets skipped by RC, not by `skip_first`.
- **`is_release` is fallible.** A network error aborts the whole listing rather than silently treating it as "skip". Users can pass `--no-validate-releases` to bypass the network entirely.
- **Stop ordering deviates from Python.** Python applies `is_release` last (i.e. release-validates a tag we'd skip anyway); this design checks `is_release` after `skip_first` but still after the cheap predicates. The user-visible result is identical; the order avoids unnecessary network calls.

### `io/git.rs` — extensions

```rust
pub fn get_ref(value: &str) -> io::Result<(RefKind, String)>;
pub fn tags_merged(reference: &str) -> io::Result<Vec<String>>;
pub fn get_commit_sha(branch_or_tag: &str) -> io::Result<String>;   // already a stub
pub fn get_commit_date(commit: &str) -> io::Result<String>;          // already a stub
pub fn get_last_n_tags(opts: GetLastNTagsOpts) -> Result<Vec<Version>, String>;
```

`get_ref` falls back to current HEAD if the value isn't a tag, branch, or commit, and emits an info message naming the fallback:

```rust
pub fn get_ref(value: &str) -> io::Result<(RefKind, String)> {
    if is_version_tag(value)?  { return Ok((RefKind::Tag,    value.into())); }
    if is_branch(value)?       { return Ok((RefKind::Branch, value.into())); }
    if is_commit_hash(value)?  { return Ok((RefKind::Commit, value.into())); }
    if value == "HEAD"         { return Ok((RefKind::Commit, get_commit_sha("HEAD")?)); }
    let cur = get_commit_sha("HEAD")?;
    cprint_info!("'{value}' is not a tag, branch, or commit; falling back to HEAD ({cur}).");
    Ok((RefKind::Commit, cur))
}
```

`tags_merged` shells out to `git tag --merged <ref> --sort=-version:refname --sort=-creatordate`, splits lines, drops empties, returns the list verbatim (no version parsing — that happens in `get_last_n_tags`).

`get_last_n_tags` is the thin wrapper that ties everything together: call `tags_merged`, parse each line to `Version` (silently dropping unparseable strings, with a single `cwarn_*!` naming each), pass to `filter_tags` with an `is_release` closure built from the validate flag plus the token.

The currently-private `is_version_tag`, `is_branch`, `is_commit_hash` become `pub` so `get_ref` can call them. The currently-`#[allow(dead_code)]` markers come off as the items become live.

### `io/github.rs` — `release_exists`

```rust
pub async fn release_exists(version: &Version, token: Option<&str>) -> Result<bool, String> {
    let octocrab = get_octocrab_instance(token)?;
    match get_bo4e_schemas_repo_handler(&octocrab)
        .releases().get_by_tag(&version.to_string()).await
    {
        Ok(_) => Ok(true),
        Err(octocrab::Error::GitHub { source, .. }) if source.status_code == 404 => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
```

This checks for a GitHub *Release object* (a first-class API resource), not just whether the tag was pushed. A pushed tag without an associated Release returns 404 from `get_by_tag`, and we treat that as `Ok(false)`.

### `console/console.rs` — channel split (prerequisite)

The current `Console::print` always uses `eprintln!`. This violates the project-wide channel rule. The refactor:

```rust
pub fn print_info(&self, message_level: Level, msg: &str) { /* stdout */ }
pub fn print_warn(&self, msg: &str)                       { /* stderr */ }
pub fn print_error(&self, msg: &str)                      { /* stderr */ }
```

Plus three macros:
- `cprint_info!` (renames the existing `cprint_normal!` and `cprint_verbose!` family — both go to stdout, just with different `Level`s)
- `cwarn_*!` → stderr, never suppressed by quiet
- `cerror_*!` → stderr, never suppressed by quiet (rare; errors usually bubble through `Result`)

Existing call sites: rename `cprint_normal!` → `cprint_info!` with `Level::Normal`; `cprint_verbose!` → `cprint_info!` with `Level::Verbose`. No call site is currently producing warnings or errors via the macros, so there's nothing to migrate to `cwarn_*!`.

### `cli/repo.rs` — clap surface

```rust
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
```

`cli/base.rs` gets one new variant:

```rust
pub enum SubcommandsLevel1 { Pull(Pull), Edit(Edit), Diff(Diff), Repo(Repo) }
```

Decisions:
- Short flags match Python (`-n -r -c -t -s`).
- `--no-validate-releases` follows the existing `--no-major` pattern in `version-bump`.
- `--token` mirrors `pull`'s pattern (`env = "GITHUB_TOKEN"`, falls back to `gh auth token` inside the runner).
- No per-command `--quiet` — the global `-q`/`--quiet` (already present on `Cli`) is what users get.

### Sync/async glue

`release_exists` is async (octocrab); everything else in the chain is sync. `Executable::run` is sync. We follow the existing `cli/pull.rs` pattern: build the runtime via `utils::tokio::get_runtime()` once at the top of `run`, and the `is_release` closure passed to `filter_tags` does `runtime.block_on(release_exists(v, token.as_deref()))`. When `--no-validate-releases` is set, the closure is `|_| Ok(true)` and the runtime is never built.

## Data flow

```
                ┌─────────────────────────┐
                │ cli/repo.rs::run        │
                └──────────┬──────────────┘
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
   ┌────────────────────┐    ┌──────────────────────┐
   │ get_ref(value)     │    │ build is_release     │
   │ → (RefKind, String)│    │   closure            │
   └─────────┬──────────┘    │ (--no-validate-...   │
             │                │  → |_| Ok(true))    │
             ▼                └──────────┬───────────┘
   ┌────────────────────┐                │
   │ tags_merged(ref)   │                │
   │ → Vec<String>      │                │
   └─────────┬──────────┘                │
             │                            │
             ▼                            │
   ┌────────────────────┐                │
   │ parse to Vec<Version>               │
   │ (drop unparseable, warn)            │
   └─────────┬──────────┘                │
             │                            │
             ▼                            │
   ┌──────────────────────────────────┐ │
   │ filter_tags(candidates, opts,    │◄┘
   │             is_release closure)  │
   │ → Vec<Version>                   │
   └─────────┬─────────────────────────┘
             │
   ┌─────────┴────────────────┐
   │                          │
   ▼ (quiet)                  ▼ (non-quiet)
┌──────────────┐    ┌─────────────────────────────┐
│ print verses │    │ for each version:           │
│ to stdout,   │    │   get_commit_sha            │
│ one per line │    │   get_commit_date           │
└──────────────┘    │ render_table(rows) → stdout │
                    └─────────────────────────────┘
```

## Output behavior

**Channel rule (project-wide):** info → stdout; warnings and errors → stderr. Non-quiet mode is not parsable; quiet mode is.

**Normal/Verbose mode:**
1. Status chatter ("Resolved 'main' to commit abc123…", "Found 47 tags, validating releases…") via `cprint_info!` → stdout.
2. Title line:
   - `n == 0` → `All versions between v202401.0.0 and {ref-display}`
   - `n != 0` → `Last {n} versions before {ref-display}`
   - `{ref-display}` is `"{tag}"`, `"latest commit on branch {branch}"`, or `"commit {sha[:6]}"` per `RefKind`.
3. Hand-rolled 3-column table to stdout: `Version | Commit SHA | Commit date`. Header bold; alternating rows dimmed; column widths computed from content.
4. Empty result: print the title and an italic `(no versions found)` line. Don't render an empty table.

**Quiet mode (`-q` / `--quiet`):**
1. No chatter (suppressed by `Console::Level::Quiet`).
2. Versions only, one per line, **stdout**. Nothing else on stdout.
3. Skip the per-version `get_commit_sha` / `get_commit_date` calls — they're only needed for the table.

**Errors and warnings (always stderr, regardless of `--quiet`):**
- Errors from `get_ref` / `tags_merged` / release validation bubble as `Err(String)` to `main`, which prints to stderr and exits non-zero.
- Warnings (e.g. "fewer than n tags found", "unparseable tag '…' skipped") use `cwarn_*!` → stderr.

## Error handling

- `Result<(), String>` end-to-end, matching the rest of the crate.
- `io::Error` → `String` at the boundary inside `cli/repo.rs::run` via `.map_err(|e| e.to_string())`. `io/git.rs` keeps `io::Result<…>` internally.
- Specific failure modes:
  - `get_ref` only fails if `git` itself fails (corrupt repo, etc.); never on "unknown ref" — that path falls back to HEAD with an info message.
  - `tags_merged` propagates git failures verbatim. An empty result is `Ok(vec![])`, not an error.
  - Tag strings that don't parse as `Version` are dropped with a single `cwarn_*!` per tag.
  - `release_exists`: 404 → `Ok(false)`; rate-limit (403) → `Err` with a message suggesting `--token` or `--no-validate-releases`; other network errors → `Err`.
  - "Asked for n=10, found 3" → `cwarn_*!`; not an error.

## Testing strategy

Three layers, in priority order:

**Layer 1 — Pure filter (`repo/filter.rs`).** Heaviest investment. Synthetic `Version` lists, closure stub for `is_release`. Cases:
- Empty candidates → empty output.
- `n=0`, threshold reached → stops at threshold (threshold itself excluded).
- `n=0`, threshold absent → returns all.
- `n=5`, only 3 available → returns 3 (no error from the pure filter; warning is the caller's job).
- `exclude_candidates` drops RCs only.
- `exclude_technical_bumps` keeps the newest of each technical group, against the *last yielded* baseline.
- `skip_first=true` drops index 0 even if it would otherwise pass.
- `skip_first=true` + RC at index 0 + `exclude_candidates` → still only one drop, not two (skip_first is by input index).
- `is_release` returns `Ok(false)` → that version skipped; iteration continues.
- `is_release` returns `Err` → filter aborts and propagates.
- Combination: `n=3`, RCs+technical-bumps excluded, `skip_first=true`, all-releases → exactly 3.

**Layer 2 — I/O wrappers (`io/git.rs`).** One real-git fixture test using `tempfile::tempdir()`:
- `git init`, configure user, make 4 commits with annotated tags `v202401.0.1`, `v202401.0.2`, `v202401.1.0`, `not-a-version`.
- Call `tags_merged("HEAD")` and assert ordering + content.
- Call `get_ref("v202401.0.1")` → `(Tag, _)`; `get_ref("nonsense")` → `(Commit, HEAD-sha)` with HEAD fallback.
- Call `get_commit_sha("v202401.0.1")` and assert it matches `git rev-parse v202401.0.1`.
- `release_exists` is **not** tested directly (live GitHub call). Coverage comes via the closure-stubbed pure filter.

**Layer 3 — CLI smoke (`cli/repo.rs`).** One end-to-end test on a temp git repo, invoking `run` with `validate_releases=false`:
- Quiet variant: capture stdout, assert it's exactly the expected version strings newline-separated.
- Non-quiet variant: capture stdout, assert it contains the title and the table headers.

## Open questions

None at this stage. Brainstorming closed.
