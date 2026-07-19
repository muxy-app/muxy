#!/usr/bin/env bash
exec "$(dirname "$0")/muxy-hook" agent-event --provider droid_hook --provider-title Droid --event "${1:-}" || exit 0
