# Dev container

A container for working on family-ai with Claude Code, permission prompts off,
without handing Claude the rest of the machine.

## Getting in

**Terminal:** `.devcontainer/start.sh` builds the image (first run only), starts
the container, and drops you into a shell. Rerun it to get back in; the
container keeps running after you exit.

| Flag        | Effect |
|-------------|--------|
| `--rebuild` | Rebuild the image from scratch. This is also how Claude Code gets upgraded. |
| `--docker`  | Privileged mode with nested Docker. Read [Nested Docker](#nested-docker) first. |

**VS Code:** *Dev Containers: Reopen in Container*. VS Code builds its own
container from the same `docker-compose.yml`, separate from the one
`start.sh` builds.

First time only: run `claude` inside and log in. The login lives in the
`family-ai-claude-config` volume, which every container for this repo
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
staged changes before every commit made inside the container. A commit that
contains a secret is blocked and the finding is redacted in the output.

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

A later PR adds CI that runs both on every push to `main` and every PR.
