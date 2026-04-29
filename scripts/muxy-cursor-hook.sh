#!/usr/bin/env bash
set -euo pipefail

if [ -z "${MUXY_SOCKET_PATH:-}" ] || [ -z "${MUXY_PANE_ID:-}" ]; then
    exit 0
fi

event="${1:-}"
input=$(cat)

send_notification() {
    local type="$1"
    local title="$2"
    local body="$3"
    printf '%s|%s|%s|%s' "$type" "$MUXY_PANE_ID" "$title" "$body" \
        | nc -U "$MUXY_SOCKET_PATH" 2>/dev/null || true
}

extract_last_message() {
    local msg=""
    msg=$(printf '%s' "$input" | grep -o '"last_assistant_message":"[^"]*"' | head -1 | cut -d'"' -f4)
    if [ -n "$msg" ]; then
        printf '%s' "$msg" | tr '|' ' ' | head -c 200
        return
    fi
    printf 'Session completed'
}

case "$event" in
    PermissionRequest|permission)
        send_notification "cursor_hook" "Cursor" "Needs attention"
        ;;
    Stop|stop)
        body=$(extract_last_message)
        send_notification "cursor_hook" "Cursor" "$body"
        ;;
esac
