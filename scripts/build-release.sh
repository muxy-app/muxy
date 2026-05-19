#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$PROJECT_ROOT/build"

ARCH=""
VERSION=""
SIGN_IDENTITY=""
SPARKLE_PUBLIC_KEY=""
SPARKLE_FEED_URL=""
SENTRY_DSN="${SENTRY_DSN:-}"

AUTO_SIGN=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --arch)
            ARCH="$2"
            shift 2
            ;;
        --version)
            VERSION="$2"
            shift 2
            ;;
        --sign)
            AUTO_SIGN=true
            shift
            ;;
        --sign-identity)
            SIGN_IDENTITY="$2"
            shift 2
            ;;
        --sparkle-public-key)
            SPARKLE_PUBLIC_KEY="$2"
            shift 2
            ;;
        --sparkle-feed-url)
            SPARKLE_FEED_URL="$2"
            shift 2
            ;;
        --sentry-dsn)
            SENTRY_DSN="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

if [[ -z "$ARCH" || -z "$VERSION" ]]; then
    echo "Usage: $0 --arch <arm64|x86_64> --version <X.Y.Z[-beta.N]> [--sign | --sign-identity <identity>] [--sparkle-public-key <key>] [--sparkle-feed-url <url>] [--sentry-dsn <dsn>]"
    exit 1
fi

if [[ "$AUTO_SIGN" == "true" && -n "$SIGN_IDENTITY" ]]; then
    echo "Error: --sign and --sign-identity are mutually exclusive"
    exit 1
fi

find_signing_identity() {
    local certificate_prefix="$1"

    security find-identity -v -p codesigning | awk -v prefix="$certificate_prefix" '
        index($0, "\"" prefix) && hash == "" {
            hash = $2
            name = $0
            sub(/^[^"]*"/, "", name)
            sub(/".*$/, "", name)
        }
        END {
            if (hash != "") {
                print hash "\t" name
            }
        }
    '
}

if [[ "$AUTO_SIGN" == "true" ]]; then
    SELECTED_IDENTITY=$(find_signing_identity "Apple Development:")
    if [[ -z "$SELECTED_IDENTITY" ]]; then
        SELECTED_IDENTITY=$(find_signing_identity "Developer ID Application:")
    fi
    if [[ -z "$SELECTED_IDENTITY" ]]; then
        echo "Error: --sign requires an 'Apple Development' or 'Developer ID Application' identity in the keychain"
        exit 1
    fi
    SIGN_IDENTITY="${SELECTED_IDENTITY%%$'\t'*}"
    SIGN_IDENTITY_LABEL="${SELECTED_IDENTITY#*$'\t'}"
    echo "==> Auto-selected signing identity: $SIGN_IDENTITY_LABEL ($SIGN_IDENTITY)"
fi

if [[ "$ARCH" != "arm64" && "$ARCH" != "x86_64" ]]; then
    echo "Error: arch must be arm64 or x86_64"
    exit 1
fi

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-beta\.[0-9]+)?$ ]]; then
    echo "Error: version must be X.Y.Z or X.Y.Z-beta.N"
    exit 1
fi

TRIPLE="${ARCH}-apple-macosx14.0"
BUILD_NUMBER=$(git -C "$PROJECT_ROOT" rev-list --count HEAD)
APP_BUNDLE="$BUILD_DIR/Muxy.app"
DSYM_BUNDLE="$BUILD_DIR/Muxy-${VERSION}-${ARCH}.app.dSYM"
DMG_NAME="Muxy-${VERSION}-${ARCH}.dmg"

rm -rf "$APP_BUNDLE"
rm -rf "$DSYM_BUNDLE"

echo "==> Building for $ARCH ($TRIPLE)"
cd "$PROJECT_ROOT"
swift build -c release --triple "$TRIPLE"

SPM_BUILD_DIR=$(swift build -c release --triple "$TRIPLE" --show-bin-path)

echo "==> Creating app bundle"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"

cp "$SPM_BUILD_DIR/Muxy" "$APP_BUNDLE/Contents/MacOS/Muxy"
install_name_tool -add_rpath @executable_path/../Frameworks "$APP_BUNDLE/Contents/MacOS/Muxy"

echo "==> Generating dSYM"
xcrun dsymutil "$APP_BUNDLE/Contents/MacOS/Muxy" -o "$DSYM_BUNDLE"

echo "==> Stripping local and debug symbols"
strip -Sx "$APP_BUNDLE/Contents/MacOS/Muxy"

if [[ -d "$SPM_BUILD_DIR/Muxy_Muxy.bundle" ]]; then
    cp -R "$SPM_BUILD_DIR/Muxy_Muxy.bundle" "$APP_BUNDLE/Contents/Resources/Muxy_Muxy.bundle"
fi

cp "$PROJECT_ROOT/Muxy/Info.plist" "$APP_BUNDLE/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$APP_BUNDLE/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $BUILD_NUMBER" "$APP_BUNDLE/Contents/Info.plist"

