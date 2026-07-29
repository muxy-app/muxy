#!/usr/bin/env bash
bin="$(dirname "$0")/muxy-hook"
[ -x "$bin" ] || exit 0
exec "$bin" agent-event --provider copilot_hook --provider-title "GitHub Copilot" --event "${1:-}"
