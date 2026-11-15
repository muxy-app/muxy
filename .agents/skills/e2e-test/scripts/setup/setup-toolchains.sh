#!/bin/bash
set -euo pipefail
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/muxy-vm"
if [ ! -x "$HOME/.cargo/bin/rustup" ]; then
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o /tmp/muxy-rustup.sh
  sh /tmp/muxy-rustup.sh -y --profile minimal --default-toolchain 1.92.0
fi
rustup toolchain install 1.92.0 --profile minimal --component rustfmt --component clippy
case "$(uname -s)" in
  Linux) zig_platform=aarch64-linux ;;
  Darwin)
    # Homebrew's zig formula is 0.17.0, the version Ghostty 1.4.0 requires.
    export HOMEBREW_NO_ANALYTICS=1
    if [ ! -x /opt/homebrew/bin/brew ]; then
      if [ ! -w /opt/homebrew ]; then
        echo 'First create /opt/homebrew as root and give the guest account ownership (see references/templates.md).' >&2
        exit 1
      fi
      curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh -o /tmp/muxy-homebrew-install.sh
      NONINTERACTIVE=1 HOMEBREW_NO_SUDO=1 bash /tmp/muxy-homebrew-install.sh
    fi
    /opt/homebrew/bin/brew analytics off
    /opt/homebrew/bin/brew install zig
    test "$(/opt/homebrew/opt/zig/bin/zig version)" = 0.17.0
    ln -sfn /opt/homebrew/opt/zig/bin/zig "$HOME/.local/bin/zig"
    rustup run 1.92.0 rustc --version
    zig version
    exit 0
    ;;
  *) exit 1 ;;
esac
zig_version=0.17.0
zig_dir="$HOME/.local/zig-$zig_version"
if [ ! -x "$zig_dir/zig" ]; then
  curl --proto '=https' --tlsv1.2 -fL "https://ziglang.org/download/$zig_version/zig-$zig_platform-$zig_version.tar.xz" -o /tmp/muxy-zig.tar.xz
  curl --proto '=https' --tlsv1.2 -fsSL https://ziglang.org/download/index.json -o /tmp/muxy-zig-index.json
  python3 - "$zig_platform" "$zig_version" <<'PY'
import hashlib, json, sys
with open('/tmp/muxy-zig-index.json') as f:
    expected=json.load(f)[sys.argv[2]][sys.argv[1]]['shasum']
with open('/tmp/muxy-zig.tar.xz','rb') as f:
    actual=hashlib.file_digest(f,'sha256').hexdigest() if hasattr(hashlib,'file_digest') else hashlib.sha256(f.read()).hexdigest()
if actual != expected:
    sys.exit('Zig checksum mismatch')
print('Verified Zig SHA-256', actual)
PY
  mkdir -p "$zig_dir"
  tar -xJf /tmp/muxy-zig.tar.xz --strip-components=1 -C "$zig_dir"
  rm /tmp/muxy-zig.tar.xz /tmp/muxy-zig-index.json
fi
ln -sfn "$zig_dir/zig" "$HOME/.local/bin/zig"
rustup run 1.92.0 rustc --version
zig version
