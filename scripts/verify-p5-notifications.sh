#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly VERIFICATION_PARENT="$PROJECT_ROOT/target"
readonly VERIFICATION_ROOT="$VERIFICATION_PARENT/p5v"
readonly OWNERSHIP_MARKER="$VERIFICATION_ROOT/.muxy-p5-verifier"
readonly OWNERSHIP_VALUE="muxy-p5-notifications-verifier-v1"
fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

assert_safe_ancestors() {
    local path
    for path in "$PROJECT_ROOT/target" "$VERIFICATION_ROOT"; do
        [[ ! -L "$path" ]] || return 1
        [[ ! -e "$path" || -d "$path" ]] || return 1
    done
    if [[ -d "$VERIFICATION_PARENT" ]]; then
        [[ "$(cd "$VERIFICATION_PARENT" && pwd -P)" == "$VERIFICATION_PARENT" ]] || return 1
    fi
}

ownership_is_valid() {
    local root="$1"
    local marker="$root/.muxy-p5-verifier"
    [[ -d "$root" && ! -L "$root" ]] || return 1
    [[ -f "$marker" && ! -L "$marker" ]] || return 1
    [[ "$(<"$marker")" == "$OWNERSHIP_VALUE" ]]
}

path_is_safe() {
    local path="$1"
    local relative component cursor old_ifs
    local -a components
    [[ "$path" == "$VERIFICATION_ROOT"/* ]] || return 1
    [[ "$path" != *"/../"* && "$path" != */.. && "$path" != *"/./"* && "$path" != */. ]] || return 1
    relative="${path#"$VERIFICATION_ROOT"/}"
    [[ -n "$relative" ]] || return 1
    cursor="$VERIFICATION_ROOT"
    old_ifs="$IFS"
    IFS='/'
    read -r -a components <<< "$relative"
    IFS="$old_ifs"
    for component in "${components[@]}"; do
        [[ -n "$component" ]] || return 1
        cursor="$cursor/$component"
        [[ ! -L "$cursor" ]] || return 1
    done
}

ensure_root() {
    assert_safe_ancestors || fail "verification path has an unsafe ancestor"
    mkdir -p "$VERIFICATION_PARENT"
    if [[ -e "$VERIFICATION_ROOT" ]]; then
        ownership_is_valid "$VERIFICATION_ROOT" || fail "verification root is not owned by this verifier"
    else
        mkdir "$VERIFICATION_ROOT"
        printf '%s\n' "$OWNERSHIP_VALUE" > "$OWNERSHIP_MARKER"
    fi
    assert_safe_ancestors || fail "verification path changed while preparing it"
}

reset_safe_path() {
    local path="$1"
    ensure_root
    path_is_safe "$path" || fail "refusing unsafe verification path: $path"
    rm -rf "$path"
    mkdir -p "$path"
}

store_shape_is_valid() {
    local path="$1"
    [[ -f "$path" && ! -L "$path" ]] || return 1
    jq -e '
        type == "array" and
        length <= 200 and
        (([.[].id] | length) == ([.[].id] | unique | length)) and
        all(.[];
            type == "object" and
            (.id | type == "string" and test("^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$")) and
            (.paneID | type == "string" and test("^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$")) and
            (.projectID | type == "string" and test("^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$")) and
            (.worktreeID | type == "string" and test("^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$")) and
            (.areaID | type == "string" and test("^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$")) and
            (.tabID | type == "string" and test("^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$")) and
            (.worktreePath | type == "string") and
            (.title | type == "string") and
            (.body | type == "string") and
            (.timestamp | type == "number") and
            (.isRead | type == "boolean") and
            (.source | type == "object") and
            (
                .source.type == "osc" or
                .source.type == "socket" or
                (.source.type == "aiProvider" and (.source.providerID | type == "string" and length > 0))
            )
        )
    ' "$path" >/dev/null
}

validate_store_file() {
    local path="$1"
    store_shape_is_valid "$path" || fail "notification store shape is invalid: $path"
    [[ "$(stat -f '%Lp' "$path")" == 600 ]] || fail "notification store mode is not 0600: $path"
    if find "$(dirname "$path")" -maxdepth 1 -name 'notifications.json.*.tmp' -print | grep -q .; then
        fail "notification store left a temporary file"
    fi
}

write_valid_store_fixture() {
    local path="$1"
    cat > "$path" <<'JSON'
[
  {
    "id": "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE",
    "paneID": "11111111-2222-4333-8444-555555555555",
    "projectID": "22222222-3333-4444-8555-666666666666",
    "worktreeID": "33333333-4444-4555-8666-777777777777",
    "areaID": "44444444-5555-4666-8777-888888888888",
    "tabID": "55555555-6666-4777-8888-999999999999",
    "worktreePath": "/tmp/p5-fixture",
    "source": {"type": "socket"},
    "title": "Fixture",
    "body": "Valid",
    "timestamp": 796000000.0,
    "isRead": false
  }
]
JSON
    chmod 0600 "$path"
}

