#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly VERIFICATION_ROOT="$PROJECT_ROOT/target/test-verification"
readonly P6_ROOT="$VERIFICATION_ROOT/p6/quick-terminal"
readonly ROOT_OWNER_FILE=".muxy-p6-owner"
readonly CASE_OWNER_FILE=".muxy-p6-case-owner"
readonly LINT_BASELINE="$SCRIPT_DIR/lint-suppression-baseline.txt"

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

prepare_root() {
    local marker="$P6_ROOT/$ROOT_OWNER_FILE" held
    reject_symlink_ancestors "$P6_ROOT"
    if [[ -e "$P6_ROOT" ]]; then
        [[ -d "$P6_ROOT" && ! -L "$P6_ROOT" ]] || fail "P6 verification root is unsafe"
        [[ -f "$marker" && ! -L "$marker" ]] || fail "P6 verification root is unowned"
        held="$(cat "$marker")"
        [[ "$held" == "$P6_ROOT" ]] || fail "P6 verification root marker does not match"
    else
        mkdir -p "$P6_ROOT"
        printf '%s\n' "$P6_ROOT" > "$marker"
        chmod 0600 "$marker"
    fi
}

prepare_case_root() {
    local root="$1" marker held
    [[ "$root" == "$P6_ROOT/"* && "$root" != "$P6_ROOT" ]] || fail "case root escaped P6 root"
    [[ "$(dirname "$root")" == "$P6_ROOT" ]] || fail "case root nesting is invalid"
    reject_symlink_ancestors "$root"
    marker="$root/$CASE_OWNER_FILE"
    if [[ -e "$root" ]]; then
        [[ -d "$root" && ! -L "$root" ]] || fail "case root is unsafe"
        [[ -f "$marker" && ! -L "$marker" ]] || fail "case root ownership marker is missing"
        held="$(cat "$marker")"
        [[ "$held" == "$root" ]] || fail "case root ownership marker does not match"
        find "$root" -mindepth 1 -maxdepth 1 ! -name "$CASE_OWNER_FILE" -exec rm -rf -- {} +
    else
        mkdir -p "$root"
        printf '%s\n' "$root" > "$marker"
        chmod 0600 "$marker"
    fi
}

lint_baseline_check() {
    local observed expected
    [[ -f "$LINT_BASELINE" ]] || fail "lint suppression baseline is missing"
    observed="$(mktemp "$VERIFICATION_ROOT/p6-lint-observed.XXXXXX")"
    expected="$(mktemp "$VERIFICATION_ROOT/p6-lint-expected.XXXXXX")"
    rg -n '#\[(allow|expect)\b|\[workspace\.lints|\[lints' Cargo.toml crates \
        --glob '*.toml' --glob '*.rs' | sort > "$observed" || true
    sort "$LINT_BASELINE" > "$expected"
    if ! cmp -s "$expected" "$observed"; then
        diff -u "$expected" "$observed" || true
        rm -f "$observed" "$expected"
        fail "lint suppression baseline changed"
    fi
    rm -f "$observed" "$expected"
}

portable_source_checks() {
    local forbidden actual allowed
    forbidden='gpui|objc2|AppKit|Carbon|CoreGraphics|NS[A-Z]|CGEvent|MainWindow|AppState|ProjectId|WorktreeId|PaneId|TabId|workspace::|views::|routes?::|effects?::'
    if rg -n "$forbidden" crates/muxy-core/src/quick_terminal; then
        fail "portable Quick Terminal core crossed a native, app, or workspace boundary"
    fi
    if rg -n 'static mut|thread_local!' crates/muxy-core/src/quick_terminal crates/muxy-core/src/prefs/settings.rs; then
        fail "Quick Terminal uses process-global or thread-local mutable state"
    fi
    if rg -n '^use ' crates/muxy-core/src/quick_terminal \
        | rg -v ':use (crate::shortcuts|serde::|std::path::Path;)'; then
        fail "portable Quick Terminal module-level import is not allowed"
    fi
    allowed="$(printf '%s\n' \
        ConflictCandidate DoubleShiftConfiguration DoubleShiftDetector DoubleShiftInput \
        Point PresentationPhase PresentationState PresentationTransition QuickTerminalShortcut \
        Rect RegistrationIdentity ShortcutConflict Size collapsed_rect cutout_rect load_from \
        panel_frame preferred_screen_index save_to should_capture_focus should_restore_focus \
        | sort -u)"
    actual="$(rg --no-filename -o '^pub (struct|enum|fn) [A-Za-z_]+' \
        crates/muxy-core/src/quick_terminal | awk '{print $3}' | sort -u)"
    [[ "$actual" == "$allowed" ]] || {
        printf 'allowed APIs:\n%s\nactual APIs:\n%s\n' "$allowed" "$actual"
        fail "portable Quick Terminal public API inventory changed"
    }
}

