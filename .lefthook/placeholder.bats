#!/usr/bin/env bats
# Tests placeholder.sh: usage errors, the loud "did NOT run" line, the changed
# file count, and that the files it is given are never read or modified.
# Also checks that lefthook.yml still calls it with the expected arguments.
# Needs no Docker, no lefthook, no network.

bats_require_minimum_version 1.5.0

setup() {
    SCRIPT="$BATS_TEST_DIRNAME/placeholder.sh"
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"
}

@test "no arguments: exit 2 with usage on stderr and nothing on stdout" {
    run --separate-stderr "$SCRIPT"
    [ "$status" -eq 2 ]
    [ -z "$output" ]
    [[ "$stderr" == usage:* ]]
}

@test "one argument: exit 2 with usage on stderr" {
    run --separate-stderr "$SCRIPT" lint
    [ "$status" -eq 2 ]
    [ -z "$output" ]
    [[ "$stderr" == usage:* ]]
}

@test "name and owner: exit 0 and prints the loud PLACEHOLDER line with owner" {
    run --separate-stderr "$SCRIPT" lint "PR 7"
    [ "$status" -eq 0 ]
    [ -z "$stderr" ]
    [ "${lines[0]}" = "PLACEHOLDER: 'lint' did NOT run. No tool is installed yet (PR 7)." ]
}

@test "zero changed files reports 0" {
    run "$SCRIPT" lint "PR 7"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Changed files it would have received: 0" ]]
}

@test "one changed file reports 1" {
    run "$SCRIPT" lint "PR 7" a.txt
    [ "$status" -eq 0 ]
    [[ "$output" == *"Changed files it would have received: 1" ]]
}

@test "several changed files report the right count" {
    run "$SCRIPT" lint "PR 7" a.txt b.txt c.txt d.txt
    [ "$status" -eq 0 ]
    [[ "$output" == *"Changed files it would have received: 4" ]]
}

@test "file names with spaces each count once" {
    run "$SCRIPT" lint "PR 7" "my file.txt" "another  spaced name.md" plain.txt
    [ "$status" -eq 0 ]
    [[ "$output" == *"Changed files it would have received: 3" ]]
}

@test "files that do not exist are counted, not checked" {
    run "$SCRIPT" lint "PR 7" "$BATS_TEST_TMPDIR/missing-1" "$BATS_TEST_TMPDIR/missing-2"
    [ "$status" -eq 0 ]
    [[ "$output" == *"Changed files it would have received: 2" ]]
}

@test "does not read or modify the files it is given" {
    local f="$BATS_TEST_TMPDIR/some file.txt"
    printf 'SECRET-MARKER\n' > "$f"
    chmod 000 "$f"            # unreadable: a read would fail the script
    local before after
    before="$(stat -c '%Y %s %a' "$f")"
    run "$SCRIPT" lint "PR 7" "$f"
    after="$(stat -c '%Y %s %a' "$f")"
    chmod 600 "$f"
    [ "$status" -eq 0 ]
    [[ "$output" != *SECRET-MARKER* ]]
    [ "$before" = "$after" ]
    [ "$(cat "$f")" = "SECRET-MARKER" ]
}

@test "does not create files in the directory it runs from" {
    mkdir "$BATS_TEST_TMPDIR/work"
    cd "$BATS_TEST_TMPDIR/work"
    run "$SCRIPT" lint "PR 7" a b
    [ "$status" -eq 0 ]
    [ -z "$(ls -A "$BATS_TEST_TMPDIR/work")" ]
}

@test "lefthook.yml: placeholder steps call an existing executable placeholder.sh" {
    local yml="$REPO_ROOT/lefthook.yml"
    # Real step lines only: a run: key, not the explanatory comment.
    run grep -E '^[[:space:]]*run:.*\.lefthook/placeholder\.sh' "$yml"
    [ "$status" -eq 0 ]
    # Every .lefthook/placeholder.sh mention outside comments is a run: line.
    local mentions runs
    mentions="$(grep -v '^[[:space:]]*#' "$yml" | grep -c '\.lefthook/placeholder\.sh')"
    runs="$(grep -cE '^[[:space:]]*run:.*\.lefthook/placeholder\.sh' "$yml")"
    [ "$mentions" -eq "$runs" ]
    [ -x "$REPO_ROOT/.lefthook/placeholder.sh" ]
}

@test "lefthook.yml: every placeholder call is <check name> \"<owner>\" {staged_files}" {
    local yml="$REPO_ROOT/lefthook.yml"
    local total ok
    total="$(grep -cE '^[[:space:]]*run:.*\.lefthook/placeholder\.sh' "$yml")"
    ok="$(grep -cE '^[[:space:]]*run: \.lefthook/placeholder\.sh [A-Za-z0-9._-]+ "[^"]+" \{staged_files\}[[:space:]]*$' "$yml")"
    [ "$total" -gt 0 ]
    [ "$ok" -eq "$total" ]
}
