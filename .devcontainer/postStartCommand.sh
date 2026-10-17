#!/usr/bin/env bash
# Runs as vscode on every container start, from both VS Code and start.sh.
# Everything here is idempotent: the Claude config dir is a volume that
# outlives containers, so this script meets its own earlier output.
set -euo pipefail

config_dir="${CLAUDE_CONFIG_DIR:?CLAUDE_CONFIG_DIR is set in the Dockerfile}"

# Git hooks: secret scan and changed-files checks on every commit (see /lefthook.yml).
if [ -d /workspace/.git ]; then
  lefthook install
fi

# Permission prompts off by default. This is the container's own Claude config,
# so the host's settings are untouched. Only the one key is set, leaving any
# other settings you have added in the volume alone.
settings="${config_dir}/settings.json"
[ -s "${settings}" ] || echo '{}' > "${settings}"
jq '.permissions.defaultMode = "bypassPermissions"' "${settings}" > "${settings}.tmp"
mv "${settings}.tmp" "${settings}"

# Rust dependencies, once there is a workspace to fetch for. `--locked` honours
# Cargo.lock and is a quick no-op when the registry cache already has everything.
if [ -f /workspace/Cargo.toml ]; then
  (cd /workspace && cargo fetch --locked)
fi

# Status, so a missing credential shows up now rather than mid-task.
if [ -z "${SSH_AUTH_SOCK:-}" ]; then
  echo "No SSH agent forwarded: git push over SSH will not work."
else
  agent_status=0
  ssh-add -l >/dev/null 2>&1 || agent_status=$?
  case "${agent_status}" in
    0) ;;
    1) echo "SSH agent reachable but holds no keys: run 'ssh-add' on the host." ;;
    *) echo "SSH agent socket is dead (host agent restarted?): run .devcontainer/start.sh --rebuild." ;;
  esac
fi

if [ -z "${GH_TOKEN:-}${GITHUB_TOKEN:-}" ]; then
  echo "Neither GH_TOKEN nor GITHUB_TOKEN is set: gh (PRs, issues) is unauthenticated."
fi

if [ ! -s "${config_dir}/.credentials.json" ]; then
  echo "Claude is not logged in yet: run 'claude' and log in once; the login persists across rebuilds."
fi
