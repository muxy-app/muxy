#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

APP_PID=""
APP_START_IDENTITY=""
ACTIVE_ROOT=""
ACTIVE_SOCKET=""
cleanup_active() {
    local status=0 current=""
    if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
        current="$(process_start_identity "$APP_PID" || true)"
        if [[ -n "$APP_START_IDENTITY" && "$current" == "$APP_START_IDENTITY" ]]; then
            kill -TERM "$APP_PID" 2>/dev/null || true
            wait "$APP_PID" 2>/dev/null || true
        elif [[ -z "$current" ]]; then
            status=1
        fi
    fi
    APP_PID=""
    APP_START_IDENTITY=""
    if [[ -n "$ACTIVE_SOCKET" && -n "$ACTIVE_ROOT" && "$ACTIVE_SOCKET" == "$ACTIVE_ROOT"/* ]]; then
        rm -f -- "$ACTIVE_SOCKET" 2>/dev/null || status=1
    fi
    ACTIVE_SOCKET=""
    ACTIVE_ROOT=""
    return "$status"
}
on_signal() {
    local status="$1"
    trap - INT TERM
    cleanup_active || true
    exit "$status"
}
trap cleanup_active EXIT
trap 'on_signal 130' INT
trap 'on_signal 143' TERM

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly VERIFICATION_ROOT="$PROJECT_ROOT/target/test-verification/p8"
readonly CASES_ROOT="$VERIFICATION_ROOT/cases"
readonly OWNER_FILE=".muxy-p8-owner"
readonly OWNER_VALUE="muxy-p8-terminal-memory-v1"
readonly STAGED_APPS_ROOT="$PROJECT_ROOT/target/test-verification/apps"
readonly STAGED_OWNER_FILE=".muxy-stage-owner"

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

reject_symlink_components() {
    local candidate="$1" current="/" part
    local -a parts
    [[ "$candidate" == /* ]] || return 1
    IFS='/' read -r -a parts <<< "${candidate#/}"
    for part in "${parts[@]}"; do
        [[ -n "$part" ]] || continue
        current="${current%/}/$part"
        [[ ! -L "$current" ]] || return 1
    done
}

root_path_is_safe() {
    local root="$1"
    [[ "$root" == "$CASES_ROOT/"* ]] || return 1
    [[ "$root" != *"/../"* && "$root" != */.. && "$root" != *"/./"* && "$root" != */. ]] || return 1
    reject_symlink_components "$root"
}

root_is_owned() {
    local root="$1" marker="$1/$OWNER_FILE"
    root_path_is_safe "$root" || return 1
    [[ -d "$root" && ! -L "$root" ]] || return 1
    [[ -f "$marker" && ! -L "$marker" ]] || return 1
    [[ "$(<"$marker")" == "$OWNER_VALUE" ]]
}

prepare_root() {
    local root="$1"
    root_path_is_safe "$root" || fail "refusing unsafe P8 root: $root"
    mkdir -p "$CASES_ROOT"
    reject_symlink_components "$CASES_ROOT" || fail "P8 cases root has a symlinked component"
    if [[ -e "$root" ]]; then
        root_is_owned "$root" || fail "P8 root is not verifier-owned: $root"
        rm -rf -- "$root"
    fi
    mkdir -p "$root"
    printf '%s\n' "$OWNER_VALUE" > "$root/$OWNER_FILE"
    chmod 0600 "$root/$OWNER_FILE"
}

process_start_identity() {
    local pid="$1" started
    started="$(ps -p "$pid" -o lstart= 2>/dev/null)" || return 1
    started="$(printf '%s' "$started" | awk '{$1=$1; print}')"
    [[ -n "$started" ]] || return 1
    printf '%s' "$started" | shasum -a 256 | awk '{print $1}'
}

cleanup_owned_root() {
    local root="$1" pid_file pid held current attempt
    root_is_owned "$root" || fail "cleanup root is not P8-owned: $root"
    pid_file="$root/app.pid"
    if [[ -f "$pid_file" && ! -L "$pid_file" ]]; then
        read -r pid held < "$pid_file" || true
        if [[ "$pid" =~ ^[1-9][0-9]*$ ]] && kill -0 "$pid" 2>/dev/null; then
            current="$(process_start_identity "$pid" || true)"
            [[ -n "$current" && "$current" == "$held" ]] || fail "refusing to signal reused or unknown PID $pid"
            kill -TERM "$pid" 2>/dev/null || true
            for ((attempt = 0; attempt < 200; attempt++)); do
                kill -0 "$pid" 2>/dev/null || break
                sleep 0.025
            done
            kill -0 "$pid" 2>/dev/null && fail "owned process did not stop: $pid"
        fi
    fi
    find "$root" -type s -delete
    rm -rf -- "$root"
}

