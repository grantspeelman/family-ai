#!/bin/bash
# Build, start and attach to the dev container from a plain terminal.
# -h, --help prints the usage line and the flag lines below.
#   --rebuild  rebuild the image from scratch (also how Claude Code gets upgraded)
#   --docker   privileged mode with nested Docker; see compose.docker.yml first
#   --claude   run Claude Code directly instead of a shell (tells herdr it's an agent)
#   --codex    run Codex directly instead of a shell (tells herdr it's an agent)
#   -- command [args...]  run the given command inside the container
set -euo pipefail

# The usage line plus the flag lines of the header comment above: one source.
usage_line() { echo "Usage: $(basename "$0") [--rebuild] [--docker] [--claude|--codex | -- command [args...]]"; }
print_help() {
    usage_line
    sed -n '/^#   -/{s/^# //;p;}' "${BASH_SOURCE[0]}"
}

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

rebuild=false
docker_mode=false
agent=""
command=(bash)
while [ "$#" -gt 0 ]; do
    arg="$1"
    case "$arg" in
        -h|--help) print_help; exit 0 ;;
        --rebuild) rebuild=true ;;
        --docker) docker_mode=true ;;
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
        *) usage_line >&2; exit 64 ;;
    esac
    shift
done

if [ -n "$agent" ]; then
    command=("$agent")
fi
case "${command[0]}" in
    claude|codex) agent="${command[0]}" ;;
esac

# One container per checkout: key the compose project to the checkout path, so
# two checkouts never share (or recreate) each other's container. The name also
# differs from the one VS Code generates, so the two never disturb each other.
checkout_slug="$(printf '%s' "$(basename "$PROJECT_ROOT")" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9_-' '-')"
checkout_digest="$(printf '%s' "$PROJECT_ROOT" | cksum | cut -d ' ' -f 1)"
COMPOSE_PROJECT="family-ai-${checkout_slug}-${checkout_digest}"
SERVICE="app"
# --env-file /dev/null: compose reads no .env file. The token comes only from
# the host file below; ${HOME} in docker-compose.yml comes from the shell.
COMPOSE=(docker compose -p "$COMPOSE_PROJECT" --env-file /dev/null
    -f "$SCRIPT_DIR/docker-compose.yml"
    -f "$SCRIPT_DIR/compose.ssh.yml")
if [ "$docker_mode" = true ]; then
    COMPOSE+=(-f "$SCRIPT_DIR/compose.docker.yml")
fi

USER_UID="$(id -u)"
USER_GID="$(id -g)"
if [ "$USER_UID" -eq 0 ] || [ "$USER_GID" -eq 0 ]; then
    echo -e "${YELLOW}Running as root: building the image for UID/GID 1000:1000 instead, so the image's root user is not remapped.${NC}"
    USER_UID=1000
    USER_GID=1000
fi
export USER_UID USER_GID

# ── SSH: make sure a host agent is running and holds a key ───────────────────
# ssh-add -l exits 0 (has keys), 1 (no keys) or 2 (no agent reachable).
agent_status() { local s=0; ssh-add -l >/dev/null 2>&1 || s=$?; echo "$s"; }

if [ -z "${SSH_AUTH_SOCK:-}" ] || [ "$(agent_status)" -eq 2 ]; then
    # No session agent: use one at a fixed path, so later runs find it again
    # instead of starting an agent per run.
    SSH_AUTH_SOCK="${XDG_RUNTIME_DIR:-$HOME/.ssh}/family-ai-ssh-agent.sock"
    export SSH_AUTH_SOCK
    if [ "$(agent_status)" -eq 2 ]; then
        rm -f "$SSH_AUTH_SOCK"
        ssh-agent -a "$SSH_AUTH_SOCK" >/dev/null
        echo -e "${GREEN}Started ssh-agent on ${SSH_AUTH_SOCK}${NC}"
    fi
fi
if [ "$(agent_status)" -eq 1 ]; then
    echo -e "${GREEN}The ssh-agent holds no keys; loading your default key...${NC}"
    if ! ssh-add; then
        echo -e "${RED}ssh-add failed. Load a key yourself (ssh-add <path>) and rerun.${NC}" >&2
        exit 1
    fi
fi
export HOST_SSH_AUTH_SOCK="$SSH_AUTH_SOCK"

# ── GitHub token: docker-compose.yml reads it from a file outside the repo ───
# Only file modes are checked here; the file is never read.
# shellcheck source=token-file-check.sh
source "$SCRIPT_DIR/token-file-check.sh"
check_token_file "$HOME/.config/family-ai/gh.env" | while IFS= read -r line; do
    echo -e "${YELLOW}${line}${NC}"
done

if [ "$docker_mode" = true ]; then
    echo -e "${YELLOW}--docker: the container runs privileged. Use it only for sessions that need nested Docker.${NC}"
fi

# ── Build and start ──────────────────────────────────────────────────────────
if [ "$rebuild" = true ]; then
    echo -e "${GREEN}Rebuilding the image from scratch...${NC}"
    "${COMPOSE[@]}" build --pull --no-cache
    "${COMPOSE[@]}" up --detach --force-recreate
else
    # --build is a cache hit unless the Dockerfile changed; `up` recreates the
    # container only when its image or config changed (e.g. toggling --docker).
    "${COMPOSE[@]}" up --detach --build
fi

if [ "$docker_mode" = true ]; then
    echo -e "${GREEN}Waiting for dockerd inside the container...${NC}"
    timeout=90
    until "${COMPOSE[@]}" exec -T "$SERVICE" docker info &>/dev/null; do
        timeout=$((timeout - 1))
        if [ "$timeout" -le 0 ]; then
            echo -e "${RED}dockerd did not start within 90s. Recent container logs:${NC}" >&2
            "${COMPOSE[@]}" logs --tail 40 "$SERVICE" >&2 || true
            exit 1
        fi
        sleep 1
    done
fi

"${COMPOSE[@]}" exec -T --user vscode "$SERVICE" .devcontainer/postStartCommand.sh

# %q so the line stays copy-pasteable from a checkout path containing spaces.
printf -v stop_command '%q ' "${COMPOSE[@]}"
echo -e "${GREEN}The container outlives this shell: rerun this script to get back in.${NC}"
echo -e "${GREEN}Stop it with: ${stop_command}down${NC}"

if [ -n "$agent" ]; then
    # herdr finds agents by the pane's host-side foreground process, which here is
    # docker, so HERDR_AGENT on this command (not inside the container) names it.
    HERDR_AGENT="$agent" exec "${COMPOSE[@]}" exec --user vscode "$SERVICE" "${command[@]}"
fi
exec "${COMPOSE[@]}" exec --user vscode "$SERVICE" "${command[@]}"