portable_fixture() {
    prepare_root
    lint_baseline_check
    portable_source_checks
    cargo test -p muxy-core --locked --offline quick_terminal
    printf 'P6 portable fixture passed\n'
}

shortcut_service_source_checks() {
    local file
    for file in \
        crates/muxy/src/quick_terminal/mod.rs \
        crates/muxy/src/quick_terminal/shortcut_service.rs \
        crates/muxy/src/quick_terminal/platform/mod.rs \
        crates/muxy/src/quick_terminal/platform/macos.rs \
        crates/muxy/src/quick_terminal/platform/unsupported.rs; do
        [[ -f "$file" ]] || fail "Phase 2 source is missing: $file"
    done
    if rg -n 'objc2|AppKit|CoreGraphics|NS[A-Z]|CGEvent|link_name|extern "C' \
        crates/muxy/src/quick_terminal/shortcut_service.rs \
        crates/muxy/src/quick_terminal/platform/unsupported.rs; then
        fail "portable or unsupported shortcut service source crossed a native boundary"
    fi
    rg -q 'link_name = "RegisterEventHotKey"' crates/muxy/src/quick_terminal/platform/macos.rs || {
        fail "Carbon hotkey registration is missing"
    }
    rg -q 'CGPreflightListenEventAccess' crates/muxy/src/quick_terminal/platform/macos.rs || {
        fail "passive Input Monitoring access check is missing"
    }
    rg -q 'CGRequestListenEventAccess' crates/muxy/src/quick_terminal/platform/macos.rs || {
        fail "explicit Input Monitoring request is missing"
    }
    rg -q 'cx.set_global\(quick_terminal\)' crates/muxy/src/main.rs || {
        fail "application-scoped shortcut service retention is missing"
    }
    if rg -n 'open_window|WindowOptions|QuickTerminalPanel' \
        crates/muxy/src/quick_terminal/mod.rs \
        crates/muxy/src/quick_terminal/shortcut_service.rs \
        crates/muxy/src/quick_terminal/platform/mod.rs \
        crates/muxy/src/quick_terminal/platform/unsupported.rs; then
        fail "the shortcut service opened a panel"
    fi
}

shortcut_service_fixture() {
    prepare_root
    lint_baseline_check
    shortcut_service_source_checks
    cargo test -p muxy --locked --offline quick_terminal_shortcut
    printf 'P6 shortcut-service fixture passed\n'
}

phase_3_source_checks() {
    local file
    for file in \
        crates/muxy/src/quick_terminal/runtime.rs \
        crates/muxy/src/quick_terminal/session.rs \
        crates/muxy/src/quick_terminal/view.rs \
        crates/muxy/src/terminal/ghostty/mod.rs \
        crates/muxy/src/terminal/unsupported.rs \
        crates/muxy/src/native_compositor.rs; do
        [[ -f "$file" ]] || fail "Phase 3 source is missing: $file"
    done
    rg -q 'WindowKind::PopUp' crates/muxy/src/quick_terminal/runtime.rs || fail "GPUI popup panel route is missing"
    rg -q 'canBecomeMainWindow' crates/muxy/src/quick_terminal/platform/macos.rs || fail "non-main panel override is missing"
    rg -q 'attach_standalone' crates/muxy/src/terminal/ghostty/mod.rs || fail "standalone Ghostty attachment is missing"
    rg -q 'with_overlay_file\(&resources.transparent_surface_config\)' crates/muxy/src/terminal/ghostty/mod.rs || fail "transparent Ghostty overlay is not applied last"
    rg -q 'MUXY_CONTEXT_KEYS' crates/muxy/src/terminal/ghostty/mod.rs || fail "standalone environment scrub is missing"
    rg -q 'on_window_should_close' crates/muxy/src/main.rs || fail "main-window whole-app close handler is missing"
    if rg -n 'SurfaceIdentity::Workspace|PaneLaunchContext|PaneId|ProjectId|WorktreeId|TabId' crates/muxy/src/quick_terminal; then
        fail "Quick Terminal fabricated a workspace identity"
    fi
}

