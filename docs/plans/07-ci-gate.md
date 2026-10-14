# 07 — CI full-suite gate

**Status:** planned

## Goal

A PR cannot merge unless the full suite passes in CI. The pre-commit hook is a fast local check; this is the check that counts.

## Builds on

- 02 — Dev container CI (the first workflow and its pinning conventions)
- 06 — Changed-files checks (the full suite is the same checks run over everything)

## In scope

- A workflow that runs the full set of project checks on every PR and on pushes to `main`. Until real tools exist (PRs 2 and 7) it runs the checks that do exist and says which are not wired yet.
- A single job whose name can be marked as a required status check.
- Branch protection on `main` requiring that check and a PR before merge, documented as a settings checklist because it lives in GitHub, not in the repo.

## Out of scope

- The individual tools it will run (PRs 2 and 7).
- Real Fly credentials. CI runs against the Fly mock and never needs a token.
- Fork-PR trust rules (plan 08).

## Acceptance checklist

- [ ] The workflow runs on `pull_request` and on `push` to `main`, and passes on this PR.
- [ ] One aggregate job (for example `ci`) fails if any underlying check fails.
- [ ] Branch protection on `main` requires that job and a PR; the PR description records the settings applied.
- [ ] A deliberately failing check turns the aggregate job red. Shown once, then reverted.
- [ ] No secrets are referenced.

## Review depth

**Close read.** This is the merge gate, so what it requires and what it can silently skip both matter.
