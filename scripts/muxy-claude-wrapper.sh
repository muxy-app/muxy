#!/usr/bin/env bash
set -euo pipefail

if [ -z "${MUXY_SOCKET_PATH:-}" ] || [ -z "${MUXY_PANE_ID:-}" ]; then
    exec claude "$@"
fi

find_real_claude() {
    local self_dir
    self_dir="$(cd "$(dirname "$0")" && pwd)"
    local IFS=:
    for d in $PATH; do
        [[ "$d" == "$self_dir" ]] && continue
        [[ -x "$d/claude" ]] && printf '%s' "$d/claude" && return 0
    done
    return 1
}

REAL_CLAUDE="$(find_real_claude)" || { echo "Error: claude not found in PATH" >&2; exit 127; }

case "${1:-}" in
    mcp|config|api-key) exec "$REAL_CLAUDE" "$@" ;;
esac

HOOK_SCRIPT="$(cd "$(dirname "$0")" && pwd)/muxy-claude-hook.sh"

escaped_hook=$(printf '%s' "$HOOK_SCRIPT" | sed 's/\\/\\\\/g; s/"/\\"/g')

HOOKS_JSON="{\"hooks\":{\"Stop\":[{\"matcher\":\"\",\"hooks\":[{\"type\":\"command\",\"command\":\"'${escaped_hook}' stop\",\"timeout\":10}]}],\"Notification\":[{\"matcher\":\"\",\"hooks\":[{\"type\":\"command\",\"command\":\"'${escaped_hook}' notification\",\"timeout\":10}]}]}}"

exec "$REAL_CLAUDE" --settings "$HOOKS_JSON" "$@"
