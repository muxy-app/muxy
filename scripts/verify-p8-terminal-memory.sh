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
    if [[ -n "$ACTIVE_ROOT" && -f "$ACTIVE_ROOT/.phase3-runtime" ]] && ! cleanup_phase_three_runtime "$ACTIVE_ROOT"; then
        printf 'error: Phase 3 owned runtime cleanup failed; preserving %s\n' "$ACTIVE_ROOT" >&2
        return 1
    fi
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

prepare_unique_tmp_root() {
    local phase="$1" root
    root="$(mktemp -d "$ISOLATED_TMP_ROOT/p8-isolated-test-staged-$phase.XXXXXX")"
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

stop_phase_three_cleanup_app() {
    local pid="$APP_PID"
    stop_job_process "$pid"
    wait "$pid" 2>/dev/null || true
    APP_PID=""
}

cleanup_phase_three_runtime() {
    local root="$1" executable profile app_support home tmp xdg app_socket session_socket attempt exit_status
    executable="$(sed -n '1p' "$root/.phase3-runtime")"
    profile="$(sed -n '2p' "$root/.phase3-runtime")"
    [[ -x "$executable" && ( "$profile" == debug || "$profile" == release ) ]] || return 1
    app_support="$root/app"
    home="$root/home"
    tmp="$root/tmp"
    xdg="$root/xdg"
    app_socket="$app_support/muxy.sock"
    session_socket="$app_support/sessions/control.sock"
    if [[ "$profile" == debug ]]; then
        app_socket="$app_support/muxy-dev.sock"
        session_socket="$app_support/sessions-dev/control.sock"
    fi
    printf '{"muxy.terminalPersistentSession.enabled":false}\n' > "$app_support/settings.json"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$root/cleanup-app.log" 2>&1 &
    APP_PID=$!
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -S "$app_socket" ]] && break
        if ! kill -0 "$APP_PID" 2>/dev/null; then
            wait "$APP_PID" 2>/dev/null || true
            APP_PID=""
            return 1
        fi
        sleep 0.05
    done
    if [[ ! -S "$app_socket" ]]; then
        stop_phase_three_cleanup_app
        return 1
    fi
    if ! osascript -e 'tell application id "com.muxy.tests" to quit' >/dev/null 2>&1; then
        stop_phase_three_cleanup_app
        return 1
    fi
    for ((attempt = 0; attempt < 400; attempt++)); do
        ! kill -0 "$APP_PID" 2>/dev/null && break
        sleep 0.05
    done
    if kill -0 "$APP_PID" 2>/dev/null; then
        stop_phase_three_cleanup_app
        return 1
    fi
    set +e
    wait "$APP_PID" 2>/dev/null
    exit_status=$?
    set -e
    APP_PID=""
    [[ "$exit_status" == 0 ]] || return 1
    for ((attempt = 0; attempt < 600; attempt++)); do
        [[ ! -S "$session_socket" ]] && break
        sleep 0.05
    done
    [[ ! -S "$session_socket" ]] || return 1
    rm -f -- "$root/.phase3-runtime"
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

phase_three_source_checks() {
    phase_two_source_checks
    for path in \
        crates/muxy/src/sessions/mod.rs \
        crates/muxy/src/terminal/surfaces.rs \
        crates/muxy/src/views/window/terminal.rs \
        crates/muxy/src/views/window/render.rs \
        crates/muxy/src/views/settings/categories/terminal.rs; do
        [[ -f "$path" ]] || fail "missing Phase 3 source: $path"
    done
    rg -q '^muxy-session\.workspace = true$' crates/muxy/Cargo.toml || fail "muxy app session facade dependency is missing"
    rg -q 'SessionCoordinator::start' crates/muxy/src/main.rs || fail "session startup coordinator is not wired"
    rg -q 'if !self\.sessions\.is_ready\(\)' crates/muxy/src/views/window/terminal.rs || fail "workspace terminal startup barrier is missing"
    rg -q 'spawn_attachment' crates/muxy/src/terminal/ghostty/mod.rs crates/muxy/src/terminal/surfaces.rs || fail "Ghostty attachment path is missing"
    rg -q 'SessionsRestartRequired' crates/muxy/src/views/settings/mod.rs crates/muxy/src/views/window/commands.rs || fail "restart-required setting effect is missing"
    rg -q 'ConfirmSessionsDisable' crates/muxy/src/views/settings/mod.rs crates/muxy/src/views/window/overlays.rs || fail "counted disable confirmation is missing"
    rg -q 'retry_session_attachment' crates/muxy/src/views/window/terminal.rs crates/muxy/src/views/workspace_view.rs || fail "attachment retry action is missing"
    rg -q 'Changing this setting requires restarting Muxy' crates/muxy/src/views/settings/categories/terminal.rs || fail "restart-required settings copy is missing"
    rg -q 'Background session is missing' crates/muxy/src/views/window/terminal.rs || fail "missing-session presentation is absent"
    rg -q 'Background session has ended' crates/muxy/src/views/window/terminal.rs || fail "ended-session presentation is absent"
    ! rg -q 'terminal-session-mode\.json' crates || fail "forbidden session mode marker is referenced"
}

phase_four_source_checks() {
    phase_three_source_checks
    [[ -f crates/muxy-session/tests/lifecycle.rs ]] || fail "missing Phase 4 lifecycle integration test"
    rg -q 'pub enum OwnerExistence' crates/muxy-api/src/truth.rs || fail "owner existence truth is missing"
    rg -q 'confirmed_missing_scopes' crates/muxy/src/sessions/mod.rs || fail "two-observation owner reconciliation is missing"
    rg -q 'SessionCoordinator::close_plan' crates/muxy/src/views/window/workspace.rs || fail "immutable close planning is missing"
    rg -q 'send_tab_to_background' crates/muxy/src/views/window/workspace.rs || fail "Send to Background action is missing"
    rg -q 'SessionReattachOutcome' crates/muxy/src/sessions/mod.rs || fail "exact session reattach model is missing"
    rg -q 'matches_operation\(&token\)' crates/muxy/src/views/window/lifecycle.rs crates/muxy/src/views/window/overlays.rs || fail "owner deletion token revalidation is missing"
    rg -q 'end_owner_cleanup_plan' crates/muxy/src/views/window/lifecycle.rs crates/muxy/src/views/window/overlays.rs || fail "immutable owner cleanup planning is missing"
    rg -q 'prepare_staged_phase_four' crates/muxy/src/views/window/mod.rs crates/muxy/src/views/window/lifecycle.rs || fail "app-owned staged lifecycle hook is missing"
    rg -q 'StartNewTerminal' crates/muxy/src/views/workspace_view.rs || fail "Start New Terminal action is missing"
}

