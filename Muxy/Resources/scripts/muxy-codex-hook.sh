#!/usr/bin/env bash
exec "$(dirname "$0")/muxy-hook" agent-event --provider codex_hook --provider-title Codex --event "${1:-}" || exit 0
