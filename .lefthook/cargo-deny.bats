#!/usr/bin/env bats
# Exercise the real hook in disposable repositories; shim cargo to avoid network.
bats_require_minimum_version 1.5.0

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"
    cd "$BATS_TEST_TMPDIR"
    git init -q
    git config user.name 'Dependency gate test'
    git config user.email 'dependency-test@example.invalid'
    git config commit.gpgsign false
    git config core.hooksPath .git/hooks
    cp "$REPO_ROOT/lefthook.yml" .
    mkdir -p bin
    export CARGO_DENY_CALLS="$BATS_TEST_TMPDIR/cargo-calls"
    cat > bin/cargo <<'SH'
#!/bin/sh
# The Rust lint step shares the manifest glob; its behavior has its own tests.
if [ "$1" = deny ]; then
    printf '%s\n' "$*" >> "$CARGO_DENY_CALLS"
    exit "${CARGO_DENY_STATUS:-0}"
fi
exit 0
SH
    chmod +x bin/cargo
    export PATH="$BATS_TEST_TMPDIR/bin:$PATH"
    lefthook install --reset-hooks-path
}

commit_file() {
    mkdir -p "$(dirname "$1")"
    printf '# fixture\n' > "$1"
    git add "$1"
    run git commit -m 'Test dependency gate'
}

assert_checked() {
    [ "$status" -eq 0 ]
    [ "$(cat "$CARGO_DENY_CALLS")" = 'deny --locked check' ]
    [[ "$output" != *"PLACEHOLDER: 'cargo-deny'"* ]]
}

@test "workspace manifest runs cargo-deny" {
    commit_file Cargo.toml
    assert_checked
}

@test "workspace lockfile runs cargo-deny" {
    commit_file Cargo.lock
    assert_checked
}

@test "nested crate manifest runs cargo-deny" {
    commit_file registry/Cargo.toml
    assert_checked
}

@test "nested lockfile runs cargo-deny" {
    commit_file registry/Cargo.lock
    assert_checked
}

@test "cargo-deny failure blocks the commit" {
    export CARGO_DENY_STATUS=1
    commit_file Cargo.toml
    [ "$status" -ne 0 ]
    [ "$(cat "$CARGO_DENY_CALLS")" = 'deny --locked check' ]
    run git rev-parse --verify HEAD
    [ "$status" -ne 0 ]
}

@test "unrelated files skip cargo-deny" {
    commit_file note.md
    [ "$status" -eq 0 ]
    [ ! -e "$CARGO_DENY_CALLS" ]
    [[ "$output" == *"cargo-deny (skip)"* ]]
}