phase_five_source_checks() {
    phase_four_source_checks
    for path in \
        crates/muxy-terminal/src/offline/process.rs \
        crates/muxy/src/terminal/idle.rs; do
        [[ -f "$path" ]] || fail "missing Phase 5 source: $path"
    done
    rg -q 'pub struct WakeQueue' crates/muxy-terminal/src/offline/state.rs || fail "bounded wake operation queue is missing"
    rg -q 'terminal_root_from_snapshot' crates/muxy-terminal/src/offline/process.rs || fail "terminal process root resolution is missing"
    rg -q 'cell_facts' crates/ghostty-host/src/surface.rs crates/muxy-terminal/src/ghostty/host_view.rs || fail "neutral Ghostty cell facts are missing"
    rg -q 'idle_reconcile_requested' crates/muxy/src/terminal/surfaces.rs crates/muxy/src/views/window/lifecycle.rs || fail "idle wake reconciliation is missing"
    rg -q 'session_identities' crates/muxy/src/terminal/surfaces.rs crates/muxy/src/views/window/terminal.rs || fail "persistent idle backing does not use immutable session identity"
    rg -q 'pending_command\.remove\(&request\.tab_id\)' crates/muxy/src/terminal/surfaces.rs || fail "ordinary wake can rerun a startup command"
    rg -q 'Frees a hidden idle terminal' crates/muxy/src/views/settings/categories/terminal.rs || fail "idle sleeping disclosure is missing"
}

