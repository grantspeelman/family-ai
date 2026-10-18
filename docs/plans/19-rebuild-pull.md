# 19 — Pull before --rebuild

**Status:** in progress

## Goal

`start.sh --rebuild` builds the image from the local checkout. If that checkout is behind its upstream, the rebuild silently produces an old image. So `--rebuild` now runs `git pull --ff-only` first and stops if the pull fails.

## Builds on

- 18 — start.sh launcher tests in bats, and --help

## In scope

- `start.sh --rebuild` runs `git -C <checkout> pull --ff-only` on whatever branch is checked out, before any Docker call. It runs after the ssh-agent setup, so an SSH remote can authenticate.
- If the pull fails (diverged branch, detached HEAD, no upstream, offline, local changes in the way), git's error is shown, `start.sh` adds a one-line hint and exits 1, and nothing is built.
- Without `--rebuild` there is no pull. `--rebuild --help` only prints help.
- The `--rebuild` lines in the `start.sh` header (the `--help` text) and in `.devcontainer/README.md` say it pulls first.
- `start.bats` gets a `git` shim, so no test runs the real git.

## Out of scope

- Pulling without `--rebuild`, or only on `main`.
- A way to rebuild without pulling. Run the compose build by hand if that is ever needed.
- VS Code's *Rebuild Container*, which does not go through `start.sh`.

## Acceptance checklist

- [x] `bats .devcontainer/start.bats` passes (58 tests).
- [x] The new tests were written first and failed against the old `start.sh`.
- [x] Mutations make them fail: not stopping on a failed pull, dropping `--ff-only`, pulling after the build, and pulling before the ssh-agent setup.
- [ ] The Bats step runs green on the PR. Unticked: needs a real CI run.

## Review depth

**Close read.** It changes what `--rebuild` does to the local checkout.
