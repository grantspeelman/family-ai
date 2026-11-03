#!/usr/bin/env bats
# The factory's allowed-paths check, run against a disposable git repo whose
# origin is a local bare repo. Needs only git; no network.
bats_require_minimum_version 1.5.0

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../../.." && pwd)"
    SCRIPT="$REPO_ROOT/.claude/skills/factory/allowed-paths.sh"
    ORIGIN="$BATS_TEST_TMPDIR/origin.git"
    WORKTREE="$BATS_TEST_TMPDIR/worktree"

    git init -q --bare -b main "$ORIGIN"
    git init -q -b main "$WORKTREE"
    cd "$WORKTREE"
    git config user.name 'Allowed paths test'
    git config user.email 'allowed-paths-test@example.invalid'
    git config commit.gpgsign false
    git config core.hooksPath /dev/null
    git remote add origin "$ORIGIN"
    mkdir -p registry/src
    printf 'seed\n' > README.md
    printf 'fn main() {}\n' > registry/src/main.rs
    printf 'seed\n' > Makefile
    git add .
    git commit -q -m seed
    git push -q origin main
    git checkout -q -b factory/job
}

commit_all() {
    git add -A
    git commit -q -m change
}

@test "a change inside the allowed paths is clean" {
    printf 'pub fn a() {}\n' > registry/src/lib.rs
    commit_all
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 1 ]
    [ -z "$output" ]
}

@test "every allowed path pattern is clean" {
    for path in app/lib/a.dart app/test/a.dart app/integration_test/a.dart app/README.md \
        family-app/src/a.rs family-app/tests/a.rs family-app/README.md family-app/migrations/a.sql \
        fly-mock/src/a.rs fly-mock/tests/a.rs fly-mock/README.md \
        registry/src/a.rs registry/tests/a.rs registry/migrations/a.sql \
        registry/schema.graphql registry/README.md README.md; do
        mkdir -p "$(dirname "$path")"
        printf 'x\n' >> "$path"
    done
    commit_all
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 1 ]
    [ -z "$output" ]
}

@test "a disallowed file is listed, and only that file" {
    printf 'pub fn a() {}\n' > registry/src/lib.rs
    mkdir -p .claude/agents
    printf 'x\n' > .claude/agents/new.md
    commit_all
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 0 ]
    [ "$output" = ".claude/agents/new.md" ]
}

@test "near misses of the allowed paths are listed" {
    mkdir -p app/lib
    printf 'x\n' > registry/src/notes.txt
    printf 'x\n' > app/lib/a.rs
    printf 'x\n' > registry/Cargo.toml
    commit_all
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 0 ]
    [ "${#lines[@]}" -eq 3 ]
}

@test "a symlink in an allowed path is listed" {
    ln -s main.rs registry/src/link.rs
    commit_all
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 0 ]
    [ "$output" = "registry/src/link.rs" ]
}

@test "a file replaced by a symlink in an allowed path is listed" {
    rm registry/src/main.rs
    ln -s ../../README.md registry/src/main.rs
    commit_all
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 0 ]
    [ "$output" = "registry/src/main.rs" ]
}

@test "a submodule in an allowed path is listed" {
    head="$(git rev-parse HEAD)"
    git update-index --add --cacheinfo "160000,$head,registry/src/sub.rs"
    git commit -q -m submodule
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 0 ]
    [ "$output" = "registry/src/sub.rs" ]
}

@test "deleting an allowed file is clean" {
    git rm -q README.md
    git commit -q -m delete
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 1 ]
    [ -z "$output" ]
}

@test "deleting a disallowed file is listed" {
    git rm -q Makefile
    git commit -q -m delete
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 0 ]
    [ "$output" = "Makefile" ]
}

@test "a job with no changes is clean" {
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 1 ]
    [ -z "$output" ]
}

@test "a non-ASCII filename in an allowed directory is listed" {
    # Set the other way here to show the script's own setting decides.
    git config core.quotepath false
    printf 'x\n' > "registry/src/caf$(printf '\303\251').rs"
    commit_all
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 0 ]
    [ "$output" = '"registry/src/caf\303\251.rs"' ]
}

@test "a filename with a newline in an allowed directory is listed" {
    printf 'x\n' > "registry/src/a"$'\n'"b.rs"
    commit_all
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 0 ]
    [ "$output" = '"registry/src/a\nb.rs"' ]
}

@test "a copy of the script inside the worktree fails the check" {
    mkdir -p .claude/skills/factory
    cp "$SCRIPT" .claude/skills/factory/
    run .claude/skills/factory/allowed-paths.sh "$WORKTREE"
    [ "$status" -eq 2 ]
    [[ "$output" == *"check failed"* ]]
}

@test "a copy inside the worktree fails when given a subdirectory of it" {
    cp "$SCRIPT" registry/
    run registry/allowed-paths.sh "$WORKTREE/registry/src"
    [ "$status" -eq 2 ]
    [[ "$output" == *"check failed"* ]]
}

@test "a copy inside the worktree fails when the argument is a symlink to it" {
    cp "$SCRIPT" registry/
    ln -s "$WORKTREE" "$BATS_TEST_TMPDIR/link"
    run registry/allowed-paths.sh "$BATS_TEST_TMPDIR/link"
    [ "$status" -eq 2 ]
    [[ "$output" == *"check failed"* ]]
}

@test "a symlink outside the worktree to a copy inside it fails the check" {
    cp "$SCRIPT" registry/
    ln -s "$WORKTREE/registry/allowed-paths.sh" "$BATS_TEST_TMPDIR/check.sh"
    run "$BATS_TEST_TMPDIR/check.sh" "$WORKTREE"
    [ "$status" -eq 2 ]
    [[ "$output" == *"check failed"* ]]
}

@test "a script in a directory that only shares the worktree's name prefix runs" {
    mkdir "${WORKTREE}c"
    cp "$SCRIPT" "${WORKTREE}c/"
    run "${WORKTREE}c/allowed-paths.sh" "$WORKTREE"
    [ "$status" -eq 1 ]
    [ -z "$output" ]
}

@test "a path that is not a repo fails the check" {
    mkdir "$BATS_TEST_TMPDIR/plain"
    run "$SCRIPT" "$BATS_TEST_TMPDIR/plain"
    [ "$status" -eq 2 ]
    [[ "$output" == *"check failed"* ]]
}

@test "a path that does not exist fails the check" {
    run "$SCRIPT" "$BATS_TEST_TMPDIR/missing"
    [ "$status" -eq 2 ]
}

@test "a repo with no origin fails the check" {
    git remote remove origin
    run "$SCRIPT" "$WORKTREE"
    [ "$status" -eq 2 ]
}

@test "a missing argument fails the check" {
    run "$SCRIPT"
    [ "$status" -eq 2 ]
}

@test "an extra argument fails the check" {
    run "$SCRIPT" "$WORKTREE" extra
    [ "$status" -eq 2 ]
}

@test "an empty argument fails the check rather than using the current directory" {
    run "$SCRIPT" ""
    [ "$status" -eq 2 ]
}

@test "the skill text holds no positional placeholders" {
    run grep -nE '\$([0-9]|ARGUMENTS)' "$REPO_ROOT/.claude/skills/factory/SKILL.md"
    [ "$status" -eq 1 ]
}
