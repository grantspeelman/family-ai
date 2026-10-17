#!/usr/bin/env bats
# Tests entrypoint.sh with fake gosu, containerd, dockerd, docker and sleep
# first on PATH. Needs no Docker, no root and no network. The script writes
# /var/log/*.log with a fixed path, so the ENABLE_DOCKER=1 tests are skipped
# when that directory is writable (they would touch the real log files).

setup() {
    SCRIPT="$BATS_TEST_DIRNAME/entrypoint.sh"
    SHIMS="$BATS_TEST_TMPDIR/shims"
    CALLS="$BATS_TEST_TMPDIR/calls"
    mkdir -p "$SHIMS"
    : > "$CALLS"
    export CALLS
    # gosu records its argv, one argument per line, then does not exec anything.
    cat > "$SHIMS/gosu" <<'SH'
#!/bin/bash
{ printf 'gosu'; printf ' [%s]' "$@"; printf '\n'; } >> "$CALLS"
SH
    for c in containerd dockerd; do
        printf '#!/bin/bash\necho %s >> "$CALLS"\n' "$c" > "$SHIMS/$c"
    done
    # docker info succeeds on the Nth call (DOCKER_OK_AFTER, default 1).
    cat > "$SHIMS/docker" <<'SH'
#!/bin/bash
n=$(cat "$CALLS.docker" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$CALLS.docker"
echo "docker $*" >> "$CALLS"
[ "${DOCKER_FAIL:-0}" = 1 ] && exit 1
[ "$n" -ge "${DOCKER_OK_AFTER:-1}" ]
SH
    # sleep is a no-op that counts calls, so the 60s loop is instant.
    printf '#!/bin/bash\necho sleep >> "$CALLS"\n' > "$SHIMS/sleep"
    chmod +x "$SHIMS"/*
    export PATH="$SHIMS:$PATH"
    unset ENABLE_DOCKER DOCKER_FAIL DOCKER_OK_AFTER
}

skip_if_log_writable() {
    if [ -w /var/log ]; then
        skip "/var/log is writable here; the script would write real log files"
    fi
}

@test "ENABLE_DOCKER unset: drops to vscode with the given command" {
    run "$SCRIPT" echo "hello world" two
    [ "$status" -eq 0 ]
    [ "$(cat "$CALLS")" = "gosu [vscode] [echo] [hello world] [two]" ]
}

@test "ENABLE_DOCKER=0: no daemons, still drops to vscode" {
    ENABLE_DOCKER=0 run "$SCRIPT" true
    [ "$status" -eq 0 ]
    [ "$(cat "$CALLS")" = "gosu [vscode] [true]" ]
}

@test "ENABLE_DOCKER set to something other than 1 is treated as off" {
    ENABLE_DOCKER=yes run "$SCRIPT" true
    [ "$status" -eq 0 ]
    ! grep -q docker "$CALLS"
    ! grep -q containerd "$CALLS"
}

@test "exit status of the script is gosu's (exec), arguments pass through" {
    printf '#!/bin/bash\nexit 7\n' > "$SHIMS/gosu"
    run "$SCRIPT" true
    [ "$status" -eq 7 ]
}

@test "ENABLE_DOCKER=1: docker ready at once, no waiting, then gosu" {
    skip_if_log_writable
    ENABLE_DOCKER=1 run "$SCRIPT" bash -c 'echo hi'
    [ "$status" -eq 0 ]
    grep -qx 'docker info' "$CALLS"
    ! grep -qx sleep "$CALLS"
    [ "$(tail -n 1 "$CALLS")" = "gosu [vscode] [bash] [-c] [echo hi]" ]
}

@test "ENABLE_DOCKER=1: waits with sleep until docker info succeeds" {
    skip_if_log_writable
    ENABLE_DOCKER=1 DOCKER_OK_AFTER=4 run "$SCRIPT" true
    [ "$status" -eq 0 ]
    [ "$(grep -cx 'docker info' "$CALLS")" -eq 4 ]
    [ "$(grep -cx sleep "$CALLS")" -eq 3 ]
    [ "$(tail -n 1 "$CALLS")" = "gosu [vscode] [true]" ]
}

@test "ENABLE_DOCKER=1: dockerd never starts, errors out and never runs the command" {
    skip_if_log_writable
    ENABLE_DOCKER=1 DOCKER_FAIL=1 run "$SCRIPT" true
    [ "$status" -eq 1 ]
    [[ "$output" == *"ERROR: dockerd did not start within 60s"* ]]
    [[ "$output" == *"start.sh --docker"* ]]
    # 60 attempts: the 60th fails the countdown before its sleep.
    [ "$(grep -cx 'docker info' "$CALLS")" -eq 60 ]
    [ "$(grep -cx sleep "$CALLS")" -eq 59 ]
    ! grep -q '^gosu' "$CALLS"
}
