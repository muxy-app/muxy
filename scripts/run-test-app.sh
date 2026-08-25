#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly PROFILE="${1:-debug}"

if (($# > 1)) || [[ "$PROFILE" != "debug" && "$PROFILE" != "release" ]]; then
    printf 'Usage: scripts/run-test-app.sh [debug|release]\n' >&2
    exit 2
fi

"$SCRIPT_DIR/build-app.sh" "$PROFILE"
staged_app="$("$SCRIPT_DIR/stage-test-app.sh" \
    "$PROJECT_ROOT/target/$PROFILE/Muxy.app" "rust-$PROFILE")"
state_directory="$PROJECT_ROOT/target/test-verification/state"
mkdir -p "$state_directory"

printf '==> Launching isolated %s test app\n' "$PROFILE"
printf '==> Bundle: %s\n' "$staged_app"
printf '==> App Support: %s\n' "$state_directory"
exec open -n -W --env "MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY=$state_directory" \
    -a "$staged_app"
