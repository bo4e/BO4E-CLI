# Dependabot auto-approve / auto-merge — Design

Status: approved
Date: 2026-05-12

## Goal

Re-introduce the Dependabot auto-approve / auto-merge workflow that previously
existed in this repository, with one improvement over the original: approval
and auto-merge happen **only after the CI workflow has succeeded** for the
Dependabot pull request.

The original (for reference) approved and enabled auto-merge on every
Dependabot PR unconditionally, regardless of CI state:

```yaml
name: Dependabot auto-approve / -merge
on: pull_request

jobs:
  dependabot:
    permissions:
      contents: write
      pull-requests: write
    runs-on: ubuntu-latest
    env:
      PR_URL: ${{github.event.pull_request.html_url}}
      GITHUB_TOKEN: ${{secrets.GITHUB_TOKEN}}
    if: ${{ github.actor == 'dependabot[bot]' }}
    steps:
      - name: Approve a PR
        run: gh pr review --approve "$PR_URL"
      - name: Enable auto-merge for Dependabot PRs
        run: gh pr merge --auto --squash "$PR_URL"
```

## Scope

In scope:

- A single new workflow file at `.github/workflows/dependabot-automerge.yml`.
- All Dependabot pull requests are eligible — no filtering by semver level
  (patch / minor / major). CI is the only gate.

Out of scope:

- No semver-based filtering (e.g. via `dependabot/fetch-metadata`).
- No grouping or batching of Dependabot PRs.
- No retries, `continue-on-error`, or other failure-recovery logic — failures
  are loud, rare, and self-healing on the next Dependabot push.
- No changes to the existing `ci.yml` workflow.
- No PAT; the standard `GITHUB_TOKEN` is sufficient.

## Approach

Use a separate workflow triggered by the
[`workflow_run`](https://docs.github.com/en/actions/writing-workflows/choosing-when-your-workflow-runs/events-that-trigger-workflows#workflow_run)
event on the `CI` workflow. Two alternatives were considered and rejected:

- **Add an `automerge` job to `ci.yml` gated with `needs: [fmt, clippy, doc,
  test]` and an `if:` on the actor.** Simpler topology, but the workflow file
  modifications from a Dependabot PR would run from the PR head rather than
  from `main`. A `workflow_run`-triggered workflow always runs the version of
  the file currently on the default branch, which is the safer default.
- **Keep `pull_request` trigger and add a wait-for-checks step.** Burns runner
  minutes idling, fragile w/r/t reruns, and requires a third-party action.

`workflow_run` is the GitHub-recommended pattern for this scenario.

## Workflow design

### Trigger

```yaml
on:
  workflow_run:
    workflows: [CI]
    types: [completed]
```

Fires every time the `CI` workflow finishes for any reason. The job-level
filter (next section) decides whether to act.

### Job-level filter

The single job runs only when **all** of the following are true:

- `github.event.workflow_run.conclusion == 'success'` — CI passed.
- `github.event.workflow_run.event == 'pull_request'` — the originating CI run
  was for a pull request (not a push to `main` / `master` / `develop`).
- `github.event.workflow_run.actor.login == 'dependabot[bot]'` — the
  originating CI run was triggered by Dependabot.

The `actor.login` check has a deliberate safety property: if a human pushes a
commit onto a Dependabot branch (e.g. to fix a failing test), the new CI run's
actor becomes the human, the automerge job is skipped, and the PR stays under
human control. Dependabot can resume autonomy on its next push.

### Steps

1. **Resolve the PR number** from
   `github.event.workflow_run.pull_requests[0].number`. This is populated for
   same-repository branches, which is always the case for Dependabot. If empty
   (defensive — should not happen under the filters above), the workflow
   exits cleanly with a log line.
2. **Approve**: `gh pr review --approve <pr-number>`.
3. **Enable auto-merge**: `gh pr merge --auto --squash <pr-number>`.

Squash strategy matches the original workflow.

### Permissions and token

```yaml
permissions:
  contents: write
  pull-requests: write
env:
  GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

`workflow_run` events run in the context of the default branch and are not
subject to the read-only `GITHUB_TOKEN` restriction that applies to
`pull_request` events from Dependabot. No personal access token is required.

The repository setting **Settings → Actions → General → Workflow permissions
→ "Allow GitHub Actions to create and approve pull requests"** must be
enabled for the approval step. This is verified during execution; if the
setting is off, the implementation step surfaces it so the maintainer can
toggle it before merging.

### Error handling

- **CI failed** → the job's `if:` evaluates false → workflow is skipped.
  This is the intended behavior.
- **No PR associated** (defensive) → the resolve step exits 0 with a log
  line, subsequent steps are skipped.
- **`gh` API failure on approve or merge** → the step fails, the workflow run
  is marked failed, but nothing is blocked. The next Dependabot push triggers
  CI again and the workflow re-runs. No retries needed.

## Concrete workflow file

The proposed file content (subject to the implementation plan's final
adjustments):

```yaml
name: Dependabot auto-approve / -merge

on:
  workflow_run:
    workflows: [CI]
    types: [completed]

jobs:
  dependabot:
    if: >-
      github.event.workflow_run.conclusion == 'success' &&
      github.event.workflow_run.event == 'pull_request' &&
      github.event.workflow_run.actor.login == 'dependabot[bot]'
    runs-on: ubuntu-latest
    permissions:
      contents: write
      pull-requests: write
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    steps:
      - name: Resolve PR number
        id: pr
        run: |
          NUM='${{ github.event.workflow_run.pull_requests[0].number }}'
          if [ -z "$NUM" ] || [ "$NUM" = "null" ]; then
            echo "No PR associated with this workflow_run; nothing to do."
            exit 0
          fi
          echo "number=$NUM" >> "$GITHUB_OUTPUT"
      - name: Approve PR
        if: steps.pr.outputs.number != ''
        run: gh pr review --approve --repo "$GITHUB_REPOSITORY" "${{ steps.pr.outputs.number }}"
      - name: Enable auto-merge
        if: steps.pr.outputs.number != ''
        run: gh pr merge --auto --squash --repo "$GITHUB_REPOSITORY" "${{ steps.pr.outputs.number }}"
```

## Testing strategy

`workflow_run` cannot be fully exercised on a feature branch — the trigger
only fires for workflows on the default branch. Pre-merge confidence:

- Static check the YAML and expression syntax with `actionlint` (or by
  inspection if `actionlint` is not available locally).
- Verify the repository setting "Allow GitHub Actions to create and approve
  pull requests" is enabled. If not, document the required toggle in the PR
  description.

Post-merge: the next Dependabot PR exercises the workflow end-to-end. If
anything is wrong, the failure surfaces in the Actions tab and the PR simply
does not auto-merge — no damage done.

## Rollback

Delete `.github/workflows/dependabot-automerge.yml`. No other file is
touched, so reversion is a one-file revert.
