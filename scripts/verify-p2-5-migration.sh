#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly VERIFICATION_ROOT="$PROJECT_ROOT/target/test-verification/p2-5-migration"
readonly OWNERSHIP_MARKER="$VERIFICATION_ROOT/.muxy-p2-5-verifier"
readonly OWNERSHIP_VALUE="muxy-p2-5-migration-verifier-v1"
readonly REAL_HOME="$HOME"
readonly REAL_SWIFT_SOURCE="$REAL_HOME/Library/Application Support/Muxy"
APP_PID=""

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

state_value() {
    local state="$1"
    local key="$2"
    plutil -extract "$key" raw -o - "$state"
}

state_array_contains() {
    local state="$1"
    local key="$2"
    local value="$3"
    plutil -extract "$key" xml1 -o - "$state" | grep -Fq "<string>$value</string>"
}

source_hash() {
    local source="$1"
    if [[ ! -d "$source" ]]; then
        printf 'missing\n' | shasum -a 256 | cut -d ' ' -f 1
        return
    fi
    find "$source" -type f -print | LC_ALL=C sort | while IFS= read -r path; do
        printf '%s\n' "${path#"$source"/}"
        shasum -a 256 "$path"
    done | shasum -a 256 | cut -d ' ' -f 1
}

assert_verification_ancestors() {
    local path
    for path in "$PROJECT_ROOT/target" "$PROJECT_ROOT/target/test-verification" "$VERIFICATION_ROOT"; do
        [[ ! -L "$path" ]] || return 1
        [[ ! -e "$path" || -d "$path" ]] || return 1
    done
    if [[ -d "$PROJECT_ROOT/target" ]]; then
        [[ "$(cd "$PROJECT_ROOT/target" && pwd -P)" == "$PROJECT_ROOT/target" ]] || return 1
    fi
    if [[ -d "$PROJECT_ROOT/target/test-verification" ]]; then
        [[ "$(cd "$PROJECT_ROOT/target/test-verification" && pwd -P)" == "$PROJECT_ROOT/target/test-verification" ]] || return 1
    fi
}

path_is_safe_under_verification_root() {
    local path="$1"
    [[ "$path" == "$VERIFICATION_ROOT"/* ]] || return 1
    [[ "$path" != *"/../"* && "$path" != */.. && "$path" != *"/./"* && "$path" != */. ]] || return 1
    local relative="${path#"$VERIFICATION_ROOT"/}"
    local component cursor="$VERIFICATION_ROOT"
    local old_ifs="$IFS"
    IFS='/'
    read -r -a components <<< "$relative"
    IFS="$old_ifs"
    for component in "${components[@]}"; do
        [[ -n "$component" ]] || return 1
        cursor="$cursor/$component"
        [[ ! -L "$cursor" ]] || return 1
    done
}

ownership_marker_is_valid() {
    local marker="$1"
    [[ ! -L "$marker" && -f "$marker" ]] || return 1
    [[ "$(cat "$marker")" == "$OWNERSHIP_VALUE" ]]
}

ensure_verification_root() {
    assert_verification_ancestors || fail "verification path has an unsafe ancestor"
    mkdir -p "$PROJECT_ROOT/target/test-verification"
    assert_verification_ancestors || fail "verification path changed while preparing it"
    if [[ -e "$VERIFICATION_ROOT" ]]; then
        ownership_marker_is_valid "$OWNERSHIP_MARKER" || fail "verification root is not owned by this verifier"
    else
        mkdir "$VERIFICATION_ROOT"
        printf '%s\n' "$OWNERSHIP_VALUE" > "$OWNERSHIP_MARKER"
    fi
}

reset_verification_root() {
    ensure_verification_root
    assert_verification_ancestors || fail "verification path became unsafe before reset"
    ownership_marker_is_valid "$OWNERSHIP_MARKER" || fail "verification ownership marker changed"
    rm -rf "$VERIFICATION_ROOT"
    mkdir "$VERIFICATION_ROOT"
    printf '%s\n' "$OWNERSHIP_VALUE" > "$OWNERSHIP_MARKER"
}