phase_six_source_checks() {
    phase_five_source_checks
    for path in \
        crates/muxy/src/resource_monitor/mod.rs \
        crates/muxy/src/resource_monitor/macos.rs \
        crates/muxy/src/resource_monitor/unsupported.rs; do
        [[ -f "$path" ]] || fail "missing Phase 6 source: $path"
    done
    rg -q 'process_tree_resources' crates/muxy-core/src/resources.rs crates/muxy/src/resource_monitor/mod.rs || fail "identity-safe resource tree aggregation is missing"
    rg -q 'proc_pid_rusage' crates/muxy/src/resource_monitor/macos.rs || fail "macOS resource sampling is missing"
    rg -q 'resource_roots' crates/muxy/src/sessions/mod.rs crates/muxy/src/views/window/mod.rs || fail "authenticated daemon and shell resource roots are missing"
    rg -q 'Effect::ResourceStatus' crates/muxy/src/views/settings/mod.rs crates/muxy/src/views/window/commands.rs || fail "resource setting runtime effect is missing"
    rg -q 'status_trailing_group' crates/muxy/src/views/status_bar.rs || fail "neutral status trailing group is missing"
    rg -q 'status_trailing_items' crates/muxy/src/views/app.rs || fail "Composer and resource status composition is missing"
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

transitions_fixture() {
    phase_three_source_checks
    cargo test -p muxy-core --locked --offline session::transition
    cargo test -p muxy --locked --offline sessions_disable_acknowledges_cleanup_before_clearing_durable_links
    cargo test -p muxy --locked --offline settings_persistent_disable_requires_confirmation_before_persistence
    cargo test -p muxy --locked --offline sessions_disable_confirmation_counts_the_destructive_scope
    cargo test -p muxy --locked --offline sessions_startup_barrier_and_restart_mode_are_explicit
    printf 'P8 restart-only transition fixture passed\n'
}

renderer_attachment_fixture() {
    phase_three_source_checks
    cargo test -p muxy --locked --offline sessions_enable_links_every_local_terminal_before_surface_materialization
    cargo test -p muxy --locked --offline sessions_attachment_retry_reuses_only_the_running_linked_session
    cargo test -p muxy --locked --offline session_attachment_exit_drops_only_the_proxy_surface
    printf 'P8 visible renderer attachment fixture passed\n'
}

exclusions_fixture() {
    phase_three_source_checks
    cargo test -p muxy --locked --offline sessions_exclude_remote_workspaces_and_escape_each_attach_argument
    cargo test -p muxy --locked --offline quick_terminal_session_creates_lazily_and_retains_one_surface
    printf 'P8 Quick Terminal and remote exclusion fixture passed\n'
}

close_background_reattach_fixture() {
    phase_four_source_checks
    cargo test -p muxy --locked --offline session_lifecycle_close_plan_ends_persistent_backing_before_workspace_mutation
    cargo test -p muxy --locked --offline session_lifecycle_background_and_reattach_preserve_identity_and_startup_command
    printf 'P8 close, Background, exact reattach, and Focus fixture passed\n'
}

all_close_modes_fixture() {
    phase_four_source_checks
    cargo test -p muxy --locked --offline session_lifecycle_all_close_modes_partition_mixed_tabs_and_validate_before_cleanup
    printf 'P8 all CloseMode candidate partitioning fixture passed\n'
}

owner_deletion_transaction_fixture() {
    phase_four_source_checks
    cargo test -p muxy --locked --offline session_lifecycle_owner_deletion_token_survives_confirmation_and_rejects_replacement
    cargo test -p muxy --locked --offline session_lifecycle_exact_owner_cleanup_never_ends_unrelated_sessions
    printf 'P8 exact-owner deletion transaction fixture passed\n'
}

external_owner_truth_fixture() {
    phase_four_source_checks
    cargo test -p muxy-api --locked --offline truth_owner_existence
    cargo test -p muxy --locked --offline session_lifecycle_external_owner_truth_requires_two_fresh_missing_observations
    printf 'P8 external owner truth fixture passed\n'
}

quit_crash_missing_fixture() {
    phase_four_source_checks
    cargo test -p muxy-session --test lifecycle --locked --offline lifecycle_background_reattach_quit_exact_owner_cleanup_and_shell_exit_are_distinct -- --exact
    cargo test -p muxy --locked --offline sessions_recover_exact_owner_and_surface_missing_or_ended_links
    cargo test -p muxy --locked --offline session_attachment_exit_drops_only_the_proxy_surface
    printf 'P8 quit, daemon lifecycle, attachment failure, missing, and shell-exit fixture passed\n'
}

startup_command_once_fixture() {
    phase_four_source_checks
    cargo test -p muxy-session --test lifecycle --locked --offline lifecycle_background_reattach_quit_exact_owner_cleanup_and_shell_exit_are_distinct -- --exact
    cargo test -p muxy --locked --offline session_lifecycle_background_and_reattach_preserve_identity_and_startup_command
    printf 'P8 one-shot startup command fixture passed\n'
}

idle_policy_fixture() {
    phase_five_source_checks
    cargo test -p muxy-terminal --locked --offline offline_policy_requires_every_sleep_predicate
    cargo test -p muxy --locked --offline terminal_idle_only_sleeps_hidden_surfaces
    cargo test -p muxy --locked --offline terminal_idle_all_activity_sources_advance_the_generation
    printf 'P8 hidden-only idle policy and activity fixture passed\n'
}

idle_races_fixture() {
    phase_five_source_checks
    cargo test -p muxy-terminal --locked --offline stale_timer_cannot_sleep_replaced_or_active_surface
    cargo test -p muxy-terminal --locked --offline wake_queue_orders_input_and_resize_and_bounds_bytes_and_operations
    cargo test -p muxy --locked --offline terminal_idle_stale_generation_surface_and_selection_fail_awake
    cargo test -p muxy --locked --offline terminal_idle_wake_queue_preserves_input_resize_order_and_bounds
    printf 'P8 stale timer and bounded wake ordering fixture passed\n'
}

idle_process_safety_fixture() {
    phase_five_source_checks
    cargo test -p muxy-terminal --locked --offline offline_process -- --nocapture
    cargo test -p muxy --locked --offline terminal_idle_transactions_and_unknown_process_facts_fail_awake
    printf 'P8 foreground, descendant, alternate-screen, unknown, and transaction safety fixture passed\n'
}

persistent_sleep_wake_fixture() {
    phase_five_source_checks
    cargo test -p muxy --locked --offline terminal_idle_persistent_sleep_drops_only_renderer_and_retains_attachment
    cargo test -p muxy --locked --offline terminal_idle_persistent_wake_preserves_session_identity_and_input
    printf 'P8 persistent renderer sleep and same-session wake fixture passed\n'
}

ordinary_sleep_wake_fixture() {
    phase_five_source_checks
    cargo test -p muxy --locked --offline terminal_idle_ordinary_sleep_restores_runtime_cwd_without_startup_command
    cargo test -p muxy --locked --offline terminal_idle_ordinary_sleep_uses_latest_working_directory
    printf 'P8 ordinary safe sleep, cwd restore, and startup-command suppression fixture passed\n'
}

resource_math_fixture() {
    phase_six_source_checks
    cargo test -p muxy-core --locked --offline resources::tests::aggregation_deduplicates_overlapping_roots_and_sums_memory_once -- --exact
    cargo test -p muxy-core --locked --offline resources::tests::pid_start_mismatch_cannot_contribute_reused_process_cpu -- --exact
    cargo test -p muxy --locked --offline resource_monitor::tests::resource_monitor_aggregates_overlapping_app_daemon_shell_and_grandchild_once -- --exact
    cargo test -p muxy --locked --offline resource_monitor::tests::resource_monitor_rejects_pid_reuse_and_reports_stale_without_false_zero -- --exact
    cargo test -p muxy --locked --offline resource_monitor::tests::resource_monitor_disable_stops_requests_and_reenable_uses_fresh_baseline -- --exact
    cargo test -p muxy --locked --offline views::app::tests::resource_status_and_composer_use_distinct_ordered_trailing_slots -- --exact
    printf 'P8 resource delta, deduplication, identity, stale, enable transition, and status coexistence fixture passed\n'
}

resource_process_tree_fixture() {
    phase_six_source_checks
    cargo test -p muxy-core --locked --offline resources::tests::process_tree -- --nocapture
    cargo test -p muxy --locked --offline resource_monitor::macos::tests::resource_monitor_macos_process_tree_contains_owned_child_once -- --exact --nocapture
    printf 'P8 authenticated process-tree expansion and real owned-child fixture passed\n'
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

close_phase_three_app() {
    local log="$1" attempt exit_status
    osascript -e 'tell application id "com.muxy.tests" to quit'
    for ((attempt = 0; attempt < 400; attempt++)); do
        ! kill -0 "$APP_PID" 2>/dev/null && break
        sleep 0.05
    done
    ! kill -0 "$APP_PID" 2>/dev/null || fail "staged Phase 3 app did not close normally"
    set +e
    wait "$APP_PID"
    exit_status=$?
    set -e
    APP_PID=""
    [[ "$exit_status" == 0 ]] || {
        cat "$log"
        fail "staged Phase 3 app exited with status $exit_status"
    }
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
    root="$(prepare_unique_tmp_root phase2)"
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

staged_phase_three() {
    local profile="$1" app="$2" case_name="$3" root app_support home tmp xdg app_socket
    local session_socket executable cli project log attempt count=0 before_ids after_ids reopened_ids
    local tabs visible_id hidden_id other_id screen visible_ready=0 token="P8_PHASE3_ATTACHMENT_OK"
    [[ "$(uname -s)" == Darwin ]] || fail "staged verification requires macOS"
    [[ "$profile" == debug || "$profile" == release ]] || fail "invalid staged profile: $profile"
    [[ "$case_name" == phase-3 ]] || fail "unsupported Phase 3 staged case: $case_name"
    validate_staged_app "$app"
    phase_three_source_checks
    root="$(prepare_unique_tmp_root phase3)"
    ACTIVE_ROOT="$root"
    app_support="$root/app"
    home="$root/home"
    tmp="$root/tmp"
    xdg="$root/xdg"
    project="$root/project"
    mkdir -p "$app_support" "$home" "$tmp" "$xdg" "$project"
    git -C "$project" init -q
    app_socket="$app_support/muxy.sock"
    session_socket="$app_support/sessions/control.sock"
    if [[ "$profile" == debug ]]; then
        app_socket="$app_support/muxy-dev.sock"
        session_socket="$app_support/sessions-dev/control.sock"
    fi
    ACTIVE_SOCKET="$app_socket"
    executable="$app/Contents/MacOS/MuxyTests"
    cli="$app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    printf '%s\n%s\n' "$executable" "$profile" > "$root/.phase3-runtime"
    log="$root/ordinary.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$log" 2>&1 &
    APP_PID=$!
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -S "$app_socket" ]] && break
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$log"
            fail "staged Phase 3 ordinary app exited before binding its socket"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" ]] || fail "staged Phase 3 ordinary app socket was not created"
    MUXY_SOCKET_PATH="$app_socket" "$cli" create-project "$project" --name P8Phase3 >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" new-tab >/dev/null
    printf '{"muxy.terminalPersistentSession.enabled":true}\n' > "$app_support/settings.json"
    sleep 0.2
    [[ ! -S "$session_socket" ]] || fail "session mode changed without restart"
    close_phase_three_app "$log"

    log="$root/persistent.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$log" 2>&1 &
    APP_PID=$!
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -S "$app_socket" && -S "$session_socket" ]] && break
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$log"
            fail "staged Phase 3 persistent app exited during startup"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" && -S "$session_socket" ]] || {
        cat "$log"
        fail "persistent session sockets were not created"
    }
    for ((attempt = 0; attempt < 400; attempt++)); do
        count="$( (rg -o '"sessionId"[[:space:]]*:[[:space:]]*"[0-9A-F-]{36}"' "$app_support/workspaces.json" 2>/dev/null || true) | wc -l | tr -d ' ')"
        ((count >= 2)) && break
        sleep 0.05
    done
    ((count >= 2)) || {
        cat "$log"
        fail "eligible local tabs were not durably linked before renderer startup"
    }
    before_ids="$(rg -o '"sessionId"[[:space:]]*:[[:space:]]*"[0-9A-F-]{36}"' "$app_support/workspaces.json" | sort)"
    tabs="$(MUXY_SOCKET_PATH="$app_socket" "$cli" list-tabs)"
    visible_id="$(printf '%s\n' "$tabs" | awk -F $'\t' '$5 == "true" { print $2; exit }')"
    hidden_id="$(printf '%s\n' "$tabs" | awk -F $'\t' '$5 == "false" { print $2; exit }')"
    [[ "$visible_id" =~ ^[0-9A-F-]{36}$ && "$hidden_id" =~ ^[0-9A-F-]{36}$ ]] || fail "could not identify visible and hidden terminal tabs"
    for ((attempt = 0; attempt < 20; attempt++)); do
        if MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$visible_id" --lines 20 > "$root/visible-screen.txt" 2>/dev/null; then
            visible_ready=1
            break
        fi
        sleep 0.05
    done
    ((visible_ready == 1)) || fail "visible persistent renderer did not materialize"
    if MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$hidden_id" --lines 20 > "$root/hidden-screen.txt" 2>&1; then
        fail "hidden persistent tab unexpectedly had a renderer"
    fi
    rg -q 'pane surface not ready' "$root/hidden-screen.txt" || fail "hidden renderer absence was not observed"
    MUXY_SOCKET_PATH="$app_socket" "$cli" switch-tab "$hidden_id" >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" send --pane "$hidden_id" "printf '$token\\n'" >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" send-keys --pane "$hidden_id" Enter >/dev/null
    for ((attempt = 0; attempt < 100; attempt++)); do
        screen="$(MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$hidden_id" --lines 50 2>/dev/null || true)"
        [[ "$screen" == *"$token"* ]] && break
        sleep 0.05
    done
    [[ "$screen" == *"$token"* ]] || fail "selected hidden tab did not attach and return terminal output"
    other_id="$visible_id"
    close_phase_three_app "$log"
    [[ -S "$session_socket" ]] || fail "background session daemon did not survive normal app close"

    log="$root/reopen.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$log" 2>&1 &
    APP_PID=$!
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -S "$app_socket" && -S "$session_socket" ]] && break
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$log"
            fail "staged Phase 3 reopen app exited during startup"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" && -S "$session_socket" ]] || fail "staged Phase 3 reopen sockets were not created"
    reopened_ids="$(rg -o '"sessionId"[[:space:]]*:[[:space:]]*"[0-9A-F-]{36}"' "$app_support/workspaces.json" | sort)"
    [[ "$reopened_ids" == "$before_ids" ]] || fail "reopen changed durable session IDs"
    for ((attempt = 0; attempt < 100; attempt++)); do
        screen="$(MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$hidden_id" --lines 50 2>/dev/null || true)"
        [[ "$screen" == *"$token"* ]] && break
        sleep 0.05
    done
    [[ "$screen" == *"$token"* ]] || fail "reopen did not reattach the exact selected session with replay"
    if MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$other_id" --lines 20 > "$root/reopen-hidden-screen.txt" 2>&1; then
        fail "reopen materialized a hidden persistent renderer"
    fi
    rg -q 'pane surface not ready' "$root/reopen-hidden-screen.txt" || fail "visible-only renderer restoration was not observed"
    printf '{"muxy.terminalPersistentSession.enabled":false}\n' > "$app_support/settings.json"
    sleep 0.2
    after_ids="$(rg -o '"sessionId"[[:space:]]*:[[:space:]]*"[0-9A-F-]{36}"' "$app_support/workspaces.json" | sort)"
    [[ "$after_ids" == "$before_ids" && -S "$session_socket" ]] || fail "disable changed live sessions without restart"
    close_phase_three_app "$log"
    [[ -S "$session_socket" ]] || fail "background session daemon did not survive disable-request close"

    log="$root/disable.log"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$log" 2>&1 &
    APP_PID=$!
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -S "$app_socket" ]] && break
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$log"
            fail "staged Phase 3 disable app exited during startup"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" ]] || fail "staged Phase 3 disable app socket was not created"
    for ((attempt = 0; attempt < 400; attempt++)); do
        if ! rg -q '"sessionId"[[:space:]]*:' "$app_support/workspaces.json"; then
            break
        fi
        sleep 0.05
    done
    ! rg -q '"sessionId"[[:space:]]*:' "$app_support/workspaces.json" || {
        cat "$log"
        fail "disable did not clear durable session links after cleanup"
    }
    close_phase_three_app "$log"
    for ((attempt = 0; attempt < 600; attempt++)); do
        [[ ! -S "$session_socket" ]] && break
        sleep 0.05
    done
    [[ ! -S "$session_socket" ]] || fail "disabled session daemon socket remained"
    [[ ! -S "$app_socket" ]] || fail "staged Phase 3 app socket remained"
    rm -f -- "$root/.phase3-runtime"
    cleanup_owned_root "$root"
    ACTIVE_SOCKET=""
    ACTIVE_ROOT=""
    printf 'P8 staged Phase 3 restart-only transition, hidden/visible renderer, attachment replay, exact-ID reopen, and disable cleanup passed\n'
}

