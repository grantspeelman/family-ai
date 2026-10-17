#!/bin/bash
set -euo pipefail

# Never bootstrap as root or repair existing volumes with privilege escalation.
[ "$(id -u)" -ne 0 ] || { echo 'Runtime must use the development user.' >&2; exit 1; }
for directory in /workspace "$CLAUDE_CONFIG_DIR" "$CODEX_HOME"; do
    [ -w "$directory" ] || {
        echo "Development directory is not writable: $directory. Recreate a disposable volume or have the trusted host repair ownership; see .devcontainer/README.md." >&2
        exit 1
    }
done

# Refuse a normal session when its external gateway is unavailable. The relay
# only adapts standard HTTP proxy clients to the Unix socket; it owns no policy.
python3 -I /usr/local/lib/egress-proxy.py check
python3 -I /usr/local/lib/egress-proxy.py relay &
relay_pid=$!
trap 'kill "$relay_pid" 2>/dev/null || true' EXIT
# Wait for the relay before setup or agent clients issue their first request.
python3 -I - <<'PYRELAY'
import socket
import time
for attempt in range(100):
    try:
        with socket.create_connection(('127.0.0.1', 3128), timeout=.1):
            break
    except OSError:
        time.sleep(.05)
else:
    raise SystemExit('Egress relay unavailable')
PYRELAY

# Docker-dependent checks run on the CI runner, outside agent sessions.
exec "$@"
