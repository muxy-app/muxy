#!/bin/bash
# Builds muxy and muxy-server from this checkout for x86_64 Linux, for trying
# unreleased builds on a remote device. It builds natively in an OrbStack
# machine running Ubuntu 22.04 (glibc 2.35, like the release builders), which
# sees this checkout at the same path. The first run creates the machine and
# installs Rust and Zig in it.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MACHINE="${MUXY_LINUX_MACHINE:-muxy-linux-x86}"
ZIG_VERSION=0.15.2
OUTPUT="$ROOT/target/linux-dev"
ARCHIVE="$OUTPUT/muxy-dev-linux-x86_64.tar.gz"

command -v orb >/dev/null || { echo "Error: install OrbStack (https://orbstack.dev) first" >&2; exit 1; }
if ! orb list --quiet | grep -qx "$MACHINE"; then
    echo "==> Creating OrbStack machine $MACHINE (Ubuntu 22.04, x86_64)"
    orb create --arch amd64 ubuntu:jammy "$MACHINE"
fi

echo "==> Building in $MACHINE"
orb -m "$MACHINE" bash -s -- "$ROOT" "$ZIG_VERSION" <<'BUILD'
set -euo pipefail
ROOT=$1
ZIG_VERSION=$2
PACKAGES="build-essential pkg-config curl xz-utils git"
if ! dpkg -s $PACKAGES >/dev/null 2>&1; then
    echo "Installing $PACKAGES"
    sudo apt-get update -qq
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq $PACKAGES >/dev/null
fi
if [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
fi
ZIG="$HOME/.local/zig-$ZIG_VERSION/zig"
if [ ! -x "$ZIG" ]; then
    mkdir -p "$HOME/.local/zig-$ZIG_VERSION"
    curl -fsSL "https://ziglang.org/download/$ZIG_VERSION/zig-x86_64-linux-$ZIG_VERSION.tar.xz" |
        tar -xJ --strip-components=1 -C "$HOME/.local/zig-$ZIG_VERSION"
fi
export PATH="$ROOT/scripts/zig:$HOME/.cargo/bin:$PATH"
export MUXY_ZIG="$ZIG"
export LIBGHOSTTY_VT_SYS_OPTIMIZE=ReleaseFast
# Apart from the Mac's builds in target/.
export CARGO_TARGET_DIR="$ROOT/target/linux"
cd "$ROOT"
rustup toolchain install
cargo build --locked --release -p muxy-cli -p muxy-server
# Without debug info, like release builds.
rm -rf "$ROOT/target/linux-dev/bin"
mkdir -p "$ROOT/target/linux-dev/bin"
for BINARY in muxy muxy-server; do
    install -m 755 "$CARGO_TARGET_DIR/release/$BINARY" "$ROOT/target/linux-dev/bin/$BINARY"
    strip --strip-debug "$ROOT/target/linux-dev/bin/$BINARY"
done
# Linux tar, so no macOS metadata ends up in the archive.
tar -czf "$ROOT/target/linux-dev/muxy-dev-linux-x86_64.tar.gz" -C "$ROOT/target/linux-dev/bin" muxy muxy-server
BUILD

echo "==> Built $ARCHIVE"
cat <<EOF

Install on an x86_64 Linux server with glibc 2.35 or newer:
  scp "$ARCHIVE" dev@box:/tmp/
  ssh dev@box 'mkdir -p ~/.local/bin && tar -xzf /tmp/muxy-dev-linux-x86_64.tar.gz -C ~/.local/bin'
If a dev server already runs there, stop it so the next connection starts the new
build. This ends its sessions:
  ssh dev@box '~/.local/bin/muxy server status && ~/.local/bin/muxy server stop --force'
EOF