staged_phase_four() {
    local profile="$1" app="$2" case_name="$3" root app_support home tmp xdg app_socket session_socket
    local executable cli helper app_log reopen_log project_one project_two status proceed attempt state pid held role
    [[ "$(uname -s)" == Darwin ]] || fail "staged verification requires macOS"
    [[ "$profile" == debug || "$profile" == release ]] || fail "invalid staged profile: $profile"
    [[ "$case_name" == phase-4 ]] || fail "unsupported Phase 4 staged case: $case_name"
    validate_staged_app "$app"
    phase_four_source_checks
    root="$(prepare_unique_tmp_root phase4)"
    ACTIVE_ROOT="$root"
    app_support="$root/app"
    home="$root/home"
    tmp="$root/tmp"
    xdg="$root/xdg"
    project_one="$root/project-one"
    project_two="$root/project-two"
    mkdir -p "$app_support" "$home" "$tmp" "$xdg" "$project_one" "$project_two"
    git -C "$project_one" init -q
    git -C "$project_two" init -q
    app_socket="$app_support/muxy.sock"
    session_socket="$app_support/sessions/control.sock"
    if [[ "$profile" == debug ]]; then
        app_socket="$app_support/muxy-dev.sock"
        session_socket="$app_support/sessions-dev/control.sock"
    fi
    ACTIVE_SOCKET="$app_socket"
    executable="$app/Contents/MacOS/MuxyTests"
    helper="$app/Contents/MacOS/muxy-session"
    cli="$app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    app_log="$root/app.log"
    reopen_log="$root/reopen.log"
    status="$root/.muxy-p8-phase4-status.json"
    proceed="$root/.muxy-p8-phase4-proceed"
    [[ -x "$helper" ]] || fail "staged session helper is missing"
    codesign --verify --strict "$helper"
    printf '%s\n%s\n' "$executable" "$profile" > "$root/.phase3-runtime"
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
            fail "staged Phase 4 seed app exited before binding its socket"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" ]] || fail "staged Phase 4 seed app socket was not created"
    MUXY_SOCKET_PATH="$app_socket" "$cli" create-project "$project_one" --name P8Phase4One >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" new-tab >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" create-project "$project_two" --name P8Phase4Two >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" new-tab >/dev/null
    printf '{"muxy.terminalPersistentSession.enabled":true}\n' > "$app_support/settings.json"
    close_phase_three_app "$app_log"

    MUXY_TEST_P8_PHASE4_CASE=background \
        MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$app_log" 2>&1 &
    APP_PID=$!
    for ((attempt = 0; attempt < 400; attempt++)); do
        state="$(sed -n 's/.*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$status" 2>/dev/null || true)"
        [[ -S "$app_socket" && -S "$session_socket" && "$state" == background ]] && break
        [[ "$state" != error ]] || {
            cat "$status"
            cat "$app_log"
            fail "staged Phase 4 app reported a Background error"
        }
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$app_log"
            fail "staged Phase 4 Background app exited"
        }
        sleep 0.05
    done
    [[ "$state" == background && -f "$root/owned-processes" && -S "$session_socket" ]] || {
        cat "$app_log"
        fail "staged Phase 4 app did not complete Background"
    }
    while IFS=' ' read -r pid held role; do
        [[ "$pid" =~ ^[1-9][0-9]*$ ]] || fail "invalid owned $role PID"
        [[ "$held" =~ ^[1-9][0-9]*$ ]] || fail "invalid owned $role start identity"
        [[ "$role" == shell || "$role" == daemon ]] || fail "invalid owned process role"
    done < "$root/owned-processes"
    close_phase_three_app "$app_log"
    [[ -S "$session_socket" ]] || fail "app-owned Phase 4 daemon did not survive normal close"

    MUXY_TEST_P8_PHASE4_CASE=reopen \
        MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$reopen_log" 2>&1 &
    APP_PID=$!
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -S "$app_socket" && -S "$session_socket" ]] && break
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$reopen_log"
            fail "staged Phase 4 reopen exited before binding its sockets"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" && -S "$session_socket" ]] || fail "staged Phase 4 reopen sockets were not created"
    touch "$proceed"
    for ((attempt = 0; attempt < 400; attempt++)); do
        state="$(sed -n 's/.*"state"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$status" 2>/dev/null || true)"
        [[ "$state" == complete ]] && break
        [[ "$state" != error ]] || {
            cat "$status"
            cat "$reopen_log"
            fail "staged Phase 4 reopen reported an error"
        }
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$reopen_log"
            fail "staged Phase 4 reopen exited before cleanup"
        }
        sleep 0.05
    done
    [[ "$state" == complete ]] || {
        cat "$reopen_log"
        fail "staged Phase 4 app did not complete reattach and cleanup"
    }
    for ((attempt = 0; attempt < 600; attempt++)); do
        [[ ! -S "$session_socket" ]] && break
        sleep 0.05
    done
    [[ ! -S "$session_socket" ]] || fail "app-owned Phase 4 daemon socket remained after End All"
    P8_STAGED_VERIFY_DEAD_ROOT="$root" \
        cargo test -p muxy-session --test staged_helper --locked --offline \
            staged_recorded_processes_are_dead -- --exact >/dev/null
    close_phase_three_app "$reopen_log"
    [[ ! -e "$root/owned-processes" ]] || fail "staged Phase 4 process identities remained"
    [[ ! -S "$app_socket" ]] || fail "staged Phase 4 app socket remained"
    rm -f -- "$root/.phase3-runtime"
    cleanup_owned_root "$root"
    ACTIVE_SOCKET=""
    ACTIVE_ROOT=""
    printf 'P8 staged Phase 4 app-owned Background survival, exact reattach, project deletion cleanup, End All, and zero residue passed\n'
}

