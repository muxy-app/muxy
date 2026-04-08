#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FORK_REPO="muxy-app/ghostty"
XCFRAMEWORK_DIR="$PROJECT_ROOT/GhosttyKit.xcframework"

if [[ -d "$XCFRAMEWORK_DIR" ]]; then
    echo "==> GhosttyKit.xcframework already exists, skipping download"
    echo "    To re-download, remove it first: rm -rf GhosttyKit.xcframework"
    exit 0
fi

echo "==> Fetching latest GhosttyKit release from $FORK_REPO"
LATEST_TAG=$(gh release list --repo "$FORK_REPO" --limit 1 --json tagName -q '.[0].tagName')
if [[ -z "$LATEST_TAG" ]]; then
    echo "Error: No releases found on $FORK_REPO"
    exit 1
fi
echo "    Tag: $LATEST_TAG"

echo "==> Downloading GhosttyKit.xcframework"
cd "$PROJECT_ROOT"
gh release download "$LATEST_TAG" \
    --pattern "GhosttyKit.xcframework.tar.gz" \
    --repo "$FORK_REPO"
tar xzf GhosttyKit.xcframework.tar.gz
rm GhosttyKit.xcframework.tar.gz

echo "==> Syncing ghostty.h from xcframework"
cp "$XCFRAMEWORK_DIR/macos-arm64_x86_64/Headers/ghostty.h" "$PROJECT_ROOT/GhosttyKit/ghostty.h"

echo "==> Done"
echo "    Run 'swift build' to build the project"
