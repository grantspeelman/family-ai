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
| **GitHub API** | A fine-grained token for this repo only. It comes from the host's credential store (or an explicitly set `GH_TOKEN` / `GITHUB_TOKEN`). See [GitHub token](#github-token). |
| **Claude** | Its own config (the volume above), with `permissions.defaultMode = bypassPermissions`. Your host Claude settings are untouched. |
| **Network** | Outbound access is not filtered. |

Inside the container the `vscode` user has passwordless `sudo`.

## GitHub token

The token comes from the host's credential store.
At `start.sh` time, if neither `GH_TOKEN` nor `GITHUB_TOKEN` is set in your
environment, `start.sh` looks the token up with `secret-tool` (libsecret, from
the `libsecret-tools` package on Debian and Ubuntu) and passes it into the
container as `GH_TOKEN`. A `GH_TOKEN` or `GITHUB_TOKEN` you set yourself always
wins. The token is never printed. `start.sh` makes compose read no `.env` file.
VS Code gets a token only from the environment it was launched from, not from
the store.

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

**Store it.** `secret-tool` prompts for the value, so it never appears on a
command line or in shell history:

```sh
secret-tool store --label=family-ai-gh service family-ai-gh
```

**Renew every 30 days.** Create a new token, store it again with the same command
(it replaces the old value), rerun `.devcontainer/start.sh`, then delete the old
token on GitHub. The container takes its environment when it is created, so it
must be restarted after a rotation: `start.sh` recreates it when the token
changed.

If the keyring is locked, `start.sh` can wait for an unlock prompt before it
continues.

If `secret-tool` is missing or has no entry, `start.sh` warns that `gh` will be
unauthenticated and prints the store command.

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

`host-token-test.sh` tests the token lookup in `start.sh` with a fake
`secret-tool`. It needs no Docker, prints no token, and CI does not run it.