panel_policy_source_checks() {
    phase_3_source_checks
    for file in \
        crates/muxy/src/quick_terminal/panel.rs \
        crates/muxy/src/quick_terminal/runtime.rs \
        crates/muxy/src/quick_terminal/view.rs \
        crates/muxy/src/quick_terminal/platform/macos.rs; do
        [[ -f "$file" ]] || fail "Phase 4 source is missing: $file"
    done
    rg -q 'Duration::from_millis\(340\)' crates/muxy/src/quick_terminal/panel.rs || fail "Quick Terminal show duration changed"
    rg -q 'Duration::from_millis\(180\)' crates/muxy/src/quick_terminal/panel.rs || fail "Quick Terminal hide duration changed"
    rg -q 'preferred_screen_index' crates/muxy/src/quick_terminal/panel.rs || fail "pointer display policy is missing"
    rg -q 'safeAreaInsets' crates/muxy/src/quick_terminal/platform/macos.rs || fail "camera cutout geometry path is missing"
    rg -q 'set_mask_frame' crates/muxy/src/quick_terminal/platform/macos.rs || fail "fixed-viewport reveal mask is missing"
    rg -q 'CATransaction::flush' crates/muxy/src/quick_terminal/platform/macos.rs || fail "first-show collapsed mask is not committed before reveal"
    rg -q 'CALayer' crates/muxy/src/quick_terminal/platform/macos.rs || fail "Core Animation reveal layer is missing"
    rg -q 'mask_host' crates/muxy/src/quick_terminal/platform/macos.rs || fail "reveal mask does not include the native terminal surface"
    rg -q 'setCornerRadius\(corner_radius\)' crates/muxy/src/quick_terminal/platform/macos.rs || fail "continuous rounded reveal corners are missing"
    ! rg -n 'animator\(\)\.setFrame|animate_frame' crates/muxy/src/quick_terminal/platform/macos.rs || fail "panel reveal resizes the terminal viewport"
    rg -q 'terminal\.set_backdrop' crates/muxy/src/quick_terminal/runtime.rs || fail "terminal backdrop tint is not below native glyph rendering"
    ! rg -n '\.bg\(tint\)' crates/muxy/src/quick_terminal/view.rs || fail "GPUI tint overlays and fades native terminal glyphs"
    rg -q 'terminal\.set_window_active\(true\)' crates/muxy/src/quick_terminal/runtime.rs || fail "visible Quick Terminal does not activate the terminal cursor"
    rg -q 'terminal\.set_window_active\(false\)' crates/muxy/src/quick_terminal/runtime.rs || fail "hidden Quick Terminal leaves the terminal focus active"
    rg -q 'accessibilityElementWithRole_frame_label_parent' crates/muxy/src/quick_terminal/platform/macos.rs || fail "native accessibility adapter is missing"
    rg -q 'BridgeAction::Close' crates/muxy/src/quick_terminal/view.rs || fail "Quick Terminal close control is missing"
    rg -q 'KeyBinding::new\("cmd-w", CloseSurface' crates/muxy/src/quick_terminal/mod.rs || fail "Quick Terminal Cmd+W binding is missing"
    rg -q 'HostViewEvent::AppShortcut' crates/muxy/src/terminal/ghostty/mod.rs || fail "native Quick Terminal shortcut routing is missing"
    rg -q 'terminal_shortcut_task' crates/muxy/src/quick_terminal/runtime.rs || fail "Quick Terminal does not consume native terminal shortcuts"
    rg -q 'hide_from_outside_click' crates/muxy/src/quick_terminal/view.rs || fail "Quick Terminal does not hide on outside clicks"
    rg -q 'Icon::Settings' crates/muxy/src/quick_terminal/view.rs || fail "Quick Terminal settings control uses the wrong icon"
    rg -q 'on_window_should_close' crates/muxy/src/quick_terminal/runtime.rs || fail "native Quick Terminal close dispatch is missing"
    rg -q 'release_panel_and_surface' crates/muxy/src/quick_terminal/runtime.rs || fail "Quick Terminal destructive close does not release the surface"
    rg -q 'BridgeAction::OpenSettings' crates/muxy/src/quick_terminal/view.rs || fail "Quick Terminal Open Settings control is missing"
    rg -q 'open_quick_terminal_settings' crates/muxy/src/quick_terminal/runtime.rs || fail "Quick Terminal gear does not open its Settings category"
    ! rg -n 'ToggleQuickSettings|SetQuickSetting|quick_settings_visible' crates/muxy/src/quick_terminal || fail "removed Quick Terminal floating settings controls remain"
    rg -q 'self\.configuration = QuickTerminalConfiguration::load\(\)' crates/muxy/src/quick_terminal/runtime.rs || fail "Quick Terminal does not reload deferred settings before opening"
    rg -q 'try_set_many' crates/muxy/src/quick_terminal/runtime.rs || fail "Quick Terminal Reset is not transactional"
    rg -q 'destroy_disabled_runtime' crates/muxy/src/quick_terminal/runtime.rs || fail "Quick Terminal disable teardown is missing"
    ! rg -n 'KeyBinding::new\([^\n]*escape|Dismiss.*Escape' crates/muxy/src/quick_terminal || fail "Quick Terminal intercepts Escape"
}

