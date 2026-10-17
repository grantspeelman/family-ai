# Dev container

A container for working on family-ai with Claude Code, permission prompts off,
without handing Claude the rest of the machine.

## Getting in

**Terminal:** `.devcontainer/start.sh` builds the image (first run only), starts
the container, and drops you into a shell. Rerun it to get back in; the
container keeps running after you exit.

| Flag        | Effect |
|-------------|--------|
| `--rebuild` | Rebuild the image from scratch. This is also how Claude Code and Codex get upgraded. |
| `--claude`  | Run Claude Code directly instead of a shell. Sets `HERDR_AGENT` so [herdr](https://herdr.dev) sees the agent. |
| `--codex`   | Same, for Codex. |
| `--docker`  | Privileged mode with nested Docker. Read [Nested Docker](#nested-docker) first. |

Run a specific command by putting it and its arguments after `--`:

```sh
.devcontainer/start.sh --rebuild -- claude --resume
.devcontainer/start.sh -- codex resume
.devcontainer/start.sh -- bash -lc 'echo "hello from the container"'
```

Everything after `--` is passed directly to the container command, preserving
argument boundaries. With no command, the launcher opens Bash. Commands named
`claude` or `codex` set `HERDR_AGENT` for [herdr](https://herdr.dev); the
`--claude` and `--codex` shortcuts still work, but cannot be combined with `--`.

**VS Code:** *Dev Containers: Reopen in Container*. VS Code builds its own
container from the same `docker-compose.yml`, separate from the one
`start.sh` builds.

First time only: run `claude` (or `codex`) inside and log in. The logins live in the
`family-ai-claude-config` and `family-ai-codex-config` volumes, which every container for this repo
shares, so it survives rebuilds and covers both entry points.

## What the container can reach

| | |
|---|---|
| **Files** | Only this checkout, at `/workspace`. No host home directory, `~/.ssh` or `~/.claude`. |
| **SSH** | Your host ssh-agent, forwarded as a socket. `start.sh` starts an agent and runs `ssh-add` if it needs to. The container can use your key to sign, but it can't read or copy the key. GitHub's host keys are pinned in the image. |
| **GitHub API** | A fine-grained token for this repo only. It comes from a file on the host, outside the repo. See [GitHub token](#github-token). |
| **Claude** | Its own config (the volume above), with `permissions.defaultMode = bypassPermissions`. Your host Claude settings are untouched. |
| **Network** | Outbound access is not filtered. |

Inside the container the `vscode` user has passwordless `sudo`.

## GitHub token

The token comes from a file on the host, outside the repo:
`~/.config/family-ai/gh.env`, a single line `GH_TOKEN=<token>`. Compose reads it
through `env_file` in `docker-compose.yml`, so `start.sh` and VS Code use the
same file. The file is optional: without it the container starts and `gh` is
unauthenticated. The long `env_file` form needs Docker Compose 2.24 or newer.
A `GH_TOKEN` exported in your shell or placed in `.devcontainer/.env` is no
longer passed into the container; use the file. `${HOME}` is expanded by
Compose from the environment it runs in, so on a non-Linux host or a
GUI-launched VS Code it may differ from your shell's and the file may not be
found. The token is never printed.

**Create the token** (GitHub, Settings, Developer settings, Fine-grained personal
access tokens):

- Resource owner: the account that owns the repository.
- Expiration: 30 days.
- Repository access: only this repository.
- Permissions: Metadata (read), Contents (read and write), Pull requests (read
  and write), Issues (read and write, if agents need issues), Actions (read),
  Checks (read), Commit statuses (read). Add Workflows (read and write) only when
  agents must edit workflow files.
- Do not grant Administration, Secrets, Environments, Pages or Webhooks.

**Store it** without putting it in shell history (the prompt hides what you
type):

```sh
mkdir -p ~/.config/family-ai && chmod 700 ~/.config/family-ai
(umask 077; read -rs T && printf 'GH_TOKEN=%s\n' "$T" > ~/.config/family-ai/gh.env; unset T)
```

The directory should be mode 700 and the file mode 600; `start.sh` warns if not.

**Renew every 30 days.** Create a new token, rewrite the file with the command
above, restart with `.devcontainer/start.sh`, then delete the old token on
GitHub. The container takes its environment when it is created, so it must be
restarted after a rotation (for VS Code, rebuild or reopen the container).

If the file is missing, `start.sh` warns that `gh` will be unauthenticated and
points here.

## Project toolchain

One container holds everything, so the registry app, the Fly mock and the
family app can all run and talk to each other over localhost.

| Tool | Source |
|---|---|
| Rust 1.93 (`rustc`, `cargo`, `rustfmt`, `clippy`) | Ubuntu archive |
| `build-essential`, `pkg-config` | Ubuntu archive |
| `sqlite3` CLI and `libsqlite3-dev` | Ubuntu archive |

Nothing here is pinned by checksum; the archive's Rust series is what the
structure test asserts. If you need a newer Rust than the archive offers (a
crate's MSRV, say), replace `rustc`/`cargo`/`rustfmt`/`rust-clippy` with a pinned,
SHA-256-checked rustup toolchain in the `Dockerfile` and update the structure test
in the same commit. The coverage and licence tools planned for PRs 2 and 7
(`cargo-llvm-cov`, `cargo-deny`) aren't installed yet; add each in its own PR.

`postStartCommand.sh` runs `cargo fetch --locked` once a root `Cargo.toml`
exists. There are no backing services: SQLite is a file on the app's volume.

## Commit checks

The image ships `lefthook` and `gitleaks`, and `postStartCommand.sh` installs
the git hooks. The hook configuration (`lefthook.yml`) runs `gitleaks` on
staged changes before every commit made inside the container, then
changed-files steps for lint, tests, duplication and complexity, and
`cargo-deny` (placeholders until the tools are installed; see
`docs/conventions.md`). A commit that contains a secret is blocked and the
finding is redacted in the output.

Commits made outside the container skip the hook (git prints "Can't find
lefthook" and lets the commit through), so CI is the backstop for those.

## Nested Docker

`start.sh --docker` recreates the container as privileged and starts `dockerd`
inside it. Use it for the sessions that need Docker, then run `start.sh`
without the flag to go back to the unprivileged container. VS Code never uses
this mode.

## Changing the image

The image is `ubuntu:26.04`. Most tools come from Ubuntu's archive; `gh` and
`gitleaks` are too old there, so they're checksummed release binaries.

`container-structure-test.yaml` is the spec for the image. Each pinned tool has
a version test there, so when you bump a pin in the `Dockerfile` (version and
SHA-256), update its test in the same commit.

`smoke-test.sh` starts a real container the way `start.sh` does and checks it
end to end: SSH forwarding, Claude settings, the project toolchain, the commit
hook, nested Docker, and switching back to unprivileged. It needs network access
(it pulls `hello-world`). It runs in about 30 seconds and cleans up after
itself, using a throwaway Claude config volume so your login is never touched.

CI (`.github/workflows/devcontainer-ci.yml`) runs the container-structure test and
the smoke test on every push to `main` and every PR.

`token-file-check-test.sh` tests the token-file mode check in `start.sh` with dummy
files in temp dirs (including symlinks). It needs no Docker, and CI does not run it.