assert_safe_environment() {
    local home="$1"
    local root="$2"
    local source="$3"
    local defaults="$4"
    local identifier="$5"
    assert_verification_ancestors || return 1
    path_is_safe_under_verification_root "$home" || return 1
    path_is_safe_under_verification_root "$root" || return 1
    path_is_safe_under_verification_root "$source" || return 1
    path_is_safe_under_verification_root "$defaults" || return 1
    [[ "$root" != "$source" ]] || return 1
    [[ "$root" != "$REAL_HOME/.muxy" ]] || return 1
    [[ "$root" != "$REAL_HOME/.muxy-dev" ]] || return 1
    [[ "$source" != "$REAL_SWIFT_SOURCE" ]] || return 1
    [[ "$identifier" == "com.muxy.tests" ]] || return 1
}

self_test() {
    ensure_verification_root
    local root="$VERIFICATION_ROOT/self-test"
    local home="$root/home"
    local state="$root/state"
    local source="$root/swift"
    local defaults="$root/defaults.json"
    rm -rf "$root"
    mkdir -p "$home" "$state" "$source"
    printf '{}\n' > "$defaults"
    assert_safe_environment "$home" "$state" "$source" "$defaults" "com.muxy.tests" || {
        fail "self-test rejected a safe environment"
    }
    if assert_safe_environment "$REAL_HOME" "$state" "$source" "$defaults" "com.muxy.tests"; then
        fail "self-test accepted the real HOME"
    fi
    if assert_safe_environment "$home" "$REAL_HOME/.muxy" "$source" "$defaults" "com.muxy.tests"; then
        fail "self-test accepted the real release root"
    fi
    if assert_safe_environment "$home" "$state" "$REAL_SWIFT_SOURCE" "$defaults" "com.muxy.tests"; then
        fail "self-test accepted the real Swift source"
    fi
    if assert_safe_environment "$home" "$state" "$source" "$REAL_HOME/defaults.json" "com.muxy.tests"; then
        fail "self-test accepted defaults outside the isolated root"
    fi
    if assert_safe_environment "$home" "$state" "$source" "$defaults" "com.muxy.app"; then
        fail "self-test accepted the production bundle identity"
    fi
    mkdir "$root/linked-target"
    ln -s "$root/linked-target" "$root/linked-state"
    if assert_safe_environment "$home" "$root/linked-state" "$source" "$defaults" "com.muxy.tests"; then
        fail "self-test accepted a symlinked state ancestor"
    fi
    printf '%s\n' "$OWNERSHIP_VALUE" > "$root/marker-target"
    ln -s "$root/marker-target" "$root/linked-marker"
    if ownership_marker_is_valid "$root/linked-marker"; then
        fail "self-test accepted a symlinked ownership marker"
    fi
    rm -f "$root/linked-marker"
    rm -f "$root/linked-state"
    rm -rf "$root"
    printf 'P2.5 migration verifier self-test passed\n'
}

launch_app() {
    local executable="$1"
    local home="$2"
    local root="$3"
    local source="$4"
    local defaults="$5"
    local failure_path="$6"
    local log="$7"
    mkdir -p "$home" "$root" "$home/tmp"
    env \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$home/tmp" \
        MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$root" \
        MUXY_TEST_SWIFT_APPLICATION_SUPPORT_DIRECTORY="$source" \
        MUXY_TEST_SWIFT_DEFAULTS_PATH="$defaults" \
        MUXY_TEST_MIGRATION_FAIL_PATH="$failure_path" \
        "$executable" >"$log" 2>&1 &
    APP_PID=$!
}

wait_for_outcome() {
    local state="$1"
    local expected="$2"
    local attempt
    for ((attempt = 0; attempt < 300; attempt++)); do
        if [[ -f "$state" ]] && [[ "$(state_value "$state" outcome 2>/dev/null || true)" == "$expected" ]]; then
            return 0
        fi
        sleep 0.05
    done
    return 1
}

