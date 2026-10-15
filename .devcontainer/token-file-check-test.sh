#!/bin/bash
# Tests check_token_file from token-file-check.sh against temp dirs with a dummy
# file. Needs no Docker. Prints no token (the file holds a dummy line).
set -euo pipefail
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
# shellcheck source=token-file-check.sh
source "$SCRIPT_DIR/token-file-check.sh"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok - $*"; }

mk() { # name dirmode filemode
    mkdir -p "$tmp/$1/real"; chmod "$2" "$tmp/$1/real"
    printf 'GH_TOKEN=dummy\n' > "$tmp/$1/real/gh.env"; chmod "$3" "$tmp/$1/real/gh.env"
}
expect() { # name want(count of output lines) file
    local out; out="$(check_token_file "$3")"
    local n; n="$(printf '%s' "$out" | grep -c . || true)"
    [ "$n" = "$2" ] || fail "$1: expected $2 warning line(s), got $n"
    pass "$1"
}
mk good 700 600;  expect "600 file in 700 dir is silent" 0 "$tmp/good/real/gh.env"
mk ro 700 400;    expect "400 file is silent" 0 "$tmp/ro/real/gh.env"
mk loose 755 644; expect "644 file in 755 dir warns twice" 2 "$tmp/loose/real/gh.env"
mk d 755 600;     expect "755 dir warns once" 1 "$tmp/d/real/gh.env"
mk f 700 640;     expect "640 file warns once" 1 "$tmp/f/real/gh.env"
mk sym 700 600; chmod 700 "$tmp/sym"
ln -s real/gh.env "$tmp/sym/link.env"
expect "symlinked file (600 target) is silent" 0 "$tmp/sym/link.env"
ln -s "$tmp/sym/real" "$tmp/symdir"
expect "symlinked dir (700 target) is silent" 0 "$tmp/symdir/gh.env"
mkdir -p "$tmp/none"
expect "missing file gives the setup pointer" 2 "$tmp/none/gh.env"
