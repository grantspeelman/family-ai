#!/usr/bin/env bash
# SessionStart hook for Claude Code on the web.
# Fetches crates and builds the workspace, tests included, so cargo test starts
# from a warm cache. Does nothing outside a cloud session, where the dev
# container covers setup. Safe to run more than once.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

cargo fetch --locked
cargo build --workspace --all-targets --locked