wait_for_socket() {
    local socket="$1"
    local attempt
    for ((attempt = 0; attempt < 300; attempt++)); do
        [[ -S "$socket" ]] && return 0
        sleep 0.05
    done
    return 1
}

stop_app() {
    local socket="$1"
    if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
        kill -TERM "$APP_PID" 2>/dev/null || true
        wait "$APP_PID" 2>/dev/null || true
    fi
    APP_PID=""
    rm -f "$socket"
}

expect_startup_failure() {
    local state="$1"
    local expected="$2"
    wait_for_outcome "$state" "$expected" || fail "timed out waiting for $expected migration state"
    local attempt
    for ((attempt = 0; attempt < 100; attempt++)); do
        if ! kill -0 "$APP_PID" 2>/dev/null; then
            set +e
            wait "$APP_PID"
            local status=$?
            set -e
            APP_PID=""
            [[ $status -ne 0 ]] || fail "migration failure launch exited successfully"
            return
        fi
        sleep 0.05
    done
    stop_app ""
    fail "migration failure launch did not exit"
}

verify_staged_identity() {
    local app="$1"
    local plist="$app/Contents/Info.plist"
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$plist")" == "com.muxy.tests" ]] || {
        fail "staged bundle identifier is not com.muxy.tests"
    }
    [[ "$(plutil -extract CFBundleExecutable raw -o - "$plist")" == "MuxyTests" ]] || {
        fail "staged executable is not MuxyTests"
    }
}

