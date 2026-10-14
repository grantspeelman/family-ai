# 08 — Workflow safety

**Status:** in progress

## Goal

Anyone can open a PR against a public repo, so no workflow runs untrusted code with secrets or write access available. This is checked in one place instead of per workflow.

## Builds on

- 07 — CI full-suite gate (there are now workflows to audit)

## In scope

- An audit of every workflow in `.github/workflows/` against a written rule set.
- Fixes for anything that breaks the rules.
- The rules recorded in the conventions document, so later workflows follow them.
- A mechanical check for the rules where one exists (for example a workflow linter such as `actionlint` or `zizmor` in CI).

The rules to record:

- Default `permissions` is read-only; anything broader is set per job with a reason.
- No `pull_request_target` or `workflow_run` that checks out or runs fork code.
- Fork PRs get no secrets; workflows must not need any.
- No real Fly token or API key in GitHub Actions.
- Untrusted input (PR titles, branch names) never goes straight into a shell `run:` step.
- Third-party actions pinned by full commit SHA.
- `persist-credentials: false` on checkout unless a job must push.

## Out of scope

- Repo settings that live in GitHub (plan 09).
- Dependabot updates for pinned actions (plan 10).

## Acceptance checklist

- [x] Each rule is written down and linked from `AGENTS.md` or the conventions document. (`docs/conventions.md`, section "Workflow safety"; linked from `AGENTS.md`.)
- [ ] Each workflow in the repo is listed in the PR description with a pass or fix note per rule. (The table is under "Audit" below; copying it into the PR description is left for when the PR is written.)
- [ ] A workflow linter runs in CI and fails on a deliberate violation. Shown once, then reverted. (Needs a real CI run. Locally: `actionlint` 1.7.12 and the rules script both exited 1 on a deliberate violation of an unpinned checkout, `${{ github.head_ref }}` in `run:` and `secrets.X`, then the change was reverted and both exit 0. The `workflows` job is wired into `needs:` of `ci` but has not run in GitHub yet.)
- [ ] The repo setting for fork-PR workflows (require approval for outside contributors) is set and the choice recorded. (Owner action. Exact steps are in `docs/github-settings.md`.)
- [x] `grep` for `secrets.` across workflows returns nothing, or each hit is justified. (`grep -rn 'secrets\.' .github` returns nothing, apart from the pattern in the rules script that looks for it.)

## Review depth

**Close read.** Mistakes here are how public repos leak secrets or hand out write access.

## Audit

Rules are numbered as in `docs/conventions.md`, "Workflow safety". Audited at the head of this branch, after this plan's one change to the workflow (the added `workflows` job).

| Rule | `devcontainer-ci.yml` |
| --- | --- |
| 1. Read-only default permissions | Pass. Top-level `permissions: contents: read`; no job widens it. |
| 2. No `pull_request_target` / `workflow_run` | Pass. Triggers are `push` to `main` and `pull_request`. |
| 3. No secrets needed | Pass. No `secrets.` reference. The `ci` job reads only `needs`. |
| 4. No Fly token or API key | Pass. None present. |
| 5. No untrusted input in `run:` | Pass. No `${{ }}` in any `run:`. The one expression the `ci` job uses (`toJSON(needs)`) goes through `env:` and is read as `$NEEDS`. `concurrency.group` uses `github.ref` outside `run:`. |
| 6. Actions pinned by SHA | Pass. `actions/checkout` is pinned by full SHA with a version comment in all three jobs. Downloaded binaries (`container-structure-test`, `actionlint`) are pinned by version and SHA-256 and verified before use. |
| 7. `persist-credentials: false` | Pass. Set on every checkout; no job pushes. |
| 8. `timeout-minutes` on every job | Pass. `image` 25, `smoke` 25, `workflows` 10, `ci` 5. |

No violations were found, so no fixes were needed. The one change to the workflow is the new `workflows` job (actionlint and the rules script) and its entry in the `ci` job's `needs:`. The actionlint SHA-256 was taken from the release's published `actionlint_1.7.12_checksums.txt` and matched the downloaded archive.
