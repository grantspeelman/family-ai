# Dev container

A container for working on family-ai with Claude Code or Codex, using a
disposable source copy and separately installed, reviewed host controls.

## Getting in

From a trusted host terminal, review and commit the desired container controls,
then install a new session **outside** the trusted checkout:

```sh
python3 -I .devcontainer/host-session.py create \
  --source "$PWD" --session "$HOME/family-ai-sessions/session-01"
```

Create the parent directory first. Installation uses committed `HEAD`, copies
`.devcontainer/` into `session-01/controls`, and makes a full independent Git clone
in `session-01/work` on branch `agent-work`. Uncommitted files and host Git
configuration/hooks are not imported. A trusted Git worktree may be the input;
the result has its own Git directory, objects and refs, without shared pointers.
The installer never replaces an existing session, even after a partial failure.

**Terminal:** run `session-01/controls/.devcontainer/start.sh` from the host.
It builds and starts the container and opens Bash; repeat to reconnect. Running
`start.sh` from an ordinary checkout or from `work` is refused. Keep using the
installed control path for subsequent launches, rebuilds and stops.

| Flag        | Effect |
|-------------|--------|
| `--stop` | Stop the terminal container while retaining source and selected provider state. |
| `--state claude`, `--state codex`, `--state none` | Select persisted state explicitly for a shell or arbitrary command. Direct agent commands select their own state automatically. |
| `--rebuild` | Rebuild the image from scratch. This is also how Claude Code and Codex get upgraded. |
| `--claude`  | Run Claude Code directly instead of a shell. Sets `HERDR_AGENT` so [herdr](https://herdr.dev) sees the agent. |
| `--codex`   | Same, for Codex. |

Run a specific command by putting it and its arguments after `--`:

```sh
session-01/controls/.devcontainer/start.sh --rebuild -- claude --resume
session-01/controls/.devcontainer/start.sh -- codex resume
session-01/controls/.devcontainer/start.sh -- bash -lc 'echo "hello from the container"'
```

Everything after `--` is passed directly to the container command, preserving
argument boundaries. With no command, the launcher opens Bash. Commands named
`claude` or `codex` set `HERDR_AGENT` for [herdr](https://herdr.dev); the
`--claude` and `--codex` shortcuts still work, but cannot be combined with `--`.

**VS Code (trusted Unix host):** run
`session-01/controls/.devcontainer/start-vscode.sh`, then use
*Dev Containers: Reopen in Container* in the new window. The launcher installs
Dev Containers into a disposable editor profile and starts a separate instance
with a fresh home directory, no inherited credentials or SSH agent, and host Git
config copying, credential helpers and Docker credential sharing disabled.
Ordinary Reopen from your existing editor is rejected by a host preflight.
Do not sign into GitHub, enable Settings Sync, or add credential helpers in this
profile. Keep the launcher terminal open; the profile is removed after the
workspace closes; the source copy remains. Extensions and editor preferences are intentionally per-session.
This launcher needs a Unix host with Bash and `code` on PATH, and local Docker
accessible through its default socket. Remote Docker contexts and Windows hosts
are not supported by this launcher.

The local editor opens the installed `controls` directory to discover the trusted
Dev Containers definition; after Reopen, the remote folder is the disposable
source at `/workspace`. It does not open the agent copy as a host workspace.
Do not use Reopen from `work`, or run its proposed host scripts. The trusted
`postStartCommand.sh` is mounted read-only at `/opt/family-ai-controls`. Source
changes to `.devcontainer/` do not change active controls or future restarts.
To activate reviewed controls, commit them in the trusted checkout and install a
new session; no automatic control update exists.

VS Code builds its own container from the same trusted `docker-compose.yml`, separate
from the one `start.sh` builds. Dev Containers ordinarily shares host Git
credentials and forwards a running SSH agent; these are host-side features, so
clearing an environment variable inside the container is insufficient. See
[VS Code's credential sharing documentation](https://code.visualstudio.com/remote/advancedcontainers/sharing-git-credentials).
Docker Desktop's host-services SSH socket must also remain absent. Before relying
on this entry point after an editor upgrade, verify its container has no SSH
forwarding sockets or Git credential helper. A graphical end-to-end check is
still required; the automated tests exercise host launch/preflight behavior and
Compose configuration.

## Provider state and authentication

A plain terminal or editor launch selects `none`: no provider state directory is
mounted. `start.sh --claude`, `start.sh --codex`, and direct commands after `--`
select the named provider automatically. For a shell that needs one provider's
login, use `start.sh --state codex -- bash` (or `--state claude`). Wrappers such as
`bash -lc 'codex'` need that explicit state flag. Conflicting direct-agent and
state selections are refused.

For the editor use `start-vscode.sh --claude` or `start-vscode.sh --codex` before
any file paths. Only that provider's extension is added to the remote container;
a plain editor launch adds neither provider extension. The selected extension
may need additional approved network endpoints or an offline extension package;
no gateway policy changes are made automatically. GUI authentication remains a
manual acceptance check.

Each installed session owns separate `state/claude` and `state/codex` directories
next to `controls` and `work`. Only the selected directory is bound into the app.
The other provider's home is an empty image directory and does not contain its
persisted state. First run: log in inside the selected session. Stop/start and
image rebuilds retain that session's login. Different session directories,
checkouts and trust levels share no provider state. Create a new installed session
for each trust level; there is no global provider-volume name or automatic import.
Both entry points intentionally share the selected state within one session.

Switching selection stops and removes that session's existing app containers,
including editor containers, before changing the trusted Compose override. Close
the editor and finish work in other terminals before switching. Never run both
launchers concurrently during selection/reset; coordinate these host operations.
Source, provider state and the egress volume are retained. Reconnecting with the
same selection keeps the current container.

The provider process can read its own credentials, as can commands running as the
same user. Selection limits which provider's storage is exposed; it cannot hide
that provider's credentials from its own process. State may also include
executable plugins, hooks, skills, MCP definitions, settings, session histories,
and caches. An image rebuild does not clean this persisted configuration.
Keep real `.env` files, publishing credentials, private keys and unrelated secrets
outside both the trusted checkout and disposable `work`; source installation
copies committed files only, so do not commit any of those files. The build
context is separately restricted to explicitly allowed image inputs through
`.dockerignore` and matching Dockerfile-specific policies. Add a reviewed allowlist
entry when a new image input is needed; arbitrary source files cannot be copied
into an image without updating that policy.

### Reset, logout and migration

Run lifecycle operations from the trusted host using installed controls:

```sh
python3 -I session-01/controls/.devcontainer/agent-state.py reset codex
python3 -I session-01/controls/.devcontainer/agent-state.py reset claude
```

Reset first stops/removes all labeled app containers for that source, including
terminal and IDE projects, and refuses cleanup if any remain. Close the editor
first so it cannot reconnect while the operation is running. Reset deletes
settings, hooks, plugins, histories and caches for the named provider, retaining
only Codex `auth.json` or Claude `.credentials.json` when present. It does not
preserve arbitrary configuration or credentials supplied through other mechanisms.
Some versions may require onboarding or login again after configuration reset;
no selective authentication-only mount is assumed. Provider state files must be
regular single-link files; symlink login files cause reset to refuse cleanup.
Reset uses dummy login markers in tests and never reads real logins in CI.

For logout, use the provider's own logout operation in the selected session
(`codex logout`, or Claude's `/logout` command). To remove all locally stored state
as well, run the same host reset command with `--including-auth`. Local deletion
does not revoke copied tokens or provider sessions: use the provider's account
controls to revoke sessions/keys and rotate affected credentials, then log in
again. No command here automatically revokes or rotates credentials.

Legacy named volumes `family-ai-claude-config` and `family-ai-codex-config` are
never mounted, migrated or deleted automatically. The preferred migration is a
fresh provider login in each new session. If retaining an old login is necessary,
stop every old container using that volume, use a trusted host backup/export
procedure, inspect the backup as data, and deliberately copy only the relevant
login file into the new selected state directory with owner-only permissions.
Do not execute or copy old hooks/plugins/settings as part of login migration.
Keep the old volume until login in the new session works; remove that exact old
volume only when deliberately retiring it. Protect backups like credentials.

Manual acceptance for each provider: create a fresh selected session, log in,
verify restart and rebuild keep the login, reset disposable settings and check
whether re-onboarding is required, log out, and verify a subsequent restart asks
for login. Repeat in the clean editor entry point, checking its effective mounts.
These authenticated/GUI checks require a user account and are not claimed by the
secret-free automated smoke fixtures.

## What the container can reach

| | |
|---|---|
| **Files** | Disposable `work` at `/workspace`; reviewed lifecycle controls read-only at `/opt/family-ai-controls`. Trusted checkout and its Git metadata are not mounted. |
| **SSH** | No host agent forwarding or host keys. The launcher never starts an agent or loads keys. |
| **GitHub API** | Public unauthenticated queries only; publishing credentials stay on the host. |
| **Provider state** | Only the selected provider directory; shell defaults to neither. Claude settings are initialized only when Claude is selected. |
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

Selected host state directories are created with owner-only permissions by the
host launcher. Existing state retains its ownership: changing host UID does not
migrate it. Startup rejects unwritable workspace/config directories. Stop all
session containers and have a trusted host administrator repair ownership to the
configured UID/GID if needed. Do not enable runtime sudo or run the app as root to
repair them. Terminal builds use the calling host UID/GID; the isolated editor
launcher passes the same arguments and disables runtime UID rewriting. Start
the editor as a non-root host user.

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

Stop the agent and all editor/terminal sessions before exporting a coherent
snapshot. For terminal containers use the installed `start.sh --stop`; close the
editor and use Dev Containers' stop operation for its separate container.
Do not run Git in `work` from the host: agent-written Git configuration and hooks
remain untrusted. Local commits stay in the disposable clone.

The installed transfer tool reads explicitly selected source paths as data,
including current uncommitted content and deletions. It never runs Git against
that copy, imports Git metadata, executes scripts, or follows source/target
symlinks. Descriptor-based traversal retains the boundary during directory swaps.
Transfers are limited to 128 files, 8 MiB per file, 32 MiB total input on each
side and 48 MiB for the JSON bundle. Review output escapes terminal control and
invisible formatting characters. Export only intended files; ignored files are not automatically excluded when
explicitly selected. Store bundles outside the disposable copy:

```sh
python3 -I session-01/controls/.devcontainer/host-session.py export \
  --path path/to/changed-file --path path/to/deleted-file --output "$HOME/review.json"
python3 -I session-01/controls/.devcontainer/host-session.py review \
  --bundle "$HOME/review.json" --target /path/to/trusted-checkout
```

Review every proposed path/content before applying. JSON bundles are data, not
Git patches; the review command renders a text diff without filters or external
diff tools. Inspect binary content separately if present. Protect the exported
bundle from changes and keep the trusted target unchanged between review and apply.
Then use the same installed tool and bundle:

```sh
python3 -I session-01/controls/.devcontainer/host-session.py apply \
  --bundle "$HOME/review.json" --target /path/to/trusted-checkout --reviewed
```

Apply copies only listed regular files/deletions. Existing executable permissions
are retained; new files default to mode 0644, so explicitly review and set any new
executable permissions. Symlinks, directories, rename metadata and original commit
history are not transferred. Select old and new paths for renames. This transfer
is not transactional across files: inspect reported failures and the trusted diff
before continuing. Review transferred host scripts before executing them.

In the trusted checkout, inspect the resulting diff, commit reviewed changes and
publish with the host's normal credentials (`git push` and `gh pr create`). Set
explicit branches/bases for stacked work. There is no authenticated container
mode. `test-isolation.py` covers data transfer against fixtures with hostile Git
configuration; `test-publishing.py` checks local commits and a fixture bare remote.
Neither contacts GitHub. Provider logins are managed separately.

## Reset and cleanup

`start.sh --stop` never removes source or provider state. A fresh session always
uses a new destination, new provider state and a new disposable clone. Before retiring an old session,
stop both entry points, export and review all work you intend to keep, and verify
it reached the trusted checkout. Only then manually delete that exact old session
directory from the host. There is no implicit workspace cleanup or operation that deletes unrelated
checkouts. Deleting an installed session directory also deletes its provider
state; retire it deliberately. Legacy named volumes remain separate.

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
active seccomp, no-new-privileges and a harmless root-owned setuid probe.
The probe is baked into a smoke-only build stage; default terminal/editor images
exclude it, and smoke setup requires no runtime privilege escalation. It runs from a Docker-capable CI runner
or trusted host, with throwaway selected provider directories and dummy host
token values. It recreates the app for Claude, Codex and plain shell profiles and
checks marker isolation, effective state mounts and normal command execution.
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
