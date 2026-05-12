# Dependabot auto-approve / auto-merge — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-add the Dependabot auto-approve / auto-merge workflow, gating both actions on a successful CI run.

**Architecture:** A new GitHub Actions workflow at `.github/workflows/dependabot-automerge.yml`. Triggered by `workflow_run` on the existing `CI` workflow with `types: [completed]`. A single job runs only when CI succeeded AND the originating run was a Dependabot pull request, and performs `gh pr review --approve` followed by `gh pr merge --auto --squash`. Specified in `docs/superpowers/specs/2026-05-12-dependabot-automerge-design.md`.

**Tech Stack:** GitHub Actions (`workflow_run` event), GitHub CLI (`gh`), default `GITHUB_TOKEN`. No PAT required. No third-party actions.

---

## File Structure

- Create: `.github/workflows/dependabot-automerge.yml` — the only new file. Self-contained workflow definition.
- No other files are modified.
- The existing `.github/workflows/ci.yml` is unchanged but is referenced by name in the new workflow's `on.workflow_run.workflows` array.

---

## Task 1: Preflight — verify repo permission setting

The approval step requires the repository setting **Settings → Actions → General → Workflow permissions → "Allow GitHub Actions to create and approve pull requests"** to be enabled. This task surfaces its current state so the maintainer can toggle it on if needed before merging.

**Files:** None (read-only check).

- [ ] **Step 1: Query the repo's Actions workflow permissions**

Run:
```bash
gh api repos/bo4e/BO4E-CLI/actions/permissions/workflow
```

Expected output (relevant fields):
```json
{
  "default_workflow_permissions": "read" | "write",
  "can_approve_pull_request_reviews": true | false
}
```

- [ ] **Step 2: Interpret the result and report to the user**

- If `can_approve_pull_request_reviews` is `true`: report "Setting is on, proceeding." and continue.
- If `can_approve_pull_request_reviews` is `false`: stop and report:

  > The repo setting "Allow GitHub Actions to create and approve pull requests" is currently OFF. The approval step will fail until you enable it at: https://github.com/bo4e/BO4E-CLI/settings/actions → "Workflow permissions" → check "Allow GitHub Actions to create and approve pull requests" → Save.
  >
  > I can proceed and create the workflow file regardless — the workflow will simply produce a failed run on the first Dependabot PR until the setting is toggled. Or I can pause here until you've toggled it. Which do you prefer?

Wait for user direction before continuing if the setting is off.

---

## Task 2: Create the workflow file

**Files:**
- Create: `.github/workflows/dependabot-automerge.yml`

- [ ] **Step 1: Write the workflow file**

Create `/repos/bo4e-cli/.github/workflows/dependabot-automerge.yml` with this exact content:

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

Notes on the contents:
- `if: >-` folds the three conditions into a single expression string for the job-level filter. All three must be `true`.
- The `actor.login` check has a deliberate safety property: when a human pushes onto a Dependabot branch, the next CI run's `actor` is the human and this job is skipped.
- `workflow_run.pull_requests[0].number` is populated because Dependabot branches live in the same repository. The defensive `null`/empty check is belt-and-braces — it should not trigger under the filters above.
- Using `--repo "$GITHUB_REPOSITORY"` plus the PR number is clearer than reconstructing `html_url`.

---

## Task 3: Validate the workflow YAML syntax locally

There is no full GitHub Actions schema validator available in this environment (no `actionlint`, no `docker`). Python is available, so we use it for YAML syntax + structural sanity. Full expression validation happens server-side on push.

**Files:** None modified.

- [ ] **Step 1: Validate YAML parses and has the expected top-level shape**

Run:
```bash
python3 -c "
import sys, yaml
with open('.github/workflows/dependabot-automerge.yml') as f:
    doc = yaml.safe_load(f)
assert isinstance(doc, dict), 'top-level is not a mapping'
assert doc.get('name'), 'missing name'
# Note: PyYAML parses unquoted 'on:' as boolean True. Accept either key.
on_key = 'on' if 'on' in doc else (True if True in doc else None)
assert on_key is not None, 'missing on trigger'
trig = doc[on_key]
assert 'workflow_run' in trig, 'missing workflow_run trigger'
assert trig['workflow_run'].get('workflows') == ['CI'], 'workflow_run.workflows mismatch'
assert trig['workflow_run'].get('types') == ['completed'], 'workflow_run.types mismatch'
job = doc['jobs']['dependabot']
assert 'if' in job, 'missing job-level if'
assert job.get('runs-on') == 'ubuntu-latest', 'wrong runs-on'
assert job['permissions'] == {'contents': 'write', 'pull-requests': 'write'}, 'permissions mismatch'
steps = job['steps']
assert len(steps) == 3, f'expected 3 steps, got {len(steps)}'
assert steps[0]['id'] == 'pr', 'first step must have id: pr'
print('OK: YAML valid; structure matches spec.')
"
```

