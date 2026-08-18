#!/usr/bin/env bash
bin="$(dirname "$0")/muxy-hook"
[ -x "$bin" ] || exit 0
exec "$bin" agent-event --provider cmd_cli --provider-title "Command Code" --event "${1:-}"
