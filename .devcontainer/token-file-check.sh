#!/bin/bash
# Sourced by start.sh. check_token_file TOKEN_FILE prints warnings and always
# returns 0. It checks modes only; the file is never read. A symlink counts as
# the file it points to (stat -L). An unknown mode (no usable stat) is skipped.

_mode_of() { stat -L -c %a "$1" 2>/dev/null || stat -L -f %Lp "$1" 2>/dev/null || true; }

check_token_file() {
    local file="$1" dir fmode dmode
    dir="$(dirname "$file")"
    if [ ! -f "$file" ]; then
        echo "No token file at ${file}: gh inside the container will be unauthenticated."
        echo "To set one up, see \"GitHub token\" in .devcontainer/README.md."
        return 0
    fi
    fmode="$(_mode_of "$file")"
    dmode="$(_mode_of "$dir")"
    # Stricter than 600 / 700 is fine: no group or other access (and no execute for the file).
    if [[ "$fmode" =~ ^[0-7]+$ ]] && (( (8#$fmode & 8#177) != 0 )); then
        echo "Token file mode is ${fmode} (want 600 or stricter): chmod 600 ${file}"
    fi
    if [[ "$dmode" =~ ^[0-7]+$ ]] && (( (8#$dmode & 8#77) != 0 )); then
        echo "Token directory mode is ${dmode} (want 700 or stricter): chmod 700 ${dir}"
    fi
    return 0
}
