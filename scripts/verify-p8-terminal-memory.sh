#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

APP_PID=""
PROBE_PID=""
ACTIVE_ROOT=""
ACTIVE_SOCKET=""
cleanup_active() {
    local status=0
    if [[ -n "$ACTIVE_ROOT" && -d "$ACTIVE_ROOT" ]]; then
        touch "$ACTIVE_ROOT/probe.proceed" 2>/dev/null || true
    fi
    finish_probe_job "$PROBE_PID" || status=1
    PROBE_PID=""
    stop_job_process "$APP_PID" || status=1
    APP_PID=""
    if [[ -n "$ACTIVE_ROOT" ]] && root_is_owned "$ACTIVE_ROOT"; then
        cleanup_owned_root "$ACTIVE_ROOT" || status=1
    elif [[ -n "$ACTIVE_SOCKET" && -n "$ACTIVE_ROOT" && "$ACTIVE_SOCKET" == "$ACTIVE_ROOT"/* ]]; then
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
ISOLATED_TMP_ROOT="$(cd /tmp && pwd -P)"
readonly ISOLATED_TMP_ROOT

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
    [[ "$root" == "$CASES_ROOT/"* || "$root" == "$ISOLATED_TMP_ROOT/p8-isolated-test-"* ]] || return 1
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
    chmod 0700 "$root"
    printf '%s\n' "$OWNER_VALUE" > "$root/$OWNER_FILE"
    chmod 0600 "$root/$OWNER_FILE"
}

prepare_unique_phase_two_root() {
    local root
    root="$(mktemp -d "$ISOLATED_TMP_ROOT/p8-isolated-test-staged-phase2.XXXXXX")"
    root_path_is_safe "$root" || fail "refusing unsafe P8 root: $root"
    chmod 0700 "$root"
    printf '%s\n' "$OWNER_VALUE" > "$root/$OWNER_FILE"
    chmod 0600 "$root/$OWNER_FILE"
    printf '%s\n' "$root"
}

finish_probe_job() {
    local pid="$1" active attempt
    [[ "$pid" =~ ^[1-9][0-9]*$ ]] || return 0
    for ((attempt = 0; attempt < 600; attempt++)); do
        active="$(jobs -pr | awk -v held="$pid" '$1 == held { print $1 }')"
        [[ "$active" == "$pid" ]] || {
            wait "$pid" 2>/dev/null || true
            return 0
        }
        sleep 0.025
    done
    stop_job_process "$pid"
}

stop_job_process() {
    local pid="$1" active attempt
    [[ "$pid" =~ ^[1-9][0-9]*$ ]] || return 0
    active="$(jobs -pr | awk -v held="$pid" '$1 == held { print $1 }')"
    [[ "$active" == "$pid" ]] || return 0
    kill -TERM "$pid" 2>/dev/null || true
    for ((attempt = 0; attempt < 200; attempt++)); do
        active="$(jobs -pr | awk -v held="$pid" '$1 == held { print $1 }')"
        [[ "$active" == "$pid" ]] || {
            wait "$pid" 2>/dev/null || true
            return 0
        }
        sleep 0.025
    done
    fail "owned shell job did not stop: $pid"
}

cleanup_recorded_processes() {
    local root="$1"
    [[ -f "$root/owned-processes" && ! -L "$root/owned-processes" ]] || return 0
    P8_STAGED_CLEANUP_ROOT="$root" \
        cargo test -p muxy-session --test staged_helper --locked --offline \
            staged_bundle_helper_detaches_survives_app_close_and_cleans -- --exact >/dev/null
}

cleanup_owned_root() {
    local root="$1"
    root_is_owned "$root" || fail "cleanup root is not P8-owned: $root"
    cleanup_recorded_processes "$root"
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
    matches="$(find . -name terminal-session-mode.json -print)"
    [[ -z "$matches" ]] || fail "forbidden terminal session marker exists"
    matches="$(rg -n 'muxy[-_]session' crates/muxy/Cargo.toml crates/muxy/src || true)"
    [[ -z "$matches" ]] || fail "P8 app runtime behavior entered before Phase 3"
}

phase_two_source_checks() {
    source_checks
    for path in \
        crates/muxy-session/src/client.rs \
        crates/muxy-session/src/daemon/mod.rs \
        crates/muxy-session/src/daemon/session.rs \
        crates/muxy-session/src/process_tree/mod.rs \
        crates/muxy-session/src/pty/unix.rs \
        crates/muxy-session/src/runtime_paths.rs \
        crates/muxy-session/src/shell.rs \
        crates/muxy-session/src/transport/unix.rs \
        docs/development/session-protocol.md; do
        [[ -f "$path" ]] || fail "missing Phase 2 source: $path"
    done
    rg -q '^muxy-session = \{ path = "crates/muxy-session" \}$' Cargo.toml || {
        fail "workspace dependency on muxy-session is missing"
    }
    rg -q '"crates/muxy-session"' Cargo.toml || fail "muxy-session workspace member is missing"
    rg -q 'authenticate_same_user\(&stream\)' crates/muxy-session/src/daemon/mod.rs || {
        fail "daemon does not authenticate peers before handshake decode"
    }
    rg -q 'LOCAL_PEERCRED' crates/muxy-session/src/transport/unix.rs || fail "macOS peer authentication is missing"
    rg -q 'SO_PEERCRED' crates/muxy-session/src/transport/unix.rs || fail "Linux peer authentication is missing"
    rg -q 'O_NOFOLLOW' crates/muxy-session/src/runtime_paths.rs || fail "runtime no-follow validation is missing"
    rg -q 'Contents/MacOS/muxy-session' scripts/build-app.sh scripts/verify-bundle.sh || {
        fail "session helper bundle contract is missing"
    }
}

portable_fixture() {
    source_checks
    cargo test -p muxy-proto --locked --offline session
    cargo test -p muxy-core --locked --offline session
    cargo test -p muxy-core --locked --offline resources
    cargo test -p muxy-terminal --locked --offline offline
    printf 'P8 portable fixture passed\n'
}

protocol_fixture() {
    phase_two_source_checks
    cargo test -p muxy-proto --locked --offline session
    cargo test -p muxy-session --lib --locked --offline transport
    printf 'P8 protocol fixture passed\n'
}

security_fixture() {
    phase_two_source_checks
    cargo test -p muxy-session --lib --locked --offline runtime_paths
    cargo test -p muxy-session --test security --locked --offline
    printf 'P8 security fixture passed with isolated runtime roots\n'
}

daemon_detach_attach_fixture() {
    phase_two_source_checks
    cargo test -p muxy-session --test daemon --locked --offline
    cargo test -p muxy-session --test attach --locked --offline
    printf 'P8 daemon detach/attach fixture passed with isolated runtime roots\n'
}

process_cleanup_fixture() {
    phase_two_source_checks
    cargo test -p muxy-session --test process_cleanup --locked --offline
    printf 'P8 process cleanup fixture passed with identity-bound processes\n'
}

shell_integration_fixture() {
    phase_two_source_checks
    cargo test -p muxy-session --lib --locked --offline shell
    printf 'P8 shell integration fixture passed\n'
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
    local executable log cli exit_status attempt
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

staged_phase_two() {
    local profile="$1" app="$2" case_name="$3" root app_support home tmp xdg app_socket
    local executable helper app_log probe_log cli exit_status probe_pid
    local ready proceed attempt pid held role
    [[ "$(uname -s)" == Darwin ]] || fail "staged verification requires macOS"
    [[ "$profile" == debug || "$profile" == release ]] || fail "invalid staged profile: $profile"
    [[ "$case_name" == phase-2 ]] || fail "unsupported Phase 2 staged case: $case_name"
    validate_staged_app "$app"
    phase_two_source_checks
    root="$(prepare_unique_phase_two_root)"
    ACTIVE_ROOT="$root"
    app_support="$root/app"
    home="$root/home"
    tmp="$root/tmp"
    xdg="$root/xdg"
    mkdir -p "$app_support" "$home" "$tmp" "$xdg"
    app_socket="$app_support/muxy.sock"
    [[ "$profile" == debug ]] && app_socket="$app_support/muxy-dev.sock"
    ACTIVE_SOCKET="$app_socket"
    executable="$app/Contents/MacOS/MuxyTests"
    helper="$app/Contents/MacOS/muxy-session"
    app_log="$root/app.log"
    probe_log="$root/probe.log"
    cli="$app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    ready="$root/probe.ready"
    proceed="$root/probe.proceed"
    [[ -x "$helper" ]] || fail "staged session helper is missing"
    codesign --verify --strict "$helper"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$app_log" 2>&1 &
    APP_PID=$!
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -S "$app_socket" ]] && break
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$app_log"
            fail "staged app exited before binding its socket"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" ]] || fail "staged app socket was not created"
    MUXY_SOCKET_PATH="$app_socket" "$cli" list-projects >/dev/null
    P8_STAGED_SESSION_HELPER="$helper" \
        P8_STAGED_SESSION_ROOT="$root" \
        P8_STAGED_READY_FILE="$ready" \
        P8_STAGED_PROCEED_FILE="$proceed" \
        cargo test -p muxy-session --test staged_helper --locked --offline \
            staged_bundle_helper_detaches_survives_app_close_and_cleans -- --exact --nocapture \
            > "$probe_log" 2>&1 &
    probe_pid=$!
    PROBE_PID="$probe_pid"
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -f "$ready" ]] && break
        kill -0 "$probe_pid" 2>/dev/null || {
            cat "$probe_log"
            fail "staged helper probe exited before detach proof"
        }
        sleep 0.05
    done
    [[ -f "$ready" && -f "$root/owned-processes" ]] || fail "staged detach proof was not ready"
    while IFS=' ' read -r pid held role; do
        [[ "$pid" =~ ^[1-9][0-9]*$ ]] || fail "invalid owned $role PID"
        [[ "$held" =~ ^[1-9][0-9]*$ ]] || fail "invalid owned $role start identity"
        [[ "$role" == shell || "$role" == daemon ]] || fail "invalid owned process role"
    done < "$root/owned-processes"
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
    [[ "$exit_status" == 0 ]] || {
        cat "$app_log"
        fail "staged app exited with status $exit_status"
    }
    touch "$proceed"
    set +e
    wait "$probe_pid"
    exit_status=$?
    set -e
    PROBE_PID=""
    [[ "$exit_status" == 0 ]] || {
        cat "$probe_log"
        fail "staged helper probe exited with status $exit_status"
    }
    [[ ! -e "$root/owned-processes" ]] || fail "staged helper process identities remained"
    [[ ! -S "$root/control.sock" ]] || fail "staged helper socket remained"
    [[ ! -S "$app_socket" ]] || fail "staged app socket remained"
    cleanup_owned_root "$root"
    ACTIVE_SOCKET=""
    ACTIVE_ROOT=""
    printf 'P8 staged Phase 2 detach, app-close survival, reattach, and cleanup passed\n'
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
        (($# == 2)) || fail "usage: scripts/verify-p8-terminal-memory.sh --fixture NAME"
        require_command cargo
        case "$2" in
            portable) portable_fixture ;;
            protocol) protocol_fixture ;;
            security) security_fixture ;;
            daemon-detach-attach) daemon_detach_attach_fixture ;;
            process-cleanup) process_cleanup_fixture ;;
            shell-integration) shell_integration_fixture ;;
            *) fail "unknown P8 fixture: $2" ;;
        esac
        ;;
    --self-test)
        (($# == 1)) || fail "usage: scripts/verify-p8-terminal-memory.sh --self-test"
        self_test
        ;;
    --staged)
        (($# == 4)) || fail "usage: scripts/verify-p8-terminal-memory.sh --staged PROFILE APP PHASE"
        for command_name in awk cmp codesign osascript pgrep plutil ps shasum; do
            require_command "$command_name"
        done
        case "$4" in
            phase-1) staged_phase_one "$2" "$3" "$4" ;;
            phase-2) staged_phase_two "$2" "$3" "$4" ;;
            *) fail "unsupported staged P8 phase: $4" ;;
        esac
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
        fail "usage: scripts/verify-p8-terminal-memory.sh --fixture NAME | --self-test | --staged PROFILE APP PHASE | --cleanup-only OWNED_ROOT"
        ;;
esac
