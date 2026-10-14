# 08 — Workflow safety

**Status:** planned

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

- [ ] Each rule is written down and linked from `AGENTS.md` or the conventions document.
- [ ] Each workflow in the repo is listed in the PR description with a pass or fix note per rule.
- [ ] A workflow linter runs in CI and fails on a deliberate violation. Shown once, then reverted.
- [ ] The repo setting for fork-PR workflows (require approval for outside contributors) is set and the choice recorded.
- [ ] `grep` for `secrets.` across workflows returns nothing, or each hit is justified.

## Review depth

**Close read.** Mistakes here are how public repos leak secrets or hand out write access.