source_checks() {
    local matches
    for path in \
        crates/muxy-proto/src/session/codec.rs \
        crates/muxy-proto/src/session/messages.rs \
        crates/muxy-proto/src/session/replay.rs \
        crates/muxy-proto/src/session/terminal_stream.rs \
        crates/muxy-proto/src/session/window_size.rs \
        crates/muxy-core/src/session/reconciliation.rs \
        crates/muxy-core/src/session/transition.rs \
        crates/muxy-core/src/resources.rs \
        crates/muxy-terminal/src/offline/policy.rs \
        crates/muxy-terminal/src/offline/state.rs; do
        [[ -f "$path" ]] || fail "missing Phase 1 source: $path"
    done
    rg -q 'HEADER_BYTES: usize = 24' crates/muxy-proto/src/session/codec.rs || fail "session header size differs"
    rg -q 'MAGIC: \[u8; 4\] = \*b"MXS8"' crates/muxy-proto/src/session/codec.rs || fail "session magic differs"
    rg -q 'MAX_STRUCTURED_FRAME_BYTES: usize = 1024 \* 1024' crates/muxy-proto/src/session/messages.rs || fail "structured frame limit differs"
    rg -q 'MAX_STREAM_CHUNK_BYTES: usize = 32 \* 1024' crates/muxy-proto/src/session/messages.rs || fail "stream chunk limit differs"
    rg -q '^muxy-proto\.workspace = true$' crates/muxy-core/Cargo.toml || fail "muxy-core does not depend on muxy-proto"
    rg -q 'pub session_id: Option<SessionId>' crates/muxy-core/src/workspace/tab.rs || fail "typed sessionId is missing"
    rg -q '"sessionId"' crates/muxy-core/src/workspace_store.rs || fail "sessionId persistence is missing"
    rg -q '"paneSessionID"' crates/muxy-core/src/workspace_store.rs || fail "paneSessionID preservation proof is missing"
    [[ ! -e crates/muxy-session ]] || fail "Phase 2 daemon crate entered Phase 1"
    matches="$(find . -name terminal-session-mode.json -print)"
    [[ -z "$matches" ]] || fail "forbidden terminal session marker exists"
    matches="$(rg -n 'muxy[-_]session' crates/muxy/Cargo.toml crates/muxy/src || true)"
    [[ -z "$matches" ]] || fail "P8 runtime behavior entered Phase 1"
}

portable_fixture() {
    source_checks
    cargo test -p muxy-proto --locked --offline session
    cargo test -p muxy-core --locked --offline session
    cargo test -p muxy-core --locked --offline resources
    cargo test -p muxy-terminal --locked --offline offline
    printf 'P8 portable fixture passed\n'
}

validate_staged_app() {
    local app="$1" parent marker executable
    [[ "$app" == "$STAGED_APPS_ROOT/"*/MuxyTests.app ]] || fail "staged app is outside the owned apps root"
    reject_symlink_components "$app" || fail "staged app has a symlinked component"
    [[ -d "$app" && ! -L "$app" ]] || fail "staged app is missing"
    parent="$(dirname "$app")"
    marker="$parent/$STAGED_OWNER_FILE"
    [[ -f "$marker" && ! -L "$marker" && "$(<"$marker")" == "$app" ]] || fail "staged app ownership marker differs"
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$app/Contents/Info.plist")" == com.muxy.tests ]] || fail "staged app identity differs"
    executable="$(plutil -extract CFBundleExecutable raw -o - "$app/Contents/Info.plist")"
    [[ "$executable" == MuxyTests && -x "$app/Contents/MacOS/$executable" ]] || fail "staged app executable differs"
    codesign --verify --deep --strict "$app"
}

staged_phase_one() {
    local profile="$1" app="$2" case_name="$3" root app_support home tmp xdg socket_name
    local executable log cli exit_status start_identity attempt
    [[ "$(uname -s)" == Darwin ]] || fail "staged verification requires macOS"
    [[ "$profile" == debug || "$profile" == release ]] || fail "invalid staged profile: $profile"
    [[ "$case_name" == phase-1 ]] || fail "unsupported Phase 1 staged case: $case_name"
    validate_staged_app "$app"
    root="$CASES_ROOT/staged-$profile-$case_name"
    prepare_root "$root"
    ACTIVE_ROOT="$root"
    app_support="$root/app"
    home="$root/home"
    tmp="$root/tmp"
    xdg="$root/xdg"
    mkdir -p "$app_support" "$home" "$tmp" "$xdg"
    socket_name="muxy.sock"
    [[ "$profile" == debug ]] && socket_name="muxy-dev.sock"
    ACTIVE_SOCKET="$app_support/$socket_name"
    ((${#ACTIVE_SOCKET} < 104)) || fail "staged app socket path exceeds the macOS limit"
    executable="$app/Contents/MacOS/MuxyTests"
    log="$root/app.log"
    cli="$app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    [[ -x "$cli" ]] || fail "retained staged CLI is missing"
    cmp -s "$PROJECT_ROOT/Muxy/Resources/scripts/muxy-cli" "$cli" || fail "retained staged CLI bytes differ"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$log" 2>&1 &
    APP_PID=$!
    start_identity="$(process_start_identity "$APP_PID")"
    APP_START_IDENTITY="$start_identity"
    printf '%s %s\n' "$APP_PID" "$start_identity" > "$root/app.pid"
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -S "$ACTIVE_SOCKET" ]] && break
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$log"
            fail "staged app exited before binding its socket"
        }
        sleep 0.05
    done
    [[ -S "$ACTIVE_SOCKET" && ! -L "$ACTIVE_SOCKET" ]] || fail "staged app socket was not created"
    MUXY_SOCKET_PATH="$ACTIVE_SOCKET" "$cli" list-projects >/dev/null
    [[ ! -e "$app_support/sessions" && ! -e "$app_support/sessions-dev" ]] || fail "P8 runtime activated during Phase 1"
    osascript -e 'tell application id "com.muxy.tests" to quit'
    for ((attempt = 0; attempt < 400; attempt++)); do
        ! kill -0 "$APP_PID" 2>/dev/null && break
        sleep 0.05
    done
    ! kill -0 "$APP_PID" 2>/dev/null || fail "staged app did not close normally"
    set +e
    wait "$APP_PID"
    exit_status=$?
    set -e
    APP_PID=""
    APP_START_IDENTITY=""
    [[ "$exit_status" == 0 ]] || {
        cat "$log"
        fail "staged app exited with status $exit_status"
    }
    [[ ! -S "$ACTIVE_SOCKET" ]] || fail "staged app socket remained after close"
    if pgrep -f "$executable$" >/dev/null; then
        fail "staged app process survived normal close"
    fi
    ACTIVE_SOCKET=""
    ACTIVE_ROOT=""
    printf 'P8 staged Phase 1 passed with zero staged process residue\n'
}

