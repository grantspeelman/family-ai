#!/usr/bin/env bats
# Tests session-start.sh with a fake cargo first on PATH that logs its working
# directory and arguments. Needs no Docker, no network and no Rust toolchain,
# and writes only under BATS_TEST_TMPDIR.

setup() {
    SCRIPT="$BATS_TEST_DIRNAME/session-start.sh"
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
    SHIMS="$BATS_TEST_TMPDIR/shims"
    CALLS="$BATS_TEST_TMPDIR/calls"
    PROJECT="$BATS_TEST_TMPDIR/project"
    mkdir -p "$SHIMS" "$PROJECT"
    : > "$CALLS"
    export CALLS
    # cargo exits with CARGO_STATUS (default 0).
    printf '#!/bin/bash\necho "$PWD cargo $*" >> "$CALLS"\nexit "${CARGO_STATUS:-0}"\n' > "$SHIMS/cargo"
    chmod +x "$SHIMS/cargo"
    export PATH="$SHIMS:$PATH"
    unset CLAUDE_CODE_REMOTE CLAUDE_PROJECT_DIR CARGO_STATUS
}

@test "CLAUDE_CODE_REMOTE unset: exit 0 and cargo is not called" {
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [ -z "$output" ]
    [ ! -s "$CALLS" ]
}

@test "CLAUDE_CODE_REMOTE=false: exit 0 and cargo is not called" {
    CLAUDE_CODE_REMOTE=false run "$SCRIPT"
    [ "$status" -eq 0 ]
    [ -z "$output" ]
    [ ! -s "$CALLS" ]
}

@test "CLAUDE_CODE_REMOTE=true: fetches then builds in CLAUDE_PROJECT_DIR" {
    CLAUDE_CODE_REMOTE=true CLAUDE_PROJECT_DIR="$PROJECT" run "$SCRIPT"
    [ "$status" -eq 0 ]
    mapfile -t calls < "$CALLS"
    [ "${#calls[@]}" -eq 2 ]
    [ "${calls[0]}" = "$PROJECT cargo fetch --locked" ]
    [ "${calls[1]}" = "$PROJECT cargo build --workspace --all-targets --locked" ]
}

@test "CLAUDE_PROJECT_DIR unset: runs in the repo root found from the script" {
    cd "$BATS_TEST_TMPDIR"
    CLAUDE_CODE_REMOTE=true run "$SCRIPT"
    [ "$status" -eq 0 ]
    mapfile -t calls < "$CALLS"
    [ "${#calls[@]}" -eq 2 ]
    [ "${calls[0]}" = "$REPO_ROOT cargo fetch --locked" ]
}

@test "a failing cargo fails the hook and stops before the build" {
    CARGO_STATUS=101 CLAUDE_CODE_REMOTE=true CLAUDE_PROJECT_DIR="$PROJECT" run "$SCRIPT"
    [ "$status" -ne 0 ]
    mapfile -t calls < "$CALLS"
    [ "${#calls[@]}" -eq 1 ]
    [ "${calls[0]}" = "$PROJECT cargo fetch --locked" ]
}
