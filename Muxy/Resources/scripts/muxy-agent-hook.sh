#!/usr/bin/env bash
set -euo pipefail

socket_type="${1:-}"
provider_title="${2:-}"
event="${3:-}"
input=$(cat)
plutil_path="${MUXY_AGENT_PLUTIL_PATH:-/usr/bin/plutil}"

if [ -z "${MUXY_SOCKET_PATH:-}" ] || [ -z "${MUXY_PANE_ID:-}" ] || [ -z "$socket_type" ]; then
    exit 0
fi

sanitize() {
    local value=""
    value=$(printf '%s' "${1:-}" | tr '\n\r|' '   ')
    printf '%.200s' "$value"
}

json_value() {
    local key="$1"
    local value=""
    if ! value=$(printf '%s' "$input" | "$plutil_path" -extract "$key" raw -o - -- - 2>/dev/null); then
        return
    fi
    printf '%s' "$value"
}

first_json_value() {
    local value=""
    for key in "$@"; do
        value=$(json_value "$key")
        if [ -n "$value" ]; then
            sanitize "$value"
            return
        fi
    done
}

send_event() {
    local phase="$1"
    local title
    local body
    local status
    title=$(sanitize "${2:-}")
    body=$(sanitize "${3:-}")
    if [ "${MUXY_AGENT_EVENT_PROTOCOL:-}" = "2" ]; then
        printf 'agent_event|%s|%s|%s|%s|%s\n' "$socket_type" "$MUXY_PANE_ID" "$phase" "$title" "$body" \
            | nc -U "$MUXY_SOCKET_PATH" 2>/dev/null || true
        return
    fi
    status="$phase"
    if [ "$phase" = "finished" ]; then
        status="idle"
    fi
    if [ -n "$title" ] || [ -n "$body" ]; then
        printf 'agent_status|%s|%s|%s\n%s|%s|%s|%s\n' \
            "$socket_type" "$MUXY_PANE_ID" "$status" "$socket_type" "$MUXY_PANE_ID" "$title" "$body" \
            | nc -U "$MUXY_SOCKET_PATH" 2>/dev/null || true
        return
    fi
    printf 'agent_status|%s|%s|%s\n' "$socket_type" "$MUXY_PANE_ID" "$status" \
        | nc -U "$MUXY_SOCKET_PATH" 2>/dev/null || true
}

notification_type() {
    first_json_value notification_type notificationType type
}

notification_body() {
    local fallback="$1"
    local body=""
    body=$(first_json_value message body title)
    if [ -n "$body" ]; then
        printf '%s' "$body"
        return
    fi
    printf '%s' "$fallback"
}

finished_body() {
    local body=""
    body=$(first_json_value last_assistant_message message body)
    if [ -n "$body" ]; then
        printf '%s' "$body"
        return
    fi
    printf 'Session completed'
}

handle_notification() {
    local type=""
    local body=""
    type=$(notification_type)
    case "$type" in
        auth_success | elicitation_complete | elicitation_response)
            ;;
        task_complete)
            body=$(notification_body "Task completed")
            send_event "finished" "$provider_title" "$body"
            ;;
        agent_error)
            body=$(notification_body "Agent error")
            send_event "finished" "$provider_title" "$body"
            ;;
        permission_prompt)
            body=$(notification_body "Permission needed")
            send_event "waiting" "$provider_title" "$body"
            ;;
        elicitation_dialog)
            body=$(notification_body "Question waiting")
            send_event "waiting" "$provider_title" "$body"
            ;;
        idle_prompt)
            body=$(notification_body "Idle prompt")
            send_event "waiting" "$provider_title" "$body"
            ;;
        *)
            body=$(notification_body "Needs attention")
            send_event "waiting" "$provider_title" "$body"
            ;;
    esac
}

case "$event" in
    user-prompt-submit | pre-tool-use | UserPromptSubmit | PreToolUse | beforeSubmitPrompt)
        send_event "working" "" ""
        ;;
    permission-request | PermissionRequest)
        send_event "waiting" "$provider_title" "Needs attention"
        ;;
    notification | Notification)
        handle_notification
        ;;
    stop | Stop)
        body=$(finished_body)
        send_event "finished" "$provider_title" "$body"
        ;;
    stop-failure | StopFailure)
        body=$(notification_body "Session failed")
        send_event "finished" "$provider_title" "$body"
        ;;
    session-end | SessionEnd | sessionEnd)
        send_event "finished" "" ""
        ;;
esac
