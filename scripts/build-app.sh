#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly PROFILE="${1:-debug}"

if (($# > 1)) || [[ "$PROFILE" != "debug" && "$PROFILE" != "release" ]]; then
    printf 'Usage: scripts/build-app.sh [debug|release]\n' >&2
    exit 2
fi

for command_name in cargo codesign iconutil plutil xcode-select xcrun; do
    command -v "$command_name" >/dev/null 2>&1 || {
        printf 'error: required command not found: %s\n' "$command_name" >&2
        exit 1
    }
done

DEVELOPER_DIR="$(xcode-select -p)"
readonly DEVELOPER_DIR
[[ -d "$DEVELOPER_DIR/Platforms/MacOSX.platform" ]] || {
    printf 'error: full Xcode is required; xcode-select points to %s\n' "$DEVELOPER_DIR" >&2
    exit 1
}
export DEVELOPER_DIR
MACOS_SDK_PATH="$(xcrun --sdk macosx --show-sdk-path)"
readonly MACOS_SDK_PATH
[[ -d "$MACOS_SDK_PATH" ]] || {
    printf 'error: xcrun returned a missing macOS SDK: %s\n' "$MACOS_SDK_PATH" >&2
    exit 1
}

readonly GHOSTTY_RESOURCES="$PROJECT_ROOT/resources/ghostty"
readonly TERMINFO_RESOURCES="$PROJECT_ROOT/resources/terminfo"
readonly OVERRIDE_RESOURCES="$PROJECT_ROOT/resources/ghostty-overrides"
readonly ICONSET="$PROJECT_ROOT/resources/AppIcon.iconset"
readonly INFO_PLIST="$PROJECT_ROOT/resources/Info.plist"

[[ -s "$GHOSTTY_RESOURCES/shell-integration/zsh/ghostty-integration" ]] || {
    printf 'error: Ghostty resources are missing; run scripts/setup.sh first\n' >&2
    exit 1
}
[[ -s "$TERMINFO_RESOURCES/67/ghostty" && -s "$TERMINFO_RESOURCES/78/xterm-ghostty" ]] || {
    printf 'error: Ghostty terminfo is missing; run scripts/setup.sh first\n' >&2
    exit 1
}
[[ -d "$OVERRIDE_RESOURCES" && -d "$ICONSET" && -f "$INFO_PLIST" ]] || {
    printf 'error: committed application resources are incomplete\n' >&2
    exit 1
}
plutil -lint "$INFO_PLIST" >/dev/null

cargo_arguments=(build --package muxy --locked --target-dir "$PROJECT_ROOT/target")
if [[ "$PROFILE" == "release" ]]; then
    cargo_arguments+=(--release)
fi

cd "$PROJECT_ROOT"
printf '==> Building muxy (%s)\n' "$PROFILE"
MACOSX_DEPLOYMENT_TARGET=14.0 cargo "${cargo_arguments[@]}"

readonly PROFILE_DIRECTORY="$PROJECT_ROOT/target/$PROFILE"
readonly APP_BUNDLE="$PROFILE_DIRECTORY/Muxy.app"
readonly STAGING_BUNDLE="$PROFILE_DIRECTORY/.Muxy.app.staging.$$"

cleanup() {
    if [[ -d "$STAGING_BUNDLE" ]]; then
        rm -rf "$STAGING_BUNDLE"
    fi
}
trap cleanup EXIT

mkdir -p "$STAGING_BUNDLE/Contents/MacOS" "$STAGING_BUNDLE/Contents/Resources"
install -m 0755 "$PROFILE_DIRECTORY/muxy" "$STAGING_BUNDLE/Contents/MacOS/muxy"
install -m 0644 "$INFO_PLIST" "$STAGING_BUNDLE/Contents/Info.plist"
iconutil --convert icns --output "$STAGING_BUNDLE/Contents/Resources/AppIcon.icns" "$ICONSET"

cp -R "$GHOSTTY_RESOURCES" "$STAGING_BUNDLE/Contents/Resources/ghostty"
cp -R "$TERMINFO_RESOURCES" "$STAGING_BUNDLE/Contents/Resources/terminfo"
cp -R "$OVERRIDE_RESOURCES" "$STAGING_BUNDLE/Contents/Resources/ghostty-overrides"

printf '==> Ad-hoc signing Muxy.app\n'
codesign --force --sign - --timestamp=none "$STAGING_BUNDLE"
"$SCRIPT_DIR/verify-bundle.sh" "$STAGING_BUNDLE"

rm -rf "$APP_BUNDLE"
mv "$STAGING_BUNDLE" "$APP_BUNDLE"
trap - EXIT

printf '==> Built %s\n' "$APP_BUNDLE"
