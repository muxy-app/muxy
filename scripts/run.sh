#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly PROFILE="${1:-debug}"

if (($# > 1)) || [[ "$PROFILE" != "debug" && "$PROFILE" != "release" ]]; then
    printf 'Usage: scripts/run.sh [debug|release]\n' >&2
    exit 2
fi

case "$(uname -s)" in
    Darwin)
        "$SCRIPT_DIR/build-app.sh" "$PROFILE"
        exec "$PROJECT_ROOT/target/$PROFILE/Muxy.app/Contents/MacOS/muxy"
        ;;
    Linux)
        command -v cargo >/dev/null 2>&1 || {
            printf 'error: required command not found: cargo\n' >&2
            exit 1
        }
        cargo_arguments=(build --package muxy --locked --target-dir "$PROJECT_ROOT/target")
        if [[ "$PROFILE" == "release" ]]; then
            cargo_arguments+=(--release)
        fi
        cd "$PROJECT_ROOT"
        printf '==> Building muxy (%s)\n' "$PROFILE"
        cargo "${cargo_arguments[@]}"
        exec "$PROJECT_ROOT/target/$PROFILE/muxy"
        ;;
    *)
        printf 'error: unsupported operating system: %s\n' "$(uname -s)" >&2
        exit 1
        ;;
esac
