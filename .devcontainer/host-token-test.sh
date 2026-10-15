#!/bin/bash
# Tests load_gh_token from host-token.sh with a fake secret-tool on PATH. Needs
# no Docker and no real credential store. Never prints a token value: the fake
# token is a dummy, and failures report only which case failed.
set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "ok - $*"; }

# Each case runs in a fresh bash whose PATH is only $tmp/bin (the fake, when
# wanted), so a real secret-tool on the host is never reached. FAKE_MODE: value | empty | fail | none.
# Prints exit status, then the resulting GH_TOKEN state, never its value.
run_case() {
    local mode="$1"; shift
    mkdir -p "$tmp/bin"; rm -f "$tmp/bin/secret-tool"
    if [ "$mode" != none ]; then
        cat > "$tmp/bin/secret-tool" <<FAKE
#!/bin/bash
[ "\$*" = "lookup service family-ai-gh" ] || exit 2
case "$mode" in
    value) printf '%s' 'dummy-store-token' ;;
    empty) ;;
    fail) exit 1 ;;
esac
FAKE
        chmod +x "$tmp/bin/secret-tool"
    fi
    env -i PATH="$tmp/bin" "$@" "$BASH" -c '
        source "$1"
        rc=0; load_gh_token || rc=$?
        state=unset
        [ -z "${GH_TOKEN:-}" ] || state=set
        [ "${GH_TOKEN:-}" != dummy-store-token ] || state=store
        [ "${GH_TOKEN:-}" != dummy-env-token ] || state=env
        echo "$rc $state ${GITHUB_TOKEN:+github}"' _ "$SCRIPT_DIR/host-token.sh"
}

check() { [ "$2" = "$3" ] || fail "$1 (got '$2', want '$3')"; pass "$1"; }

check "store value is exported as GH_TOKEN" "$(run_case value)" "0 store "
check "pre-set GH_TOKEN wins over the store" "$(run_case value GH_TOKEN=dummy-env-token)" "0 env "
check "pre-set GITHUB_TOKEN wins; GH_TOKEN stays unset" "$(run_case value GITHUB_TOKEN=dummy-env-token)" "0 unset github"
check "missing secret-tool returns 1, no token" "$(run_case none)" "1 unset "
check "empty lookup returns 1, no token" "$(run_case empty)" "1 unset "
check "failing lookup returns 1, no token" "$(run_case fail)" "1 unset "

# Output hygiene: sourcing and calling the function prints nothing.
run_case value >/dev/null
out="$(env -i PATH="$tmp/bin" "$BASH" -c 'source "$1"; load_gh_token' _ "$SCRIPT_DIR/host-token.sh" 2>&1 || true)"
[ -z "$out" ] || fail "load_gh_token printed output"
pass "load_gh_token prints nothing"

echo "all host-token tests passed"