source_checks() {
    local changes matches
    changes="$(git status --short --untracked-files=all -- \
        crates/muxy-proto \
        crates/muxy/src/socket/catalog.rs \
        crates/muxy-core/src/migration.rs \
        Muxy/Resources/scripts/muxy-cli \
        .github || true)"
    changes="$(printf '%s\n' "$changes" | rg -v 'crates/muxy-proto/src/session/|crates/muxy-proto/src/lib.rs$|crates/muxy/src/socket/catalog.rs$' || true)"
    [[ -z "$changes" ]] || {
        printf '%s\n' "$changes"
        fail "a locked protocol, catalog, migration, CLI, bundle, or CI path changed"
    }
    "$PROJECT_ROOT/scripts/stage-test-app.sh" --self-test >/dev/null
    if rg -n 'notifications\.json' crates/muxy-core/src/migration.rs; then
        fail "notifications.json entered the migration implementation"
    fi
    matches="$(rg -n 'gpui|objc2|NSSound|UNUserNotification|MainWindow|AppState' \
        crates/muxy-core/src/notifications || true)"
    [[ -z "$matches" ]] || {
        printf '%s\n' "$matches"
        fail "portable notification core crossed a platform or app boundary"
    }
    matches="$(rg -n 'objc2_user_notifications|UNUserNotification|NSSound|write_private' \
        crates/muxy/src/views || true)"
    [[ -z "$matches" ]] || {
        printf '%s\n' "$matches"
        fail "notification platform or persistence ownership escaped into views"
    }
    if rg -n 'static mut|OnceLock<[^>]*Notification|LazyLock<[^>]*Notification' \
        crates/muxy-core/src/notifications crates/muxy/src/notifications crates/muxy/src/views; then
        fail "notification state uses process-global mutable ownership"
    fi
    if rg -n 'name\s*=\s*"muxy-(extension-host|hook)"|notification\.posted|notificationPosted' \
        Cargo.toml crates --glob 'Cargo.toml' --glob '*.rs'; then
        fail "P10 or P11 notification implementation entered P5"
    fi
    [[ ! -e "$PROJECT_ROOT/crates/muxy-extension-host" ]] || fail "P10 extension host exists during P5"
    [[ ! -e "$PROJECT_ROOT/crates/muxy-hook" ]] || fail "P11 hook executable exists during P5"
}

self_test() {
    local root="$VERIFICATION_ROOT/self-test"
    local outside="$PROJECT_ROOT/p5-unsafe"
    local ownership="$root/ownership"
    local invalid="$root/invalid.json"
    local valid="$root/valid.json"
    reset_safe_path "$root"
    path_is_safe "$root/safe" || fail "safe containment path was rejected"
    if path_is_safe "$outside"; then
        fail "containment accepted a path outside the verification root"
    fi
    mkdir -p "$root/linked-target"
    ln -s "$root/linked-target" "$root/linked"
    if path_is_safe "$root/linked/child"; then
        fail "containment accepted a symlinked ancestor"
    fi
    rm -f "$root/linked"
    mkdir -p "$ownership"
    if ownership_is_valid "$ownership"; then
        fail "ownership accepted a missing marker"
    fi
    printf '%s\n' wrong > "$ownership/.muxy-p5-verifier"
    if ownership_is_valid "$ownership"; then
        fail "ownership accepted a mismatched marker"
    fi
    printf '%s\n' "$OWNERSHIP_VALUE" > "$ownership/.muxy-p5-verifier"
    ownership_is_valid "$ownership" || fail "ownership rejected the exact marker"
    printf '%s\n' '{}' > "$invalid"
    chmod 0600 "$invalid"
    if store_shape_is_valid "$invalid"; then
        fail "fixture validation accepted a non-array store"
    fi
    write_valid_store_fixture "$valid"
    validate_store_file "$valid"
    reset_safe_path "$root/cleanup-proof"
    printf '%s\n' retained > "$root/cleanup-proof/sentinel"
    reset_safe_path "$root/cleanup-proof"
    [[ ! -e "$root/cleanup-proof/sentinel" ]] || fail "safe cleanup retained stale fixture data"
    source_checks
    rm -rf "$root"
    printf 'P5 notifications verifier self-test passed\n'
}

run_catalog_contract() {
    cargo test -p muxy --locked --offline recognized_catalog_matches_the_frozen_inventory
}

run_fixture() {
    local case_name="$1"
    local root="$VERIFICATION_ROOT/fixtures/$case_name"
    local store="$root/notifications.json"
    reset_safe_path "$root"
    write_valid_store_fixture "$store"
    validate_store_file "$store"
    cd "$PROJECT_ROOT"
    export CARGO_NET_OFFLINE=true
    cargo test -p muxy-core --locked --offline notifications
    cargo test -p ghostty-host --locked --offline desktop_notification
    cargo test -p muxy-terminal --locked --offline desktop_notification
    cargo test -p muxy --locked --offline notifications
    cargo test -p muxy --locked --offline notification_panel
    cargo test -p muxy --locked --offline notification_navigation
    cargo test -p muxy --locked --offline persistent_stores_wire_quit_and_drop_flush_paths
    cargo test -p muxy --locked --offline notifications_startup
    run_catalog_contract
    source_checks
    printf 'P5 notification fixture passed: %s\n' "$case_name"
}


for command_name in bash cargo chmod cmp find git grep jq rg stat; do
    require_command "$command_name"
done

cd "$PROJECT_ROOT"

case "${1:-}" in
    --self-test)
        (($# == 1)) || fail "--self-test accepts no additional arguments"
        self_test
        ;;
    --fixture)
        (($# == 2)) || fail "usage: scripts/verify-p5-notifications.sh --fixture full"
        [[ "$2" == full ]] || fail "unknown fixture case: $2"
        run_fixture "$2"
        ;;
    --staged)
        fail "app-launching E2E verification is disabled; use headless notification fixtures and ask the user to verify native behavior"
        ;;
    *)
        fail "usage: scripts/verify-p5-notifications.sh --self-test | --fixture full"
        ;;
esac