self_test() {
    local nonce="$$" owned unowned mismatch linked target outside
    source_checks
    mkdir -p "$CASES_ROOT"
    owned="$CASES_ROOT/self-owned-$nonce"
    prepare_root "$owned"
    root_is_owned "$owned" || fail "owned root was rejected"
    outside="$PROJECT_ROOT/target/p8-outside-$nonce"
    root_path_is_safe "$outside" && fail "outside root was accepted"
    for production in "$HOME/.muxy" "$HOME/.muxy-dev" "$HOME/Library/Application Support/Muxy"; do
        root_path_is_safe "$production" && fail "production root was accepted: $production"
    done
    unowned="$CASES_ROOT/self-unowned-$nonce"
    mkdir -p "$unowned"
    printf 'held\n' > "$unowned/sentinel"
    if (cleanup_owned_root "$unowned") >/dev/null 2>&1; then
        fail "unowned cleanup root was accepted"
    fi
    [[ -f "$unowned/sentinel" ]] || fail "unowned data was removed"
    mismatch="$CASES_ROOT/self-mismatch-$nonce"
    mkdir -p "$mismatch"
    printf 'wrong\n' > "$mismatch/$OWNER_FILE"
    printf 'held\n' > "$mismatch/sentinel"
    if (cleanup_owned_root "$mismatch") >/dev/null 2>&1; then
        fail "wrong-owner cleanup root was accepted"
    fi
    [[ -f "$mismatch/sentinel" ]] || fail "wrong-owner data was removed"
    target="$CASES_ROOT/self-target-$nonce"
    linked="$CASES_ROOT/self-linked-$nonce"
    mkdir -p "$target"
    printf 'held\n' > "$target/sentinel"
    ln -s "$target" "$linked"
    root_path_is_safe "$linked" && fail "symlinked root was accepted"
    [[ -f "$target/sentinel" ]] || fail "symlink rejection removed target data"
    cleanup_owned_root "$owned"
    rm -f -- "$linked"
    rm -rf -- "$unowned" "$mismatch" "$target" "$outside"
    printf 'P8 terminal memory verifier self-test passed\n'
}

for command_name in find rg; do
    require_command "$command_name"
done

case "${1:-}" in
    --fixture)
        (($# == 2)) || fail "usage: scripts/verify-p8-terminal-memory.sh --fixture portable"
        [[ "$2" == portable ]] || fail "unknown Phase 1 fixture: $2"
        require_command cargo
        portable_fixture
        ;;
    --self-test)
        (($# == 1)) || fail "usage: scripts/verify-p8-terminal-memory.sh --self-test"
        self_test
        ;;
    --staged)
        (($# == 4)) || fail "usage: scripts/verify-p8-terminal-memory.sh --staged PROFILE APP phase-1"
        for command_name in awk cmp codesign osascript pgrep plutil ps shasum; do
            require_command "$command_name"
        done
        staged_phase_one "$2" "$3" "$4"
        ;;
    --cleanup-only)
        (($# == 2)) || fail "usage: scripts/verify-p8-terminal-memory.sh --cleanup-only OWNED_ROOT"
        for command_name in awk ps shasum; do
            require_command "$command_name"
        done
        cleanup_owned_root "$2"
        printf 'P8 cleanup completed: %s\n' "$2"
        ;;
    *)
        fail "usage: scripts/verify-p8-terminal-memory.sh --fixture portable | --self-test | --staged PROFILE APP phase-1 | --cleanup-only OWNED_ROOT"
        ;;
esac
