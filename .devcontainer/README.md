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

**VS Code (trusted Unix host):** run `.devcontainer/start-vscode.sh`, then use
*Dev Containers: Reopen in Container* in the new window. The launcher installs
Dev Containers into a disposable editor profile and starts a separate instance
with a fresh home directory, no inherited credentials or SSH agent, and host Git
config copying, credential helpers and Docker credential sharing disabled.
Ordinary Reopen from your existing editor is rejected by a host preflight.
Do not sign into GitHub, enable Settings Sync, or add credential helpers in this
profile. Keep the launcher terminal open; the profile is removed after the
workspace closes. Extensions and editor preferences are intentionally per-session.
This launcher needs a Unix host with Bash and `code` on PATH, and local Docker
accessible through its default socket. Remote Docker contexts and Windows hosts
are not supported by this launcher.

VS Code builds its own container from the same `docker-compose.yml`, separate
from the one `start.sh` builds. Dev Containers ordinarily shares host Git
credentials and forwards a running SSH agent; these are host-side features, so
clearing an environment variable inside the container is insufficient. See
[VS Code's credential sharing documentation](https://code.visualstudio.com/remote/advancedcontainers/sharing-git-credentials).
Docker Desktop's host-services SSH socket must also remain absent. Before relying
on this entry point after an editor upgrade, verify its container has no SSH
forwarding sockets or Git credential helper. A graphical end-to-end check is
still required; the automated tests exercise host launch/preflight behavior and
Compose configuration.

First time only: run `claude` (or `codex`) inside and log in. The logins live in the
`family-ai-claude-config` and `family-ai-codex-config` volumes, which every container for this repo
shares, so it survives rebuilds and covers both entry points.

## What the container can reach

| | |
|---|---|
| **Files** | Only this checkout, at `/workspace`. No host home directory, `~/.ssh` or `~/.claude`. |
| **SSH** | No host agent forwarding or host keys. The launcher never starts an agent or loads keys. |
| **GitHub API** | Public unauthenticated queries only; publishing credentials stay on the host. |
| **Claude** | Its own config (the volume above), with `permissions.defaultMode = bypassPermissions`. Your host Claude settings are untouched. |
| **Network** | Loopback and approved HTTPS destinations through the separate gateway. |

The application, its relay, terminal shells and editor commands start directly as
`vscode`. The user has no supplementary groups or sudo grant. Runtime capabilities
are dropped, `no-new-privileges` prevents setuid execution from gaining privileges,
and Docker's default seccomp and host security profiles remain enabled. System
tools are installed as root at image build time; rebuild to update them. Install
project dependencies and build outputs in the checkout, user home or `/tmp`.

The application is limited to 4 CPUs, 8 GiB of memory and 512 processes/threads.
These bounds allow the representative Rust build/test and hook checks in CI;
large future builds may need a reviewed adjustment. A host must have enough
resources available; limits do not reserve memory. Keep the bounds in the shared
Compose definition so terminal and editor sessions use the same policy.

Fresh named config volumes inherit `vscode` ownership from the built image.
Existing volumes keep their ownership: changing the host UID does not migrate
stored data. Startup rejects unwritable workspace/config directories. Recreate
disposable volumes, or stop the containers and have a trusted host administrator
repair retained volume ownership to the configured UID/GID. Do not enable runtime
sudo or run the app as root to repair them. Terminal builds use the calling host
UID/GID; the isolated editor launcher passes the same build arguments and disables
Dev Containers' runtime UID rewriting. Start the editor as a non-root host user.

### Host namespace options

Host rootless Docker and daemon user-namespace remapping are additional host
protections worth evaluating in a separate disposable host. They map container
IDs into host ranges, which can make existing bind-mounted checkout/config volume
ownership incompatible with the image's host-UID strategy. Neither mode is enabled
or reconfigured by these launchers. The editor launcher currently requires local
Docker through the default socket, so rootless Docker's per-user socket is not a
supported editor configuration. Rootless daemon deployments also need working
cgroup delegation to enforce the resource limits; verify effective limits on the
host before use. Daemon user-namespace remapping requires an administrator-managed
subordinate UID/GID range and matching bind-mount ownership.

The supported acceptance path is the normal local Docker daemon (or Docker
Desktop Linux VM) with writable host-UID bind mounts. Namespace configurations
outside that path need their own smoke run and editor acceptance check. An
unwritable mount or unavailable engine must fail rather than silently disable
namespace isolation, seccomp, capability drops or resource limits.

## Publishing reviewed work

Edit, test and commit in the container. Configure a local Git author identity
with `git config user.name` and `git config user.email` if needed; identity is
not authentication. Do not run `gh auth login` or place publishing tokens, SSH
keys, or credential files in the checkout or agent volumes.

After the agent stops, use a trusted host terminal with your normal credentials.
Review the branch commits and diff, including changes to Git hooks/configuration
and host-executed files, before publishing. From that reviewed checkout:

```sh
git log --oneline main..<branch>
git diff main...<branch>
git push -u origin <branch>
gh pr create --base <base-branch> --head <branch> --title '<title>' --body '<description>'
```

Use an explicit branch and the appropriate base for stacked work. Do not invoke
publishing commands from an agent process. There is no authenticated container
mode. `python3 -I .devcontainer/test-publishing.py` verifies the local commit and
host push sequence against a disposable local bare remote; it does not contact
GitHub. Recreate existing containers after this change and remove any previously
saved publishing credentials before using them for agent work. Provider logins
are managed separately.

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

Use a trusted host session for publishing and these commands. The existing
`image` and
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
to end: runtime privileges and mounts, credential absence, Claude settings, the
project toolchain, the commit hook, direct non-root identity, dropped capabilities,
active seccomp, no-new-privileges and a harmless root-owned setuid probe. It runs from a Docker-capable CI runner
or trusted host, with throwaway Claude and Codex config volumes and dummy host
token values.
It does not mount a Docker socket or start a nested daemon.

CI (`.github/workflows/devcontainer-ci.yml`) runs the image and smoke checks on
every push to `main` and every PR. Launcher tests and effective Compose
configuration checks run in the existing `image` job without agent logins.

## Runtime network access

Both launchers use an application container with loopback networking only.
HTTPS clients use a local relay to a separate gateway that permits exact provider
and Cargo registry destinations. Direct internet, DNS, host/LAN and sibling-service
connections are unavailable. Local development servers still work on loopback.
See [the runtime network policy](egress/README.md) for the allowlist, authentication
steps, limitations, and Docker-host acceptance tests. Rebuild both images after a
reviewed policy update; do not enable unrestricted networking for troubleshooting.
