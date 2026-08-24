#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly APP_BUNDLE="${1:-$PROJECT_ROOT/target/debug/Muxy.app}"

if (($# > 1)); then
    printf 'Usage: scripts/verify-bundle.sh [path/to/Muxy.app]\n' >&2
    exit 2
fi

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

for command_name in codesign iconutil plutil; do
    command -v "$command_name" >/dev/null 2>&1 || fail "required command not found: $command_name"
done
[[ -x /usr/libexec/PlistBuddy ]] || fail "required command not found: /usr/libexec/PlistBuddy"

readonly CONTENTS="$APP_BUNDLE/Contents"
readonly PLIST="$CONTENTS/Info.plist"
readonly RESOURCES="$CONTENTS/Resources"
readonly EXECUTABLE="$CONTENTS/MacOS/muxy"
readonly ICON="$RESOURCES/AppIcon.icns"

[[ -d "$APP_BUNDLE" ]] || fail "app bundle not found: $APP_BUNDLE"
[[ -x "$EXECUTABLE" ]] || fail "bundle executable is missing or not executable"
[[ -f "$PLIST" ]] || fail "Info.plist is missing"
[[ -s "$ICON" ]] || fail "AppIcon.icns is missing or empty"
plutil -lint "$PLIST" >/dev/null

plist_value() {
    /usr/libexec/PlistBuddy -c "Print :$1" "$PLIST"
}

[[ "$(plist_value CFBundleName)" == "Muxy" ]] || fail "CFBundleName must be Muxy"
[[ "$(plist_value CFBundleDisplayName)" == "Muxy" ]] || fail "CFBundleDisplayName must be Muxy"
[[ "$(plist_value CFBundleExecutable)" == "muxy" ]] || fail "CFBundleExecutable must be muxy"
[[ "$(plist_value CFBundleIdentifier)" == "com.muxy.app" ]] || {
    fail "CFBundleIdentifier must be com.muxy.app"
}
[[ "$(plist_value CFBundlePackageType)" == "APPL" ]] || fail "CFBundlePackageType must be APPL"
[[ "$(plist_value CFBundleIconFile)" == "AppIcon" ]] || fail "CFBundleIconFile must be AppIcon"
[[ "$(plist_value LSMinimumSystemVersion)" == "14.0" ]] || {
    fail "LSMinimumSystemVersion must be 14.0"
}

[[ -s "$RESOURCES/ghostty/shell-integration/bash/ghostty.bash" ]] || {
    fail "bundled bash shell integration is missing"
}
[[ -s "$RESOURCES/ghostty/shell-integration/fish/vendor_conf.d/ghostty-shell-integration.fish" ]] || {
    fail "bundled fish shell integration is missing"
}
[[ -s "$RESOURCES/ghostty/shell-integration/zsh/ghostty-integration" ]] || {
    fail "bundled zsh shell integration is missing"
}
[[ -s "$RESOURCES/terminfo/67/ghostty" ]] || fail "bundled ghostty terminfo is missing"
[[ -s "$RESOURCES/terminfo/78/xterm-ghostty" ]] || fail "bundled xterm-ghostty terminfo is missing"
[[ -s "$RESOURCES/ghostty-overrides/muxy-defaults.conf" ]] || {
    fail "bundled Muxy defaults are missing"
}
[[ -s "$RESOURCES/ghostty-overrides/transparent-surface.conf" ]] || {
    fail "bundled transparent surface overrides are missing"
}

icon_check_directory="$(mktemp -d "${TMPDIR:-/tmp}/muxy-icon-check.XXXXXX")"
cleanup() {
    rm -rf "$icon_check_directory"
}
trap cleanup EXIT
iconutil --convert iconset --output "$icon_check_directory/AppIcon.iconset" "$ICON"
for icon_name in \
    icon_16x16.png icon_16x16@2x.png \
    icon_32x32.png icon_32x32@2x.png \
    icon_128x128.png icon_128x128@2x.png \
    icon_256x256.png icon_256x256@2x.png \
    icon_512x512.png icon_512x512@2x.png; do
    [[ -s "$icon_check_directory/AppIcon.iconset/$icon_name" ]] || {
        fail "compiled icon is missing $icon_name"
    }
done

codesign --verify --deep --strict --verbose=2 "$APP_BUNDLE"
signature_details="$(codesign --display --verbose=4 "$APP_BUNDLE" 2>&1)"
grep -q '^Signature=adhoc$' <<<"$signature_details" || fail "bundle signature is not ad-hoc"

printf '==> Verified %s\n' "$APP_BUNDLE"
