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

LOCAL_XCFRAMEWORK_TAR="${1:-}"
if [[ -n "$LOCAL_XCFRAMEWORK_TAR" ]]; then
    if [[ ! -f "$LOCAL_XCFRAMEWORK_TAR" ]]; then
        echo "Error: local xcframework tar not found: $LOCAL_XCFRAMEWORK_TAR"
        exit 1
    fi
    LOCAL_XCFRAMEWORK_TAR="$(cd "$(dirname "$LOCAL_XCFRAMEWORK_TAR")" && pwd)/$(basename "$LOCAL_XCFRAMEWORK_TAR")"
fi

LOCAL_RIPGREP_TAR="${2:-}"
if [[ -n "$LOCAL_RIPGREP_TAR" ]]; then
    if [[ ! -f "$LOCAL_RIPGREP_TAR" ]]; then
        echo "Error: local ripgrep tar not found: $LOCAL_RIPGREP_TAR"
        exit 1
    fi
    LOCAL_RIPGREP_TAR="$(cd "$(dirname "$LOCAL_RIPGREP_TAR")" && pwd)/$(basename "$LOCAL_RIPGREP_TAR")"
fi

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
    local tmp
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' RETURN
    if [[ -n "$LOCAL_RIPGREP_TAR" ]]; then
        echo "==> Extracting ripgrep from $LOCAL_RIPGREP_TAR"
        tar xzf "$LOCAL_RIPGREP_TAR" -C "$tmp"
    else
        local url="https://github.com/BurntSushi/ripgrep/releases/download/${RIPGREP_VERSION}/${archive}"
        echo "==> Downloading ripgrep ${RIPGREP_VERSION} (${arch})"
        curl -fsSL "$url" -o "$tmp/$archive"
        tar xzf "$tmp/$archive" -C "$tmp"
    fi
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

cd "$PROJECT_ROOT"

NEEDS_XCFRAMEWORK_DOWNLOAD=false
if [[ ! -d "$XCFRAMEWORK_DIR" && -z "$LOCAL_XCFRAMEWORK_TAR" ]]; then
    NEEDS_XCFRAMEWORK_DOWNLOAD=true
fi

NEEDS_RESOURCES_DOWNLOAD=false
if [[ ! -d "$RESOURCES_DIR/shell-integration" || ! -d "$TERMINFO_DIR" ]]; then
    NEEDS_RESOURCES_DOWNLOAD=true
fi

LATEST_TAG=""
if [[ "$NEEDS_XCFRAMEWORK_DOWNLOAD" == "true" || "$NEEDS_RESOURCES_DOWNLOAD" == "true" ]]; then
    echo "==> Fetching latest GhosttyKit release from $FORK_REPO"
    LATEST_TAG=$(gh release list --repo "$FORK_REPO" --limit 1 --json tagName -q '.[0].tagName')
    if [[ -z "$LATEST_TAG" ]]; then
        echo "Error: No releases found on $FORK_REPO"
        exit 1
    fi
    echo "    Tag: $LATEST_TAG"
fi

if [[ ! -d "$XCFRAMEWORK_DIR" ]]; then
    if [[ -n "$LOCAL_XCFRAMEWORK_TAR" ]]; then
        echo "==> Extracting GhosttyKit.xcframework from $LOCAL_XCFRAMEWORK_TAR"
        tar xzf "$LOCAL_XCFRAMEWORK_TAR"
    else
        echo "==> Downloading GhosttyKit.xcframework"
        gh release download "$LATEST_TAG" \
            --pattern "GhosttyKit.xcframework.tar.gz" \
            --repo "$FORK_REPO"
        tar xzf GhosttyKit.xcframework.tar.gz
        rm GhosttyKit.xcframework.tar.gz
    fi

    echo "==> Syncing ghostty.h from xcframework"
    cp "$XCFRAMEWORK_DIR/macos-arm64_x86_64/Headers/ghostty.h" "$PROJECT_ROOT/GhosttyKit/ghostty.h"
fi

if [[ "$NEEDS_RESOURCES_DOWNLOAD" == "true" ]]; then
    echo "==> Downloading GhosttyKit runtime resources"
    gh release download "$LATEST_TAG" \
        --pattern "GhosttyKit-resources.tar.gz" \
        --repo "$FORK_REPO"
    THEMES_BACKUP=""
    if [[ -d "$RESOURCES_DIR/themes" ]]; then
        THEMES_BACKUP="$(mktemp -d)/themes"
        mv "$RESOURCES_DIR/themes" "$THEMES_BACKUP"
    fi
    rm -rf "$RESOURCES_DIR" "$TERMINFO_DIR"
    mkdir -p "$(dirname "$RESOURCES_DIR")"
    tar xzf GhosttyKit-resources.tar.gz -C "$(dirname "$RESOURCES_DIR")"
    rm GhosttyKit-resources.tar.gz
    rm -rf "$RESOURCES_DIR/themes"
    if [[ -n "$THEMES_BACKUP" ]]; then
        mv "$THEMES_BACKUP" "$RESOURCES_DIR/themes"
        rmdir "$(dirname "$THEMES_BACKUP")" 2>/dev/null || true
    fi
fi

echo "==> Done"
echo "    Run 'swift build' to build the project"
