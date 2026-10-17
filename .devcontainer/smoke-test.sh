#!/bin/bash
# End-to-end check of the dev container through the same compose files start.sh
# uses: builds for the calling user's UID, runs
# the post-start setup, then checks normal tooling and runtime isolation.
# Runs from a Docker-capable CI runner or trusted host, outside agent sessions. It uses
# its own compose project, throwaway Claude and Codex config volumes (never your real
# logins'), and removes them all on exit.
set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT="family-ai-smoke-$$"
BASE=(docker compose -p "$PROJECT"
    -f "$SCRIPT_DIR/docker-compose.yml"
    -f "$SCRIPT_DIR/smoke-test.compose.yml")
in_container() { "${BASE[@]}" exec -T --user vscode app bash -c "$1"; }

fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok - $*"; }

tmp="$(mktemp -d)"
cleanup() {
    "${BASE[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
    rm -rf "$tmp"
}
trap cleanup EXIT

export USER_UID="$(id -u)" USER_GID="$(id -g)"
export SMOKE_CLAUDE_VOLUME="$PROJECT-claude-config"
export SMOKE_CODEX_VOLUME="$PROJECT-codex-config"

# Dummy host values prove Compose does not transport publishing credentials.
export GH_TOKEN=dummy-host-gh-token GITHUB_TOKEN=dummy-host-github-token
export SSH_AUTH_SOCK="$tmp/unforwarded-agent.sock"

echo "# standard agent runtime"
"${BASE[@]}" up --detach --build --quiet-pull
container_id="$("${BASE[@]}" ps -q app)"
docker inspect "$container_id" | jq -e '
    .[0] | .HostConfig.Privileged == false
    and ((.HostConfig.Devices // []) | length == 0)
    and ((.HostConfig.CapAdd // []) | length == 0)
    and (.Config.User == "vscode")
    and (.HostConfig.CapDrop == ["ALL"])
    and (.HostConfig.SecurityOpt == ["no-new-privileges:true"])
    and (.AppArmorProfile != "unconfined")
    and (.HostConfig.Memory == 8589934592)
    and (.HostConfig.NanoCpus == 4000000000)
    and (.HostConfig.PidsLimit == 512)
    and (.HostConfig.NetworkMode == "none")
    and (.HostConfig.PidMode != "host")
    and all(.Mounts[]; .Destination == "/workspace"
        or .Destination == "/home/vscode/.claude"
        or .Destination == "/home/vscode/.codex"
        or (.Destination == "/run/egress" and .RW == false))' >/dev/null \
    || fail "unexpected agent runtime privileges or mounts"
pass "agent runtime privileges and mounts checked"
# The smoke-only image stage builds the harmless root-owned setuid fixture.
# Runtime capabilities and identity remain identical to the development image.
# No --user override: verify the default terminal/IDE container execution user.
"${BASE[@]}" exec -T app python3 -I /workspace/.devcontainer/test-privileges.py \
    || fail "non-root runtime checks failed"
pass "runtime identity, privilege boundaries, immutable tools and fresh volumes checked"
in_container 'python3 -I /workspace/.devcontainer/egress/test-network.py' \
    || fail "network isolation checks failed"
pass "DNS, IPv4, IPv6 and proxy-bypass checks passed"
# Real public, secret-free HTTPS requests check the relay and approved registry.
in_container 'curl --fail --silent --show-error --max-time 30 https://index.crates.io/config.json >/dev/null' \
    || fail "allowed registry HTTPS request failed"
in_container 'if curl --silent --fail --max-time 5 https://example.invalid/; then exit 1; fi' \
    || fail "unapproved proxy destination accepted"
pass "allowed HTTPS destination works and unapproved destination is denied"
in_container '! command -v docker && ! command -v dockerd && ! command -v containerd' \
    || fail "Docker binaries present in agent image"
in_container 'test ! -S /var/run/docker.sock && test ! -S /run/containerd/containerd.sock' \
    || fail "Docker runtime socket present"
pass "no Docker binaries or runtime sockets"

in_container '.devcontainer/postStartCommand.sh' >/dev/null
pass "postStartCommand.sh ran"

[ "$(in_container 'id -u')" = "$USER_UID" ] || fail "vscode UID does not match host UID $USER_UID"
pass "vscode runs as host UID $USER_UID"

in_container 'test -z "${GH_TOKEN:-}${GITHUB_TOKEN:-}${SSH_AUTH_SOCK:-}" &&
    test ! -S /run/ssh-agent.sock && test ! -S /run/host-services/ssh-auth.sock &&
    test ! -e "$HOME/.config/gh/hosts.yml" &&
    test -z "$(git config --get-all credential.helper || true)"' \
    || fail "publishing credentials reached container"
pass "no publishing tokens, SSH socket or Git credential helper"

[ "$(in_container 'jq -r .permissions.defaultMode "$CLAUDE_CONFIG_DIR/settings.json"')" = bypassPermissions ] \
    || fail "bypassPermissions not set"
pass "Claude defaults to bypassPermissions"

# A throwaway crate proves the Rust toolchain end to end without touching the
# checkout or needing the network: build, test, lint, format and a SQLite round trip.
in_container '
    set -e; cargo new --quiet --vcs none /tmp/smoke-crate; cd /tmp/smoke-crate
    cargo build --quiet && cargo test --quiet && cargo clippy --quiet -- -D warnings && cargo fmt --check' \
    || fail "Rust build/test/clippy/fmt failed"
pass "cargo build, test, clippy and fmt work"
in_container 'cp -r /workspace/.devcontainer/egress/fetch-fixture /tmp/fetch-fixture &&
    CARGO_HOME=/tmp/fetch-home cargo fetch --locked --manifest-path /tmp/fetch-fixture/Cargo.toml' \
    || fail "locked dependency fetch through gateway failed"
pass "locked dependency fetch works through gateway"

[ "$(in_container "sqlite3 :memory: 'select 21*2'")" = 42 ] || fail "sqlite3 did not answer"
pass "sqlite3 works"

in_container 'test -x /workspace/.git/hooks/pre-commit' || fail "lefthook pre-commit hook not installed"
pass "pre-commit hook installed"

in_container '
    set -e; git init -q /tmp/t; cp /workspace/lefthook.yml /tmp/t/; cd /tmp/t
    git config user.email smoke@example.com; git config user.name smoke; lefthook install >/dev/null
    printf "token = \"ghp_%s\"\n" "$(head -c 300 /dev/urandom | tr -dc A-Za-z0-9 | head -c 36)" > leak.txt
    git add leak.txt
    if out="$(git commit -qm leak 2>&1)"; then echo "commit succeeded" >&2; exit 1; fi
    printf "%s\n" "$out" | grep -q "leaks found"' || fail "gitleaks hook did not block a token"
pass "gitleaks hook blocks a token"

in_container '
    set -e; cd /tmp/t; git rm -qf --cached leak.txt; rm leak.txt
    echo clean > ok.txt; git add ok.txt; git commit -qm clean >/dev/null 2>&1' || fail "clean change was not committed"
pass "clean change commits normally"

# Losing the gateway cannot reopen direct egress. New sessions fail closed.
"${BASE[@]}" stop egress >/dev/null
in_container 'if curl --fail --silent --max-time 3 https://index.crates.io/config.json; then exit 1; fi' \
    || fail "gateway loss did not stop egress"
if "${BASE[@]}" run --rm --no-deps -T app true >/dev/null 2>&1; then
    fail "normal session started without gateway"
fi
pass "gateway loss denies requests and normal session startup"
echo "all smoke tests passed"
