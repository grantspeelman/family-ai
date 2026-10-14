# 07 — CI full-suite gate

**Status:** in progress

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

- [ ] The workflow runs on `pull_request` and on `push` to `main`, and passes on this PR. Unticked: needs a real CI run on the PR (triggers already exist on the workflow).
- [ ] One aggregate job (for example `ci`) fails if any underlying check fails. Job `ci` in `devcontainer-ci.yml` runs with `if: always()` and exits non-zero unless every needed job is `success`; actionlint-clean, but the failure behaviour is not yet seen in a CI run (see the demo item).
- [ ] Branch protection on `main` requires that job and a PR; the PR description records the settings applied. Unticked: owner action. Exact steps are in `docs/github-settings.md`; the `ci` name only becomes selectable after the first run.
- [ ] A deliberately failing check turns the aggregate job red. Shown once, then reverted. Unticked: needs a real CI run.
- [x] No secrets are referenced. `grep secrets\.` over `.github/workflows/` finds nothing.

## Design notes

- The aggregate job lives in the existing `devcontainer-ci.yml` and `needs:` its two jobs (`image`, `smoke`). A separate workflow cannot `needs:` jobs in another one, and calling the existing workflow as a reusable workflow would make both share the same concurrency group and block each other. One file means one required check name, `ci`, is enough.
- Whole-tree hook run: `lefthook run pre-commit --all-files` runs inside the built image. Its gitleaks step only scans staged changes, so it scans nothing in CI; the full-history scan stays the real secret check. The other steps skip until PRs 2 and 7 land `.rs` and `Cargo.*` files.
- `if: always()` plus an explicit `success` check, because a plain `needs:` leaves the job skipped on failure, and a skipped required check counts as passing.

## Review depth

**Close read.** This is the merge gate, so what it requires and what it can silently skip both matter.
