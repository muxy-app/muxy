#!/usr/bin/env bash
set -euo pipefail

find_real_pi() {
    local self_dir
    self_dir="$(cd "$(dirname "$0")" && pwd)"
    local IFS=:
    for d in $PATH; do
        [[ "$d" == "$self_dir" ]] && continue
        [[ -x "$d/pi" ]] && printf '%s' "$d/pi" && return 0
    done
    return 1
}

REAL_PI="$(find_real_pi)" || { echo "Error: pi not found in PATH" >&2; exit 127; }

if [ -z "${MUXY_SOCKET_PATH:-}" ] || [ -z "${MUXY_PANE_ID:-}" ]; then
    exec "$REAL_PI" "$@"
fi

case "${1:-}" in
    install|remove|uninstall|update|list|config) exec "$REAL_PI" "$@" ;;
esac

for arg in "$@"; do
    case "$arg" in
        --help|-h|--version|-v) exec "$REAL_PI" "$@" ;;
    esac
done

EXTENSION_PATH="$(cd "$(dirname "$0")" && pwd)/muxy-pi-extension.ts"
exec "$REAL_PI" --extension "$EXTENSION_PATH" "$@"
