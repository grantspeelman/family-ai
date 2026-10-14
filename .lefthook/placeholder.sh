#!/usr/bin/env bash
# Stand-in for a check whose tool is not installed yet.
# Usage: placeholder.sh <check name> <owner> [changed files...]
# Prints loudly that nothing was checked, then exits zero so the commit is not
# blocked by a tool that does not exist. Replace the step in lefthook.yml with
# the real command when the tool lands.
set -euo pipefail
if [ "$#" -lt 2 ]; then
  echo "usage: placeholder.sh <check name> <owner> [changed files...]" >&2
  exit 2
fi
name="$1"
owner="$2"
shift 2
echo "PLACEHOLDER: '${name}' did NOT run. No tool is installed yet (${owner})."
echo "  This is not a pass of a real check. Changed files it would have received: $#"
