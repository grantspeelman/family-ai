#!/bin/bash
# End-to-end check of the dev container through the same compose files start.sh
# uses: builds for the calling user's UID, forwards a throwaway ssh-agent, runs
# the post-start setup, then exercises nested Docker and dropping back out of it.
# Runs in CI (added in a later PR) and locally alike. Needs network access to pull hello-world. It uses
# its own compose project, a throwaway Claude config volume (never your real
# login's) and a temporary key in a fresh agent, and removes them all on exit.
set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT="family-ai-smoke-$$"
BASE=(docker compose -p "$PROJECT"
    -f "$SCRIPT_DIR/docker-compose.yml"
    -f "$SCRIPT_DIR/compose.ssh.yml"
    -f "$SCRIPT_DIR/smoke-test.compose.yml")
DOCKER=("${BASE[@]}" -f "$SCRIPT_DIR/compose.docker.yml")
in_container() { "${DOCKER[@]}" exec -T --user vscode app bash -c "$1"; }

fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok - $*"; }

tmp="$(mktemp -d)"
cleanup() {
    "${DOCKER[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
    [ -n "${SSH_AGENT_PID:-}" ] && kill "$SSH_AGENT_PID" 2>/dev/null || true
    rm -rf "$tmp"
}
trap cleanup EXIT

# A fresh agent holding a throwaway key: never the caller's own agent or keys.
eval "$(ssh-agent -a "$tmp/agent.sock")" >/dev/null
ssh-keygen -q -t ed25519 -N '' -f "$tmp/key"
ssh-add -q "$tmp/key"
export HOST_SSH_AUTH_SOCK="$SSH_AUTH_SOCK"
export USER_UID="$(id -u)" USER_GID="$(id -g)"
export SMOKE_CLAUDE_VOLUME="$PROJECT-claude-config"

echo "# privileged, with nested Docker"
"${DOCKER[@]}" up --detach --build --quiet-pull
timeout=90
until "${DOCKER[@]}" exec -T app docker info >/dev/null 2>&1; do
    timeout=$((timeout - 1))
    [ "$timeout" -gt 0 ] || { "${DOCKER[@]}" logs --tail 40 app >&2; fail "dockerd did not start"; }
    sleep 1
done
pass "dockerd started"

in_container '.devcontainer/postStartCommand.sh' >/dev/null
pass "postStartCommand.sh ran"

[ "$(in_container 'id -u')" = "$USER_UID" ] || fail "vscode UID does not match host UID $USER_UID"
pass "vscode runs as host UID $USER_UID"

[ "$(in_container 'ssh-add -l | wc -l')" = 1 ] || fail "forwarded agent does not hold the test key"
pass "ssh-agent forwarded"

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

[ "$(in_container "sqlite3 :memory: 'select 21*2'")" = 42 ] || fail "sqlite3 did not answer"
pass "sqlite3 works"

in_container 'test -x /workspace/.git/hooks/pre-commit' || fail "lefthook pre-commit hook not installed"
pass "pre-commit hook installed"

in_container '
    set -e; git init -q /tmp/t; cp /workspace/lefthook.yml /tmp/t/; cd /tmp/t
    git config user.email smoke@example.com; git config user.name smoke; lefthook install >/dev/null
    printf "token = \"ghp_%s\"\n" "$(head -c 300 /dev/urandom | tr -dc A-Za-z0-9 | head -c 36)" > leak.txt
    git add leak.txt; ! git commit -qm leak >/dev/null 2>&1' || fail "gitleaks hook let a token through"
pass "gitleaks hook blocks a token"

in_container 'docker run --rm --quiet hello-world' | grep -q 'Hello from Docker!' || fail "nested docker run failed"
pass "nested docker run works"

echo "# back to unprivileged"
"${BASE[@]}" up --detach
[ "$(docker inspect "$("${BASE[@]}" ps -q app)" --format '{{.HostConfig.Privileged}}')" = false ] \
    || fail "container still privileged without compose.docker.yml"
pass "container recreated unprivileged"
"${BASE[@]}" exec -T --user vscode app bash -c '! docker info >/dev/null 2>&1' || fail "dockerd running while unprivileged"
pass "no dockerd while unprivileged"

echo "all smoke tests passed"
