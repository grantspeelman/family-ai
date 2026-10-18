#!/usr/bin/env bats
# Tests start.sh without Docker, a network or the real HOME. Every external
# command it runs (docker, git, ssh-add, ssh-agent, id, sleep) is a shim in a temp
# dir that comes first on PATH; the shims log their calls to files there. HOME
# is a temp dir holding at most a dummy token file.

bats_require_minimum_version 1.5.0

setup() {
    SCRIPT="$BATS_TEST_DIRNAME/start.sh"
    SHIMS="$BATS_TEST_TMPDIR/shims"
    FAKE_HOME="$BATS_TEST_TMPDIR/home"
    mkdir -p "$SHIMS" "$FAKE_HOME"
    # Token file present with strict modes unless a test changes it.
    mkdir -p "$FAKE_HOME/.config/family-ai"
    chmod 700 "$FAKE_HOME/.config/family-ai"
    printf 'GH_TOKEN=dummy\n' > "$FAKE_HOME/.config/family-ai/gh.env"
    chmod 600 "$FAKE_HOME/.config/family-ai/gh.env"

    # docker: log argv (one line per call) and the env start.sh exports.
    cat > "$SHIMS/docker" <<'EOF'
#!/bin/bash
d="$(dirname "$0")"
echo "$*" >> "$d/docker.log"
printf '%s\0' "$@" > "$d/last_argv"
echo "HERDR_AGENT=${HERDR_AGENT-unset}" > "$d/last_agent"
echo "USER_UID=${USER_UID-unset} USER_GID=${USER_GID-unset} HOST_SSH_AUTH_SOCK=${HOST_SSH_AUTH_SOCK-unset}" >> "$d/docker.env.log"
if [ -e "$d/docker_info_fails" ] && [[ " $* " == *" docker info "* ]]; then
    echo "info" >> "$d/docker.info.log"
    exit 1
fi
if [[ " $* " == *" docker info "* ]]; then echo "info" >> "$d/docker.info.log"; fi
# Marker in the same output stream as start.sh's own messages, to check ordering.
if [[ " $* " == *" up "* ]]; then echo "SHIM-COMPOSE-UP"; fi
exit 0
EOF
    # ssh-add -l pops the next status from l_seq (the last one repeats).
    cat > "$SHIMS/ssh-add" <<'EOF'
#!/bin/bash
d="$(dirname "$0")"
if [ "${1:-}" = "-l" ]; then
    echo "-l" >> "$d/ssh-add.log"
    s="$(head -n1 "$d/l_seq")"
    if [ "$(wc -l < "$d/l_seq")" -gt 1 ]; then sed -i 1d "$d/l_seq"; fi
    exit "$s"
fi
echo "add $*" >> "$d/ssh-add.log"
exit "$(cat "$d/add_status")"
EOF
    cat > "$SHIMS/ssh-agent" <<'EOF'
#!/bin/bash
d="$(dirname "$0")"
echo "$*" >> "$d/ssh-agent.log"
exit "$(cat "$d/agent_status")"
EOF
    cat > "$SHIMS/id" <<'EOF'
#!/bin/bash
d="$(dirname "$0")"
case "${1:-}" in
    -u) cat "$d/fake_uid" ;;
    -g) cat "$d/fake_gid" ;;
    *) exit 1 ;;
esac
EOF
    # git: log argv, whether docker had already run, and the agent socket it saw;
    # exit with git_status, printing a git-style error when that is non-zero.
    cat > "$SHIMS/git" <<'EOF'
