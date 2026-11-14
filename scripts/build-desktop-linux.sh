#!/bin/bash
set -euo pipefail

if [ "$(uname -s)" != Linux ]; then
    echo "Run this script inside Linux (for example, the SwiftVM Ubuntu guest)." >&2
    exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export MUXY_ZIG="${MUXY_ZIG:-$(command -v zig || true)}"
if [ ! -x "$MUXY_ZIG" ] || [ "$MUXY_ZIG" = "$ROOT/scripts/zig/zig" ]; then
    echo "Set MUXY_ZIG to the real Zig 0.17.0 executable." >&2
    exit 1
fi
export PATH="$ROOT/scripts/zig:$PATH"
export LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target/linux-desktop}"
cd "$ROOT"
cargo build --locked -p muxy-app -p muxy-cli -p muxy-server

echo "Built desktop and matching helpers in $CARGO_TARGET_DIR/debug"
echo "From a Linux desktop terminal, run: $CARGO_TARGET_DIR/debug/muxy-app"
