#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <arm64|x86_64> <2.0.0-beta-N>" >&2
    exit 1
fi
ARCH="$1"
VERSION="$2"
case "$ARCH" in
    arm64) TARGET=aarch64-unknown-linux-gnu; MACHINE=aarch64 ;;
    x86_64) TARGET=x86_64-unknown-linux-gnu; MACHINE=x86_64 ;;
    *) echo "Error: unsupported architecture: $ARCH" >&2; exit 1 ;;
esac
python3 "$ROOT/scripts/beta_release.py" check-version "$VERSION"
if [[ "$(uname -s)" != Linux || "$(uname -m)" != "$MACHINE" || "$(getconf GNU_LIBC_VERSION)" != 'glibc 2.35' ]]; then
    echo "Error: build on native $ARCH Linux with glibc 2.35" >&2
    exit 1
fi
export CARGO_TARGET_DIR="$ROOT/target"
export LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast
export MUXY_ZIG="$(command -v zig)"
export PATH="$ROOT/scripts/zig:$PATH"
OUTPUT="$CARGO_TARGET_DIR/beta/$VERSION/linux-$ARCH"
if [[ -e "$OUTPUT" ]]; then
    echo "Error: output already exists: $OUTPUT" >&2
    exit 1
fi
cd "$ROOT"
# A cached native Zig object may otherwise depend on the previous runner's CPU.
cargo clean -p libghostty-vt-sys
cargo build --locked --release --target "$TARGET" -p muxy-cli -p muxy-server
mkdir -p "$(dirname "$OUTPUT")"
STAGING="$(mktemp -d "$(dirname "$OUTPUT")/.linux-${ARCH}.XXXXXX")"
trap 'rm -rf "$STAGING"' EXIT
mkdir "$STAGING/cli" "$STAGING/symbols" "$STAGING/artifacts"
for BINARY in muxy muxy-server; do
    install -m 755 "$CARGO_TARGET_DIR/$TARGET/release/$BINARY" "$STAGING/cli/$BINARY"
    objcopy --only-keep-debug "$STAGING/cli/$BINARY" "$STAGING/symbols/$BINARY.debug"
    strip --strip-debug "$STAGING/cli/$BINARY"
    python3 scripts/audit-linux.py "$STAGING/cli/$BINARY" --target "$TARGET" > "$STAGING/artifacts/$BINARY-audit.txt"
    cat "$STAGING/artifacts/$BINARY-audit.txt"
    python3 scripts/beta_release.py check-build "$VERSION" "$STAGING/cli/$BINARY"
done
cat LICENSE crates/muxy-server/src/detection/THIRD_PARTY.md \
    crates/muxy-server/src/detection/LICENSE-herdr > "$STAGING/cli/LICENSE"
tar -czf "$STAGING/artifacts/muxy-${VERSION}-linux-${ARCH}.tar.gz" -C "$STAGING/cli" muxy muxy-server LICENSE
tar -czf "$STAGING/artifacts/muxy-${VERSION}-linux-${ARCH}-symbols.tar.gz" -C "$STAGING/symbols" .
mv "$STAGING/artifacts" "$OUTPUT"
echo "==> Built $OUTPUT"
