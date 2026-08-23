#!/usr/bin/env bash
set -euo pipefail

output=$(mktemp)
trap 'rm -f "$output"' EXIT

exit_code=0
"$@" > "$output" 2>&1 || exit_code=$?
cat "$output"

if [ "$exit_code" -ne 0 ]; then
  exit "$exit_code"
fi

if LC_ALL=C sed $'s/\033\[[0-9;]*m//g' "$output" | grep -E '(^|[[:space:]])warning:' > /dev/null; then
  printf 'Build emitted warning diagnostics\n' >&2
  exit 1
fi
