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
| **GitHub API** | `GH_TOKEN` / `GITHUB_TOKEN`, passed through from the host environment. Use a fine-grained token scoped to this repo. |
| **Claude** | Its own config (the volume above), with `permissions.defaultMode = bypassPermissions`. Your host Claude settings are untouched. |
| **Network** | Outbound access is not filtered. |

Inside the container the `vscode` user has passwordless `sudo`.

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

## Docker checks

Docker-dependent checks run on GitHub Actions runners, outside agent sessions.
The agent image contains no Docker CLI or daemon, and neither terminal nor
VS Code sessions receive a host Docker socket, devices, or Docker API access.
The former `--docker` launcher flag exits with guidance instead of starting a
container. Do not add a host Docker socket mount as a replacement.

Open or update the pull request to request the `devcontainer` workflow;
subsequent pushes to that PR branch trigger another run. Inspect results with:

```sh
gh run list --workflow devcontainer-ci.yml --branch <branch>
gh run view <run-id>
gh run view <run-id> --log-failed
gh pr checks <pr-number>
```

Use a trusted host session for the push and these commands when publishing
credentials are unavailable inside the container. The existing `image` and
`smoke` jobs build and test the image and check the runtime on a disposable
GitHub-hosted runner. Both are included in the required `ci` gate. These jobs
use read-only workflow permissions, no repository secrets, and disposable
smoke-test credentials; no agent session receives the runner's Docker access.

Container isolation depends on the host: native Linux containers share the host
kernel; Docker Desktop runs its Linux containers inside a VM. Neither boundary
is a substitute for keeping privileged Docker work outside agent sessions. If
local Docker checks are needed, run them from a trusted host or a separate
disposable VM, rather than exposing Docker to the agent container.

## Changing the image

The image is `ubuntu:26.04`. Most tools come from Ubuntu's archive; `gh` and
`gitleaks` are too old there, so they're checksummed release binaries.

`container-structure-test.yaml` is the spec for the image. Each pinned tool has
a version test there, so when you bump a pin in the `Dockerfile` (version and
SHA-256), update its test in the same commit.

`smoke-test.sh` starts a standard, unprivileged container and checks it end
to end: runtime privileges and mounts, SSH forwarding, Claude settings, the
project toolchain, and the commit hook. It runs from a Docker-capable CI runner
or trusted host, with throwaway Claude and Codex config volumes and SSH keys.
It does not mount a Docker socket or start a nested daemon.

CI (`.github/workflows/devcontainer-ci.yml`) runs the image and smoke checks on
every push to `main` and every PR. Launcher tests and effective Compose
configuration checks run in the existing `image` job without agent logins.
