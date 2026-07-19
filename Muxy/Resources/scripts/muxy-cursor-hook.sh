#!/usr/bin/env bash
exec "$(dirname "$0")/muxy-hook" agent-event --provider cursor_hook --provider-title Cursor --event "${1:-}" || exit 0