panel_policy_fixture() {
    prepare_root
    lint_baseline_check
    panel_policy_source_checks
    cargo test -p muxy --locked --offline quick_terminal_runtime
    cargo test -p muxy --locked --offline quick_terminal_view
    printf 'P6 panel-policy fixture passed\n'
}

settings_transaction_source_checks() {
    local file
    for file in \
        crates/muxy/src/quick_terminal/settings_transaction.rs \
        crates/muxy/src/views/settings/categories/quick_terminal.rs \
        crates/muxy/src/views/settings/mod.rs \
        crates/muxy/src/views/shortcut_editor.rs; do
        [[ -f "$file" ]] || fail "Phase 5 source is missing: $file"
    done
    rg -q 'prepare_shortcut_for_enabled' crates/muxy/src/quick_terminal/settings_transaction.rs || fail "settings transaction does not prepare native registration"
    rg -q 'commit_proposal' crates/muxy/src/quick_terminal/settings_transaction.rs || fail "settings transaction does not commit portable settings"
    rg -q 'service\.commit_shortcut' crates/muxy/src/quick_terminal/settings_transaction.rs || fail "settings transaction does not publish the prepared backend last"
    rg -q 'ShortcutRecorder' crates/muxy/src/quick_terminal/platform/macos.rs || fail "physical shortcut recorder is missing"
    rg -q 'shortcut_recording_action' crates/muxy/src/views/settings/mod.rs || fail "shortcut recorder generation handling is missing"
    rg -q 'category_supported' crates/muxy/src/views/settings/mod.rs || fail "Linux settings category predicate is missing"
    ! rg -n 'Active system-wide' crates/muxy/src/views/settings/categories/quick_terminal.rs || fail "Quick Terminal settings still use a false unconditional status"
}

