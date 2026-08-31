#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly VERIFICATION_ROOT="$PROJECT_ROOT/target/test-verification/p8"
readonly CASES_ROOT="$VERIFICATION_ROOT/cases"
readonly OWNER_FILE=".muxy-p8-owner"
readonly OWNER_VALUE="muxy-p8-terminal-memory-v1"
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

path_mode() {
    local path="$1" mode
    if mode="$(stat -f '%Lp' "$path" 2>/dev/null)"; then
        printf '%s\n' "$mode"
    else
        stat -c '%a' "$path" 2>/dev/null
    fi
}

path_uid() {
    local path="$1" uid
    if uid="$(stat -f '%u' "$path" 2>/dev/null)"; then
        printf '%s\n' "$uid"
    else
        stat -c '%u' "$path" 2>/dev/null
    fi
}

root_is_owned() {
    local root="$1" marker="$1/$OWNER_FILE" current_uid
    root_path_is_safe "$root" || return 1
    [[ -d "$root" && ! -L "$root" ]] || return 1
    [[ -f "$marker" && ! -L "$marker" ]] || return 1
    current_uid="$(id -u)"
    [[ "$(path_uid "$root")" == "$current_uid" && "$(path_mode "$root")" == 700 ]] || return 1
    [[ "$(path_uid "$marker")" == "$current_uid" && "$(path_mode "$marker")" == 600 ]] || return 1
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

cleanup_recorded_processes() {
    local root="$1"
    [[ -f "$root/owned-processes" && ! -L "$root/owned-processes" ]] || return 0
    P8_STAGED_CLEANUP_ROOT="$root" \
        cargo test -p muxy-session --test staged_helper --locked --offline \
            staged_bundle_helper_detaches_survives_app_close_and_cleans -- --exact >/dev/null
}

cleanup_owned_root() {
    local root="$1" socket
    root_is_owned "$root" || fail "cleanup root is not P8-owned: $root"
    cleanup_recorded_processes "$root"
    while IFS= read -r socket; do
        if lsof -t -- "$socket" >/dev/null 2>&1; then
            fail "cleanup root contains a live socket: $socket"
        fi
    done < <(find "$root" -type s -print)
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

phase_seven_source_checks() {
    phase_six_source_checks
    require_command shasum
    for path in \
        crates/muxy/src/views/session_manager.rs \
        crates/muxy/src/socket/commands/sessions.rs; do
        [[ -f "$path" ]] || fail "missing Phase 7 source: $path"
    done
    rg -q 'SessionManager' crates/muxy/src/views/overlay.rs crates/muxy/src/views/window/overlays.rs || fail "session manager popover is not wired"
    rg -q 'SessionManagerActionKind::Focus' crates/muxy/src/views/session_manager.rs crates/muxy/src/views/window/lifecycle.rs || fail "session manager Focus action is missing"
    rg -q 'end_all_plan_sessions' crates/muxy/src/sessions/mod.rs crates/muxy/src/views/window/overlays.rs || fail "immutable End All action is missing"
    rg -q 'open_terminal_settings' crates/muxy/src/views/window/overlays.rs || fail "Terminal Settings action is missing"
    rg -q 'P8_IMPLEMENTED_LEGACY_HEADS' crates/muxy/src/socket/catalog.rs || fail "retained P8 CLI heads are not activated"
    rg -q '"list-sessions"' crates/muxy/src/socket/commands/sessions.rs || fail "list-sessions handler is missing"
    rg -q '"kill-session"' crates/muxy/src/socket/commands/sessions.rs || fail "kill-session handler is missing"
    [[ "$(shasum -a 256 Muxy/Resources/scripts/muxy-cli | awk '{ print $1 }')" == e9fe05bf57067cc0bd3345bc37a09730fb44fef85e96a37d18ec92b4d4d7ac32 ]] || fail "retained CLI source bytes changed"
}

phase_eight_source_checks() {
    local pane_session_files
    phase_seven_source_checks
    for path in \
        ARCHITECTURE.md \
        PLAN.md \
        docs/development/session-protocol.md \
        docs/development/testing.md \
        docs/features/background-sessions.md \
        docs/features/terminal.md \
        docs/features/muxy-cli.md \
        docs/user-guide/settings.md; do
        [[ -s "$path" ]] || fail "missing Phase 8 documentation: $path"
    done
    rg -q 'flag\("muxy\.showResourceUsageInStatusBar", true\)' crates/muxy-core/src/prefs/settings.rs || fail "resource status default differs"
    rg -q 'flag\("muxy\.terminalOffline\.enabled", false\)' crates/muxy-core/src/prefs/settings.rs || fail "terminal offline default differs"
    rg -q 'flag\("muxy\.terminalPersistentSession\.enabled", false\)' crates/muxy-core/src/prefs/settings.rs || fail "persistent session default differs"
    rg -q 'double\("muxy\.terminalOffline\.idleThresholdSeconds", 300\.0\)' crates/muxy-core/src/prefs/settings.rs || fail "terminal idle threshold default differs"
    pane_session_files="$(rg -l 'paneSessionID|pane_session_id' crates || true)"
    [[ "$pane_session_files" == crates/muxy-core/src/workspace_store.rs ]] || fail "paneSessionID escaped raw workspace preservation"
    rg -q 'one active renderer' docs/development/session-protocol.md || fail "single-renderer protocol limit is undocumented"
    rg -q 'Manual native acceptance remains pending' docs/features/background-sessions.md || fail "manual acceptance boundary is undocumented"
    rg -q 'P8 — Terminal memory features\. IMPLEMENTED; MANUAL NATIVE ACCEPTANCE PENDING\.' PLAN.md || fail "P8 roadmap status differs"
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
    cargo test -p muxy --locked --offline views::app::tests::composer_resource_and_sessions_use_distinct_ordered_trailing_slots -- --exact
    printf 'P8 resource delta, deduplication, identity, stale, enable transition, and status coexistence fixture passed\n'
}

session_manager_fixture() {
    phase_seven_source_checks
    cargo test -p muxy --locked --offline session_manager
    cargo test -p muxy --locked --offline session_lifecycle_exact_owner_cleanup_never_ends_unrelated_sessions
    printf 'P8 Session Manager sections, actions, confirmations, owner validation, and stale cleanup fixture passed\n'
}

cli_sessions_fixture() {
    phase_seven_source_checks
    cargo test -p muxy --locked --offline socket_list_sessions_preserves_exact_columns_and_placement_attachment
    cargo test -p muxy --locked --offline socket_kill_session_pins_usage_invalid_not_found_and_success
    cargo test -p muxy --locked --offline socket_kill_session_pins_owner_mismatch_and_daemon_unavailable
    cargo test -p muxy --locked --offline socket_catalog_marks_create_worktree_implemented_without_changing_recognition
    printf 'P8 retained CLI session heads, exact columns, errors, permissions, and frozen catalog fixture passed\n'
}

status_trailing_group_fixture() {
    phase_seven_source_checks
    cargo test -p muxy --locked --offline views::app::tests::composer_resource_and_sessions_use_distinct_ordered_trailing_slots -- --exact
    rg -q 'div\(\)\.flex\(\)\.flex_row\(\)\.flex_none\(\)\.items_center\(\)\.h_full\(\)' crates/muxy/src/views/status_bar.rs || fail "status trailing group is not fixed-width at narrow widths"
    rg -q 'status-terminal-sessions' crates/muxy/src/views/status_bar.rs || fail "session status button is missing"
    printf 'P8 Composer, resource, and session status trailing group fixture passed\n'
}

all_fixture() {
    phase_eight_source_checks
    portable_fixture
    protocol_fixture
    security_fixture
    daemon_detach_attach_fixture
    process_cleanup_fixture
    shell_integration_fixture
    transitions_fixture
    renderer_attachment_fixture
    exclusions_fixture
    close_background_reattach_fixture
    all_close_modes_fixture
    owner_deletion_transaction_fixture
    external_owner_truth_fixture
    quit_crash_missing_fixture
    startup_command_once_fixture
    idle_policy_fixture
    idle_races_fixture
    idle_process_safety_fixture
    persistent_sleep_wake_fixture
    ordinary_sleep_wake_fixture
    resource_math_fixture
    resource_process_tree_fixture
    session_manager_fixture
    cli_sessions_fixture
    status_trailing_group_fixture
    printf 'P8 complete fixture matrix passed\n'
}

resource_process_tree_fixture() {
    phase_six_source_checks
    cargo test -p muxy-core --locked --offline resources::tests::process_tree -- --nocapture
    cargo test -p muxy --locked --offline resource_monitor::macos::tests::resource_monitor_macos_process_tree_contains_owned_child_once -- --exact --nocapture
    printf 'P8 authenticated process-tree expansion and real owned-child fixture passed\n'
}

self_test() {
    local nonce="$$" owned unowned mismatch unsafe_mode unsafe_marker stale live linked target outside
    local live_socket live_pid
    phase_eight_source_checks
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
    mkdir -p "$unowned/app"
    printf 'held\n' > "$unowned/app/settings.json"
    printf '%s\n%s\n' /usr/bin/true debug > "$unowned/.phase3-runtime"
    chmod 0600 "$unowned/.phase3-runtime"
    if "$SCRIPT_DIR/verify-p8-terminal-memory.sh" --cleanup-only "$unowned" >/dev/null 2>&1; then
        fail "cleanup-only accepted an unowned runtime record"
    fi
    [[ "$(<"$unowned/app/settings.json")" == held ]] || fail "unowned runtime data was modified"
    mismatch="$CASES_ROOT/self-mismatch-$nonce"
    mkdir -p "$mismatch"
    printf 'wrong\n' > "$mismatch/$OWNER_FILE"
    printf 'held\n' > "$mismatch/sentinel"
    if (cleanup_owned_root "$mismatch") >/dev/null 2>&1; then
        fail "wrong-owner cleanup root was accepted"
    fi
    [[ -f "$mismatch/sentinel" ]] || fail "wrong-owner data was removed"
    unsafe_mode="$CASES_ROOT/self-unsafe-mode-$nonce"
    prepare_root "$unsafe_mode"
    printf 'held\n' > "$unsafe_mode/sentinel"
    chmod 0755 "$unsafe_mode"
    if (cleanup_owned_root "$unsafe_mode") >/dev/null 2>&1; then
        fail "unsafe-mode cleanup root was accepted"
    fi
    [[ -f "$unsafe_mode/sentinel" ]] || fail "unsafe-mode data was removed"
    chmod 0700 "$unsafe_mode"
    cleanup_owned_root "$unsafe_mode"
    unsafe_marker="$CASES_ROOT/self-unsafe-marker-$nonce"
    prepare_root "$unsafe_marker"
    printf 'held\n' > "$unsafe_marker/sentinel"
    chmod 0644 "$unsafe_marker/$OWNER_FILE"
    if (cleanup_owned_root "$unsafe_marker") >/dev/null 2>&1; then
        fail "unsafe ownership-marker mode was accepted"
    fi
    [[ -f "$unsafe_marker/sentinel" ]] || fail "unsafe-marker data was removed"
    chmod 0600 "$unsafe_marker/$OWNER_FILE"
    cleanup_owned_root "$unsafe_marker"
    stale="$CASES_ROOT/self-stale-$nonce"
    prepare_root "$stale"
    printf '%s\n' 'muxy-p8-terminal-memory-v0' > "$stale/$OWNER_FILE"
    printf 'held\n' > "$stale/sentinel"
    if (cleanup_owned_root "$stale") >/dev/null 2>&1; then
        fail "stale ownership marker was accepted"
    fi
    [[ -f "$stale/sentinel" ]] || fail "stale-marker data was removed"
    printf '%s\n' "$OWNER_VALUE" > "$stale/$OWNER_FILE"
    cleanup_owned_root "$stale"
    live="$CASES_ROOT/self-live-socket-$nonce"
    prepare_root "$live"
    live_socket="$live/live.sock"
    nc -lU -w 5 "$live_socket" >/dev/null 2>&1 &
    live_pid=$!
    for _ in {1..100}; do
        [[ -S "$live_socket" ]] && break
        sleep 0.01
    done
    [[ -S "$live_socket" ]] || fail "live-socket self-test listener did not bind"
    if (cleanup_owned_root "$live") >/dev/null 2>&1; then
        fail "cleanup unlinked a live socket"
    fi
    [[ -S "$live_socket" ]] || fail "live socket was removed"
    printf 'done\n' | nc -U "$live_socket" >/dev/null 2>&1 || true
    wait "$live_pid" 2>/dev/null || true
    cleanup_owned_root "$live"
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
    --source-checks)
        (($# == 1)) || fail "usage: scripts/verify-p8-terminal-memory.sh --source-checks"
        phase_eight_source_checks
        printf 'P8 Phase 8 source checks passed\n'
        ;;
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
            session-manager) session_manager_fixture ;;
            cli-sessions) cli_sessions_fixture ;;
            status-trailing-group) status_trailing_group_fixture ;;
            all) all_fixture ;;
            *) fail "unknown P8 fixture: $2" ;;
        esac
        ;;
    --self-test)
        (($# == 1)) || fail "usage: scripts/verify-p8-terminal-memory.sh --self-test"
        for command_name in id lsof nc stat; do
            require_command "$command_name"
        done
        self_test
        ;;
    --staged|--manual)
        fail "app-launching E2E verification is disabled; use headless terminal-memory fixtures and ask the user to verify native behavior"
        ;;
    --cleanup-only)
        (($# == 2)) || fail "usage: scripts/verify-p8-terminal-memory.sh --cleanup-only OWNED_ROOT"
        for command_name in id lsof stat; do
            require_command "$command_name"
        done
        root_is_owned "$2" || fail "cleanup root is not P8-owned: $2"
        rm -f -- "$2/.phase3-runtime"
        cleanup_owned_root "$2"
        printf 'P8 cleanup completed: %s\n' "$2"
        ;;
    *)
        fail "usage: scripts/verify-p8-terminal-memory.sh --source-checks | --fixture NAME | --self-test | --cleanup-only OWNED_ROOT"
        ;;
esac
