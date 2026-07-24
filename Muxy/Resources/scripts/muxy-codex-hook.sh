#!/usr/bin/env bash
bin="$(dirname "$0")/muxy-hook"
[ -x "$bin" ] || exit 0
exec "$bin" agent-event --provider codex_hook --provider-title Codex --event "${1:-}"
