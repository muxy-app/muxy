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
readonly APPS_ROOT="$VERIFICATION_ROOT/apps"
readonly OWNER_FILE=".muxy-stage-owner"
readonly PROFILE_FILE=".muxy-stage-profile"

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

reject_symlink_ancestors() {
    local candidate="$1" current="/" part
    local -a parts
    [[ "$candidate" == /* ]] || fail "path must be absolute: $candidate"
    IFS='/' read -r -a parts <<< "${candidate#/}"
    for part in "${parts[@]}"; do
        [[ -n "$part" ]] || continue
        current="${current%/}/$part"
        [[ ! -L "$current" ]] || fail "symlinked path component is not allowed: $current"
    done
}

prepare_roots() {
    reject_symlink_ancestors "$VERIFICATION_ROOT"
    mkdir -p "$APPS_ROOT"
    reject_symlink_ancestors "$APPS_ROOT"
    [[ "$(cd "$APPS_ROOT" && pwd -P)" == "$APPS_ROOT" ]] || fail "verification root changed identity"
}

prepare_destination() {
    local destination="$1" parent marker held
    [[ "$destination" == "$APPS_ROOT/"*/MuxyTests.app ]] || {
        fail "destination is outside the staging root: $destination"
    }
    parent="$(dirname "$destination")"
    [[ "$(dirname "$parent")" == "$APPS_ROOT" ]] || fail "destination nesting is invalid"
    reject_symlink_ancestors "$destination"
    marker="$parent/$OWNER_FILE"
    if [[ -e "$parent" ]]; then
        [[ -d "$parent" && ! -L "$parent" ]] || fail "staging parent is not an owned directory"
        [[ -f "$marker" && ! -L "$marker" ]] || fail "staging ownership marker is missing"
        held="$(cat "$marker")"
        [[ "$held" == "$destination" ]] || fail "staging ownership marker does not match"
    else
        mkdir -p "$parent"
        printf '%s\n' "$destination" > "$marker"
        chmod 0600 "$marker"
    fi
    if [[ -e "$destination" || -L "$destination" ]]; then
        [[ -d "$destination" && ! -L "$destination" ]] || fail "staged destination is not a directory"
        [[ "$(cat "$marker")" == "$destination" ]] || fail "staging ownership changed"
        rm -rf -- "$destination"
    fi
}

stage_app() {
    local source_app="$1" label="$2"
    local source_plist source_executable_name source_executable source_identifier source_profile source_helper expected_identifier destination destination_plist

    [[ "$label" =~ ^[A-Za-z0-9_-]+$ ]] || fail "invalid stage label: $label"
    [[ -d "$source_app" ]] || fail "source app not found: $source_app"
    source_app="$(cd "$(dirname "$source_app")" && pwd -P)/$(basename "$source_app")"
    source_plist="$source_app/Contents/Info.plist"
    [[ -f "$source_plist" ]] || fail "source Info.plist not found: $source_plist"
    source_executable_name="$(plutil -extract CFBundleExecutable raw -o - "$source_plist")"
    source_executable="$source_app/Contents/MacOS/$source_executable_name"
    [[ -x "$source_executable" ]] || fail "source executable not found: $source_executable"

    prepare_roots
    destination="$APPS_ROOT/$label/MuxyTests.app"
    [[ "$source_app" != "$destination" ]] || fail "source and destination must differ"
    prepare_destination "$destination"
    source_identifier="$(plutil -extract CFBundleIdentifier raw -o - "$source_plist")"
    source_helper="$source_app/Contents/MacOS/muxy-session"
    source_profile=unknown
    if [[ -x "$source_helper" ]]; then
        source_profile="$("$source_helper" build-mode)"
        case "$source_profile" in
            debug) expected_identifier=com.muxy.dev ;;
            release) expected_identifier="$(plutil -extract CFBundleIdentifier raw -o - "$PROJECT_ROOT/resources/Info.plist")" ;;
            *) fail "source session helper returned an invalid build mode" ;;
        esac
        [[ "$source_identifier" == "$expected_identifier" ]] || {
            fail "source app and session helper profiles differ"
        }
    fi
    printf '%s\n' "$source_profile" > "$(dirname "$destination")/$PROFILE_FILE"
    chmod 0600 "$(dirname "$destination")/$PROFILE_FILE"
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
    if [[ -e "$destination/Contents/MacOS/muxy-session" ]]; then
        [[ -x "$destination/Contents/MacOS/muxy-session" ]] || {
            fail "staged session helper is not executable"
        }
        codesign --force --sign - --timestamp=none \
            "$destination/Contents/MacOS/muxy-session" >/dev/null
    fi
    codesign --force --sign - --timestamp=none "$destination" >/dev/null

    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$destination_plist")" == \
        "$TEST_BUNDLE_IDENTIFIER" ]] || fail "staged bundle identifier mismatch"
    [[ "$(plutil -extract CFBundleExecutable raw -o - "$destination_plist")" == \
        "$TEST_EXECUTABLE" ]] || fail "staged executable name mismatch"
    [[ -x "$destination/Contents/MacOS/$TEST_EXECUTABLE" ]] || fail "staged executable not found"
    if [[ -e "$destination/Contents/MacOS/muxy-session" ]]; then
        codesign --verify --strict "$destination/Contents/MacOS/muxy-session"
        [[ "$("$destination/Contents/MacOS/muxy-session" build-mode)" == "$source_profile" ]] || {
            fail "staged app and session helper profiles differ"
        }
    fi
    codesign --verify --deep --strict "$destination"
    printf '%s\n' "$destination"
}

