#!/usr/bin/env bash
bin="$(dirname "$0")/muxy-hook"
[ -x "$bin" ] || exit 0
exec "$bin" agent-event --provider kiro_hook --provider-title "Kiro CLI" --event "${1:-}"
