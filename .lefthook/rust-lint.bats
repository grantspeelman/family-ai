#!/usr/bin/env bats
# The hook's 2-lint step (cargo fmt and clippy) with real cargo and real commits
# in a disposable Cargo workspace. Needs cargo with clippy and rustfmt, lefthook
# and gitleaks, as in the dev container; no network (the crate has no dependencies).
bats_require_minimum_version 1.5.0

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"
    cd "$BATS_TEST_TMPDIR"
    git init -q
    git config user.name 'Rust lint test'
    git config user.email 'rust-lint-test@example.invalid'
    git config commit.gpgsign false
    git config core.hooksPath .git/hooks
    cp "$REPO_ROOT/lefthook.yml" .
    # Later placeholder steps share the *.rs glob and call this script.
    mkdir -p .lefthook
    cp "$REPO_ROOT/.lefthook/placeholder.sh" .lefthook/
    printf 'target/\n' > .gitignore

    # A workspace whose only crate is in a subdirectory, so the checks must be
    # workspace-wide to reach it.
    cat > Cargo.toml <<'TOML'
[workspace]
members = ["crates/demo"]
resolver = "3"
TOML
    mkdir -p crates/demo/src
    cat > crates/demo/Cargo.toml <<'TOML'
[package]
name = "demo"
version = "0.1.0"
edition = "2024"
TOML
    cat > crates/demo/src/lib.rs <<'RS'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
RS
    cargo generate-lockfile --offline --quiet
    lefthook install --reset-hooks-path
}

assert_not_committed() {
    run git rev-parse --verify HEAD
    [ "$status" -ne 0 ]
}

@test "formatted, clippy-clean Rust commits with the lint step run" {
    git add -A
    run git commit -m 'Add demo crate'
    [ "$status" -eq 0 ]
    [[ "$output" == *2-lint* ]]
    [[ "$output" != *"2-lint (skip)"* ]]
    [[ "$output" == *"Checking demo"* ]]
}

@test "badly formatted .rs file blocks the commit" {
    printf 'pub fn add(a:i32,b:i32)->i32{a+b}\n' > crates/demo/src/lib.rs
    git add -A
    run git commit -m 'Add unformatted code'
    [ "$status" -ne 0 ]
    [[ "$output" == *"Diff in"* ]]
    assert_not_committed
}

@test "clippy warning blocks the commit" {
    # Formatted, compiles cleanly, but clippy::ptr_arg warns (denied by -D warnings).
    cat > crates/demo/src/lib.rs <<'RS'
pub fn count(values: &Vec<i32>) -> usize {
    values.len()
}
RS
    git add -A
    run git commit -m 'Add clippy warning'
    [ "$status" -ne 0 ]
    [[ "$output" == *"clippy::ptr_arg"* ]]
    assert_not_committed
}

@test "staged Cargo.toml in a member crate runs the workspace-wide checks" {
    git add .gitignore Cargo.lock lefthook.yml .lefthook crates/demo/src
    git commit -q -m 'Initial'
    printf 'pub fn add(a:i32,b:i32)->i32{a+b}\n' > crates/demo/src/lib.rs
    printf 'description = "Demo"\n' >> crates/demo/Cargo.toml
    git add crates/demo/Cargo.toml
    run git commit -m 'Change only the member manifest'
    [ "$status" -ne 0 ]
    [[ "$output" == *"Diff in"* ]]
    [ "$(git rev-list --count HEAD)" -eq 1 ]
}

@test "unrelated staged file skips the lint step" {
    # Unformatted Rust in the working tree would fail the step if it ran.
    printf 'pub fn add(a:i32,b:i32)->i32{a+b}\n' > crates/demo/src/lib.rs
    printf 'An unrelated change.\n' > note.md
    git add note.md
    run git commit -m 'Add unrelated file'
    [ "$status" -eq 0 ]
    [[ "$output" == *"2-lint (skip)"* ]]
}
