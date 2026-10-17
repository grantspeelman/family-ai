#!/usr/bin/env bash
# Run on the trusted Unix host. Each editor session gets a disposable identity.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
# Do not let editor flags redirect this session to an existing user profile.
for arg in "$@"; do
    case "$arg" in
        --goto|-g) ;;
        -*) echo "Only file paths and --goto are accepted." >&2; exit 64 ;;
    esac
done
editor="$(command -v code)"
profile_root="$(mktemp -d "${TMPDIR:-/tmp}/family-ai-vscode.XXXXXXXX")"
# --wait keeps the profile alive until the editor workspace has closed.
trap 'rm -rf "$profile_root"' EXIT
mkdir -p "$profile_root/home" "$profile_root/data/User" "$profile_root/extensions"
touch "$profile_root/home/.family-ai-clean-editor"
cat > "$profile_root/data/User/settings.json" <<'JSON'
{
  "dev.containers.copyGitConfig": false,
  "dev.containers.gitCredentialHelperConfigLocation": "none",
  "dev.containers.dockerCredentialHelper": false,
  "git.terminalAuthentication": false,
  "github.gitAuthentication": false,
  "settingsSync.enabled": false
}
JSON
# Start a separate process, with no inherited tokens, askpass helpers, SSH agent
# or provider keys. Preserve only the host variables needed to reach the GUI and
# Docker context. Never pass a host Docker API credential environment variable.
# Match bind-mount ownership at build time rather than mutating runtime users.
editor_uid="$(id -u)"
editor_gid="$(id -g)"
if [ "$editor_uid" -eq 0 ] || [ "$editor_gid" -eq 0 ]; then
    echo 'Start the editor as a non-root host user.' >&2
    exit 64
fi
clean_env=(env -i "USER_UID=$editor_uid" "USER_GID=$editor_gid" "PATH=$PATH" "HOME=$profile_root/home"
    "XDG_CONFIG_HOME=$profile_root/home/.config"
    "XDG_DATA_HOME=$profile_root/home/.local/share"
    "XDG_CACHE_HOME=$profile_root/home/.cache"
    "GIT_CONFIG_GLOBAL=/dev/null" "GIT_CONFIG_NOSYSTEM=1")
for name in USER LOGNAME DISPLAY WAYLAND_DISPLAY XDG_RUNTIME_DIR XAUTHORITY SYSTEMROOT; do
    if [ -n "${!name:-}" ]; then clean_env+=("$name=${!name}"); fi
done
editor_args=(--user-data-dir "$profile_root/data" --extensions-dir "$profile_root/extensions")
cd "$PROJECT_ROOT"
"${clean_env[@]}" "$editor" "${editor_args[@]}" --install-extension ms-vscode-remote.remote-containers
printf '%s\n' 'Use Dev Containers: Reopen in Container in this fresh window. Do not sign into GitHub or enable Settings Sync.'
"${clean_env[@]}" "$editor" "${editor_args[@]}" --new-window --wait "$PROJECT_ROOT" "$@"
