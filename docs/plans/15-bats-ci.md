# 15 — bats in CI

**Status:** in progress

## Goal

CI finds and runs every `*.bats` file in the repo by itself. Adding a bash test later needs no workflow change, and a failing bats test turns the `ci` gate red.

## Builds on

- 07 — CI full-suite gate (the `ci` aggregate job)
- 14 — bats-core (the pinned runner in the dev container image)

## In scope

- A step in the `image` job of `.github/workflows/devcontainer-ci.yml`. It lists tracked files with `git ls-files -z -- '*.bats'` (null-separated, no `--recursive`, no hard-coded paths) and runs `bats` once over all of them inside the image the job just built. Its result is the `image` job's result, which `ci` already requires.
- The step fails when discovery finds zero files. The repo has at least one `.bats` file, so zero means discovery broke, not that there is nothing to test. If every bats test is ever removed on purpose, remove the guard in the same PR.
- `.devcontainer/token-file-check-test.sh` is converted to `.devcontainer/token-file-check.bats` with the same cases (600, 700, 400 silent; 644 and 755 warn; symlinked file and directory; missing file). Dummy values and `BATS_TEST_TMPDIR` only. The old script is deleted and mentions of it in the README and plans 13 and 14 are updated.
- `docs/conventions.md` says CI discovers every tracked `*.bats` automatically, and lists the check in the enforcement table.

## Out of scope

- A separate `bats` job and the `ci` `needs:` edit (see the design notes for why not).
- Running bats from the pre-commit hook.
- `bats-support` and `bats-assert` (plan 14: add them when a test first needs them).
- Untracked files. A new test must be committed (or at least added to the PR) to be found.

## Acceptance checklist

- [x] Discovery: the exact `git ls-files` command from the workflow, run against the repo, lists every `.bats` file (today one) and the file-list array handles paths with spaces (null-separated).
- [x] `.devcontainer/token-file-check.bats` passes locally under bats-core 1.14.0 installed from the pinned tarball (SHA-256 verified), 8 of 8.
- [x] A deliberately failing assertion makes bats exit non-zero; shown once, then reverted.
- [x] `python3 -I .github/scripts/check-workflow-rules.py` passes; the step uses no `${{ }}` in `run:`, no secrets, and adds no action, so the pin rules are unchanged.
- [x] No workflow change is needed to add a new test: a second `.bats` file under any directory is picked up by the same command (checked locally with a temporary file, removed).
- [ ] The step runs green on the PR. Unticked: needs a real CI run (no Docker daemon here).
- [ ] A deliberately failing `.bats` test turns the `ci` job red. Unticked: needs a real CI run.

## Design notes

- **Where bats comes from.** Run inside the image the `image` job already builds. That image is the one place bats's version and SHA-256 are pinned (Dockerfile `BATS_VERSION`, `BATS_SHA256`), so CI cannot drift from the dev container, and there is no second download or checksum to maintain. The alternatives were worse: a new job would rebuild the image (jobs do not share it, and the build is the slow part of CI) or repeat the pin in the workflow, which Dependabot does not update either; installing from apt gives an older bats than the container.
- **Why a step, not a job.** The `ci` gate already requires `image`, so the check is gated with no change to `needs:` and the "skipped counts as failure" logic is inherited. The cost is that a bats failure and a structure-test failure share one job; the step name keeps them apart in the log. The step runs after the structure test, so a broken image reports there first.
- **Discovery on the host, run in the container.** `git` runs on the runner (the image's git would reject the mounted checkout as owned by another user). Only the list of paths crosses into `docker run`, with the checkout mounted at `/repo`, as the other steps do, and the runner's UID so no root-owned files appear.
- **Tracked files only.** `git ls-files` ignores build output, vendored directories and stray local files, so what CI runs is what is in the PR.

## Review depth

**Skim.** One workflow step and a mechanical test conversion; check that the discovery command and the zero-files guard read right.