run_acceptance() {
    local debug_source="$PROJECT_ROOT/target/debug/Muxy.app"
    local release_source="$PROJECT_ROOT/target/release/Muxy.app"
    local debug_app release_app debug_executable release_executable
    local home defaults source_hash_before source_hash_after state socket

    reset_verification_root
    "$SCRIPT_DIR/build-app.sh" debug
    "$SCRIPT_DIR/build-app.sh" release
    "$SCRIPT_DIR/verify-bundle.sh" "$debug_source" debug
    "$SCRIPT_DIR/verify-bundle.sh" "$release_source" release
    debug_app="$("$SCRIPT_DIR/stage-test-app.sh" "$debug_source" p2-5-debug)"
    release_app="$("$SCRIPT_DIR/stage-test-app.sh" "$release_source" p2-5-release)"
    verify_staged_identity "$debug_app"
    verify_staged_identity "$release_app"
    debug_executable="$debug_app/Contents/MacOS/MuxyTests"
    release_executable="$release_app/Contents/MacOS/MuxyTests"

    home="$VERIFICATION_ROOT/home"
    defaults="$VERIFICATION_ROOT/defaults.json"
    mkdir -p "$home"
    cat > "$defaults" <<'JSON'
{
  "muxy.activeProjectID": "swift-project",
  "muxy.sidebarExpanded": true,
  "muxy.settings.selectedRoute": "builtin.appearance",
  "muxy.unknown": "excluded"
}
JSON

    local debug_root="$VERIFICATION_ROOT/debug-root"
    local debug_source_fixture="$VERIFICATION_ROOT/debug-swift-source"
    mkdir -p "$debug_source_fixture"
    printf 'debug must not inspect this\n' > "$debug_source_fixture/sentinel"
    source_hash_before="$(source_hash "$debug_source_fixture")"
    assert_safe_environment "$home" "$debug_root" "$debug_source_fixture" "$defaults" "com.muxy.tests" || {
        fail "debug launch environment is unsafe"
    }
    socket="$debug_root/muxy-dev.sock"
    rm -f "$socket"
    launch_app "$debug_executable" "$home" "$debug_root" "$debug_source_fixture" "$defaults" "sentinel" "$VERIFICATION_ROOT/debug.log"
    wait_for_socket "$socket" || fail "debug staged launch did not start its development socket"
    [[ ! -e "$debug_root/swift-profile-migration.json" ]] || fail "debug inspected the Swift source"
    source_hash_after="$(source_hash "$debug_source_fixture")"
    [[ "$source_hash_before" == "$source_hash_after" ]] || fail "debug changed the Swift source"
    stop_app "$socket"

    local success_root="$VERIFICATION_ROOT/success-root"
    local success_source="$VERIFICATION_ROOT/success-swift-source"
    mkdir -p "$success_source/logos/imported" "$success_source/sessions" "$success_root/extensions"
    printf '[ ]\n' > "$success_source/projects.json"
    printf '{"muxy.showTips":false,"muxy.app.blur":42}\n' > "$success_source/settings.json"
    printf 'logo bytes\n' > "$success_source/logos/imported/logo.txt"
    printf 'excluded runtime\n' > "$success_source/sessions/runtime"
    printf '[]\n' > "$success_root/projects.json"
    printf 'extension\n' > "$success_root/extensions/keep.txt"
    source_hash_before="$(source_hash "$success_source")"
    assert_safe_environment "$home" "$success_root" "$success_source" "$defaults" "com.muxy.tests" || {
        fail "success launch environment is unsafe"
    }
    state="$success_root/swift-profile-migration.json"
    socket="$success_root/muxy.sock"
    rm -f "$socket"
    launch_app "$release_executable" "$home" "$success_root" "$success_source" "$defaults" "" "$VERIFICATION_ROOT/success.log"
    wait_for_outcome "$state" completed || fail "release migration did not complete"
    wait_for_socket "$socket" || fail "release launch did not start after migration"
    [[ "$(cat "$success_root/projects.json")" == "[]" ]] || fail "Swift replaced an existing destination"
    [[ "$(cat "$success_root/logos/imported/logo.txt")" == "logo bytes" ]] || fail "missing allowlisted data was not imported"
    [[ "$(cat "$success_root/extensions/keep.txt")" == "extension" ]] || fail "existing extension data changed"
    [[ ! -e "$success_root/sessions" ]] || fail "excluded runtime data was imported"
    state_array_contains "$state" existing_paths "projects.json" || fail "existing path was not reported"
    state_array_contains "$state" imported_paths "logos/imported/logo.txt" || fail "imported path was not reported"
    state_array_contains "$state" missing_paths "ui-scale.json" || fail "missing path was not reported"
    [[ "$(plutil -extract 'muxy\.app\.blur' raw -o - "$success_root/settings.json")" == "42" ]] || fail "imported settings were not preserved"
    [[ "$(state_value "$state" defaults_import_completed)" == "true" ]] || fail "defaults import was not reported complete"
    [[ "$(plutil -extract 'muxy\.activeProjectID' raw -o - "$success_root/preferences.json")" == "swift-project" ]] || fail "allowed defaults were not imported"
    if plutil -extract 'muxy\.unknown' raw -o - "$success_root/preferences.json" >/dev/null 2>&1; then
        fail "unapproved defaults were imported"
    fi
    source_hash_after="$(source_hash "$success_source")"
    [[ "$source_hash_before" == "$source_hash_after" ]] || fail "successful migration changed the Swift source"
    stop_app "$socket"
    rm -rf "$success_source"
    launch_app "$release_executable" "$home" "$success_root" "$success_source" "$defaults" "" "$VERIFICATION_ROOT/success-relaunch.log"
    wait_for_socket "$socket" || fail "completed migration inspected the removed source"
    [[ "$(state_value "$state" outcome)" == "completed" ]] || fail "completed migration state changed"
    [[ "$(state_value "$state" attempt_count)" == "1" ]] || fail "completed migration retried"
    stop_app "$socket"

    local missing_root="$VERIFICATION_ROOT/missing-root"
    local missing_source="$VERIFICATION_ROOT/missing-swift-source"
    state="$missing_root/swift-profile-migration.json"
    socket="$missing_root/muxy.sock"
    assert_safe_environment "$home" "$missing_root" "$missing_source" "$defaults" "com.muxy.tests" || {
        fail "missing-source launch environment is unsafe"
    }
    launch_app "$release_executable" "$home" "$missing_root" "$missing_source" "$defaults" "" "$VERIFICATION_ROOT/missing.log"
    wait_for_outcome "$state" source_missing || fail "missing source was not terminal"
    wait_for_socket "$socket" || fail "missing-source launch did not continue"
    stop_app "$socket"
    mkdir -p "$missing_source"
    printf 'late source\n' > "$missing_source/projects.json"
    launch_app "$release_executable" "$home" "$missing_root" "$missing_source" "$defaults" "" "$VERIFICATION_ROOT/missing-relaunch.log"
    wait_for_socket "$socket" || fail "source_missing state inspected the late source"
    [[ "$(state_value "$state" outcome)" == "source_missing" ]] || fail "source_missing state changed"
    [[ ! -e "$missing_root/projects.json" ]] || fail "source_missing outcome retried migration"
    stop_app "$socket"

    local failure_root="$VERIFICATION_ROOT/failure-root"
    local failure_source="$VERIFICATION_ROOT/failure-swift-source"
    mkdir -p "$failure_source"
    printf '[]\n' > "$failure_source/projects.json"
    printf '[]\n' > "$failure_source/recently-removed-projects.json"
    source_hash_before="$(source_hash "$failure_source")"
    state="$failure_root/swift-profile-migration.json"
    socket="$failure_root/muxy.sock"
    assert_safe_environment "$home" "$failure_root" "$failure_source" "$defaults" "com.muxy.tests" || {
        fail "failure launch environment is unsafe"
    }
    launch_app "$release_executable" "$home" "$failure_root" "$failure_source" "$defaults" "recently-removed-projects.json" "$VERIFICATION_ROOT/failure-one.log"
    expect_startup_failure "$state" pending
    [[ "$(state_value "$state" attempt_count)" == "1" ]] || fail "first failure did not record attempt one"
    [[ "$(state_value "$state" failure.path)" == "recently-removed-projects.json" ]] || fail "first failure path was not reported"
    [[ -f "$failure_root/projects.json" ]] || fail "atomic import before failure was not preserved"
    source_hash_after="$(source_hash "$failure_source")"
    [[ "$source_hash_before" == "$source_hash_after" ]] || fail "first failure changed the Swift source"
    launch_app "$release_executable" "$home" "$failure_root" "$failure_source" "$defaults" "recently-removed-projects.json" "$VERIFICATION_ROOT/failure-two.log"
    wait_for_outcome "$state" abandoned || fail "second failure did not abandon migration"
    wait_for_socket "$socket" || fail "abandoned migration did not continue startup"
    [[ "$(state_value "$state" attempt_count)" == "2" ]] || fail "abandoned migration attempt count is not two"
    [[ -f "$failure_root/projects.json" ]] || fail "abandonment removed imported data"
    source_hash_after="$(source_hash "$failure_source")"
    [[ "$source_hash_before" == "$source_hash_after" ]] || fail "second failure changed the Swift source"
    stop_app "$socket"
    rm -rf "$failure_source"
    launch_app "$release_executable" "$home" "$failure_root" "$failure_source" "$defaults" "" "$VERIFICATION_ROOT/abandoned-relaunch.log"
    wait_for_socket "$socket" || fail "abandoned migration inspected the removed source"
    [[ "$(state_value "$state" outcome)" == "abandoned" ]] || fail "abandoned state changed"
    [[ "$(state_value "$state" attempt_count)" == "2" ]] || fail "abandoned migration retried"
    stop_app "$socket"

    printf 'P2.5 staged migration verification passed\n'
}

trap 'stop_app ""' EXIT
for command_name in codesign cut find grep plutil shasum sort; do
    require_command "$command_name"
done

if (($# > 1)); then
    printf 'Usage: scripts/verify-p2-5-migration.sh [--self-test]\n' >&2
    exit 2
fi
if [[ "${1:-}" == "--self-test" ]]; then
    self_test
elif (($# == 0)); then
    run_acceptance
else
    printf 'Usage: scripts/verify-p2-5-migration.sh [--self-test]\n' >&2
    exit 2
fi
