#!/usr/bin/env bash
# Runs as vscode on every container start, from both VS Code and start.sh.
# Setup is idempotent. Only explicitly selected provider state survives rebuilds.
set -euo pipefail

config_dir="${CLAUDE_CONFIG_DIR:?CLAUDE_CONFIG_DIR is set in the Dockerfile}"

# Git hooks: secret scan and changed-files checks on every commit (see /lefthook.yml).
if [ -d /workspace/.git ]; then
  lefthook install
fi

# Permission prompts off by default. This is the container's own Claude config,
# so the host's settings are untouched. Only selected Claude state is initialized.
if [ "${FAMILY_AI_AGENT:-none}" = claude ]; then
settings="${config_dir}/settings.json"
[ -s "${settings}" ] || echo '{}' > "${settings}"
jq '.permissions.defaultMode = "bypassPermissions"' "${settings}" > "${settings}.tmp"
mv "${settings}.tmp" "${settings}"
fi

# Rust dependencies, once there is a workspace to fetch for. `--locked` honours
# Cargo.lock and is a quick no-op when the registry cache already has everything.
if [ -f /workspace/Cargo.toml ]; then
  (cd /workspace && cargo fetch --locked)
fi

echo "Publishing stays on the host: edit, test and commit locally here; see .devcontainer/README.md."

if [ "${FAMILY_AI_AGENT:-none}" = claude ] && [ ! -s "${config_dir}/.credentials.json" ]; then
  echo "Claude is not logged in yet: run 'claude' and log in once; the login persists across rebuilds."
fi
