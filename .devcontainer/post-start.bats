#!/usr/bin/env bats
# Tests postStartCommand.sh with an isolated HOME and git config, a temp Claude
# config dir, and fake lefthook, cargo and ssh-add first on PATH. Needs no
# Docker and no network, and installs no real hooks. Only dummy values are used.
#
# The script checks /workspace/.git and /workspace/Cargo.toml by absolute path.
# Those branches depend on the machine, so the tests compute what to expect.

setup() {
    SCRIPT="$BATS_TEST_DIRNAME/postStartCommand.sh"
    SHIMS="$BATS_TEST_TMPDIR/shims"
    CALLS="$BATS_TEST_TMPDIR/calls"
    mkdir -p "$SHIMS" "$BATS_TEST_TMPDIR/home" "$BATS_TEST_TMPDIR/claude"
    : > "$CALLS"
    : > "$BATS_TEST_TMPDIR/gitconfig"
    export CALLS
    for c in lefthook cargo; do
        printf '#!/bin/bash\necho "%s $*" >> "$CALLS"\n' "$c" > "$SHIMS/$c"
    done
    # ssh-add -l exits with SSH_ADD_STATUS (default 0).
    printf '#!/bin/bash\nexit "${SSH_ADD_STATUS:-0}"\n' > "$SHIMS/ssh-add"
    chmod +x "$SHIMS"/*
    export PATH="$SHIMS:$PATH"
    export HOME="$BATS_TEST_TMPDIR/home"
    export GIT_CONFIG_GLOBAL="$BATS_TEST_TMPDIR/gitconfig"
    export GIT_CONFIG_SYSTEM="$BATS_TEST_TMPDIR/gitconfig"
    export CLAUDE_CONFIG_DIR="$BATS_TEST_TMPDIR/claude"
    unset SSH_AUTH_SOCK GH_TOKEN GITHUB_TOKEN SSH_ADD_STATUS
    # A logged-in, tokened, agent-less baseline is set per test as needed.
    echo '{}' > "$CLAUDE_CONFIG_DIR/.credentials.json"
}

@test "fails with a clear message when CLAUDE_CONFIG_DIR is unset" {
    unset CLAUDE_CONFIG_DIR
    run "$SCRIPT"
    [ "$status" -ne 0 ]
    [[ "$output" == *"CLAUDE_CONFIG_DIR is set in the Dockerfile"* ]]
}

@test "lefthook install runs only when /workspace/.git is a directory" {
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    if [ -d /workspace/.git ]; then
        grep -qx 'lefthook install' "$CALLS"
    else
        if grep -q lefthook "$CALLS"; then false; fi
    fi
}

@test "cargo fetch --locked runs only when /workspace/Cargo.toml exists" {
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    if [ -f /workspace/Cargo.toml ]; then
        grep -qx 'cargo fetch --locked' "$CALLS"
    else
        if grep -q cargo "$CALLS"; then false; fi
    fi
}

@test "missing settings.json is created with bypassPermissions" {
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [ "$(jq -r .permissions.defaultMode "$CLAUDE_CONFIG_DIR/settings.json")" = bypassPermissions ]
    [ ! -e "$CLAUDE_CONFIG_DIR/settings.json.tmp" ]
}

@test "empty settings.json is treated as missing" {
    : > "$CLAUDE_CONFIG_DIR/settings.json"
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [ "$(jq -r .permissions.defaultMode "$CLAUDE_CONFIG_DIR/settings.json")" = bypassPermissions ]
}

@test "existing settings keep their other keys and a different mode is overwritten" {
    echo '{"theme":"dark","permissions":{"allow":["Bash(ls)"],"defaultMode":"default"}}' \
        > "$CLAUDE_CONFIG_DIR/settings.json"
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [ "$(jq -r .theme "$CLAUDE_CONFIG_DIR/settings.json")" = dark ]
    [ "$(jq -r '.permissions.allow[0]' "$CLAUDE_CONFIG_DIR/settings.json")" = "Bash(ls)" ]
    [ "$(jq -r .permissions.defaultMode "$CLAUDE_CONFIG_DIR/settings.json")" = bypassPermissions ]
}

@test "running twice is idempotent" {
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    first=$(cat "$CLAUDE_CONFIG_DIR/settings.json")
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [ "$(cat "$CLAUDE_CONFIG_DIR/settings.json")" = "$first" ]
}

@test "invalid settings.json fails the script and is left unchanged" {
    echo 'not json' > "$CLAUDE_CONFIG_DIR/settings.json"
    run "$SCRIPT"
    [ "$status" -ne 0 ]
    [ "$(cat "$CLAUDE_CONFIG_DIR/settings.json")" = "not json" ]
}

@test "no SSH_AUTH_SOCK: says the agent is not forwarded" {
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *"No SSH agent forwarded"* ]]
}

@test "SSH agent with keys: no SSH message" {
    SSH_AUTH_SOCK=/dummy.sock SSH_ADD_STATUS=0 run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" != *"SSH agent"* ]]
}

@test "SSH agent without keys (ssh-add status 1): says to run ssh-add" {
    SSH_AUTH_SOCK=/dummy.sock SSH_ADD_STATUS=1 run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *"holds no keys"* ]]
}

@test "dead SSH agent (ssh-add status 2): says to rebuild" {
    SSH_AUTH_SOCK=/dummy.sock SSH_ADD_STATUS=2 run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *"socket is dead"* ]]
    [[ "$output" == *"start.sh --rebuild"* ]]
}

@test "no GH_TOKEN or GITHUB_TOKEN: prints the missing-token message" {
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *"No GitHub token in the container"* ]]
}

@test "GH_TOKEN set: no token message" {
    GH_TOKEN=dummy run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" != *"No GitHub token"* ]]
}

@test "GITHUB_TOKEN set: no token message" {
    GITHUB_TOKEN=dummy run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" != *"No GitHub token"* ]]
}

@test "no credentials file: says Claude is not logged in" {
    rm "$CLAUDE_CONFIG_DIR/.credentials.json"
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Claude is not logged in yet"* ]]
}

@test "empty credentials file counts as not logged in" {
    : > "$CLAUDE_CONFIG_DIR/.credentials.json"
    run "$SCRIPT"
    [[ "$output" == *"Claude is not logged in yet"* ]]
}

@test "fully set up: prints nothing and touches no real git config" {
    GH_TOKEN=dummy SSH_AUTH_SOCK=/dummy.sock run "$SCRIPT"
    [ "$status" -eq 0 ]
    [ -z "$output" ]
    # Only a guard: the script is not expected to write the global git config here.
    [ ! -s "$GIT_CONFIG_GLOBAL" ]
}