Expected output:
```
OK: YAML valid; structure matches spec.
```

If any assertion fails: re-read the file vs. the YAML in Task 2 Step 1, fix the divergence, and re-run.

- [ ] **Step 2: Confirm GitHub Actions expression syntax balances**

Run a simple grep to confirm `${{` and `}}` counts match (catches typos in the expressions):
```bash
python3 -c "
import re
src = open('.github/workflows/dependabot-automerge.yml').read()
opens = len(re.findall(r'\\\${{', src))
closes = len(re.findall(r'}}', src))
assert opens == closes, f'\${{}} mismatch: {opens} open vs {closes} close'
print(f'OK: {opens} balanced \${{...}} expressions.')
"
```

Expected output:
```
OK: 4 balanced ${...} expressions.
```

The four are: `secrets.GITHUB_TOKEN` in `env:`, `github.event.workflow_run.pull_requests[0].number` in the resolve step's script, and `steps.pr.outputs.number` used as the positional `gh` argument in each of the two final steps. (Bare expressions inside `if:` job/step conditions do not use `${{ }}` braces, so they're not counted.)

If the counts diverge: fix the workflow file.

---

## Task 4: Commit the workflow file

**Files:** Stage only the newly created workflow.

- [ ] **Step 1: Stage the file**

Run:
```bash
git add .github/workflows/dependabot-automerge.yml
git status
```

Expected: exactly one new file staged — `.github/workflows/dependabot-automerge.yml`.

- [ ] **Step 2: Create the commit**

Run:
```bash
git commit -m "$(cat <<'EOF'
ci: re-add dependabot auto-approve / auto-merge gated on CI success

Triggered by workflow_run on CI completion. Approves and enables
--auto --squash merge only when the originating CI run was a
pull_request from dependabot[bot] and concluded successfully.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

Expected: commit created on branch `feat/dependabot-automerge`, 1 file changed.

- [ ] **Step 3: Verify commit landed**

Run:
```bash
git log --oneline -2
```

Expected: top entry is the new `ci:` commit; second entry is the spec `docs:` commit from brainstorming.

---

## Task 5: Push and open a pull request

Confirm with the user before doing this — it makes the change visible. Skip this task if they want to push themselves.

**Files:** None modified.

- [ ] **Step 1: Push the branch**

Run:
```bash
git push -u origin feat/dependabot-automerge
```

Expected: branch pushed; remote tracking set.

- [ ] **Step 2: Open the PR**

Run:
```bash
gh pr create --title "ci: re-add dependabot auto-approve / auto-merge gated on CI success" --body "$(cat <<'EOF'
## Summary
- Re-introduces the Dependabot auto-approve / auto-merge workflow that previously existed in the repo.
- **Improvement over the original:** approval and auto-merge now happen only when the CI workflow has concluded successfully for the Dependabot PR. The original approved + merged unconditionally on PR open.

## How it works
- Triggered by `workflow_run` on the `CI` workflow with `types: [completed]`.
- Job-level filter requires all three: `conclusion == 'success'`, `event == 'pull_request'`, `actor.login == 'dependabot[bot]'`.
- Approves via `gh pr review --approve`, then enables auto-merge with `--auto --squash`.
- Uses the default `GITHUB_TOKEN`; no PAT required.

## Prerequisite
The repository setting **Settings → Actions → General → Workflow permissions → "Allow GitHub Actions to create and approve pull requests"** must be enabled. This was verified in Task 1 of the implementation plan — see PR comments / logs for the result.

## Test plan
- [ ] Merge this PR.
- [ ] On the next Dependabot PR, confirm CI runs first, then the new `Dependabot auto-approve / -merge` workflow runs and approves + enables auto-merge.
- [ ] On a non-Dependabot PR, confirm the new workflow either does not run or runs and is skipped (`if:` evaluates false).

## Spec & plan
- Spec: `docs/superpowers/specs/2026-05-12-dependabot-automerge-design.md`
- Plan: `docs/superpowers/plans/2026-05-12-dependabot-automerge.md`

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

Expected: `gh` prints the PR URL. Report it to the user.

---

## Post-merge validation (informational, no checklist)

`workflow_run` workflows only fire from the default branch, so end-to-end verification happens after merge. Expected sequence on the next Dependabot PR:

1. Dependabot opens PR → `CI` workflow runs against the PR.
2. CI concludes (success or failure).
3. `Dependabot auto-approve / -merge` workflow fires from `workflow_run`.
4. If CI succeeded AND actor was Dependabot: the job approves the PR and enables auto-merge with squash.
5. GitHub's auto-merge waits for any branch-protection required checks (which, post-merge, will be the same CI run that just succeeded) and then merges.

If the new workflow fails on the first real Dependabot PR, the failure is visible in the Actions tab; the PR simply stays open until a human reviews. No damage done.