settings_transaction_fixture() {
    local case_root boundary
    local -a boundaries=(
        quick-terminal-shortcut
        app-shortcuts
        custom-commands
        muxy.quickTerminal.enabled
        muxy.quickTerminal.width
        muxy.ui.scale
        muxy.theme.dark
        editor.richInputFontFamily
        ai.providers
        mobile.approvedDevices
        settings-mirror
    )
    prepare_root
    lint_baseline_check
    settings_transaction_source_checks
    case_root="$P6_ROOT/settings"
    prepare_case_root "$case_root"
    for boundary in "${boundaries[@]}"; do
        mkdir -p "$case_root/$boundary"
        MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$case_root/$boundary" \
            MUXY_TEST_P6_SETTINGS_FAILURE_BOUNDARY="$boundary" \
            cargo test -p muxy-core --locked --offline \
                quick_terminal_settings_injected_commit_failure_restores_exact_files \
                -- --test-threads=1
    done
    cargo test -p muxy-core --locked --offline settings::tests
    cargo test -p muxy --locked --offline quick_terminal_settings
    cargo test -p muxy --locked --offline shortcut_editor
    printf 'P6 settings-transaction fixture passed\n'
}

phase_6_documentation_checks() {
    local file
    for file in \
        ARCHITECTURE.md \
        docs/features/terminal.md \
        docs/user-guide/settings.md \
        docs/user-guide/troubleshooting.md; do
        [[ -f "$file" ]] || fail "P6 documentation is missing: $file"
    done
    if rg -n -i \
        'vibrancy uses a continuous|vibrancy controls how much of the native|background vibrancy continuously|vibrancy control mixes the system material continuously|progressively stronger native material' \
        ARCHITECTURE.md docs/features/terminal.md docs/user-guide/settings.md docs/user-guide/troubleshooting.md; then
        fail "Quick Terminal documentation still claims continuous native blur intensity"
    fi
    rg -q 'any nonzero.*same.*blurred|any nonzero.*same.*blur mode' \
        ARCHITECTURE.md docs/features/terminal.md docs/user-guide/settings.md \
        || fail "binary nonzero Quick Terminal blur mapping is not documented"
    rg -q 'fixed viewport|does not resize the terminal viewport' \
        ARCHITECTURE.md docs/features/terminal.md docs/user-guide/settings.md \
        || fail "fixed-viewport Quick Terminal presentation is not documented"
    rg -q 'Quick Terminal is not available on Linux|macOS-only runtime' \
        docs/features/terminal.md docs/user-guide/settings.md docs/user-guide/troubleshooting.md \
        || fail "Linux Quick Terminal runtime absence is not documented"
    if rg -n -i \
        'Quick Terminal (is|remains) available on Linux|Linux (supports|provides|shows|opens) (the )?Quick Terminal' \
        ARCHITECTURE.md docs; then
        fail "documentation claims a Linux Quick Terminal runtime"
    fi
}

