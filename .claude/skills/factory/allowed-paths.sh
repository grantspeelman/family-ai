#!/usr/bin/env bash
# The factory's allowed-paths check. Usage: allowed-paths.sh <worktree>
#
# Prints every file the job's branch changes outside the allowed paths, and any
# symlink or submodule it adds.
#   exit 0: files listed (needs-human)
#   exit 1: clean
#   any other exit: the check failed
#
# This is a script because skill text must not hold literal positional shell
# variables: Claude Code substitutes them with the skill's arguments, which
# corrupts the command.
set -u

fail() {
    echo "check failed"
    exit 2
}

[ "$#" -eq 1 ] && [ -n "$1" ] || {
    echo "usage: allowed-paths.sh <worktree>" >&2
    fail
}
worktree=$1

f=$(git -C "$worktree" fetch -q origin main &&
    git -C "$worktree" diff --name-only --no-renames origin/main...HEAD) &&
    r=$(git -C "$worktree" diff --raw --no-renames origin/main...HEAD) || fail

# Raw lines look like ":<old mode> <new mode> <old id> <new id> <status>\t<path>".
# 120000 is a symlink and 160000 a submodule.
links=$(printf '%s\n' "$r" | awk -F'\t' '{split($1, m, " ")} m[2] ~ /^1[26]0000$/ {print $2}') || fail

outside=
if [ -n "$f" ]; then
    outside=$(printf '%s\n' "$f" | grep -vE '^(app/(lib|test|integration_test)/.+\.dart|app/README\.md|family-app/(src|tests)/.+\.rs|family-app/README\.md|family-app/migrations/.+\.sql|fly-mock/(src|tests)/.+\.rs|fly-mock/README\.md|registry/(src|tests)/.+\.rs|registry/migrations/.+\.sql|registry/schema\.graphql|registry/README\.md|README\.md)$')
    # grep exits 1 when every file is allowed; anything above that is an error.
    [ "$?" -le 1 ] || fail
fi

printf '%s\n%s\n' "$links" "$outside" | grep .
