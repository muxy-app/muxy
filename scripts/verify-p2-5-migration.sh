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

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
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


require_command cat

case "${1:-}" in
    --self-test)
        (($# == 1)) || fail "usage: scripts/verify-p2-5-migration.sh --self-test"
        self_test
        ;;
    --staged)
        fail "app-launching E2E verification is disabled; use headless migration tests and ask the user to verify native startup behavior"
        ;;
    *)
        fail "usage: scripts/verify-p2-5-migration.sh --self-test"
        ;;
esac
