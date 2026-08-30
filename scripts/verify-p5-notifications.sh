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
readonly SOURCE_CLI="$PROJECT_ROOT/Muxy/Resources/scripts/muxy-cli"
readonly PRODUCTION_SOCKET="$HOME/Library/Application Support/Muxy/muxy.sock"
readonly PRODUCTION_PROFILE="$HOME/Library/Application Support/Muxy/profile.json"
APP_PID=""
APP_EXECUTABLE=""
APP_SUPPORT=""
SOCKET=""
APP_LOG=""
PRODUCTION_SOCKET_IDENTITY=""
PRODUCTION_SOCKET_PID=""
PRODUCTION_PROFILE_STATE=""

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
        scripts/build-app.sh \
        scripts/verify-bundle.sh \
        .github || true)"
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

cleanup_app() {
    if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
        kill -TERM "$APP_PID" 2>/dev/null || true
        wait "$APP_PID" 2>/dev/null || true
    fi
    APP_PID=""
    if [[ -n "$SOCKET" && -e "$SOCKET" ]] && path_is_safe "$SOCKET"; then
        rm -f "$SOCKET"
    fi
}

capture_production_state() {
    [[ -S "$PRODUCTION_SOCKET" ]] || fail "production socket is not live: $PRODUCTION_SOCKET"
    PRODUCTION_SOCKET_IDENTITY="$(stat -f '%d:%i' "$PRODUCTION_SOCKET")"
    PRODUCTION_SOCKET_PID="$(lsof -t "$PRODUCTION_SOCKET" | head -n 1)"
    [[ -n "$PRODUCTION_SOCKET_PID" ]] || fail "production socket has no owner"
    kill -0 "$PRODUCTION_SOCKET_PID" 2>/dev/null || fail "production socket owner is not live"
    if [[ -e "$PRODUCTION_PROFILE" ]]; then
        [[ -f "$PRODUCTION_PROFILE" && ! -L "$PRODUCTION_PROFILE" ]] || fail "production profile is not a regular file"
        PRODUCTION_PROFILE_STATE="present:$(shasum -a 256 "$PRODUCTION_PROFILE" | cut -d ' ' -f 1)"
    else
        PRODUCTION_PROFILE_STATE="absent"
    fi
}

verify_production_state() {
    [[ -S "$PRODUCTION_SOCKET" ]] || fail "production socket disappeared"
    [[ "$(stat -f '%d:%i' "$PRODUCTION_SOCKET")" == "$PRODUCTION_SOCKET_IDENTITY" ]] || fail "production socket identity changed"
    [[ "$(lsof -t "$PRODUCTION_SOCKET" | head -n 1)" == "$PRODUCTION_SOCKET_PID" ]] || fail "production socket owner changed"
    kill -0 "$PRODUCTION_SOCKET_PID" 2>/dev/null || fail "production socket owner exited"
    if [[ "$PRODUCTION_PROFILE_STATE" == absent ]]; then
        [[ ! -e "$PRODUCTION_PROFILE" ]] || fail "production profile was created"
    else
        [[ -f "$PRODUCTION_PROFILE" && ! -L "$PRODUCTION_PROFILE" ]] || fail "production profile disappeared or changed type"
        [[ "present:$(shasum -a 256 "$PRODUCTION_PROFILE" | cut -d ' ' -f 1)" == "$PRODUCTION_PROFILE_STATE" ]] || fail "production profile contents changed"
    fi
}