staged_phase_five() {
    local profile="$1" app="$2" case_name="$3" root app_support home tmp xdg app_socket session_socket
    local executable cli project app_log tabs visible_id hidden_id target_id other_id screen attempt ready=0
    [[ "$(uname -s)" == Darwin ]] || fail "staged verification requires macOS"
    [[ "$profile" == debug || "$profile" == release ]] || fail "invalid staged profile: $profile"
    [[ "$case_name" == phase-5 ]] || fail "unsupported Phase 5 staged case: $case_name"
    validate_staged_app "$app"
    phase_five_source_checks
    root="$(prepare_unique_tmp_root phase5)"
    ACTIVE_ROOT="$root"
    app_support="$root/app"
    home="$root/home"
    tmp="$root/tmp"
    xdg="$root/xdg"
    project="$root/project"
    mkdir -p "$app_support" "$home" "$tmp" "$xdg" "$project"
    git -C "$project" init -q
    app_socket="$app_support/muxy.sock"
    session_socket="$app_support/sessions/control.sock"
    if [[ "$profile" == debug ]]; then
        app_socket="$app_support/muxy-dev.sock"
        session_socket="$app_support/sessions-dev/control.sock"
    fi
    ACTIVE_SOCKET="$app_socket"
    executable="$app/Contents/MacOS/MuxyTests"
    cli="$app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    app_log="$root/app.log"
    printf '%s\n%s\n' "$executable" "$profile" > "$root/.phase3-runtime"
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
            fail "staged Phase 5 seed app exited before binding its socket"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" ]] || fail "staged Phase 5 seed app socket was not created"
    MUXY_SOCKET_PATH="$app_socket" "$cli" create-project "$project" --name P8Phase5 >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" new-tab >/dev/null
    printf '{"muxy.terminalPersistentSession.enabled":true,"muxy.terminalOffline.enabled":true,"muxy.terminalOffline.idleThresholdSeconds":10}\n' > "$app_support/settings.json"
    close_phase_three_app "$app_log"

    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$app_log" 2>&1 &
    APP_PID=$!
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -S "$app_socket" && -S "$session_socket" ]] && break
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$app_log"
            fail "staged Phase 5 persistent app exited during startup"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" && -S "$session_socket" ]] || fail "staged Phase 5 persistent sockets were not created"
    tabs="$(MUXY_SOCKET_PATH="$app_socket" "$cli" list-tabs)"
    visible_id="$(printf '%s\n' "$tabs" | awk -F $'\t' '$5 == "true" { print $2; exit }')"
    hidden_id="$(printf '%s\n' "$tabs" | awk -F $'\t' '$5 == "false" { print $2; exit }')"
    [[ "$visible_id" =~ ^[0-9A-F-]{36}$ && "$hidden_id" =~ ^[0-9A-F-]{36}$ ]] || fail "could not identify Phase 5 persistent tabs"
    target_id="$visible_id"
    other_id="$hidden_id"
    for ((attempt = 0; attempt < 100; attempt++)); do
        if MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$target_id" --lines 20 >/dev/null 2>&1; then
            ready=1
            break
        fi
        sleep 0.05
    done
    ((ready == 1)) || fail "persistent target renderer did not materialize"
    MUXY_SOCKET_PATH="$app_socket" "$cli" send --pane "$target_id" "printf 'P8_PHASE5_REPLAY\\n'" >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" send-keys --pane "$target_id" Enter >/dev/null
    for ((attempt = 0; attempt < 100; attempt++)); do
        screen="$(MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$target_id" --lines 30 2>/dev/null || true)"
        [[ "$screen" == *P8_PHASE5_REPLAY* ]] && break
        sleep 0.05
    done
    [[ "$screen" == *P8_PHASE5_REPLAY* ]] || fail "persistent replay token was not emitted"
    P8_STAGED_PHASE5_ROOT="$root" \
        P8_STAGED_PHASE5_SOCKET="$session_socket" \
        P8_STAGED_PHASE5_MODE=snapshot \
        cargo test -p muxy-session --test staged_helper --locked --offline \
            staged_phase_five_session_identities_are_stable -- --exact >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" switch-tab "$other_id" >/dev/null
    for ((attempt = 0; attempt < 100; attempt++)); do
        if MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$other_id" --lines 20 >/dev/null 2>&1; then
            break
        fi
        sleep 0.05
    done
    sleep 12
    if MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$target_id" --lines 20 > "$root/persistent-sleep.txt" 2>&1; then
        cat "$app_log"
        fail "hidden persistent renderer remained after the idle timeout"
    fi
    rg -q 'pane surface not ready' "$root/persistent-sleep.txt" || fail "persistent renderer sleep was not observed"
    P8_STAGED_PHASE5_ROOT="$root" \
        P8_STAGED_PHASE5_SOCKET="$session_socket" \
        P8_STAGED_PHASE5_MODE=verify \
        cargo test -p muxy-session --test staged_helper --locked --offline \
            staged_phase_five_session_identities_are_stable -- --exact >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" switch-tab "$target_id" >/dev/null
    for ((attempt = 0; attempt < 120; attempt++)); do
        screen="$(MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$target_id" --lines 40 2>/dev/null || true)"
        [[ "$screen" == *P8_PHASE5_REPLAY* ]] && break
        sleep 0.05
    done
    [[ "$screen" == *P8_PHASE5_REPLAY* ]] || fail "persistent wake did not reattach with replay"
    close_phase_three_app "$app_log"
    printf '{"muxy.terminalPersistentSession.enabled":false}\n' > "$app_support/settings.json"
    cleanup_phase_three_runtime "$root" || fail "staged Phase 5 persistent cleanup failed"

    printf '{"muxy.terminalPersistentSession.enabled":false,"muxy.terminalOffline.enabled":true,"muxy.terminalOffline.idleThresholdSeconds":10}\n' > "$app_support/settings.json"
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
            fail "staged Phase 5 ordinary app exited during startup"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" && ! -S "$session_socket" ]] || fail "ordinary Phase 5 runtime paths differ"
    tabs="$(MUXY_SOCKET_PATH="$app_socket" "$cli" list-tabs)"
    target_id="$(printf '%s\n' "$tabs" | awk -F $'\t' '$5 == "true" { print $2; exit }')"
    other_id="$(printf '%s\n' "$tabs" | awk -F $'\t' -v held="$target_id" '$2 != held { print $2; exit }')"
    [[ "$target_id" =~ ^[0-9A-F-]{36}$ && "$other_id" =~ ^[0-9A-F-]{36}$ ]] || fail "could not identify Phase 5 ordinary tabs"
    MUXY_SOCKET_PATH="$app_socket" "$cli" send --pane "$target_id" "cd '$project'; sleep 30" >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" send-keys --pane "$target_id" Enter >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" switch-tab "$other_id" >/dev/null
    sleep 12
    MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$target_id" --lines 20 >/dev/null 2>&1 || fail "active child did not keep an ordinary renderer awake"
    MUXY_SOCKET_PATH="$app_socket" "$cli" send-keys --pane "$target_id" 'Ctrl+C' >/dev/null
    sleep 1
    MUXY_SOCKET_PATH="$app_socket" "$cli" send --pane "$target_id" "cd '$project'; printf 'P8_ORDINARY_READY\\n'" >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" send-keys --pane "$target_id" Enter >/dev/null
    sleep 1
    MUXY_SOCKET_PATH="$app_socket" "$cli" switch-tab "$other_id" >/dev/null
    sleep 12
    if ! rg -q "P8_PHASE5_SLEEP $target_id" "$app_log"; then
        cat "$app_log"
        fail "safe hidden ordinary renderer remained after the idle timeout"
    fi
    MUXY_SOCKET_PATH="$app_socket" "$cli" switch-tab "$target_id" >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" send --pane "$target_id" 'pwd' >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" send-keys --pane "$target_id" Enter >/dev/null
    for ((attempt = 0; attempt < 120; attempt++)); do
        screen="$(MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$target_id" --lines 30 2>/dev/null || true)"
        [[ "$screen" == *"$project"* ]] && break
        sleep 0.05
    done
    [[ "$screen" == *"$project"* ]] || fail "ordinary wake did not restore the runtime cwd"
    close_phase_three_app "$app_log"
    [[ ! -S "$app_socket" && ! -S "$session_socket" ]] || fail "staged Phase 5 sockets remained"
    rm -f -- "$root/phase5-session-identities"
    cleanup_owned_root "$root"
    ACTIVE_SOCKET=""
    ACTIVE_ROOT=""
    printf 'P8 staged Phase 5 persistent sleep/replay identity, ordinary process safety, cwd wake, and zero residue passed\n'
}

