#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FORK_REPO="muxy-app/ghostty"
XCFRAMEWORK_DIR="$PROJECT_ROOT/GhosttyKit.xcframework"
RESOURCES_DIR="$PROJECT_ROOT/Muxy/Resources/ghostty"
TERMINFO_DIR="$PROJECT_ROOT/Muxy/Resources/terminfo"

if [[ -d "$XCFRAMEWORK_DIR" && -d "$RESOURCES_DIR/shell-integration" && -d "$TERMINFO_DIR" ]]; then
    echo "==> GhosttyKit.xcframework and resources already present, skipping download"
    echo "    To re-download, remove: rm -rf GhosttyKit.xcframework Muxy/Resources/ghostty Muxy/Resources/terminfo"
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
