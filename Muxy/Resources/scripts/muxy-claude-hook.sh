#!/usr/bin/env bash
set -euo pipefail
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
exec /bin/bash "$script_dir/muxy-agent-hook.sh" "claude_hook" "Claude Code" "${1:-}"
