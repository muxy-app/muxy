#!/usr/bin/env bash
bin="$(dirname "$0")/muxy-hook"
event="${1:-}"
case "$event" in
PreToolUse) response='{"decision":"allow"}' ;;
Stop) response='{"decision":"stop"}' ;;
*) response='{}' ;;
esac
[ -x "$bin" ] || { printf '%s\n' "$response"; exit 0; }
"$bin" agent-event --provider antigravity_hook --provider-title "Antigravity CLI" --event "$event"
status=$?
printf '%s\n' "$response"
exit "$status"
