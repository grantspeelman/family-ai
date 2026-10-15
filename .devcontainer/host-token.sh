#!/bin/bash
# Sourced by start.sh. Kept separate so host-token-test.sh can exercise it
# without Docker.
#
# load_gh_token: make sure GH_TOKEN is set for the container, taking it from the
# host's libsecret store when the environment has none.
#   0  a token is available (already in the environment, or loaded from the store)
#   1  no token (secret-tool missing, or the lookup was empty)
# An explicitly set GH_TOKEN or GITHUB_TOKEN always wins. The value is only
# captured and exported: never printed, never passed as an argument.
load_gh_token() {
    [ -z "${GH_TOKEN:-}${GITHUB_TOKEN:-}" ] || return 0
    command -v secret-tool >/dev/null 2>&1 || return 1
    local value
    value="$(secret-tool lookup service family-ai-gh 2>/dev/null)" || true
    [ -n "$value" ] || return 1
    export GH_TOKEN="$value"
}
