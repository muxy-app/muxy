#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly RELEASE_REPOSITORY="muxy-app/ghostty"
readonly RELEASE_TAG="build-2026-04-29"
readonly XCFRAMEWORK_ASSET="GhosttyKit.xcframework.tar.gz"
readonly RESOURCES_ASSET="GhosttyKit-resources.tar.gz"
readonly XCFRAMEWORK_SHA256="8f30a557470383e21f1dcfcf0b8278a3a08eb6ca5a886c32238425fa8b43bf8e"
readonly RESOURCES_SHA256="877081c96cf4bc97fa7a15c397ad285f6e5c544ec43778b0903011eff9a74ca2"
readonly XCFRAMEWORK_DIR="$PROJECT_ROOT/vendor/GhosttyKit.xcframework"
readonly GHOSTTY_RESOURCES_DIR="$PROJECT_ROOT/resources/ghostty"
readonly TERMINFO_DIR="$PROJECT_ROOT/resources/terminfo"
readonly INSTALL_STAMP_NAME=".muxy-release"

xcframework_archive=""
resources_archive=""
temporary_directory=""

usage() {
    cat <<'USAGE'
Usage: scripts/setup.sh [options]

Install the pinned GhosttyKit framework and runtime resources.

Options:
  --xcframework-archive PATH  Use a local GhosttyKit.xcframework.tar.gz.
  --resources-archive PATH    Use a local GhosttyKit-resources.tar.gz.
  -h, --help                  Show this help.

Pass both archive options for a fully offline installation. Local archives are
subject to the same pinned SHA256 checks as downloaded release assets.
USAGE
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

canonical_file() {
    local path="$1"
    local directory

    [[ -f "$path" ]] || fail "archive not found: $path"
    directory="$(cd "$(dirname "$path")" && pwd -P)"
    printf '%s/%s\n' "$directory" "$(basename "$path")"
}

cleanup() {
    if [[ -n "$temporary_directory" && -d "$temporary_directory" ]]; then
        rm -rf "$temporary_directory"
    fi
}
trap cleanup EXIT

while (($# > 0)); do
    case "$1" in
        --xcframework-archive)
            (($# >= 2)) || fail "$1 requires a path"
            xcframework_archive="$(canonical_file "$2")"
            shift 2
            ;;
        --resources-archive)
            (($# >= 2)) || fail "$1 requires a path"
            resources_archive="$(canonical_file "$2")"
            shift 2
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            usage >&2
            fail "unknown argument: $1"
            ;;
    esac
done

[[ "$(uname -s)" == "Darwin" ]] || fail "GhosttyKit setup requires macOS"
for command_name in xcode-select xcodebuild xcrun gh tar shasum plutil; do
    require_command "$command_name"
done

DEVELOPER_DIR="$(xcode-select -p)"
readonly DEVELOPER_DIR
[[ -d "$DEVELOPER_DIR/Platforms/MacOSX.platform" ]] || {
    fail "full Xcode is required; xcode-select points to $DEVELOPER_DIR"
}
export DEVELOPER_DIR

xcodebuild -version >/dev/null
MACOS_SDK_PATH="$(xcrun --sdk macosx --show-sdk-path)"
readonly MACOS_SDK_PATH
[[ -d "$MACOS_SDK_PATH" ]] || fail "xcrun returned a missing macOS SDK: $MACOS_SDK_PATH"
xcrun --sdk macosx --find clang >/dev/null
xcrun --sdk macosx --find lipo >/dev/null
gh --version >/dev/null
tar --version >/dev/null 2>&1 || true
shasum --version >/dev/null 2>&1 || true

stamp_value() {
    printf '%s %s' "$RELEASE_TAG" "$1"
}

validate_xcframework_contents() {
    local directory="$1"
    local slice="$directory/macos-arm64_x86_64"

    [[ -f "$directory/Info.plist" ]] || return 1
    [[ -s "$slice/Headers/ghostty.h" ]] || return 1
    [[ -s "$slice/ghostty-internal.a" ]] || return 1
    plutil -lint "$directory/Info.plist" >/dev/null || return 1
    xcrun --sdk macosx lipo "$slice/ghostty-internal.a" -verify_arch arm64 x86_64 \
        >/dev/null 2>&1 || return 1
}

validate_xcframework_install() {
    validate_xcframework_contents "$XCFRAMEWORK_DIR" || return 1
    [[ -f "$XCFRAMEWORK_DIR/$INSTALL_STAMP_NAME" ]] || return 1
    [[ "$(<"$XCFRAMEWORK_DIR/$INSTALL_STAMP_NAME")" == \
        "$(stamp_value "$XCFRAMEWORK_SHA256")" ]]
}

validate_resources_contents() {
    local ghostty_directory="$1"
    local terminfo_directory="$2"

    [[ -s "$ghostty_directory/shell-integration/bash/ghostty.bash" ]] || return 1
    [[ -s "$ghostty_directory/shell-integration/fish/vendor_conf.d/ghostty-shell-integration.fish" ]] || return 1
    [[ -s "$ghostty_directory/shell-integration/zsh/ghostty-integration" ]] || return 1
    [[ -s "$terminfo_directory/67/ghostty" ]] || return 1
    [[ -s "$terminfo_directory/78/xterm-ghostty" ]] || return 1
}

validate_resources_install() {
    validate_resources_contents "$GHOSTTY_RESOURCES_DIR" "$TERMINFO_DIR" || return 1
    [[ -f "$GHOSTTY_RESOURCES_DIR/$INSTALL_STAMP_NAME" ]] || return 1
    [[ -f "$TERMINFO_DIR/$INSTALL_STAMP_NAME" ]] || return 1
    [[ "$(<"$GHOSTTY_RESOURCES_DIR/$INSTALL_STAMP_NAME")" == \
        "$(stamp_value "$RESOURCES_SHA256")" ]] || return 1
    [[ "$(<"$TERMINFO_DIR/$INSTALL_STAMP_NAME")" == \
        "$(stamp_value "$RESOURCES_SHA256")" ]]
}

verify_sha256() {
    local archive="$1"
    local expected="$2"
    local actual

    actual="$(shasum -a 256 "$archive")"
    actual="${actual%% *}"
    [[ "$actual" == "$expected" ]] || {
        fail "SHA256 mismatch for $archive (expected $expected, got $actual)"
    }
}

validate_archive_paths() {
    local archive="$1"
    local entry

    tar tzf "$archive" >/dev/null
    while IFS= read -r entry; do
        case "$entry" in
            /* | ../* | */../* | */..)
                fail "archive contains an unsafe path: $entry"
                ;;
        esac
    done < <(tar tzf "$archive")
}

download_asset() {
    local asset="$1"
    local destination="$temporary_directory/downloads"

    mkdir -p "$destination"
    printf '==> Downloading %s from %s %s\n' \
        "$asset" "$RELEASE_REPOSITORY" "$RELEASE_TAG" >&2
    gh release download "$RELEASE_TAG" \
        --repo "$RELEASE_REPOSITORY" \
        --pattern "$asset" \
        --dir "$destination" \
        --clobber >&2
    printf '%s/%s\n' "$destination" "$asset"
}

install_xcframework() {
    local archive="$1"
    local staging_directory="$temporary_directory/xcframework"
    local staged_framework="$staging_directory/GhosttyKit.xcframework"

    verify_sha256 "$archive" "$XCFRAMEWORK_SHA256"
    validate_archive_paths "$archive"
    mkdir -p "$staging_directory"
    tar xzf "$archive" -C "$staging_directory"
    validate_xcframework_contents "$staged_framework" || {
        fail "xcframework archive does not contain a valid GhosttyKit.xcframework"
    }
    printf '%s\n' "$(stamp_value "$XCFRAMEWORK_SHA256")" > \
        "$staged_framework/$INSTALL_STAMP_NAME"

    mkdir -p "$PROJECT_ROOT/vendor"
    rm -rf "$XCFRAMEWORK_DIR"
    mv "$staged_framework" "$XCFRAMEWORK_DIR"
    validate_xcframework_install || fail "installed xcframework failed validation"
}

install_resources() {
    local archive="$1"
    local staging_directory="$temporary_directory/resources"
    local staged_ghostty="$staging_directory/ghostty"
    local staged_terminfo="$staging_directory/terminfo"

    verify_sha256 "$archive" "$RESOURCES_SHA256"
    validate_archive_paths "$archive"
    mkdir -p "$staging_directory"
    tar xzf "$archive" -C "$staging_directory"
    validate_resources_contents "$staged_ghostty" "$staged_terminfo" || {
        fail "resources archive does not contain the required ghostty and terminfo trees"
    }
    printf '%s\n' "$(stamp_value "$RESOURCES_SHA256")" > \
        "$staged_ghostty/$INSTALL_STAMP_NAME"
    printf '%s\n' "$(stamp_value "$RESOURCES_SHA256")" > \
        "$staged_terminfo/$INSTALL_STAMP_NAME"

    rm -rf "$GHOSTTY_RESOURCES_DIR" "$TERMINFO_DIR"
    mv "$staged_ghostty" "$GHOSTTY_RESOURCES_DIR"
    mv "$staged_terminfo" "$TERMINFO_DIR"
    validate_resources_install || fail "installed resources failed validation"
}

need_xcframework=true
if validate_xcframework_install >/dev/null 2>&1; then
    need_xcframework=false
fi
need_resources=true
if validate_resources_install >/dev/null 2>&1; then
    need_resources=false
fi

if [[ "$need_xcframework" == false && "$need_resources" == false ]]; then
    printf '==> GhosttyKit %s is already installed and valid\n' "$RELEASE_TAG"
    exit 0
fi

temporary_directory="$(mktemp -d "${TMPDIR:-/tmp}/muxy-setup.XXXXXX")"

if [[ "$need_xcframework" == true ]]; then
    if [[ -z "$xcframework_archive" ]]; then
        xcframework_archive="$(download_asset "$XCFRAMEWORK_ASSET")"
    fi
    printf '==> Installing GhosttyKit.xcframework\n'
    install_xcframework "$xcframework_archive"
else
    printf '==> Existing GhosttyKit.xcframework is valid\n'
fi

if [[ "$need_resources" == true ]]; then
    if [[ -z "$resources_archive" ]]; then
        resources_archive="$(download_asset "$RESOURCES_ASSET")"
    fi
    printf '==> Installing Ghostty runtime resources\n'
    install_resources "$resources_archive"
else
    printf '==> Existing Ghostty runtime resources are valid\n'
fi

printf '==> GhosttyKit %s setup complete\n' "$RELEASE_TAG"
