#!/usr/bin/env bash
set -euo pipefail

if [ -z "${MUXY_SOCKET_PATH:-}" ] || [ -z "${MUXY_PANE_ID:-}" ]; then
    exit 0
fi

event="${1:-}"

send_notification() {
    local type="$1"
    local title="$2"
    local body="$3"
    printf '%s|%s|%s|%s\n' "$type" "$MUXY_PANE_ID" "$title" "$body" \
        | nc -U "$MUXY_SOCKET_PATH" 2>/dev/null || true
}

send_status() {
    local status="$1"
    printf 'agent_status|codex_hook|%s|%s\n' "$MUXY_PANE_ID" "$status" \
        | nc -U "$MUXY_SOCKET_PATH" 2>/dev/null || true
}

extract_last_message() {
    local input="$1"
    local msg=""
    msg=$(printf '%s' "$input" | grep -o '"last_assistant_message":"[^"]*"' | head -1 | cut -d'"' -f4)
    if [ -n "$msg" ]; then
        printf '%s' "$msg" | tr '|' ' ' | head -c 200
        return
    fi
    printf 'Session completed'
}

case "$event" in
    user-prompt-submit | pre-tool-use | UserPromptSubmit | PreToolUse)
        cat >/dev/null
        send_status "working"
        ;;
    permission-request | PermissionRequest)
        cat >/dev/null
        send_notification "codex_hook" "Codex" "Needs attention"
        ;;
    stop | Stop)
        input=$(cat)
        send_status "idle"
        body=$(extract_last_message "$input")
        send_notification "codex_hook" "Codex" "$body"
        ;;
    *)
        cat >/dev/null
        ;;
esac
