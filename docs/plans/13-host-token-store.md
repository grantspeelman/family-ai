# 13 — Host-side token file

**Status:** in progress

## Goal

The GitHub token for the dev container comes from a file on the host, outside the repo: `~/.config/family-ai/gh.env` (directory mode 700, file mode 600, one line `GH_TOKEN=<token>`). The token is a fine-grained, single-repo, short-lived personal access token that the owner creates and renews.

## Builds on

- 01 — Dev container (`start.sh`, the compose file and the README)
- 12 — Which docs are committed (the repo is public; what the docs may say)

## In scope

- `docker-compose.yml` reads the file with `env_file` in the long form (`path` plus `required: false`), so a missing file breaks nothing: CI, the smoke test and contributors without a token still work. The long form needs Docker Compose 2.24 or newer.
- No `GH_TOKEN` / `GITHUB_TOKEN` pass-through next to it. Compose resolves a pass-through from a project `.env` file too, which would shadow the file; the file is the single source.
- `start.sh` keeps `--env-file /dev/null`. If the file is missing it warns that `gh` will be unauthenticated and points at the README. If it exists, it warns (does not fail) when the file is not mode 600 or its directory not 700. It checks modes with `stat -L` (symlinks count as their targets; BSD `stat -f` as fallback; unknown modes are skipped), accepts 600 or stricter and 700 or stricter, and never reads the file. The check lives in `token-file-check.sh`.
- `.env.example` says nothing token-related lives in the repo.
- `.devcontainer/README.md` documents how to create the token, store it without shell history, renew it every 30 days and restart after a rotation.

## Out of scope

- Any token handling inside the container beyond what `env_file` provides.
- Credential stores (libsecret, macOS Keychain, Windows).
- Running the mode-check test in CI. It was a local check (`token-file-check-test.sh`) when this plan landed; [plan 15](15-bats-ci.md) converted it to `token-file-check.bats` and CI runs it.

## Acceptance checklist

- [x] `docker compose config` resolves with the file missing (`config --quiet` with the same command and files as the CI validation step exits 0).
- [x] With a dummy file present, `config` shows `GH_TOKEN` from the file; a `GH_TOKEN` set in the calling shell does not override it (the file is the only source). Dummy values only, output masked.
- [x] `--env-file /dev/null` still resolves `${HOME}` (it comes from the shell).
- [x] The mode-check test (then `token-file-check-test.sh`, since converted to `token-file-check.bats` by plan 15) passes (temp dirs, dummy file): 600/700 and 400 are silent; 644 file and 755 directory warn; a symlinked file and a symlinked directory pointing at 600/700 are silent; a missing file gives the unauthenticated warning and README pointer.
- [x] `start.sh`, `token-file-check.sh` and the then-current `token-file-check-test.sh` passed `bash -n`. (`shellcheck` is not installed here.)
- [x] `python3 .github/scripts/check-workflow-rules.py` and `lefthook run pre-commit` pass (via the commit hook).
- [x] `postStartCommand.sh`, the Dockerfile comment and the README no longer describe a pass-through; the README says a shell-exported or `.env` token is not passed in.
- [x] The README describes token creation (permissions, 30-day expiry, single repository), the prompted store command, renewal, the restart after a rotation, and the Compose 2.24 minimum.
- [ ] The CI runner's Docker Compose is 2.24 or newer. Not checked here (local Compose is 2.40.3); confirm in the first CI run.
- [ ] With a real token file, the container builds and starts, and `gh auth status` inside is authenticated. Not run: no Docker daemon here.
- [ ] VS Code's *Reopen in Container* picks the token up from the same file. Not run: needs VS Code and a daemon.
- [ ] Owner: create the token, store it, and later rotate it and delete the old token on GitHub.

## Review depth

**Close read.** It handles a credential: check that the value is never printed or put on a command line, and that no file in the repo can supply it.
