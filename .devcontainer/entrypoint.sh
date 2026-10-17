#!/bin/bash
set -euo pipefail

# Docker-dependent checks run on the CI runner, outside agent sessions.
exec gosu vscode "$@"
