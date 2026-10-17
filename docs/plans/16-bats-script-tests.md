# 16 — bats tests for the repo's bash scripts

**Status:** in progress

## Goal

Every bash script in the repo that can be exercised without Docker, a network or the real `HOME` has bats tests, and CI runs them automatically. Scripts that cannot be tested that way stay as they are, with the reason recorded.

## Builds on

- 14 — bats-core (the runner in the dev container image)
- 15 — bats in CI (discovery of every tracked `*.bats` file)

## In scope

- `.devcontainer/start.bats` (42 tests) for `start.sh`. Every external command it runs (`docker`, `ssh-agent`, `ssh-add`, `id`, `sleep`) is a shim in a temp dir first on `PATH`; `HOME` is a temp dir with a dummy token file.
- `.devcontainer/post-start.bats` (18 tests) for `postStartCommand.sh`.
- `.devcontainer/entrypoint.bats` (7 tests) for `entrypoint.sh`.
- `.lefthook/placeholder.bats` (12 tests) for `.lefthook/placeholder.sh`.
- All tests use dummy values and `BATS_TEST_TMPDIR` only, and need no Docker, network or real `HOME`.
- `docs/conventions.md` lists what is covered and names `smoke-test.sh` as the deliberate exception. The plan index lists this plan.

## Out of scope

- Converting `.devcontainer/smoke-test.sh`. It builds and starts a real container and needs Docker and network access, so it stays a script run by CI as a separate step.
- Changing any script's behaviour. Possible bugs found while writing the tests are listed under Findings for the owner and are not fixed here.
- `bats-support` and `bats-assert` (plan 14: add them when a test first needs them).

## Acceptance checklist

- [x] All five tracked-or-new `.bats` files (87 tests) pass from the repo root and from another directory using absolute paths, three runs in a row with no flakiness.
- [x] The CI discovery logic (mapfile, zero-files guard, `./` prefix) finds all five files when run locally.
- [x] Tests leave nothing outside `BATS_TEST_TMPDIR` (`git status` and the real `HOME` unchanged by a run).
- [x] Total runtime is under 30 seconds.
- [x] `bash -n` passes on the scripts, and `python3 -I .github/scripts/check-workflow-rules.py` passes.
- [x] `docs/conventions.md` lists the covered scripts and the `smoke-test.sh` exception.
- [ ] The Bats step runs green on the PR with the new files. Unticked: needs a real CI run (no Docker daemon here).

## Findings

Possible bugs seen while writing the tests. None is fixed in this PR; the owner decides.

- (a) `entrypoint.sh`: the background daemon log redirects fail silently when `/var/log` is not writable, and the script then waits the full 60 seconds.
- (b) `postStartCommand.sh`: an invalid `settings.json` makes the script abort after `jq` has truncated `settings.json.tmp`, leaving a stale `.tmp` file.
- (c) `start.sh` treats `-h` and `--help` as an unknown flag (exit 64).
- (d) `start.sh`: a failing `ssh-agent -a` aborts with no `start.sh` message (ssh-agent's stdout goes to `/dev/null`; its stderr still shows). The same happens when the fixed socket's directory (`~/.ssh`) does not exist.
- (e) `start.sh --docker` keeps polling for 90 seconds even if the container has exited.
- (f) `start.sh` replaces a dead session agent silently.
- (g) `placeholder.sh` counts arguments, not files, so nonexistent paths and empty-string arguments count.
- (h) `entrypoint.sh` hardcodes the dockerd and containerd log paths under `/var/log`. A configurable log path would let the tests assert that the daemons are launched (see Known test limits).

## Known test limits

- `post-start.bats` depends on whether `/workspace/.git` and `/workspace/Cargo.toml` exist on the machine, because the script uses those absolute paths.
- The docker-mode tests in `entrypoint.bats` skip when `/var/log` is writable (running as root).
- The `chmod 000` test in `placeholder.bats` is weaker when run as root, since root ignores file modes.

- `entrypoint.bats` never asserts that dockerd and containerd are launched. The log paths are hardcoded under `/var/log`: in CI it is not writable, so the redirect fails before the shim runs, and as root the docker-mode tests skip. See finding (h).
- `post-start.bats` covers one side of each `/workspace` branch per environment (in CI the "not called" side, since `/workspace/Cargo.toml` is absent), so a mutation to `cargo fetch --locked` survives in CI.
- Mid-test assertions use `if cmd; then false; fi` instead of `! cmd`, because bash ignores a failing `!` command under `set -e` unless it is the last one.

## Review depth

**Skim.** Tests and docs only, no script changes; check the Findings list reads right and the test files shim every external command.
