#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FORK_REPO="muxy-app/ghostty"
XCFRAMEWORK_DIR="$PROJECT_ROOT/GhosttyKit.xcframework"
RESOURCES_DIR="$PROJECT_ROOT/Muxy/Resources/ghostty"
TERMINFO_DIR="$PROJECT_ROOT/Muxy/Resources/terminfo"
RIPGREP_VERSION="15.1.0"
RIPGREP_BINARY="$PROJECT_ROOT/Muxy/Resources/rg"

fetch_ripgrep() {
    if [[ -x "$RIPGREP_BINARY" ]]; then
        return 0
    fi
    local arch
    case "$(uname -m)" in
        arm64) arch="aarch64-apple-darwin" ;;
        x86_64) arch="x86_64-apple-darwin" ;;
        *) echo "Error: unsupported architecture $(uname -m)"; return 1 ;;
    esac
    local archive="ripgrep-${RIPGREP_VERSION}-${arch}.tar.gz"
    local url="https://github.com/BurntSushi/ripgrep/releases/download/${RIPGREP_VERSION}/${archive}"
    echo "==> Downloading ripgrep ${RIPGREP_VERSION} (${arch})"
    local tmp
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' RETURN
    curl -fsSL "$url" -o "$tmp/$archive"
    tar xzf "$tmp/$archive" -C "$tmp"
    mkdir -p "$(dirname "$RIPGREP_BINARY")"
    cp "$tmp/ripgrep-${RIPGREP_VERSION}-${arch}/rg" "$RIPGREP_BINARY"
    chmod +x "$RIPGREP_BINARY"
    codesign --force --sign - "$RIPGREP_BINARY" >/dev/null 2>&1 || true
    echo "    Installed: $RIPGREP_BINARY"
}

if [[ -d "$XCFRAMEWORK_DIR" && -d "$RESOURCES_DIR/shell-integration" && -d "$TERMINFO_DIR" && -x "$RIPGREP_BINARY" ]]; then
    echo "==> GhosttyKit.xcframework, resources, and ripgrep already present, skipping download"
    echo "    To re-download, remove: rm -rf GhosttyKit.xcframework Muxy/Resources/ghostty Muxy/Resources/terminfo Muxy/Resources/rg"
    exit 0
fi

fetch_ripgrep

if [[ -d "$XCFRAMEWORK_DIR" && -d "$RESOURCES_DIR/shell-integration" && -d "$TERMINFO_DIR" ]]; then
    echo "==> GhosttyKit.xcframework and resources already present"
    exit 0
fi

echo "==> Fetching latest GhosttyKit release from $FORK_REPO"
LATEST_TAG=$(gh release list --repo "$FORK_REPO" --limit 1 --json tagName -q '.[0].tagName')
if [[ -z "$LATEST_TAG" ]]; then
    echo "Error: No releases found on $FORK_REPO"
    exit 1
fi
echo "    Tag: $LATEST_TAG"

cd "$PROJECT_ROOT"

if [[ ! -d "$XCFRAMEWORK_DIR" ]]; then
    echo "==> Downloading GhosttyKit.xcframework"
    gh release download "$LATEST_TAG" \
        --pattern "GhosttyKit.xcframework.tar.gz" \
        --repo "$FORK_REPO"
    tar xzf GhosttyKit.xcframework.tar.gz
    rm GhosttyKit.xcframework.tar.gz

    echo "==> Syncing ghostty.h from xcframework"
    cp "$XCFRAMEWORK_DIR/macos-arm64_x86_64/Headers/ghostty.h" "$PROJECT_ROOT/GhosttyKit/ghostty.h"
fi

if [[ ! -d "$RESOURCES_DIR/shell-integration" || ! -d "$TERMINFO_DIR" ]]; then
    echo "==> Downloading GhosttyKit runtime resources"
    gh release download "$LATEST_TAG" \
        --pattern "GhosttyKit-resources.tar.gz" \
        --repo "$FORK_REPO"
    rm -rf "$RESOURCES_DIR" "$TERMINFO_DIR"
    mkdir -p "$(dirname "$RESOURCES_DIR")"
    tar xzf GhosttyKit-resources.tar.gz -C "$(dirname "$RESOURCES_DIR")"
    rm GhosttyKit-resources.tar.gz
fi

echo "==> Done"
echo "    Run 'swift build' to build the project"
