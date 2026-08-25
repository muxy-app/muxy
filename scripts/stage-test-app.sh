#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly TEST_BUNDLE_IDENTIFIER="com.muxy.tests"
readonly TEST_EXECUTABLE="MuxyTests"
readonly VERIFICATION_ROOT="$PROJECT_ROOT/target/test-verification"

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

stage_app() {
    local source_app="$1"
    local label="$2"
    local source_plist source_executable_name source_executable destination destination_plist

    [[ "$label" =~ ^[A-Za-z0-9_-]+$ ]] || fail "invalid stage label: $label"
    [[ -d "$source_app" ]] || fail "source app not found: $source_app"
    source_app="$(cd "$(dirname "$source_app")" && pwd -P)/$(basename "$source_app")"
    source_plist="$source_app/Contents/Info.plist"
    [[ -f "$source_plist" ]] || fail "source Info.plist not found: $source_plist"
    source_executable_name="$(plutil -extract CFBundleExecutable raw -o - "$source_plist")"
    source_executable="$source_app/Contents/MacOS/$source_executable_name"
    [[ -x "$source_executable" ]] || fail "source executable not found: $source_executable"

    destination="$VERIFICATION_ROOT/apps/$label/MuxyTests.app"
    [[ "$source_app" != "$destination" ]] || fail "source and destination must differ"
    rm -rf "$destination"
    mkdir -p "$(dirname "$destination")"
    ditto "$source_app" "$destination"

    destination_plist="$destination/Contents/Info.plist"
    if [[ "$source_executable_name" != "$TEST_EXECUTABLE" ]]; then
        mv "$destination/Contents/MacOS/$source_executable_name" \
            "$destination/Contents/MacOS/$TEST_EXECUTABLE"
    fi
    plutil -replace CFBundleExecutable -string "$TEST_EXECUTABLE" "$destination_plist"
    plutil -replace CFBundleIdentifier -string "$TEST_BUNDLE_IDENTIFIER" "$destination_plist"
    for key in CFBundleName CFBundleDisplayName; do
        if plutil -extract "$key" raw -o - "$destination_plist" >/dev/null 2>&1; then
            plutil -replace "$key" -string "$TEST_EXECUTABLE" "$destination_plist"
        else
            plutil -insert "$key" -string "$TEST_EXECUTABLE" "$destination_plist"
        fi
    done
    if plutil -extract LSMultipleInstancesProhibited raw -o - "$destination_plist" \
        >/dev/null 2>&1; then
        plutil -remove LSMultipleInstancesProhibited "$destination_plist"
    fi
    codesign --deep --force --sign - --timestamp=none "$destination" >/dev/null

    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$destination_plist")" == \
        "$TEST_BUNDLE_IDENTIFIER" ]] || fail "staged bundle identifier mismatch"
    [[ "$(plutil -extract CFBundleExecutable raw -o - "$destination_plist")" == \
        "$TEST_EXECUTABLE" ]] || fail "staged executable name mismatch"
    [[ -x "$destination/Contents/MacOS/$TEST_EXECUTABLE" ]] || {
        fail "staged executable not found"
    }
    codesign --verify --deep --strict "$destination"
    printf '%s\n' "$destination"
}

self_test() {
    local fixture_root source source_plist before_plist before_executable staged

    fixture_root="$VERIFICATION_ROOT/self-test"
    source="$fixture_root/source/Muxy.app"
    source_plist="$source/Contents/Info.plist"
    rm -rf "$fixture_root" "$VERIFICATION_ROOT/apps/self-test"
    mkdir -p "$source/Contents/MacOS"
    cp /usr/bin/true "$source/Contents/MacOS/SyntheticMuxy"
    plutil -create xml1 "$source_plist"
    plutil -insert CFBundleExecutable -string SyntheticMuxy "$source_plist"
    plutil -insert CFBundleIdentifier -string com.example.synthetic "$source_plist"
    plutil -insert CFBundleInfoDictionaryVersion -string 6.0 "$source_plist"
    plutil -insert CFBundleName -string Muxy "$source_plist"
    plutil -insert CFBundlePackageType -string APPL "$source_plist"
    plutil -insert CFBundleVersion -string 1 "$source_plist"
    before_plist="$(shasum -a 256 "$source_plist")"
    before_executable="$(shasum -a 256 "$source/Contents/MacOS/SyntheticMuxy")"

    staged="$(stage_app "$source" self-test)"
    [[ "$before_plist" == "$(shasum -a 256 "$source_plist")" ]] || {
        fail "self-test source plist changed"
    }
    [[ "$before_executable" == \
        "$(shasum -a 256 "$source/Contents/MacOS/SyntheticMuxy")" ]] || {
        fail "self-test source executable changed"
    }
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$staged/Contents/Info.plist")" == \
        "$TEST_BUNDLE_IDENTIFIER" ]] || fail "self-test bundle identifier mismatch"
    [[ "$(plutil -extract CFBundleExecutable raw -o - "$staged/Contents/Info.plist")" == \
        "$TEST_EXECUTABLE" ]] || fail "self-test executable mismatch"
    [[ -x "$staged/Contents/MacOS/$TEST_EXECUTABLE" ]] || {
        fail "self-test staged executable is missing"
    }

    rm -rf "$fixture_root" "$VERIFICATION_ROOT/apps/self-test"
    printf 'stage-test-app self-test passed\n'
}

for command_name in codesign ditto plutil shasum; do
    require_command "$command_name"
done

if [[ "${1:-}" == "--self-test" ]]; then
    (($# == 1)) || fail "--self-test accepts no additional arguments"
    self_test
    exit 0
fi

(($# == 2)) || fail "usage: scripts/stage-test-app.sh SOURCE_APP LABEL"
stage_app "$1" "$2"
