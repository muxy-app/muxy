#!/usr/bin/env bash
bin="$(dirname "$0")/muxy-hook"
[ -x "$bin" ] || exit 0
exec "$bin" agent-event --provider antigravity_hook --provider-title "Antigravity CLI" --event "${1:-}"