launch_app() {
    local root="$1"
    local log_name="$2"
    local socket_name="$3"
    local close_request_enabled="$4"
    mkdir -p "$root/home/tmp" "$root/xdg-config" "$root/app-support"
    APP_SUPPORT="$root/app-support"
    SOCKET="$APP_SUPPORT/$socket_name"
    APP_LOG="$root/$log_name.log"
    [[ ${#SOCKET} -lt 104 ]] || fail "fixture socket path is too long: $SOCKET"
    env -u MUXY_SOCKET_PATH -u MUXY_PANE_ID -u MUXY_PROJECT_ID -u MUXY_WORKTREE_ID \
        HOME="$root/home" \
        CFFIXED_USER_HOME="$root/home" \
        TMPDIR="$root/home/tmp/" \
        XDG_CONFIG_HOME="$root/xdg-config" \
        MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$APP_SUPPORT" \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST="$close_request_enabled" \
        "$APP_EXECUTABLE" > "$APP_LOG" 2>&1 &
    APP_PID=$!
    for _ in $(jot 400); do
        [[ -S "$SOCKET" ]] && break
        kill -0 "$APP_PID" 2>/dev/null || fail "staged app exited before binding; log: $APP_LOG"
        sleep 0.05
    done
    [[ -S "$SOCKET" ]] || fail "staged app did not create its socket; log: $APP_LOG"
    [[ "$(stat -f '%Lp' "$SOCKET")" == 600 ]] || fail "staged socket mode is not 0600"
    verify_production_state
}

run_cli() {
    MUXY_SOCKET_PATH="$SOCKET" MUXY_CLI_TIMEOUT=5 "$SOURCE_CLI" "$@"
}

first_pane_id() {
    local pane
    pane="$(run_cli list-panes | head -n 1 | cut -f 1)"
    [[ "$pane" =~ ^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$ ]] || fail "could not resolve a staged pane ID: $pane"
    printf '%s\n' "$pane"
}

send_legacy() {
    local type="$1"
    local pane="$2"
    local title="$3"
    local body="$4"
    local output="$5"
    printf '%s|%s|%s|%s\n' "$type" "$pane" "$title" "$body" | nc -w 1 -U "$SOCKET" > "$output"
    [[ ! -s "$output" ]] || fail "legacy notification unexpectedly returned bytes"
}

send_hook() {
    local pane="$1"
    local output="$2"
    printf '%s\n' "{\"v\":3,\"kind\":\"agent_event\",\"id\":\"AAAAAAAA-1111-4111-8111-111111111111\",\"provider\":\"codex\",\"paneID\":\"$pane\",\"phase\":\"finished\",\"title\":\"Hook Persisted\",\"body\":\"Hook Body\",\"pids\":[],\"ts\":7,\"test\":true}" | nc -w 2 -U "$SOCKET" > "$output"
    [[ "$(cat "$output")" == '{"kind":"ack","ok":true,"v":3}' ]] || fail "hook acknowledgement differed"
}

wait_for_store() {
    local path="$1"
    local expression="$2"
    local description="$3"
    for _ in $(jot 120); do
        if [[ -f "$path" ]] && jq -e "$expression" "$path" >/dev/null 2>&1; then
            return
        fi
        sleep 0.05
    done
    fail "notification store did not reach $description"
}

wait_for_app_exit() {
    local reason="$1" status
    for _ in $(jot 400); do
        ! kill -0 "$APP_PID" 2>/dev/null && break
        sleep 0.05
    done
    kill -0 "$APP_PID" 2>/dev/null && fail "staged app did not exit after $reason"
    set +e
    wait "$APP_PID"
    status=$?
    set -e
    APP_PID=""
    [[ "$status" == 0 ]] || fail "staged app exited with status $status after $reason"
    [[ ! -e "$SOCKET" ]] || fail "staged socket remained after $reason"
    verify_production_state
}

normal_quit() {
    osascript -e 'tell application id "com.muxy.tests" to quit' >/dev/null
    wait_for_app_exit "normal quit"
}

request_main_window_close() {
    [[ "$APP_SUPPORT" == "$VERIFICATION_ROOT"/* ]] || fail "close request escaped the verification root"
    printf '%s\n' close > "$APP_SUPPORT/.muxy-test-close-main-window"
}

write_staged_seed() {
    local path="$1"
    cat > "$path" <<'JSON'
[
  {
    "id": "BBBBBBBB-2222-4222-8222-222222222222",
    "paneID": "11111111-2222-4333-8444-555555555555",
    "projectID": "22222222-3333-4444-8555-666666666666",
    "worktreeID": "33333333-4444-4555-8666-777777777777",
    "areaID": "44444444-5555-4666-8777-888888888888",
    "tabID": "55555555-6666-4777-8888-999999999999",
    "worktreePath": "/tmp/p5-staged-seed",
    "source": {"type": "socket"},
    "title": "Seed Retained",
    "body": "Seed Body",
    "timestamp": 796000000.0,
    "isRead": false
  },
  {"broken": true}
]
JSON
    chmod 0600 "$path"
}

run_staged() {
    local profile="$1"
    local case_name="$2"
    local socket_name source_app staged_app source_cli staged_cli
    local root lifecycle_root drop_root store pane
    source_app="$PROJECT_ROOT/target/$profile/Muxy.app"
    staged_app="$PROJECT_ROOT/target/test-verification/apps/p5-final-$profile/MuxyTests.app"
    source_cli="$source_app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    staged_cli="$staged_app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    APP_EXECUTABLE="$staged_app/Contents/MacOS/MuxyTests"
    [[ "$profile" == debug ]] && socket_name="muxy-dev.sock" || socket_name="muxy.sock"
    [[ -d "$source_app" ]] || fail "source bundle is missing: $source_app"
    [[ -d "$staged_app" ]] || fail "final staged bundle is missing: $staged_app"
    [[ -x "$APP_EXECUTABLE" ]] || fail "staged executable is missing: $APP_EXECUTABLE"
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$staged_app/Contents/Info.plist")" == "com.muxy.tests" ]] || fail "staged bundle identity differs"
    codesign --verify --deep --strict "$staged_app"
    for bundled_cli in "$source_cli" "$staged_cli"; do
        [[ -x "$bundled_cli" ]] || fail "bundled CLI is not executable: $bundled_cli"
        cmp -s "$SOURCE_CLI" "$bundled_cli" || fail "bundled CLI differs from retained source: $bundled_cli"
    done
    source_checks
    run_catalog_contract
    capture_production_state
    root="$VERIFICATION_ROOT/staged/$profile/$case_name"
    reset_safe_path "$root"
    lifecycle_root="$root/lifecycle"
    mkdir -p "$lifecycle_root/app-support"
    store="$lifecycle_root/app-support/notifications.json"
    write_staged_seed "$store"
    launch_app "$lifecycle_root" first "$socket_name" 0
    wait_for_store "$store" 'length == 1 and .[0].title == "Seed Retained" and .[0].isRead == true' "startup malformed-row isolation and read clearing"
    pane="$(first_pane_id)"
    send_legacy unknown "$pane" "Legacy Persisted" "Legacy Body" "$lifecycle_root/legacy.out"
    send_hook "$pane" "$lifecycle_root/hook.out"
    wait_for_store "$store" 'length == 3 and .[0].title == "Hook Persisted" and .[1].title == "Legacy Persisted" and .[2].title == "Seed Retained"' "ordinary debounce persistence"
    validate_store_file "$store"
    jq -e --arg pane "$pane" '
        .[0].paneID == $pane and
        .[1].paneID == $pane and
        .[0].source == {"type":"aiProvider","providerID":"codex"} and
        .[1].source == {"type":"socket"}
    ' "$store" >/dev/null || fail "staged ingress source or target data differed"
    send_legacy codex_hook "$pane" "Quit Flush Persisted" "Quit Flush Body" "$lifecycle_root/quit.out"
    normal_quit
    validate_store_file "$store"
    jq -e 'length == 4 and .[0].title == "Quit Flush Persisted" and .[0].source == {"type":"aiProvider","providerID":"codex"}' "$store" >/dev/null || fail "app-quit final flush did not persist the unique row"
    launch_app "$lifecycle_root" restart "$socket_name" 0
    wait_for_store "$store" 'length == 4 and all(.[]; .isRead == true)' "restart retention and startup read clearing"
    validate_store_file "$store"
    normal_quit
    drop_root="$root/main-window-drop"
    mkdir -p "$drop_root/app-support"
    store="$drop_root/app-support/notifications.json"
    launch_app "$drop_root" drop "$socket_name" 1
    pane="$(first_pane_id)"
    send_legacy unknown "$pane" "Main Window Drop Persisted" "Drop Body" "$drop_root/drop.out"
    request_main_window_close
    wait_for_app_exit "main-window close"
    validate_store_file "$store"
    jq -e 'length == 1 and .[0].title == "Main Window Drop Persisted"' "$store" >/dev/null || fail "main-window close did not persist its unique row"
    [[ ! -S "$lifecycle_root/app-support/$socket_name" ]] || fail "lifecycle socket remained"
    [[ ! -S "$drop_root/app-support/$socket_name" ]] || fail "drop socket remained"
    verify_production_state
    printf 'P5 staged %s verification passed: %s\n' "$profile" "$case_name"
    printf 'Observed: socket ingress, persistence, malformed-row isolation, debounce, app-quit flush, restart/read clearing, main-window drop flush, CLI bytes, catalog count, cleanup, production preservation\n'
    printf 'Not observed by this verifier: rendered UI, accessibility traversal, native authorization/banner/click behavior, sound audibility, real terminal OSC escape sequences\n'
}

trap cleanup_app EXIT

for command_name in bash cargo chmod cmp cut find git grep jq rg stat; do
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
        (($# == 3)) || fail "usage: scripts/verify-p5-notifications.sh --staged <debug|release> full"
        [[ "$2" == debug || "$2" == release ]] || fail "unknown staged profile: $2"
        [[ "$3" == full ]] || fail "unknown staged case: $3"
        for command_name in codesign jot lsof nc osascript plutil shasum; do
            require_command "$command_name"
        done
        run_staged "$2" "$3"
        ;;
    *)
        fail "usage: scripts/verify-p5-notifications.sh --self-test | --fixture full | --staged <debug|release> full"
        ;;
esac