staged_phase_six() {
    local profile="$1" app="$2" case_name="$3" root app_support home tmp xdg app_socket session_socket
    local executable cli project app_log status disable tabs visible_id screen attempt availability enabled
    local resident process_count app_processes session_processes session_count cpu poll_count held_poll_count
    local baseline_session_processes descendant_processes recorded_processes pid held role
    [[ "$(uname -s)" == Darwin ]] || fail "staged verification requires macOS"
    [[ "$profile" == debug || "$profile" == release ]] || fail "invalid staged profile: $profile"
    [[ "$case_name" == phase-6 ]] || fail "unsupported Phase 6 staged case: $case_name"
    validate_staged_app "$app"
    phase_six_source_checks
    root="$(prepare_unique_tmp_root phase6)"
    ACTIVE_ROOT="$root"
    app_support="$root/app"
    home="$root/home"
    tmp="$root/tmp"
    xdg="$root/xdg"
    project="$root/project"
    mkdir -p "$app_support" "$home" "$tmp" "$xdg" "$project"
    git -C "$project" init -q
    app_socket="$app_support/muxy.sock"
    session_socket="$app_support/sessions/control.sock"
    if [[ "$profile" == debug ]]; then
        app_socket="$app_support/muxy-dev.sock"
        session_socket="$app_support/sessions-dev/control.sock"
    fi
    ACTIVE_SOCKET="$app_socket"
    executable="$app/Contents/MacOS/MuxyTests"
    cli="$app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    app_log="$root/app.log"
    status="$root/.muxy-p8-phase6-status.json"
    disable="$root/.muxy-p8-phase6-disable"
    printf '%s\n%s\n' "$executable" "$profile" > "$root/.phase3-runtime"
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
            fail "staged Phase 6 seed app exited before binding its socket"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" ]] || fail "staged Phase 6 seed app socket was not created"
    MUXY_SOCKET_PATH="$app_socket" "$cli" create-project "$project" --name P8Phase6 >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" new-tab >/dev/null
    printf '{"muxy.terminalPersistentSession.enabled":true,"muxy.showResourceUsageInStatusBar":true}\n' > "$app_support/settings.json"
    close_phase_three_app "$app_log"

    MUXY_TEST_P8_PHASE6_CASE=monitor \
        MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        HOME="$home" \
        CFFIXED_USER_HOME="$home" \
        TMPDIR="$tmp" \
        XDG_CONFIG_HOME="$xdg" \
        "$executable" > "$app_log" 2>&1 &
    APP_PID=$!
    for ((attempt = 0; attempt < 400; attempt++)); do
        [[ -S "$app_socket" && -S "$session_socket" && -f "$status" && -f "$root/owned-processes" ]] && break
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$app_log"
            fail "staged Phase 6 monitor app exited during startup"
        }
        sleep 0.05
    done
    [[ -S "$app_socket" && -S "$session_socket" && -f "$status" && -f "$root/owned-processes" ]] || {
        cat "$app_log"
        fail "staged Phase 6 sockets, status, or identities were not created"
    }
    while IFS=' ' read -r pid held role; do
        [[ "$pid" =~ ^[1-9][0-9]*$ ]] || fail "invalid owned $role PID"
        [[ "$held" =~ ^[1-9][0-9]*$ ]] || fail "invalid owned $role start identity"
        [[ "$role" == shell || "$role" == daemon || "$role" == descendant ]] || fail "invalid owned process role"
    done < "$root/owned-processes"
    [[ "$(awk '$3 == "daemon" { count++ } END { print count + 0 }' "$root/owned-processes")" == 1 ]] || fail "staged Phase 6 daemon identity was not recorded exactly once"
    [[ "$(awk '$3 == "shell" { count++ } END { print count + 0 }' "$root/owned-processes")" =~ ^[1-9][0-9]*$ ]] || fail "staged Phase 6 shell identities were not recorded"
    for ((attempt = 0; attempt < 240; attempt++)); do
        availability="$(plutil -extract availability raw -o - "$status" 2>/dev/null || true)"
        session_count="$(plutil -extract sessionCount raw -o - "$status" 2>/dev/null || true)"
        resident="$(plutil -extract residentBytes raw -o - "$status" 2>/dev/null || true)"
        session_processes="$(plutil -extract sessionProcessCount raw -o - "$status" 2>/dev/null || true)"
        if [[ "$availability" == live && "$session_count" =~ ^[1-9][0-9]*$ && "$resident" =~ ^[1-9][0-9]*$ && "$session_processes" =~ ^[1-9][0-9]*$ ]]; then
            break
        fi
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$app_log"
            fail "staged Phase 6 monitor app exited before its first live sample"
        }
        sleep 0.05
    done
    [[ "$availability" == live && "$session_count" =~ ^[1-9][0-9]*$ && "$resident" =~ ^[1-9][0-9]*$ && "$session_processes" =~ ^[1-9][0-9]*$ ]] || {
        cat "$status"
        cat "$app_log"
        fail "staged Phase 6 did not publish a live nonzero session snapshot"
    }
    baseline_session_processes="$session_processes"
    tabs="$(MUXY_SOCKET_PATH="$app_socket" "$cli" list-tabs)"
    visible_id="$(printf '%s\n' "$tabs" | awk -F $'\t' '$5 == "true" { print $2; exit }')"
    [[ "$visible_id" =~ ^[0-9A-F-]{36}$ ]] || fail "could not identify the staged Phase 6 visible tab"
    MUXY_SOCKET_PATH="$app_socket" "$cli" send --pane "$visible_id" "yes > /dev/null & printf 'P8_PHASE6_LOAD\\n'" >/dev/null
    MUXY_SOCKET_PATH="$app_socket" "$cli" send-keys --pane "$visible_id" Enter >/dev/null
    for ((attempt = 0; attempt < 400; attempt++)); do
        screen="$(MUXY_SOCKET_PATH="$app_socket" "$cli" read-screen --pane "$visible_id" --lines 30 2>/dev/null || true)"
        availability="$(plutil -extract availability raw -o - "$status" 2>/dev/null || true)"
        cpu="$(plutil -extract cpuPercent raw -o - "$status" 2>/dev/null || true)"
        resident="$(plutil -extract residentBytes raw -o - "$status" 2>/dev/null || true)"
        process_count="$(plutil -extract processCount raw -o - "$status" 2>/dev/null || true)"
        app_processes="$(plutil -extract appProcessCount raw -o - "$status" 2>/dev/null || true)"
        session_processes="$(plutil -extract sessionProcessCount raw -o - "$status" 2>/dev/null || true)"
        session_count="$(plutil -extract sessionCount raw -o - "$status" 2>/dev/null || true)"
        descendant_processes="$(awk '$3 == "descendant" { count++ } END { print count + 0 }' "$root/owned-processes")"
        recorded_processes="$(awk 'NF == 3 { count++ } END { print count + 0 }' "$root/owned-processes")"
        if [[ "$screen" == *P8_PHASE6_LOAD* && "$availability" == live && "$resident" =~ ^[1-9][0-9]*$ && "$process_count" =~ ^[1-9][0-9]*$ && "$process_count" == "$app_processes" && "$session_processes" =~ ^[1-9][0-9]*$ && "$session_count" =~ ^[1-9][0-9]*$ && "$descendant_processes" =~ ^[1-9][0-9]*$ && "$recorded_processes" == "$session_processes" ]] \
            && ((session_processes > baseline_session_processes)) \
            && awk -v value="$cpu" 'BEGIN { exit !(value + 0 > 0) }'; then
            break
        fi
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$app_log"
            fail "staged Phase 6 monitor app exited during descendant sampling"
        }
        sleep 0.05
    done
    if ! [[ "$screen" == *P8_PHASE6_LOAD* && "$availability" == live && "$resident" =~ ^[1-9][0-9]*$ && "$process_count" =~ ^[1-9][0-9]*$ && "$process_count" == "$app_processes" && "$session_processes" =~ ^[1-9][0-9]*$ && "$session_count" =~ ^[1-9][0-9]*$ && "$descendant_processes" =~ ^[1-9][0-9]*$ && "$recorded_processes" == "$session_processes" ]] \
        || ! ((session_processes > baseline_session_processes)) \
        || ! awk -v value="$cpu" 'BEGIN { exit !(value + 0 > 0) }'; then
        cat "$status"
        cat "$root/owned-processes"
        cat "$app_log"
        fail "staged Phase 6 did not record and count the descendant load with nonzero CPU and RAM"
    fi
    [[ "$(awk '$3 == "daemon" { count++ } END { print count + 0 }' "$root/owned-processes")" == 1 ]] || fail "staged Phase 6 daemon identity count changed"
    [[ "$(awk '$3 == "shell" { count++ } END { print count + 0 }' "$root/owned-processes")" =~ ^[1-9][0-9]*$ ]] || fail "staged Phase 6 shell identity count changed"
    touch "$disable"
    for ((attempt = 0; attempt < 240; attempt++)); do
        enabled="$(plutil -extract enabled raw -o - "$status" 2>/dev/null || true)"
        availability="$(plutil -extract availability raw -o - "$status" 2>/dev/null || true)"
        poll_count="$(plutil -extract pollCount raw -o - "$status" 2>/dev/null || true)"
        if [[ "$enabled" == false && "$availability" == unavailable && "$poll_count" =~ ^[0-9]+$ ]] \
            && rg -q '"cpuPercent":null' "$status" \
            && rg -q '"residentBytes":null' "$status"; then
            break
        fi
        sleep 0.05
    done
    [[ "$enabled" == false && "$availability" == unavailable && "$poll_count" =~ ^[0-9]+$ ]] || {
        cat "$status"
        fail "staged Phase 6 resource disable did not clear prior state"
    }
    held_poll_count="$poll_count"
    sleep 2
    poll_count="$(plutil -extract pollCount raw -o - "$status" 2>/dev/null || true)"
    [[ "$poll_count" == "$held_poll_count" ]] || fail "staged Phase 6 sampling continued after resource disable"
    close_phase_three_app "$app_log"
    [[ -S "$session_socket" ]] || fail "staged Phase 6 daemon did not survive normal app close"
    printf '{"muxy.terminalPersistentSession.enabled":false}\n' > "$app_support/settings.json"
    cleanup_phase_three_runtime "$root" || fail "staged Phase 6 persistent cleanup failed"
    P8_STAGED_VERIFY_DEAD_ROOT="$root" \
        cargo test -p muxy-session --test staged_helper --locked --offline \
            staged_recorded_processes_are_dead -- --exact >/dev/null
    [[ ! -e "$root/owned-processes" ]] || fail "staged Phase 6 process identities remained"
    [[ ! -S "$app_socket" && ! -S "$session_socket" ]] || fail "staged Phase 6 sockets remained"
    cleanup_owned_root "$root"
    ACTIVE_SOCKET=""
    ACTIVE_ROOT=""
    printf 'P8 staged Phase 6 app, daemon, shell, descendant resource sampling, disable stop, and zero residue passed\n'
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
            transitions) transitions_fixture ;;
            renderer-attachment) renderer_attachment_fixture ;;
            exclusions) exclusions_fixture ;;
            close-background-reattach) close_background_reattach_fixture ;;
            all-close-modes) all_close_modes_fixture ;;
            owner-deletion-transaction) owner_deletion_transaction_fixture ;;
            external-owner-truth) external_owner_truth_fixture ;;
            quit-crash-missing) quit_crash_missing_fixture ;;
            startup-command-once) startup_command_once_fixture ;;
            idle-policy) idle_policy_fixture ;;
            idle-races) idle_races_fixture ;;
            idle-process-safety) idle_process_safety_fixture ;;
            persistent-sleep-wake) persistent_sleep_wake_fixture ;;
            ordinary-sleep-wake) ordinary_sleep_wake_fixture ;;
            resource-math) resource_math_fixture ;;
            resource-process-tree) resource_process_tree_fixture ;;
            *) fail "unknown P8 fixture: $2" ;;
        esac
        ;;
    --self-test)
        (($# == 1)) || fail "usage: scripts/verify-p8-terminal-memory.sh --self-test"
        self_test
        ;;
    --staged)
        (($# == 4)) || fail "usage: scripts/verify-p8-terminal-memory.sh --staged PROFILE APP PHASE"
        for command_name in awk cmp codesign git osascript pgrep plutil ps shasum; do
            require_command "$command_name"
        done
        case "$4" in
            phase-1) staged_phase_one "$2" "$3" "$4" ;;
            phase-2) staged_phase_two "$2" "$3" "$4" ;;
            phase-3) staged_phase_three "$2" "$3" "$4" ;;
            phase-4) staged_phase_four "$2" "$3" "$4" ;;
            phase-5) staged_phase_five "$2" "$3" "$4" ;;
            phase-6) staged_phase_six "$2" "$3" "$4" ;;
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
