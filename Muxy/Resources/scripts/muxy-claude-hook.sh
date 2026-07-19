#!/usr/bin/env bash
exec "$(dirname "$0")/muxy-hook" agent-event --provider claude_hook --provider-title "Claude Code" --event "${1:-}" || exit 0
