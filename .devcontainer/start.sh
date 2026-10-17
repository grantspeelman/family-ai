#!/bin/bash
# Build, start and attach to the dev container from a plain terminal.
#   --rebuild  rebuild the image from scratch (also how Claude Code gets upgraded)
#   --claude   run Claude Code directly instead of a shell (tells herdr it's an agent)
#   --codex    run Codex directly instead of a shell (tells herdr it's an agent)
#   -- command [args...]  run the given command inside the container
set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

rebuild=false
stop=false
agent=""
command=(bash)
while [ "$#" -gt 0 ]; do
    arg="$1"
    case "$arg" in
        --rebuild) rebuild=true ;;
        --stop) stop=true ;;
        --docker)
            echo "--docker is no longer supported. Run Docker checks in CI; see .devcontainer/README.md (Docker checks)." >&2
            exit 64 ;;
        --claude|--codex)
            if [ -n "$agent" ] && [ "$agent" != "${arg#--}" ]; then
                echo "--claude and --codex are mutually exclusive" >&2; exit 64
            fi
            agent="${arg#--}" ;;
        --)
            shift
            if [ "$#" -eq 0 ] || [ -n "$agent" ]; then
                echo "Supply a command after --, without --claude or --codex." >&2
                exit 64
            fi
            command=("$@")
            break ;;
        *) echo "Usage: $(basename "$0") [--rebuild] [--claude|--codex | -- command [args...]]" >&2; exit 64 ;;
    esac
    shift
done

if [ -n "$agent" ]; then
    command=("$agent")
fi
case "${command[0]}" in
    claude|codex) agent="${command[0]}" ;;
esac

WORKSPACE_SOURCE="$(python3 -I "$SCRIPT_DIR/host-session.py" path)"
export WORKSPACE_SOURCE

# One container per checkout: key the compose project to the checkout path, so
# two checkouts never share (or recreate) each other's container. The name also
# differs from the one VS Code generates, so the two never disturb each other.
checkout_slug="$(printf '%s' "$(basename "$PROJECT_ROOT")" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9_-' '-')"
checkout_digest="$(printf '%s' "$PROJECT_ROOT" | cksum | cut -d ' ' -f 1)"
COMPOSE_PROJECT="family-ai-${checkout_slug}-${checkout_digest}"
SERVICE="app"
COMPOSE=(docker compose --env-file /dev/null -p "$COMPOSE_PROJECT"
    -f "$SCRIPT_DIR/docker-compose.yml")

USER_UID="$(id -u)"
USER_GID="$(id -g)"
if [ "$USER_UID" -eq 0 ] || [ "$USER_GID" -eq 0 ]; then
    echo -e "${YELLOW}Running as root: building the image for UID/GID 1000:1000 instead, so the image's root user is not remapped.${NC}"
    USER_UID=1000
    USER_GID=1000
fi
export USER_UID USER_GID

if [ "$stop" = true ]; then
    "${COMPOSE[@]}" down
    echo "Stopped. Disposable source and config volumes are preserved."
    exit 0
fi

# ── Build and start ──────────────────────────────────────────────────────────
if [ "$rebuild" = true ]; then
    echo -e "${GREEN}Rebuilding the image from scratch...${NC}"
    "${COMPOSE[@]}" build --pull --no-cache
    "${COMPOSE[@]}" up --detach --force-recreate
else
    # --build is a cache hit unless the Dockerfile changed; `up` recreates the
    # container only when its image or config changed.
    "${COMPOSE[@]}" up --detach --build
fi

"${COMPOSE[@]}" exec -T --user vscode "$SERVICE" /opt/family-ai-controls/postStartCommand.sh

# %q so the line stays copy-pasteable from a checkout path containing spaces.
printf -v stop_command '%q ' "$SCRIPT_DIR/start.sh" --stop
echo -e "${GREEN}The container outlives this shell: rerun this script to get back in.${NC}"
echo -e "${GREEN}Stop it with: ${stop_command}${NC}"

if [ -n "$agent" ]; then
    # herdr finds agents by the pane's host-side foreground process, which here is
    # docker, so HERDR_AGENT on this command (not inside the container) names it.
    HERDR_AGENT="$agent" exec "${COMPOSE[@]}" exec --user vscode "$SERVICE" "${command[@]}"
fi
exec "${COMPOSE[@]}" exec --user vscode "$SERVICE" "${command[@]}"
