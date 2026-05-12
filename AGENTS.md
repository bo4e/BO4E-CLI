# AGENTS.md — BO4E-CLI

Guidance for AI agents (Claude Code, Codex, Cursor, Gemini, …) working in this repo.
Human-targeted prose belongs in `README.md`; deep structural detail belongs in `STRUCTURE.md` (root) and per-crate `STRUCTURE.md` files. This file is the playbook.

## 1. Project orientation

BO4E-CLI is a Rust workspace that ships a single binary `bo4e` for developers working with the [BO4E](https://www.bo4e.de/) energy-industry data model. The [BO4E-Schemas](https://github.com/bo4e/BO4E-Schemas) GitHub repo is the upstream source of truth.

Data flow (a mental model — see `STRUCTURE.md` for detail):

```
GitHub (BO4E-Schemas)  ── bo4e pull ──▶  schemas dir (.json + .version)
                                           │
                              bo4e edit ──┤  (config-driven transforms)
                                           ▼
                                       edited schemas dir
                                           │
                          bo4e generate ──▶ Python code (pydantic / sql-model / …)

      old schemas dir  ── bo4e diff schemas ──▶ JSON diff
      JSON diffs        ── bo4e diff matrix  ──▶ CSV/JSON compatibility matrix
      JSON diff         ── bo4e diff version-bump ──▶ technical / functional / major
      BO4E-python repo  ── bo4e repo versions    ──▶ version tag list (CI helper)
```

Three crates: `bo4e-schemas` (model + IO), `bo4e-codegen` (template-driven generators), `bo4e-cli` (commands, console, IO glue).

## 2. Documentation contract

- `README.md` — for end users: features, install, usage. Keep implementation-agnostic.
- `STRUCTURE.md` (root) — workspace overview, crate graph, architectural decisions.
- `crates/<crate>/STRUCTURE.md` — what the crate does, how it's used, key implementation details.
- Keep STRUCTURE.md lean — agents have small context windows. Prefer short pointers (`see crates/bo4e-codegen/src/python/pydantic.rs`) over inlining code.
- When you change structure or surface-area, update the matching STRUCTURE.md in the same PR. When you change user-visible behaviour, update README.md.
- Don't create other ad-hoc `.md` files (summaries, handoffs, planning notes) unless asked.

## 3. Build, verify, run

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check         # CI uses `cargo fmt -- --check`
cargo doc --no-deps --all-features # CI uses nightly + RUSTDOCFLAGS=--cfg docsrs

# Run the dev binary
cargo run -p bo4e-cli -- --help
cargo run -p bo4e-cli -- pull -t latest -o ./schemas
```

CI runs `fmt`, `clippy`, `doc`, and `test` (on macOS + Windows). Match it locally before pushing.

**Verify before claiming done.** Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` and confirm both pass. Never assert "should work" without evidence in the conversation.

## 4. Testing

- Unit tests live inline with `#[cfg(test)]` in the same file.
- Integration tests live in `crates/<crate>/tests/`.
- `tempfile` is a dev-dependency in every crate — use it for filesystem tests.
- Tests that mutate `std::env::set_current_dir` MUST take `bo4e_cli::test_lock::CWD_LOCK` (a process-global `Mutex`) for the duration of the test. Cargo runs tests in parallel and unprotected `set_current_dir` races silently corrupt other tests.
- Bug fixes need a regression test: **make it fail on the unfixed code first, then fix, then confirm it passes**. Note the failure mode in the test name.
- When you significantly change behaviour, add or update tests in the same PR.

## 5. Code conventions

- Rust 2024 edition.
- Library crates (`bo4e-schemas`, `bo4e-codegen`) surface errors with `thiserror`. The CLI binary uses `color-eyre` / `String` at the `Executable::run` boundary.
- No emoji in code or docs unless explicitly requested.
- Don't add features, abstractions, or backwards-compat shims that the task doesn't need. Three similar lines beat a premature trait. Don't design for hypothetical future requirements.
- Don't write comments that explain *what* — well-named identifiers do that. Only document non-obvious *why*: hidden constraints, workarounds, subtle invariants.
- Don't silence warnings/errors with `#[allow(...)]`, `.unwrap()`-spam, or `// TODO: fix later`. Investigate the root cause.
- Don't introduce unrelated refactors during a focused task.

## 6. Feature flags & output types

Output generators are gated by Cargo features. Existing flags (defined in both `bo4e-cli/Cargo.toml` and `bo4e-codegen/Cargo.toml`):

| Feature             | Effect                                                  |
| ------------------- | ------------------------------------------------------- |
| `python-pydantic`   | Compile in the Pydantic-v2 generator and its templates. |
| `python-sql-model`  | Compile in the SQLModel generator and its templates.    |
| `python`            | Umbrella feature — both of the above.                   |
| `default`           | All Python generators.                                  |

`OutputType` (in `bo4e-codegen`) has its variants gated by these features, so clap's `--output-type` only accepts compiled-in values.

**When asked to add a new output type:**

1. Generalize first. Find code shared with existing generators (`naming.rs`, `python/mod.rs` helpers, the MiniJinja environment, root-`__init__.py` / `__version__.py` plumbing) and lift it to a shared layer if a clean abstraction emerges. Be willing to redesign existing generators when it lets you reuse code — copy-paste is not acceptable.
2. Add a Cargo feature in `bo4e-codegen` (and re-export it from `bo4e-cli`).
3. Add a variant to `OutputType` gated by that feature.
4. Add embedded templates under `crates/bo4e-codegen/src/templates/<lang>/<flavour>/` and wire them up in `env.rs::load_embedded`.
5. Cover the new generator with at least one integration test under `crates/bo4e-codegen/tests/` and a smoke test under `crates/bo4e-cli/tests/`.
6. Update the relevant STRUCTURE.md files and the README feature list.

## 7. Design / plan workflow

Non-trivial work follows a design-then-plan workflow. Look at existing files under `docs/plans/` (e.g. `2026-05-08-generate-command-design.md`, `…-plan.md`) for the format and naming convention `YYYY-MM-DD-<topic>-{design,plan}.md`. Note: `docs/plans/` is `.gitignore`d — it's a personal scratchpad, not a shipped artefact.

## 8. Git, branches, and GitHub

- **Never commit directly on `main`.** Always work on a descriptively named feature branch (e.g. `feat/python-msgspec`, `fix/diff-dirty-version`).
- **Never attempt to merge PRs.**
- **Conventional commits — for both commit messages and PR titles.** The CHANGELOG is generated by `git-cliff` (see `cliff.toml`) from commit messages; only `feat`, `fix`, `perf`, `refactor`, and `docs` appear in the changelog (`chore`/`ci`/`build`/`style`/`test` are skipped).
- PR descriptions briefly explain what changes versus the base branch.
- **Ask before any GitHub create / update / delete action.** This covers: issues, PRs, comments, labels, releases, branch deletions, and **pushing a brand-new branch to origin**. The only exception is pushing additional commits to an already-existing remote branch (never `main`).
- Never bypass safety: no `--no-verify`, no editing CI to skip checks, no force-pushing shared branches.

## 9. CHANGELOG updates

`CHANGELOG.md` is generated by `git-cliff` with the config in `cliff.toml`. cargo-dist reads each `## X.Y.Z` section into the GitHub Release notes.

When asked to update the changelog:

1. Make sure you're on the latest commit of `main`, or on the branch / commit produced by the `release-prepare.yml` workflow.
2. Look at the new and the prior version sections side-by-side.
3. Rewrite the auto-generated bullets into a developer-facing summary — concise, grouped, focused on *what changed for users of this CLI*. Keep the heading shape `## X.Y.Z - YYYY-MM-DD` so cargo-dist can match it against the tag.

## 10. What NOT to do

- Don't create new top-level `.md` files (summaries, handoff notes) unless asked.
- Don't refactor unrelated code while doing a focused task.
- Don't silence type / lint / test failures — fix the root cause.
- Don't introduce a new dependency without a clear reason; check `Cargo.toml` for something existing first.
- Don't run destructive git operations (`reset --hard`, `clean -fd`, branch deletion, force push) without confirmation.
- Don't add emojis to code, commits, or docs unless explicitly asked.