phase_6_scope_checks() {
    local expected_manifests actual_manifests expected_bins actual_bins locked untracked changes
    if rg -n 'static mut|thread_local!' \
        crates/muxy-core/src/quick_terminal crates/muxy-core/src/prefs/settings.rs \
        crates/muxy/src/quick_terminal; then
        fail "Quick Terminal uses process-global or thread-local mutable state"
    fi
    if rg -n 'static [A-Z0-9_]+: (Mutex|RwLock|RefCell|Cell|UnsafeCell|Atomic)' \
        crates/muxy-core/src/quick_terminal crates/muxy-core/src/prefs/settings.rs \
        crates/muxy/src/quick_terminal; then
        fail "Quick Terminal uses a mutable process-global singleton"
    fi
    if rg -n 'objc2|AppKit|CoreGraphics|CGEvent|NSWindow|NSPanel|NSScreen|CALayer|RegisterEventHotKey|extern "C"|#\[link' \
        crates/muxy/src/quick_terminal \
        --glob '!**/platform/macos.rs'; then
        fail "Quick Terminal native implementation escaped the macOS platform adapter"
    fi
    if rg -n -i 'quick.?terminal' crates/muxy-ui crates/muxy-terminal; then
        fail "Quick Terminal caller policy escaped into a reusable UI or terminal crate"
    fi
    if rg -n -i 'quick.?terminal' crates --glob '**/*extension*'; then
        fail "P6 implemented extension shortcut integration owned by P10"
    fi
    if rg -n 'SurfaceIdentity::Workspace|PaneLaunchContext|PaneId|ProjectId|WorktreeId|TabId' \
        crates/muxy/src/quick_terminal; then
        fail "Quick Terminal fabricated a workspace identity"
    fi
    for mutation in \
        NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification \
        NSTextInputContextKeyboardSelectionDidChangeNotification \
        NSApplicationDidChangeScreenParametersNotification; do
        rg -q "$mutation" crates/muxy/src/quick_terminal/platform/macos.rs \
            || fail "Quick Terminal system observer is missing: $mutation"
    done
    rg -q 'runtime\.refresh_on_activation' crates/muxy/src/views/window/mod.rs \
        || fail "Quick Terminal does not refresh Input Monitoring and keyboard layout on activation"
    rg -q 'runtime\.update_appearance' crates/muxy/src/views/window/commands.rs \
        || fail "Quick Terminal does not receive live theme and scale changes"
    expected_manifests="$(printf '%s\n' \
        crates/ghostty-host/Cargo.toml \
        crates/ghostty-sys/Cargo.toml \
        crates/muxy-api/Cargo.toml \
        crates/muxy-core/Cargo.toml \
        crates/muxy-proto/Cargo.toml \
        crates/muxy-session/Cargo.toml \
        crates/muxy-terminal/Cargo.toml \
        crates/muxy-ui/Cargo.toml \
        crates/muxy/Cargo.toml)"
    actual_manifests="$(find crates -mindepth 2 -maxdepth 2 -name Cargo.toml -print | sort)"
    [[ "$actual_manifests" == "$expected_manifests" ]] || {
        printf 'expected crate manifests:\n%s\nactual crate manifests:\n%s\n' "$expected_manifests" "$actual_manifests"
        fail "P6 introduced a new crate"
    }
    expected_bins="$(printf '%s\n' crates/muxy-session/src/main.rs crates/muxy/src/main.rs)"
    actual_bins="$(find crates -type f \( -path '*/src/bin/*' -o -name main.rs \) -print | sort)"
    [[ "$actual_bins" == "$expected_bins" ]] || {
        printf 'expected binaries:\n%s\nactual binaries:\n%s\n' "$expected_bins" "$actual_bins"
        fail "P6 introduced a new executable"
    }
    if rg -n '^\s*\[\[bin\]\]' Cargo.toml crates --glob Cargo.toml; then
        fail "P6 introduced an explicit binary target"
    fi
    for locked in \
        crates/muxy-core/src/migration.rs \
        Muxy \
        Tests \
        resources/Info.plist \
        .github; do
        git diff --quiet -- "$locked" || fail "P6 changed locked path: $locked"
        git diff --cached --quiet -- "$locked" || fail "P6 staged a change to locked path: $locked"
        untracked="$(git ls-files --others --exclude-standard -- "$locked")"
        [[ -z "$untracked" ]] || {
            printf 'untracked locked paths:\n%s\n' "$untracked"
            fail "P6 added an untracked file under locked path: $locked"
        }
    done
    changes="$(git status --short --untracked-files=all -- crates/muxy/src/socket \
        | rg -v 'crates/muxy/src/socket/catalog.rs$|crates/muxy/src/socket/runtime.rs$|crates/muxy/src/socket/commands/mod.rs$|crates/muxy/src/socket/commands/sessions.rs$' || true)"
    [[ -z "$changes" ]] || {
        printf '%s\n' "$changes"
        fail "P6 changed a socket path outside the P8 session CLI integration"
    }
    changes="$(git status --short --untracked-files=all -- crates/muxy-proto \
        | rg -v 'crates/muxy-proto/src/session/|crates/muxy-proto/src/lib.rs$' || true)"
    [[ -z "$changes" ]] || {
        printf '%s\n' "$changes"
        if printf '%s\n' "$changes" | rg -q '^\?\? '; then
            fail "P6 added an untracked file under locked path: crates/muxy-proto"
        fi
        fail "P6 changed locked path: crates/muxy-proto"
    }
}

phase_6_guardrails() {
    prepare_root
    lint_baseline_check
    portable_source_checks
    shortcut_service_source_checks
    panel_policy_source_checks
    settings_transaction_source_checks
    phase_6_documentation_checks
    phase_6_scope_checks
    printf 'P6 Phase 6 guardrails passed\n'
}

