#!/usr/bin/env bash
exec "$(dirname "$0")/muxy-hook" agent-event --provider grok_hook --provider-title Grok --event "${1:-}" || exit 0
