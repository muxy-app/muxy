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
    printf '%s|%s|%s|%s\n' "$type" "$MUXY_PANE_ID" "$title" "$body" \
        | nc -U "$MUXY_SOCKET_PATH" 2>/dev/null || true
}

send_status() {
    local status="$1"
    printf 'agent_status|grok_hook|%s|%s\n' "$MUXY_PANE_ID" "$status" \
        | nc -U "$MUXY_SOCKET_PATH" 2>/dev/null || true
}

notification_body() {
    local ntype=""
    local msg=""
    ntype=$(printf '%s' "$input" | grep -o '"notificationType":"[^"]*"' | head -1 | cut -d'"' -f4)
    msg=$(printf '%s' "$input" | grep -o '"message":"[^"]*"' | head -1 | cut -d'"' -f4)
    if [ -n "$msg" ]; then
        printf '%s' "$msg" | tr '|' ' ' | head -c 200
        return
    fi
    case "$ntype" in
        permission_prompt) printf 'Permission needed' ;;
        elicitation_dialog) printf 'Question waiting' ;;
        task_complete) printf 'Task completed' ;;
        agent_error) printf 'Agent error' ;;
        idle_prompt) printf 'Idle prompt' ;;
        *) printf 'Needs attention' ;;
    esac
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
    user-prompt-submit | pre-tool-use)
        send_status "working"
        ;;
    notification)
        send_status "waiting"
        body=$(notification_body)
        send_notification "grok_hook" "Grok" "$body"
        ;;
    stop)
        send_status "idle"
        body=$(extract_last_message)
        send_notification "grok_hook" "Grok" "$body"
        ;;
esac