self_test() {
    local nonce="$$" missing mismatch outside linked linked_target locked_probe locked_log
    prepare_root
    "$SCRIPT_DIR/stage-test-app.sh" --self-test
    lint_baseline_check
    portable_source_checks
    shortcut_service_source_checks
    phase_3_source_checks
    panel_policy_source_checks
    settings_transaction_source_checks
    phase_6_documentation_checks
    phase_6_scope_checks
    locked_probe="crates/muxy-proto/.p6-self-untracked-$nonce"
    locked_log="$P6_ROOT/self-untracked-locked-$nonce.log"
    printf 'held\n' > "$locked_probe"
    if (phase_6_scope_checks) > "$locked_log" 2>&1; then
        rm -f -- "$locked_probe" "$locked_log"
        fail "P6 self-test accepted an untracked file under a locked path"
    fi
    rm -f -- "$locked_probe"
    rg -q 'untracked file under locked path' "$locked_log" || {
        cat "$locked_log"
        rm -f -- "$locked_log"
        fail "P6 locked-path self-test failed for the wrong reason"
    }
    rm -f -- "$locked_log"
    missing="$P6_ROOT/self-missing-$nonce"
    mkdir -p "$missing"
    printf 'held\n' > "$missing/sentinel"
    if (prepare_case_root "$missing") >/dev/null 2>&1; then
        fail "P6 self-test accepted a missing case marker"
    fi
    [[ -f "$missing/sentinel" ]] || fail "missing case marker deleted data"
    mismatch="$P6_ROOT/self-mismatch-$nonce"
    mkdir -p "$mismatch"
    printf 'other\n' > "$mismatch/$CASE_OWNER_FILE"
    printf 'held\n' > "$mismatch/sentinel"
    if (prepare_case_root "$mismatch") >/dev/null 2>&1; then
        fail "P6 self-test accepted a mismatched case marker"
    fi
    [[ -f "$mismatch/sentinel" ]] || fail "mismatched case marker deleted data"
    linked_target="$P6_ROOT/self-linked-target-$nonce"
    linked="$P6_ROOT/self-linked-$nonce"
    mkdir -p "$linked_target"
    printf 'held\n' > "$linked_target/sentinel"
    ln -s "$linked_target" "$linked"
    if (prepare_case_root "$linked") >/dev/null 2>&1; then
        fail "P6 self-test accepted a symlinked case root"
    fi
    [[ -f "$linked_target/sentinel" ]] || fail "symlinked case rejection deleted data"
    outside="$PROJECT_ROOT/target/p6-self-outside-$nonce"
    mkdir -p "$outside"
    printf 'held\n' > "$outside/sentinel"
    if (prepare_case_root "$outside") >/dev/null 2>&1; then
        fail "P6 self-test accepted an outside case root"
    fi
    [[ -f "$outside/sentinel" ]] || fail "outside case rejection deleted data"
    rm -- "$linked"
    rm -rf -- "$missing" "$mismatch" "$linked_target" "$outside"
    printf 'P6 Quick Terminal verifier self-test passed\n'
}

for command_name in awk cargo cat cmp diff find git ln mktemp rg sort; do
    require_command "$command_name"
done
case "${1:-}" in
    --self-test)
        (($# == 1)) || fail "usage: scripts/verify-p6-quick-terminal.sh --self-test"
        self_test
        ;;
    --fixture)
        (($# == 2)) || fail "usage: scripts/verify-p6-quick-terminal.sh --fixture CASE"
        case "$2" in
            portable) portable_fixture ;;
            shortcut-service) shortcut_service_fixture ;;
            panel-policy) panel_policy_fixture ;;
            settings-transaction) settings_transaction_fixture ;;
            guardrails) phase_6_guardrails ;;
            all)
                phase_6_guardrails
                portable_fixture
                shortcut_service_fixture
                panel_policy_fixture
                settings_transaction_fixture
                ;;
            *) fail "unknown fixture: $2" ;;
        esac
        ;;
    --staged)
        fail "app-launching E2E verification is disabled; use headless Quick Terminal fixtures and ask the user to verify native behavior"
        ;;
    *) fail "usage: scripts/verify-p6-quick-terminal.sh --self-test | --fixture CASE" ;;
esac
