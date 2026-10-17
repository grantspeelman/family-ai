# 07 — CI gate

**Status:** in progress

## Goal

One required check, `ci`, that is green only if every CI job is green. A PR cannot merge unless it is.

"The full test suite" means every test and lint job that exists in CI. Each stack PR adds its own job and wires it into `ci`'s `needs:` in the same PR (for example cargo test and clippy arrive with PR 1, the registry app's tests with PR 6). The pre-commit hook is only a fast local check on changed files; CI is the check that counts.

Today there is no application code. What the gate covers is the container checks (compose validation, image build, structure test, smoke test) and the full-history secret scan. Later PRs add jobs and wire them into `ci`. Nothing covers a whole source tree yet, because there is none.

## Builds on

- 02 — Dev container CI (the first workflow and its pinning conventions)

## In scope

- An aggregate job `ci` in the existing workflow that runs on every PR and on pushes to `main`, `needs:` every other CI job, and fails if any of them fails, is cancelled or is skipped.
- A comment on `ci`'s `needs:` list saying every job that must pass before merge is listed there and a new job must be added when introduced.
- Branch protection on `main` requiring `ci` and a PR before merge, documented as a settings checklist because it lives in GitHub, not in the repo.

## Out of scope

- The test and lint jobs themselves. Each stack PR adds its own and wires it into `needs:`.
- Running the pre-commit hook in CI. It is a local check, not the test suite.
- Real Fly credentials. CI runs against the Fly mock and never needs a token.
- Fork-PR trust rules (plan 08).

## Acceptance checklist

- [ ] The workflow runs on `pull_request` and on `push` to `main`, and passes on this PR. Unticked: needs a real CI run on the PR (the triggers already exist on the workflow).
- [x] One aggregate job, `ci`, fails if any job in its `needs:` fails, is cancelled or is skipped. Verified by running the step's shell locally against sample `NEEDS` JSON: all success exits 0; one failure, one skipped and one cancelled each exit 1. Not yet seen in a CI run.
- [x] `ci` lists every CI job that exists on this branch (`image`, `smoke`), with a comment that new jobs must be added. Checked by reading the workflow.
- [ ] Branch protection on `main` requires `ci` and a PR; the PR description records the settings applied. Unticked: owner action. Steps are in `docs/github-settings.md`; the `ci` name only becomes selectable after the first run.
- [ ] A deliberately failing job turns `ci` red. Shown once, then reverted. Unticked: needs a real CI run.
- [x] No secrets are referenced. `grep secrets\.` over `.github/workflows/` finds nothing.
- [ ] Each later stack PR that adds a test or lint job adds it to `needs:`. Unticked: ongoing rule for PRs 1, 6 and later, recorded in `docs/conventions.md`.

## Design notes

- The aggregate job lives in the existing `devcontainer-ci.yml`. A separate workflow cannot `needs:` jobs in another one, and calling the existing workflow as a reusable workflow would make both share the same concurrency group and block each other. One file means one required check name, `ci`, is enough.
- `if: always()` plus an explicit `success` check, because a plain `needs:` leaves the job skipped on failure, and a skipped required check counts as passing.
- The job reads `toJSON(needs)` through an environment variable, not inline in the script, so no workflow expression is expanded into shell.

## Review depth

**Close read.** This is the merge gate, so what it requires and what it can silently skip both matter.
