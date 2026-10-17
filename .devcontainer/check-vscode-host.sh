#!/usr/bin/env bash
# initializeCommand executes on the host before the editor creates the container.
set -euo pipefail
refuse() {
    echo 'Use .devcontainer/start-vscode.sh from the trusted host; ordinary Reopen in Container is disabled.' >&2
    exit 1
}
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[ "${WORKSPACE_SOURCE:-}" = "$(python3 -I "$SCRIPT_DIR/host-session.py" path)" ] || refuse
[ -f "${HOME:-}/.family-ai-clean-editor" ] || refuse
[ "${GIT_CONFIG_GLOBAL:-}" = /dev/null ] || refuse
[ "${GIT_CONFIG_NOSYSTEM:-}" = 1 ] || refuse
[ -z "${GH_TOKEN:-}${GITHUB_TOKEN:-}${SSH_AUTH_SOCK:-}${HOST_SSH_AUTH_SOCK:-}${GIT_ASKPASS:-}" ] || refuse
for path in .gitconfig .ssh .config/gh .git-credentials; do
    [ ! -e "$HOME/$path" ] || refuse
done
