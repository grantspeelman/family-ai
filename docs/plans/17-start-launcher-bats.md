# 17 — start.sh launcher tests in bats, and --help

**Status:** in progress

## Goal

The command-launch behaviour of `start.sh` (`--`, `--claude`, `--codex`) is tested by `start.bats`, which CI runs. Until now it was covered only by `.devcontainer/test-start.py`, which CI never ran. `start.sh` also gets a real `-h` / `--help`.

## Builds on

- 16 — bats tests for bash scripts (`start.bats` and its shims)
- [devcontainer-command-launch](devcontainer-command-launch.md) (the behaviour being tested)

## In scope

- New `start.bats` tests for everything `test-start.py` checked:
  - `--rebuild -- claude --resume`: `build --pull --no-cache`, the exec ends in `app claude --resume`, `HERDR_AGENT=claude`.
  - Arguments after `--` arrive unchanged (spaces, an empty argument, `$(touch nope)` not executed, a `--docker` after `--` not treated as a flag), and `HERDR_AGENT` is unset for a plain command.
  - `-- codex resume`, `--codex` and `--claude` set `HERDR_AGENT`.
  - `--claude --codex`, a bare `--` and `--claude -- echo hi` exit 64 with no Docker call. (`--unknown` was already covered by "unknown flag prints usage...", so it is not repeated.)
  - Already covered, not duplicated: the default shell ending in `exec --user vscode app bash` ("after up it runs postStartCommand.sh as vscode, then execs a shell as vscode").
- The docker shim also records the last call's argv (NUL-separated, so empty arguments survive) and the `HERDR_AGENT` it saw.
- `.devcontainer/test-start.py` is deleted.
- `start.sh -h` / `--help`: prints the usage line and the flag lines to stdout and exits 0. It is handled while parsing arguments, so it runs no Docker, `ssh-add` or `ssh-agent` and does no token-file check. It works combined with other flags. After `--`, `-h` and `--help` are ordinary command arguments. Unknown flags still print the usage line to stderr and exit 64.
- One source for the flag text: the help prints the flag lines of the header comment in `start.sh`, so they cannot drift.
- The old test "--help is not a flag" is replaced by the help tests. `.devcontainer/README.md` mentions `--help`.

## Out of scope

- The other plan 16 findings ((a), (b), (d) to (h)) stay unfixed; finding (c) is fixed here.
- Any other change to `start.sh` or other scripts.

## Acceptance checklist

- [x] `bats .devcontainer/start.bats` passes (54 tests).
- [x] The new help tests were written first and failed against the old `start.sh`.
- [x] Mutations make the new tests fail: removing the `-h|--help` case, dropping an argument after `--`, unquoting `"$@"` (empty and spaced arguments), and removing `HERDR_AGENT`.
- [x] Nothing in the repo references `test-start.py` except this plan.
- [ ] The Bats step runs green on the PR. Unticked: needs a real CI run.

## Findings

- `--claude --codex --help` and `--bogus --help` exit 64 rather than printing help, because help is handled in argument order. Flags before `--help` are validated first. Left as is.

## Review depth

**Close read** of the `--help` parsing in `start.sh` (it must not be reachable after `--`); skim the tests.
