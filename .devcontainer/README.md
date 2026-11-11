# Dev container

A container for working on family-ai with Claude Code, permission prompts off,
without handing Claude the rest of the machine.

## Getting in

**Terminal:** `.devcontainer/start.sh` builds the image (first run only), starts
the container, and drops you into a shell. Rerun it to get back in; the
container keeps running after you exit.

| Flag        | Effect |
|-------------|--------|
| `--rebuild` | Run `git pull --ff-only` on the checked-out branch, then rebuild the image from scratch. The image is built from this checkout, so the pull keeps it current; if the pull fails, nothing is rebuilt. This is also how Claude Code gets upgraded. |
| `--claude`  | Run Claude Code directly instead of a shell. Sets `HERDR_AGENT` so [herdr](https://herdr.dev) sees the agent. |
| `--docker`  | Privileged mode with nested Docker. Read [Nested Docker](#nested-docker) first. |

`start.sh --help` prints the usage and the flags.

Run a specific command by putting it and its arguments after `--`:

```sh
.devcontainer/start.sh --rebuild -- claude --resume
.devcontainer/start.sh -- bash -lc 'echo "hello from the container"'
```

Everything after `--` is passed directly to the container command, preserving
argument boundaries. With no command, the launcher opens Bash. Commands named
`claude` set `HERDR_AGENT` for [herdr](https://herdr.dev); the
`--claude` shortcut still works, but cannot be combined with `--`.

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
| **GitHub API** | A fine-grained token for this repo only, plus an optional classic token with only the `project` scope for the `/factory` board. They come from a file on the host, outside the repo. See [GitHub token](#github-token). |
| **Claude** | Its own config (the volume above), with `permissions.defaultMode = bypassPermissions`. Your host Claude settings are untouched. |
| **Network** | Outbound access is not filtered. |

Inside the container the `vscode` user has passwordless `sudo`.

## GitHub token

The token comes from a file on the host, outside the repo:
`~/.config/family-ai/gh.env`, a line `GH_TOKEN=<token>` and an optional second
line `GH_PROJECT_TOKEN=<token>`. Compose reads it through `env_file` in
`docker-compose.yml`, so `start.sh` and VS Code use the same file. The file is optional: without it the container starts and `gh` is
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

**Project token (optional).** Only the `/factory` board needs it: fine-grained
tokens can't reach a user-owned GitHub Project, so `gh project` commands use this
one instead. Create a classic token (Developer settings, Personal access tokens,
Tokens (classic)) with only the `project` scope and a 30-day expiry, and don't
authorise it for any SSO organisation. The first time, append it to the same
file (renewal uses the replace command below):

```sh
(umask 077; read -rs T && printf 'GH_PROJECT_TOKEN=%s\n' "$T" >> ~/.config/family-ai/gh.env; unset T)
```

**Renew every 30 days.** The tokens expire on their own dates, so each renews
alone. Create the new token, then run its command, which replaces only its line:

```sh
# GH_TOKEN
(umask 077; f=~/.config/family-ai/gh.env; grep -v '^GH_TOKEN=' "$f" > "$f.new"; read -rs T && printf 'GH_TOKEN=%s\n' "$T" >> "$f.new" && mv "$f.new" "$f"; unset T)
# GH_PROJECT_TOKEN
(umask 077; f=~/.config/family-ai/gh.env; grep -v '^GH_PROJECT_TOKEN=' "$f" > "$f.new"; read -rs T && printf 'GH_PROJECT_TOKEN=%s\n' "$T" >> "$f.new" && mv "$f.new" "$f"; unset T)
```

Restart with `.devcontainer/start.sh`, then delete the old token on GitHub. The
container takes its environment when it is created, so it must be restarted
after a rotation (for VS Code, rebuild or reopen the container).

If the file is missing, `start.sh` warns that `gh` will be unauthenticated and
points here.

## Git identity

The container has no git `user.name` or `user.email` of its own. Export them in
the host shell that runs `start.sh` (or launches VS Code), for example in your
shell profile:

```sh
export GIT_USER_NAME="Your Name"
export GIT_USER_EMAIL="you@users.noreply.github.com"
```

Compose passes them into the container, and `postStartCommand.sh` writes them
with `git config --global` on start. A value already in the container's global
git config is kept. Without them, commits fail and the start-up output says so.
The container takes its environment when it is created, so recreate it
(`start.sh --rebuild`, or rebuild in VS Code) after setting them.

## Project toolchain

One container holds everything, so the registry app, the Fly mock and the
family app can all run and talk to each other over localhost.

| Tool | Source |
|---|---|
| Rust (`rustc`, `cargo`, `rustfmt`, `clippy`, `llvm-tools-preview`) at `RUST_VERSION` | rustup, from a SHA-256-verified `rustup-init` |
| `cargo-llvm-cov` 0.9.1 | SHA-256-verified upstream archive |
| `build-essential`, `pkg-config` | Ubuntu archive |
| `sqlite3` CLI and `libsqlite3-dev` | Ubuntu archive |
| Java 25 (`default-jre-headless`) | Ubuntu archive |
| PMD CPD 7.28.0 (`pmd`) | SHA-256-verified upstream ZIP |
| Flutter 3.47.9 (`flutter`, `dart`) | SHA-256-verified upstream archive, amd64 only |
| Chrome and chromedriver 156.0.8078.80 (`google-chrome`, `chromedriver`) | SHA-256-verified Chrome for Testing ZIPs, amd64 only |

The archive packages follow the base image; a structure test asserts the Java
series. PMD uses the same version and checksum as the duplication CI job;
its Java bytecode runs on both amd64 and arm64. Run `pmd --version` to check the
installation and `pmd cpd --help` for local duplication checks. Installing it
does not wire the duplication pre-commit placeholder yet.

Flutter publishes no Linux arm64 archive, so on an arm64 host the image builds
without Flutter, Chrome or chromedriver, and the `app` checks run only in CI.

Rust is installed through a pinned, checksummed rustup. The toolchain lives in
`/usr/local/rustup` and its proxies in `/usr/local/cargo/bin`, both root-owned, so
adding a component means changing the `Dockerfile` and rebuilding. `vscode` keeps its
own `~/.cargo` for the registry cache. To bump Rust, change `RUST_VERSION` in the
`Dockerfile`, the `rust` tag in `registry/Dockerfile`, `rust-version` in the root
`Cargo.toml` and the Rust tests in the structure test together.
`rust-version-check.sh` fails CI if the first three differ.

The pinned `cargo-deny` checks workspace dependencies. `cargo-llvm-cov` runs CI's
coverage check locally; `AGENTS.md`, "Running the checks", has the command.

`postStartCommand.sh` runs `cargo fetch --locked` once a root `Cargo.toml`
exists. There are no backing services: SQLite is a file on the app's volume.

## Commit checks

The image ships `lefthook`, `gitleaks`, `actionlint` and `zizmor`, and `postStartCommand.sh` installs
the git hooks. The hook configuration (`lefthook.yml`) runs `gitleaks` on
staged changes before every commit made inside the container. It also runs
`actionlint` (with shellcheck) and `zizmor --offline --persona=pedantic` on staged workflow files
(`.github/workflows/*.{yml,yaml}`), skipping both for unrelated files,
`cargo fmt` and `cargo clippy` on the whole Cargo workspace when a `*.rs`,
`Cargo.toml` or `Cargo.lock` file is staged, and `cargo deny --locked check`
for workspace dependencies when a manifest or lockfile changes. The tests,
duplication and complexity steps remain placeholders until their tools are
installed; see `docs/conventions.md`. A commit that contains a secret is
blocked and the finding is redacted in the output.

Commits made outside the container skip the hook (git prints "Can't find
lefthook" and lets the commit through), so CI is the backstop for those.

## Nested Docker

`start.sh --docker` recreates the container as privileged and starts `dockerd`
inside it. Use it for the sessions that need Docker, then run `start.sh`
without the flag to go back to the unprivileged container. VS Code never uses
this mode.

## Changing the image

The image is `ubuntu:26.04`. Most tools come from Ubuntu's archive; `gh` and
`gitleaks` are too old there, so they're checksummed release binaries. `lefthook`, `actionlint`, `zizmor`, `cargo-deny` and `cargo-llvm-cov`
are checksummed binaries too. Rust comes from rustup, installed from a checksummed `rustup-init`. PMD CPD is a checksummed ZIP with its Java runtime
from the Ubuntu archive, Flutter and Chrome for Testing are checksummed archives, and `bats-core` (the bash test runner) is a
checksummed source tarball installed with its own `install.sh`.

`container-structure-test.yaml` is the spec for the image. Each pinned tool has
a version test there, so when you bump a pin in the `Dockerfile` (version and
SHA-256), update its test in the same commit.

`smoke-test.sh` starts a real container the way `start.sh` does and checks it
end to end: SSH forwarding, Claude settings, the project toolchain, local
coverage, the commit hook and its Rust lint step, nested Docker, and switching back to unprivileged. It needs network access
(it pulls `hello-world`). It takes a few minutes, mostly the local coverage run, and cleans up after
itself, using a throwaway Claude config volume so your login is never touched.

CI (`.github/workflows/devcontainer-ci.yml`) runs the container-structure test, the
smoke test and every tracked `*.bats` file on every push to `main` and every PR.

`token-file-check.bats` tests the token-file mode check in `start.sh` with dummy
files in temp dirs (including symlinks). It needs no Docker: run it with
`bats .devcontainer/token-file-check.bats`. CI finds and runs it, and any other
tracked `*.bats` file, automatically.

Besides that, `start.bats`, `post-start.bats` and `entrypoint.bats` test the
scripts here, and `.lefthook/placeholder.bats` the hook helper, with shimmed commands.
