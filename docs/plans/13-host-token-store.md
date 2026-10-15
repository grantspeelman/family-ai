# 13 — Host token store

**Status:** in progress

## Goal

The GitHub token for the dev container comes from the host's credential store. The token is a fine-grained, single-repo, short-lived personal access token that the owner creates and renews.

## Builds on

- 01 — Dev container (`start.sh`, the compose pass-through and the README)
- 12 — Which docs are committed (the repo is public; what the docs may say)

## In scope

- `start.sh` takes the token from the host's libsecret store (`secret-tool lookup service family-ai-gh`) and exports it as `GH_TOKEN` before `compose up`, when neither `GH_TOKEN` nor `GITHUB_TOKEN` is already set. An explicitly set variable always wins.
- When `secret-tool` is missing or the lookup is empty, `start.sh` keeps its "unauthenticated" warning and adds one line saying how to store the token. It never prints the token or any part of it, and never passes it on a command line.
- The lookup lives in a small function (`.devcontainer/host-token.sh`) with a Docker-free test (`.devcontainer/host-token-test.sh`) that uses a fake `secret-tool`.
- `start.sh` makes compose read no `.env` file. `.gitignore` keeps its `.devcontainer/.env` rule as a safety net, but nothing reads that file.
- `.devcontainer/README.md` documents how to create the token, store it, renew it every 30 days and restart the container after a rotation.

## Out of scope

- Any token handling inside the container beyond the existing `GH_TOKEN` pass-through.
- Credential stores other than libsecret (macOS Keychain, Windows). The owner's host is Linux.
- Running the new test in CI. It is a local check for now.

## Acceptance checklist

- [x] With a fake `secret-tool` on `PATH`: a pre-set `GH_TOKEN` wins; a stored value is exported as `GH_TOKEN`; a missing `secret-tool` and an empty lookup both leave no token and return non-zero. `.devcontainer/host-token-test.sh` passes, and prints no token value.
- [x] `start.sh`, `host-token.sh` and `host-token-test.sh` pass `bash -n`. (`shellcheck` is not installed here; the `source` line carries a `shellcheck source=` directive for when it is run.)
- [x] No mechanism reads a token from a file in the repo: there is no compose `env_file`, no sourcing of `.env`, and `.env.example` lists no token entries. Compose's implicit read of `.devcontainer/.env` is switched off for `start.sh` with `--env-file /dev/null` (VS Code's compose is unchanged).
- [x] The README describes token creation (permissions, 30-day expiry, single repository), storing with `secret-tool` (prompted), renewal and the restart after a rotation, and says the token comes from the host's credential store. It notes that a locked keyring can make `start.sh` wait, and that VS Code gets a token only from its launch environment.
- [x] `python3 .github/scripts/check-workflow-rules.py` and `lefthook run pre-commit` pass.
- [ ] `start.sh` with a real token in libsecret: `gh auth status` inside the container is authenticated. Not run here: no Docker daemon and no libsecret store in this environment. Owner action.
- [x] `docker compose --env-file /dev/null -f docker-compose.yml -f compose.ssh.yml config` resolves, with a dummy token only (checked by the reviewer).
- [ ] With `--env-file /dev/null`, the container still builds and starts. Not run: no Docker daemon.
- [ ] After storing a new token and rerunning `start.sh`, the container is recreated with it, and the old token is deleted on GitHub. Owner action.
- [ ] The VS Code entry point gets the token (it reads the environment VS Code was launched from, not the store). Owner to decide if that needs a follow-up.

## Review depth

**Close read.** It handles a credential: check that the value is never printed, never on a command line, and that an explicit environment variable wins.
