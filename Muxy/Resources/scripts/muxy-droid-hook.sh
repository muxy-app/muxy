#!/usr/bin/env bash
bin="$(dirname "$0")/muxy-hook"
[ -x "$bin" ] || exit 0
exec "$bin" agent-event --provider droid_hook --provider-title Droid --event "${1:-}"
