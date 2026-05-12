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
- **STRUCTURE.md updates are mandatory and unprompted.** Whenever you change repo structure, crate boundaries, public API, file layout, feature flags, or template directories, update the root `STRUCTURE.md` *and* each affected crate's `STRUCTURE.md` **in the same change** — don't wait to be asked. List the STRUCTURE.md updates explicitly when writing a design/plan migration list.
- When you change user-visible behaviour, update `README.md` in the same change.
- **Never modify `CHANGELOG.md` unless the user explicitly asks.** Even on breaking changes, release-worthy features, or version bumps — leave `CHANGELOG.md` alone. The user owns release-note framing and the `git-cliff` / `cargo-dist` pipeline. Do not list `CHANGELOG.md` edits in any plan or design. See §9 for the on-request workflow.
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

- **Commit finished work without asking.** Once a discrete unit of work is done, stage and commit it — don't wait for an explicit "commit it" prompt. The meaningful confirmation gate is the push, not the local commit.
- **Never commit directly on `main`.** If the working tree is on `main`, create a descriptively named feature branch first (e.g. `feat/python-msgspec`, `fix/diff-dirty-version`, `docs/agent-guidance`), then commit on that branch. The branch-create + commit happen in the same turn — no extra prompt needed.
- **Never attempt to merge PRs.**
- **Conventional commits — for both commit messages and PR titles.** The CHANGELOG is generated by `git-cliff` (see `cliff.toml`) from commit messages; only `feat`, `fix`, `perf`, `refactor`, and `docs` appear in the changelog (`chore`/`ci`/`build`/`style`/`test` are skipped).
- PR descriptions briefly explain what changes versus the base branch.
- **Ask before pushing a brand-new branch to origin.** The local commit is fine without a prompt; the *first push* of a new branch is the moment it becomes visible to others, so confirm first. Once the branch exists on origin, push follow-up commits without asking (never to `main`).
- **Ask before any other GitHub create / update / delete action** — issues, PRs, comments, labels, releases, branch deletions.
- Never bypass safety: no `--no-verify`, no editing CI to skip checks, no force-pushing shared branches.

## 9. Stacked PR workflow

When a piece of work naturally splits into a refactor + feature (or any A-then-B sequence) **and** the user asks for two PRs, do not block on PR A's merge before starting PR B. The workflow is:

1. **Branch A off `main`.** Implement the first piece (e.g. the refactor). Test, commit, ask before the first push, then push. Create PR A → `main`. **Do not attempt to merge it.**
2. **Branch B off branch A's HEAD** (the commit you just pushed). Implement the second piece (e.g. the feature). Test, commit, ask before the first push, push. Create PR B → `main`. PR B will show the union of A's and B's diffs against `main`; that's expected and collapses to just B's changes once A merges.
3. **If during B you discover a bug that belongs in A:**
   1. Switch to branch A (`git switch <branch-A>`).
   2. Fix the bug, commit, run the full test loop locally.
   3. Push (no extra prompt needed — branch already exists on origin).
   4. Switch back to branch B (`git switch <branch-B>`).
   5. Merge A into B (`git merge <branch-A>`) so B picks up the fix.
   6. Resume work on B.
4. **Never merge PR A locally into PR B by rebasing** unless the user explicitly asks. `git merge` keeps the lineage clear; the merge commit makes the dependency obvious to a reviewer of PR B.
5. **Confirmation gates** are unchanged: first-push prompt per branch, ask before any GitHub create/update action, never merge a PR yourself.

This pattern applies to any sequence of dependent PRs, not just refactor-then-feature.

## 10. CHANGELOG updates

**Do not touch `CHANGELOG.md` unless the user explicitly asks.** This is non-negotiable — see §2. The rules below describe the workflow *when asked*.



`CHANGELOG.md` is generated by `git-cliff` with the config in `cliff.toml`. cargo-dist reads each `## X.Y.Z` section into the GitHub Release notes.

When asked to update the changelog:

1. Make sure you're on the latest commit of `main`, or on the branch / commit produced by the `release-prepare.yml` workflow.
2. Look at the new and the prior version sections side-by-side.
3. Rewrite the auto-generated bullets into a developer-facing summary — concise, grouped, focused on *what changed for users of this CLI*. Keep the heading shape `## X.Y.Z - YYYY-MM-DD` so cargo-dist can match it against the tag.

## 11. What NOT to do

- Don't modify `CHANGELOG.md` unless explicitly asked — see §2 and §10.
- Don't skip the matching `STRUCTURE.md` update when you change repo structure or public API — see §2.
- Don't create new top-level `.md` files (summaries, handoff notes) unless asked.
- Don't refactor unrelated code while doing a focused task.
- Don't silence type / lint / test failures — fix the root cause.
- Don't introduce a new dependency without a clear reason; check `Cargo.toml` for something existing first.
- Don't run destructive git operations (`reset --hard`, `clean -fd`, branch deletion, force push) without confirmation.
- Don't add emojis to code, commits, or docs unless explicitly asked.