if [[ -n "$SENTRY_DSN" ]]; then
    echo "==> Injecting Sentry DSN into Info.plist"
    /usr/libexec/PlistBuddy -c "Set :SentryDSN $SENTRY_DSN" "$APP_BUNDLE/Contents/Info.plist"
fi

echo "==> Generating app icon"
"$SCRIPT_DIR/create-icns.sh" "$APP_BUNDLE/Contents/Resources/AppIcon.icns"

echo "==> Embedding Sparkle.framework"
SPARKLE_FRAMEWORK="$PROJECT_ROOT/.build/artifacts/sparkle/Sparkle/Sparkle.xcframework/macos-arm64_x86_64/Sparkle.framework"
if [[ ! -d "$SPARKLE_FRAMEWORK" ]]; then
    echo "Error: Sparkle.framework not found at $SPARKLE_FRAMEWORK"
    exit 1
fi
mkdir -p "$APP_BUNDLE/Contents/Frameworks"
cp -R "$SPARKLE_FRAMEWORK" "$APP_BUNDLE/Contents/Frameworks/Sparkle.framework"

if [[ -n "$SPARKLE_PUBLIC_KEY" ]]; then
    echo "==> Injecting Sparkle keys into Info.plist"
    APP_PLIST="$APP_BUNDLE/Contents/Info.plist"
    /usr/libexec/PlistBuddy -c "Add :SUPublicEDKey string $SPARKLE_PUBLIC_KEY" "$APP_PLIST"
    if [[ -n "$SPARKLE_FEED_URL" ]]; then
        /usr/libexec/PlistBuddy -c "Add :SUFeedURL string $SPARKLE_FEED_URL" "$APP_PLIST"
    fi
fi

if [[ -n "$SIGN_IDENTITY" ]]; then
    SPARKLE_DIR="$APP_BUNDLE/Contents/Frameworks/Sparkle.framework"

    echo "==> Signing Sparkle.framework (inside-out)"
    /usr/bin/codesign --force --options runtime --preserve-metadata=entitlements \
        --sign "$SIGN_IDENTITY" \
        "$SPARKLE_DIR/Versions/B/XPCServices/Installer.xpc"

    /usr/bin/codesign --force --options runtime --preserve-metadata=entitlements \
        --sign "$SIGN_IDENTITY" \
        "$SPARKLE_DIR/Versions/B/XPCServices/Downloader.xpc"

    /usr/bin/codesign --force --options runtime --preserve-metadata=entitlements \
        --sign "$SIGN_IDENTITY" \
        "$SPARKLE_DIR/Versions/B/Updater.app"

    /usr/bin/codesign --force --options runtime --preserve-metadata=entitlements \
        --sign "$SIGN_IDENTITY" \
        "$SPARKLE_DIR/Versions/B/Autoupdate"

    /usr/bin/codesign --force --options runtime \
        --sign "$SIGN_IDENTITY" \
        "$SPARKLE_DIR"

    echo "==> Signing embedded resource binaries"
    while IFS= read -r -d '' binary; do
        if file "$binary" | grep -q "Mach-O"; then
            /usr/bin/codesign --force --options runtime --timestamp \
                --sign "$SIGN_IDENTITY" \
                "$binary"
        fi
    done < <(find "$APP_BUNDLE/Contents/Resources" -type f -perm -u+x -print0)

    echo "==> Signing app bundle"
    /usr/bin/codesign --force --options runtime --timestamp \
        --entitlements "$PROJECT_ROOT/Muxy/Muxy.entitlements" \
        --sign "$SIGN_IDENTITY" \
        "$APP_BUNDLE"
fi

echo "==> Creating DMG"
if ! command -v create-dmg &> /dev/null; then
    echo "Error: create-dmg not found. Install with: pnpm add --global create-dmg"
    exit 1
fi

cd "$BUILD_DIR"
rm -f "$BUILD_DIR/$DMG_NAME"
create-dmg "$APP_BUNDLE" "$BUILD_DIR"

GENERATED_DMG=$(find "$BUILD_DIR" -maxdepth 1 -name "Muxy*.dmg" -not -name "$DMG_NAME" | head -1)
if [[ -n "$GENERATED_DMG" ]]; then
    mv "$GENERATED_DMG" "$BUILD_DIR/$DMG_NAME"
fi

if [[ ! -f "$BUILD_DIR/$DMG_NAME" ]]; then
    echo "Error: expected DMG was not created at $BUILD_DIR/$DMG_NAME"
    exit 1
fi

if [[ -n "$SIGN_IDENTITY" && -f "$BUILD_DIR/$DMG_NAME" ]]; then
    echo "==> Signing DMG"
    /usr/bin/codesign --force --sign "$SIGN_IDENTITY" "$BUILD_DIR/$DMG_NAME"
fi

echo "==> Done: $BUILD_DIR/$DMG_NAME"