#!/bin/bash
d="$(dirname "$0")"
echo "$*" >> "$d/git.log"
if [ -e "$d/docker.log" ]; then echo yes; else echo no; fi > "$d/git_after_docker"
echo "SSH_AUTH_SOCK=${SSH_AUTH_SOCK-unset}" > "$d/git_env"
s="$(cat "$d/git_status")"
if [ "$s" -ne 0 ]; then echo "fatal: Not possible to fast-forward, aborting." >&2; fi
exit "$s"
EOF
    printf '#!/bin/bash\necho "$*" >> "$(dirname "$0")/sleep.log"\n' > "$SHIMS/sleep"
    chmod +x "$SHIMS"/*
    echo 0 > "$SHIMS/l_seq"
    echo 0 > "$SHIMS/add_status"
    echo 0 > "$SHIMS/agent_status"
    echo 0 > "$SHIMS/git_status"
    echo 1234 > "$SHIMS/fake_uid"
    echo 5678 > "$SHIMS/fake_gid"

    # A live session agent that holds keys, unless a test changes it.
    SOCK="$BATS_TEST_TMPDIR/session.sock"
    ENVV=(HOME="$FAKE_HOME" PATH="$SHIMS:/usr/bin:/bin" SSH_AUTH_SOCK="$SOCK"
          XDG_RUNTIME_DIR="$BATS_TEST_TMPDIR/run")
    mkdir -p "$BATS_TEST_TMPDIR/run"
}

start() { # args for start.sh
    run env -i "${ENVV[@]}" "$BASH" "$SCRIPT" "$@"
}

# ssh_seq STATUS...: successive exit codes of `ssh-add -l`.
ssh_seq() { printf '%s\n' "$@" > "$SHIMS/l_seq"; }

# no_session_agent: run without SSH_AUTH_SOCK (agent socket falls back to a fixed path).
no_session_agent() {
    ENVV=(HOME="$FAKE_HOME" PATH="$SHIMS:/usr/bin:/bin" XDG_RUNTIME_DIR="$BATS_TEST_TMPDIR/run")
}

# last_argv: fills the array LAST with the last docker call's argv, one entry
# per argument (empty ones and spaces kept). last_agent: HERDR_AGENT it saw.
last_argv() { mapfile -d '' LAST < "$SHIMS/last_argv"; }
last_agent() { cat "$SHIMS/last_agent"; }

calls() { cat "$SHIMS/docker.log" 2>/dev/null || true; }
call_count() { calls | grep -c . || true; }
# nth_call N: the Nth recorded docker call line.
nth_call() { calls | sed -n "${1}p"; }
no_docker_calls() { [ ! -e "$SHIMS/docker.log" ]; }

# copy_checkout NAME: copies the scripts under test into a temp checkout
# (the originals are not touched) and sets SCRIPT to the copy.
copy_checkout() {
    mkdir -p "$BATS_TEST_TMPDIR/$1/.devcontainer"
    cp "$BATS_TEST_DIRNAME/start.sh" "$BATS_TEST_DIRNAME/token-file-check.sh" \
        "$BATS_TEST_TMPDIR/$1/.devcontainer/"
    SCRIPT="$BATS_TEST_TMPDIR/$1/.devcontainer/start.sh"
}

# ── arguments ────────────────────────────────────────────────────────────────

@test "unknown flag prints usage and exits 64 before doing anything" {
    start --bogus
    [ "$status" -eq 64 ]
    [[ "$output" == *"Usage: start.sh [--rebuild] [--docker]"* ]]
    no_docker_calls
    [ ! -e "$SHIMS/ssh-add.log" ]
}

HELP_TEXT='Usage: start.sh [--rebuild] [--docker] [--claude|--codex | -- command [args...]]
  --rebuild  git pull --ff-only, then rebuild the image from scratch (also how Claude Code gets upgraded)
  --docker   privileged mode with nested Docker; see compose.docker.yml first
  --claude   run Claude Code directly instead of a shell (tells herdr it'"'"'s an agent)
  --codex    run Codex directly instead of a shell (tells herdr it'"'"'s an agent)
  -- command [args...]  run the given command inside the container'

@test "--help prints the usage and flag list on stdout, exits 0, touches nothing" {
    run --separate-stderr env -i "${ENVV[@]}" "$BASH" "$SCRIPT" --help
    [ "$status" -eq 0 ]
    [ "$output" = "$HELP_TEXT" ]
    [ -z "$stderr" ]
    no_docker_calls
    [ ! -e "$SHIMS/ssh-add.log" ]
    [ ! -e "$SHIMS/ssh-agent.log" ]
}

@test "-h is the same as --help" {
    run --separate-stderr env -i "${ENVV[@]}" "$BASH" "$SCRIPT" -h
    [ "$status" -eq 0 ]
    [ "$output" = "$HELP_TEXT" ]
    no_docker_calls
}

@test "--help does no token-file check, even with a loose or missing token file" {
    chmod 644 "$FAKE_HOME/.config/family-ai/gh.env"
    start --help
    [ "$status" -eq 0 ]
    [[ "$output" != *"Token file mode"* ]]
    rm "$FAKE_HOME/.config/family-ai/gh.env"
    start --help
    [ "$status" -eq 0 ]
    [[ "$output" != *"unauthenticated"* ]]
}

@test "--help combined with other flags still just prints help" {
    start --rebuild --docker --help
    [ "$status" -eq 0 ]
    [ "$output" = "$HELP_TEXT" ]
    no_docker_calls
    [ ! -e "$SHIMS/ssh-add.log" ]
    start --help --rebuild
    [ "$status" -eq 0 ]
    [ "$output" = "$HELP_TEXT" ]
    no_docker_calls
}

@test "after --, -h and --help are ordinary command arguments" {
    start -- claude --help
    [ "$status" -eq 0 ]
    last_argv
    [ "${LAST[-2]}" = claude ]
    [ "${LAST[-1]}" = --help ]
    [[ "$output" != *"Usage:"* ]]
    start -- ls -h
    [ "$status" -eq 0 ]
    last_argv
    [ "${LAST[*]: -3}" = "app ls -h" ]
}

@test "an unknown flag after a valid one is still rejected" {
    start --rebuild --nope
    [ "$status" -eq 64 ]
    no_docker_calls
}

@test "a positional argument is rejected" {
    start extra
    [ "$status" -eq 64 ]
    no_docker_calls
}

# ── command launch (-- command, --claude, --codex) ───────────────────────────
# The default shell (no arguments) ends in "exec --user vscode app bash" and is
# covered under "compose command line" below.

@test "--rebuild -- claude --resume: builds with --pull --no-cache, execs the command, names the agent" {
    start --rebuild -- claude --resume
    [ "$status" -eq 0 ]
    [[ "$(nth_call 1)" == *"build --pull --no-cache" ]]
    last_argv
    [ "${LAST[*]: -3}" = "app claude --resume" ]
    [ "$(last_agent)" = "HERDR_AGENT=claude" ]
}

@test "arguments after -- arrive unchanged: spaces, empty, shell text and --docker" {
    args=(printf '%s\n' 'two words' '' '$(touch nope)' --docker)
    cd "$BATS_TEST_TMPDIR"
    start -- "${args[@]}"
    [ "$status" -eq 0 ]
    last_argv
    n=${#args[@]}
    [ "${#LAST[@]}" -ge "$n" ]
    for i in "${!args[@]}"; do
        [ "${LAST[$(( ${#LAST[@]} - n + i ))]}" = "${args[$i]}" ]
    done
    [ "${LAST[$(( ${#LAST[@]} - n - 1 ))]}" = app ]
    # Not run, and --docker after -- is not the flag.
    [ ! -e "$BATS_TEST_TMPDIR/nope" ]
    if calls | grep -q 'compose.docker.yml'; then false; fi
    # A plain command does not set HERDR_AGENT.
    [ "$(last_agent)" = "HERDR_AGENT=unset" ]
}

@test "-- codex resume sets HERDR_AGENT=codex" {
    start -- codex resume
    [ "$status" -eq 0 ]
    last_argv
    [ "${LAST[*]: -3}" = "app codex resume" ]
    [ "$(last_agent)" = "HERDR_AGENT=codex" ]
}

@test "--codex runs codex and sets HERDR_AGENT=codex" {
    start --codex
    [ "$status" -eq 0 ]
    last_argv
    [ "${LAST[-2]}" = app ]
    [ "${LAST[-1]}" = codex ]
    [ "$(last_agent)" = "HERDR_AGENT=codex" ]
}

@test "--claude runs claude and sets HERDR_AGENT=claude" {
    start --claude
    [ "$status" -eq 0 ]
    last_argv
    [ "${LAST[-2]}" = app ]
    [ "${LAST[-1]}" = claude ]
    [ "$(last_agent)" = "HERDR_AGENT=claude" ]
}

@test "--claude --codex fails with exit 64 before docker" {
    start --claude --codex
    [ "$status" -eq 64 ]
    [[ "$output" == *"mutually exclusive"* ]]
    no_docker_calls
}

@test "a bare -- with no command fails with exit 64 before docker" {
    start --
    [ "$status" -eq 64 ]
    [[ "$output" == *"Supply a command after --"* ]]
    no_docker_calls
}

@test "--claude -- echo hi fails with exit 64 before docker" {
    start --claude -- echo hi
    [ "$status" -eq 64 ]
    [[ "$output" == *"Supply a command after --"* ]]
    no_docker_calls
}

# ── compose command line ─────────────────────────────────────────────────────

@test "default run builds the compose line and runs up --detach --build" {
    start
    [ "$status" -eq 0 ]
    d="$BATS_TEST_DIRNAME"
    [[ "$(nth_call 1)" =~ ^"compose -p family-ai-"[a-z0-9_-]+-[0-9]+" --env-file /dev/null -f $d/docker-compose.yml -f $d/compose.ssh.yml up --detach --build"$ ]]
}

@test "default run does not use compose.docker.yml, --no-cache or --force-recreate" {
    start
    [ "$status" -eq 0 ]
    if calls | grep -q 'compose.docker.yml'; then false; fi
    if calls | grep -q -e '--no-cache' -e '--force-recreate' -e ' build '; then false; fi
}

@test "the project name is family-ai-<slug>-<cksum of the checkout path>" {
    start
    root="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"
    digest="$(printf '%s' "$root" | cksum | cut -d ' ' -f 1)"
    [[ "$(nth_call 1)" == "compose -p family-ai-"*"-${digest} --env-file"* ]]
}

# A literal digest, so changing the algorithm (cksum) or its input is caught.
# The checkout has to sit at a fixed path for this; it is removed afterwards.
teardown() { rm -rf /tmp/family-ai-bats-fixed; }

@test "the project name for a fixed checkout path has a known digest" {
    rm -rf /tmp/family-ai-bats-fixed
    mkdir -p /tmp/family-ai-bats-fixed/.devcontainer
    cp "$BATS_TEST_DIRNAME/start.sh" "$BATS_TEST_DIRNAME/token-file-check.sh" \
        /tmp/family-ai-bats-fixed/.devcontainer/
    SCRIPT=/tmp/family-ai-bats-fixed/.devcontainer/start.sh
    start
    [ "$status" -eq 0 ]
    # printf '%s' /tmp/family-ai-bats-fixed | cksum  ->  3421399678
    [[ "$(nth_call 1)" == "compose -p family-ai-family-ai-bats-fixed-3421399678 --env-file"* ]]
}

@test "different checkout paths get different project names; the same path is stable" {
    copy_checkout "one/a"; first="$SCRIPT"
    copy_checkout "two/a"; second="$SCRIPT"
    # Each run makes three docker calls; the first of each run carries the name.
    SCRIPT="$first"; start
    name1="$(nth_call 1 | cut -d ' ' -f 3)"
    SCRIPT="$first"; start
    name1b="$(nth_call 4 | cut -d ' ' -f 3)"
    SCRIPT="$second"; start
    name2="$(nth_call 7 | cut -d ' ' -f 3)"
    [[ "$name1" == family-ai-a-* ]]
    [ "$name1" = "$name1b" ]
    [ "$name1" != "$name2" ]
}

@test "the project name slug is lowercased with unsafe characters squeezed to -" {
    copy_checkout "My Repo.X"
    start
    [ "$status" -eq 0 ]
    [[ "$(nth_call 1)" =~ ^"compose -p family-ai-my-repo-x-"[0-9]+" --env-file" ]]
    [[ "$(nth_call 1)" == *"-f $BATS_TEST_TMPDIR/My Repo.X/.devcontainer/docker-compose.yml "* ]]
}

@test "--docker adds compose.docker.yml after the other -f files and warns" {
    start --docker
    [ "$status" -eq 0 ]
    d="$BATS_TEST_DIRNAME"
    [[ "$(nth_call 1)" == *"-f $d/docker-compose.yml -f $d/compose.ssh.yml -f $d/compose.docker.yml up --detach --build" ]]
    [[ "$output" == *"--docker: the container runs privileged."* ]]
}

@test "--docker waits for dockerd with compose exec -T app docker info" {
    start --docker
    [ "$status" -eq 0 ]
    [[ "$output" == *"Waiting for dockerd inside the container..."* ]]
    [[ "$(nth_call 2)" == *"compose.docker.yml exec -T app docker info" ]]
    [ "$(wc -l < "$SHIMS/docker.info.log")" -eq 1 ]
}

@test "without --docker the dockerd wait and warning are skipped" {
    start
    [ "$status" -eq 0 ]
    [[ "$output" != *"dockerd"* ]]
    [[ "$output" != *"runs privileged"* ]]
    [ ! -e "$SHIMS/docker.info.log" ]
}

@test "dockerd that never comes up: 90 tries, logs shown, exit 1, no shell" {
    touch "$SHIMS/docker_info_fails"
    start --docker
    [ "$status" -eq 1 ]
    [[ "$output" == *"dockerd did not start within 90s."* ]]
    [ "$(wc -l < "$SHIMS/docker.info.log")" -eq 90 ]
    [ "$(wc -l < "$SHIMS/sleep.log")" -eq 89 ]
    calls | grep -q ' logs --tail 40 app$'
    if calls | grep -q 'postStartCommand'; then false; fi
    if calls | grep -q ' bash$'; then false; fi
}

@test "--rebuild builds with --pull --no-cache, then up --detach --force-recreate" {
    start --rebuild
    [ "$status" -eq 0 ]
    [[ "$output" == *"Rebuilding the image from scratch..."* ]]
    [[ "$(nth_call 1)" == *"compose.ssh.yml build --pull --no-cache" ]]
    [[ "$(nth_call 2)" == *"compose.ssh.yml up --detach --force-recreate" ]]
    if calls | grep -q -e '--build'; then false; fi
}

@test "--rebuild runs git pull --ff-only in the checkout before building" {
    start --rebuild
    [ "$status" -eq 0 ]
    [ "$(cat "$SHIMS/git.log")" = "-C $(cd "$BATS_TEST_DIRNAME/.." && pwd) pull --ff-only" ]
    [ "$(cat "$SHIMS/git_after_docker")" = no ]
    [[ "$(nth_call 1)" == *" build --pull --no-cache" ]]
}

@test "--rebuild pulls after the ssh-agent setup, with the socket start.sh chose" {
    # No session agent: start.sh starts one at the fixed path and points
    # SSH_AUTH_SOCK at it, so git only sees that path if it runs afterwards.
    no_session_agent
    ssh_seq 2 0
    start --rebuild
    [ "$status" -eq 0 ]
    [ "$(cat "$SHIMS/git_env")" = "SSH_AUTH_SOCK=$BATS_TEST_TMPDIR/run/family-ai-ssh-agent.sock" ]
}

@test "a failing pull stops --rebuild before docker: git's error, a hint, exit 1" {
    echo 1 > "$SHIMS/git_status"
    start --rebuild
    [ "$status" -eq 1 ]
    [[ "$output" == *"fatal: Not possible to fast-forward, aborting."* ]]
    [[ "$output" == *"git pull --ff-only failed, so nothing was rebuilt"* ]]
    no_docker_calls
}

@test "without --rebuild, and for --rebuild --help, there is no git pull" {
    start
    [ "$status" -eq 0 ]
    [ ! -e "$SHIMS/git.log" ]
    start --rebuild --help
    [ "$status" -eq 0 ]
    [ ! -e "$SHIMS/git.log" ]
}

@test "--rebuild --docker uses the docker compose file for build and up" {
    start --rebuild --docker
    [ "$status" -eq 0 ]
    [[ "$(nth_call 1)" == *"compose.docker.yml build --pull --no-cache" ]]
    [[ "$(nth_call 2)" == *"compose.docker.yml up --detach --force-recreate" ]]
}

@test "flag order does not matter" {
    start --docker --rebuild
    [ "$status" -eq 0 ]
    [[ "$(nth_call 1)" == *"compose.docker.yml build --pull --no-cache" ]]
}

@test "after up it runs postStartCommand.sh as vscode, then execs a shell as vscode" {
    start
    [ "$status" -eq 0 ]
    [ "$(call_count)" -eq 3 ]
    [[ "$(nth_call 2)" == *"compose.ssh.yml exec -T --user vscode app .devcontainer/postStartCommand.sh" ]]
    [[ "$(nth_call 3)" == *"compose.ssh.yml exec --user vscode app bash" ]]
}

@test "the final exec's exit status is the script's exit status" {
    # Replace the shim with one that fails only on the final shell exec.
    cat > "$SHIMS/docker" <<'EOF'
#!/bin/bash
[[ " $* " == *" exec --user vscode app bash "* ]] && exit 7
exit 0
EOF
    start
    [ "$status" -eq 7 ]
}

@test "a failing compose up stops the script (set -e): no postStart, no shell" {
    cat > "$SHIMS/docker" <<'EOF'
#!/bin/bash
echo "$*" >> "$(dirname "$0")/docker.log"
[[ " $* " == *" up "* ]] && exit 3
exit 0
EOF
    start
    [ "$status" -eq 3 ]
    [ "$(call_count)" -eq 1 ]
}

@test "prints a copy-pasteable stop command ending in down" {
    start
    [ "$status" -eq 0 ]
    [[ "$output" == *"The container outlives this shell: rerun this script to get back in."* ]]
    [[ "$output" == *"Stop it with: docker compose -p family-ai-"*" --env-file /dev/null -f $BATS_TEST_DIRNAME/docker-compose.yml -f $BATS_TEST_DIRNAME/compose.ssh.yml down"* ]]
}

@test "the stop command escapes a checkout path containing a space" {
    copy_checkout "with space"
    start
    [ "$status" -eq 0 ]
    [[ "$output" == *"-f $BATS_TEST_TMPDIR/with\\ space/.devcontainer/docker-compose.yml "* ]]
}

@test "the result does not depend on the current directory" {
    cd /
    start
    [ "$status" -eq 0 ]
    [[ "$(nth_call 1)" == *"-f $BATS_TEST_DIRNAME/docker-compose.yml "* ]]
}

# ── UID / GID ────────────────────────────────────────────────────────────────

@test "a non-root user passes its own UID and GID to compose" {
    start
    [ "$status" -eq 0 ]
    [[ "$output" != *"Running as root"* ]]
    head -n1 "$SHIMS/docker.env.log" | grep -q '^USER_UID=1234 USER_GID=5678 '
}

@test "UID 0 warns and falls back to 1000:1000" {
    echo 0 > "$SHIMS/fake_uid"
    start
    [ "$status" -eq 0 ]
    [[ "$output" == *"Running as root: building the image for UID/GID 1000:1000"* ]]
    head -n1 "$SHIMS/docker.env.log" | grep -q '^USER_UID=1000 USER_GID=1000 '
}

@test "GID 0 alone also replaces both with 1000" {
    echo 0 > "$SHIMS/fake_gid"
    start
    [ "$status" -eq 0 ]
    [[ "$output" == *"Running as root"* ]]
    head -n1 "$SHIMS/docker.env.log" | grep -q '^USER_UID=1000 USER_GID=1000 '
}

# ── ssh-agent ────────────────────────────────────────────────────────────────

@test "live agent with keys: nothing started or added, socket handed to compose" {
    ssh_seq 0
    start
    [ "$status" -eq 0 ]
    [ ! -e "$SHIMS/ssh-agent.log" ]
    if grep -q '^add' "$SHIMS/ssh-add.log"; then false; fi
    [[ "$output" != *"holds no keys"* ]]
    head -n1 "$SHIMS/docker.env.log" | grep -q "HOST_SSH_AUTH_SOCK=$SOCK\$"
}

@test "live agent without keys: runs ssh-add and continues when it succeeds" {
    ssh_seq 1 1 0
    start
    [ "$status" -eq 0 ]
    [[ "$output" == *"The ssh-agent holds no keys; loading your default key..."* ]]
    grep -qx 'add ' "$SHIMS/ssh-add.log"
    [ ! -e "$SHIMS/ssh-agent.log" ]
    head -n1 "$SHIMS/docker.env.log" | grep -q "HOST_SSH_AUTH_SOCK=$SOCK\$"
}

@test "failed ssh-add: message on stderr, exit 1, compose never runs" {
    ssh_seq 1
    echo 1 > "$SHIMS/add_status"
    start
    [ "$status" -eq 1 ]
    [[ "$output" == *"ssh-add failed. Load a key yourself (ssh-add <path>) and rerun."* ]]
    no_docker_calls
}

@test "dead session socket: switches to the fixed-path agent and starts one" {
    ssh_seq 2 2 0
    stale="$BATS_TEST_TMPDIR/run/family-ai-ssh-agent.sock"
    touch "$stale"
    start
    [ "$status" -eq 0 ]
    [ "$(cat "$SHIMS/ssh-agent.log")" = "-a $stale" ]
    [ ! -e "$stale" ]
    [[ "$output" == *"Started ssh-agent on $stale"* ]]
    head -n1 "$SHIMS/docker.env.log" | grep -q "HOST_SSH_AUTH_SOCK=$stale\$"
}

@test "dead session socket but a live agent at the fixed path: reuses it" {
    ssh_seq 2 0
    start
    [ "$status" -eq 0 ]
    [ ! -e "$SHIMS/ssh-agent.log" ]
    head -n1 "$SHIMS/docker.env.log" | grep -q "HOST_SSH_AUTH_SOCK=$BATS_TEST_TMPDIR/run/family-ai-ssh-agent.sock\$"
}

@test "no SSH_AUTH_SOCK: starts an agent at the fixed path under XDG_RUNTIME_DIR" {
    no_session_agent
    ssh_seq 2 0
    start
    [ "$status" -eq 0 ]
    [ "$(cat "$SHIMS/ssh-agent.log")" = "-a $BATS_TEST_TMPDIR/run/family-ai-ssh-agent.sock" ]
    # The first -l is skipped when the variable is unset: one probe, then one check.
    [ "$(wc -l < "$SHIMS/ssh-add.log")" -eq 2 ]
}

@test "no SSH_AUTH_SOCK and no XDG_RUNTIME_DIR: fixed path is under HOME/.ssh" {
    ENVV=(HOME="$FAKE_HOME" PATH="$SHIMS:/usr/bin:/bin")
    ssh_seq 2 0
    start
    [ "$status" -eq 0 ]
    [ "$(cat "$SHIMS/ssh-agent.log")" = "-a $FAKE_HOME/.ssh/family-ai-ssh-agent.sock" ]
}

@test "no SSH_AUTH_SOCK, fixed-path agent already running: none started" {
    no_session_agent
    ssh_seq 0
    start
    [ "$status" -eq 0 ]
    [ ! -e "$SHIMS/ssh-agent.log" ]
    head -n1 "$SHIMS/docker.env.log" | grep -q "HOST_SSH_AUTH_SOCK=$BATS_TEST_TMPDIR/run/family-ai-ssh-agent.sock\$"
}

@test "freshly started agent holds no keys: ssh-add runs after it" {
    no_session_agent
    ssh_seq 2 1
    start
    [ "$status" -eq 0 ]
    [ -e "$SHIMS/ssh-agent.log" ]
    [[ "$output" == *"Started ssh-agent on"* ]]
    [[ "$output" == *"holds no keys; loading your default key..."* ]]
    grep -qx 'add ' "$SHIMS/ssh-add.log"
}

@test "ssh-agent that fails to start aborts before compose" {
    no_session_agent
    ssh_seq 2
    echo 1 > "$SHIMS/agent_status"
    start
    [ "$status" -eq 1 ]
    [[ "$output" != *"Started ssh-agent"* ]]
    no_docker_calls
}

# ── GitHub token file ────────────────────────────────────────────────────────

@test "strict token file and directory: no token warnings" {
    start
    [ "$status" -eq 0 ]
    [[ "$output" != *"unauthenticated"* ]]
    [[ "$output" != *"Token file mode"* ]]
    [[ "$output" != *"Token directory mode"* ]]
}

@test "missing token file: warns that gh will be unauthenticated and still starts" {
    rm "$FAKE_HOME/.config/family-ai/gh.env"
    start
    [ "$status" -eq 0 ]
    [[ "$output" == *"No token file at $FAKE_HOME/.config/family-ai/gh.env: gh inside the container will be unauthenticated."* ]]
    [[ "$output" == *"\"GitHub token\" in .devcontainer/README.md"* ]]
    [ "$(call_count)" -eq 3 ]
}

@test "missing token directory gets the same unauthenticated message" {
    rm -r "$FAKE_HOME/.config"
    start
    [ "$status" -eq 0 ]
    [[ "$output" == *"will be unauthenticated"* ]]
}

@test "loose file and directory modes warn with the chmod fix, and still start" {
    chmod 644 "$FAKE_HOME/.config/family-ai/gh.env"
    chmod 755 "$FAKE_HOME/.config/family-ai"
    start
    [ "$status" -eq 0 ]
    [[ "$output" == *"Token file mode is 644 (want 600 or stricter): chmod 600 $FAKE_HOME/.config/family-ai/gh.env"* ]]
    [[ "$output" == *"Token directory mode is 755 (want 700 or stricter): chmod 700 $FAKE_HOME/.config/family-ai"* ]]
    [ "$(call_count)" -eq 3 ]
}

@test "the token file is looked up under HOME, not the real home" {
    rm "$FAKE_HOME/.config/family-ai/gh.env"
    other="$BATS_TEST_TMPDIR/other"
    mkdir -p "$other"
    ENVV=(HOME="$other" PATH="$SHIMS:/usr/bin:/bin" SSH_AUTH_SOCK="$SOCK")
    start
    [[ "$output" == *"No token file at $other/.config/family-ai/gh.env"* ]]
}

@test "the token warnings are printed before compose runs" {
    rm "$FAKE_HOME/.config/family-ai/gh.env"
    start
    [ "$status" -eq 0 ]
    # The shim prints SHIM-COMPOSE-UP when compose up runs; the warning must come first.
    before="${output%%SHIM-COMPOSE-UP*}"
    [[ "$output" == *SHIM-COMPOSE-UP* ]]
    [[ "$before" == *unauthenticated* ]]
}