self_test() {
    local nonce="$$" fixture_root source source_plist before_plist before_executable staged
    local good_label symlink_label missing_label mismatch_label real_parent outside

    prepare_roots
    fixture_root="$VERIFICATION_ROOT/stage-self-test-$nonce"
    good_label="self-test-$nonce"
    symlink_label="self-test-symlink-$nonce"
    missing_label="self-test-missing-$nonce"
    mismatch_label="self-test-mismatch-$nonce"
    outside="$PROJECT_ROOT/target/stage-self-test-outside-$nonce"
    source="$fixture_root/source/Muxy.app"
    source_plist="$source/Contents/Info.plist"
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

    staged="$(stage_app "$source" "$good_label")"
    [[ "$before_plist" == "$(shasum -a 256 "$source_plist")" ]] || fail "self-test source plist changed"
    [[ "$before_executable" == "$(shasum -a 256 "$source/Contents/MacOS/SyntheticMuxy")" ]] || {
        fail "self-test source executable changed"
    }
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$staged/Contents/Info.plist")" == \
        "$TEST_BUNDLE_IDENTIFIER" ]] || fail "self-test bundle identifier mismatch"
    [[ -x "$staged/Contents/MacOS/$TEST_EXECUTABLE" ]] || fail "self-test executable is missing"
    [[ "$(<"$(dirname "$staged")/$PROFILE_FILE")" == unknown ]] || fail "self-test source profile marker differs"

    real_parent="$fixture_root/real-parent"
    mkdir -p "$real_parent/MuxyTests.app"
    printf 'held\n' > "$real_parent/MuxyTests.app/sentinel"
    ln -s "$real_parent" "$APPS_ROOT/$symlink_label"
    if (prepare_destination "$APPS_ROOT/$symlink_label/MuxyTests.app") >/dev/null 2>&1; then
        fail "self-test accepted a symlinked staging ancestor"
    fi
    [[ -f "$real_parent/MuxyTests.app/sentinel" ]] || fail "symlink rejection deleted outside data"

    mkdir -p "$APPS_ROOT/$missing_label/MuxyTests.app"
    printf 'held\n' > "$APPS_ROOT/$missing_label/MuxyTests.app/sentinel"
    if (prepare_destination "$APPS_ROOT/$missing_label/MuxyTests.app") >/dev/null 2>&1; then
        fail "self-test accepted a missing ownership marker"
    fi
    [[ -f "$APPS_ROOT/$missing_label/MuxyTests.app/sentinel" ]] || fail "missing marker deleted data"

    mkdir -p "$APPS_ROOT/$mismatch_label/MuxyTests.app"
    printf 'other\n' > "$APPS_ROOT/$mismatch_label/$OWNER_FILE"
    printf 'held\n' > "$APPS_ROOT/$mismatch_label/MuxyTests.app/sentinel"
    if (prepare_destination "$APPS_ROOT/$mismatch_label/MuxyTests.app") >/dev/null 2>&1; then
        fail "self-test accepted a mismatched ownership marker"
    fi
    [[ -f "$APPS_ROOT/$mismatch_label/MuxyTests.app/sentinel" ]] || fail "mismatched marker deleted data"

    mkdir -p "$outside/MuxyTests.app"
    printf 'held\n' > "$outside/MuxyTests.app/sentinel"
    if (prepare_destination "$outside/MuxyTests.app") >/dev/null 2>&1; then
        fail "self-test accepted an outside-root destination"
    fi
    [[ -f "$outside/MuxyTests.app/sentinel" ]] || fail "outside-root rejection deleted data"

    rm -rf -- "$fixture_root" "$APPS_ROOT/${good_label:?}" "$APPS_ROOT/${missing_label:?}" \
        "$APPS_ROOT/${mismatch_label:?}" "$outside"
    rm -- "$APPS_ROOT/$symlink_label"
    printf 'stage-test-app self-test passed\n'
}

for command_name in cat codesign ditto plutil shasum; do
    require_command "$command_name"
done

if [[ "${1:-}" == "--self-test" ]]; then
    (($# == 1)) || fail "--self-test accepts no additional arguments"
    self_test
    exit 0
fi

(($# == 2)) || fail "usage: scripts/stage-test-app.sh SOURCE_APP LABEL"
stage_app "$1" "$2"
