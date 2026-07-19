#!/usr/bin/env bash
bin="$(dirname "$0")/muxy-hook"
[ -x "$bin" ] || exit 0
exec "$bin" agent-event --provider claude_hook --provider-title "Claude Code" --event "${1:-}"
