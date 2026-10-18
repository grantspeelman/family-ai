#!/usr/bin/env bats
# Tests check_token_file from token-file-check.sh against temp dirs holding a
# dummy file. Needs no Docker. Prints no token (the file holds a dummy line).

setup() {
    # shellcheck source=token-file-check.sh
    source "$BATS_TEST_DIRNAME/token-file-check.sh"
}

mk() { # name dirmode filemode
    mkdir -p "$BATS_TEST_TMPDIR/$1/real"
    chmod "$2" "$BATS_TEST_TMPDIR/$1/real"
    printf 'GH_TOKEN=dummy\n' > "$BATS_TEST_TMPDIR/$1/real/gh.env"
    chmod "$3" "$BATS_TEST_TMPDIR/$1/real/gh.env"
}

# warning_lines FILE: number of lines check_token_file prints for FILE.
warning_lines() {
    check_token_file "$1" | grep -c . || true
}

@test "600 file in 700 dir is silent" {
    mk good 700 600
    [ "$(warning_lines "$BATS_TEST_TMPDIR/good/real/gh.env")" = 0 ]
}

@test "400 file is silent" {
    mk ro 700 400
    [ "$(warning_lines "$BATS_TEST_TMPDIR/ro/real/gh.env")" = 0 ]
}

@test "644 file in 755 dir warns twice" {
    mk loose 755 644
    [ "$(warning_lines "$BATS_TEST_TMPDIR/loose/real/gh.env")" = 2 ]
}

@test "755 dir warns once" {
    mk d 755 600
    [ "$(warning_lines "$BATS_TEST_TMPDIR/d/real/gh.env")" = 1 ]
}

@test "640 file warns once" {
    mk f 700 640
    [ "$(warning_lines "$BATS_TEST_TMPDIR/f/real/gh.env")" = 1 ]
}

@test "symlinked file (600 target) is silent" {
    mk sym 700 600
    chmod 700 "$BATS_TEST_TMPDIR/sym"
    ln -s real/gh.env "$BATS_TEST_TMPDIR/sym/link.env"
    [ "$(warning_lines "$BATS_TEST_TMPDIR/sym/link.env")" = 0 ]
}

@test "symlinked dir (700 target) is silent" {
    mk sym 700 600
    ln -s "$BATS_TEST_TMPDIR/sym/real" "$BATS_TEST_TMPDIR/symdir"
    [ "$(warning_lines "$BATS_TEST_TMPDIR/symdir/gh.env")" = 0 ]
}

@test "missing file gives the setup pointer" {
    mkdir -p "$BATS_TEST_TMPDIR/none"
    [ "$(warning_lines "$BATS_TEST_TMPDIR/none/gh.env")" = 2 ]
}
