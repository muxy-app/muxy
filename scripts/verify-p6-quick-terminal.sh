#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly VERIFICATION_ROOT="$PROJECT_ROOT/target/test-verification"
readonly APPS_ROOT="$VERIFICATION_ROOT/apps"
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
        crates/muxy/src/socket \
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

run_staged_fixture() {
    local mode="$1" app="$2" fixture="$3" case_root="$4"
    local app_support socket_name socket log executable_name executable app_pid before=""
    local staged_case="" status_file
    app_support="$case_root/$fixture"
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    socket_name="muxy.sock"
    [[ "$mode" == debug ]] && socket_name="muxy-dev.sock"
    socket="$app_support/$socket_name"
    ((${#socket} < 104)) || fail "staged socket path exceeds the macOS limit"
    log="$app_support/app.log"
    case "$fixture" in
        a) ;;
        b)
            printf '{"type":"unknown","held":true}' > "$app_support/quick-terminal-shortcut.json"
            before="$(shasum -a 256 "$app_support/quick-terminal-shortcut.json")"
            ;;
        c) printf '{"type":"unassigned"}' > "$app_support/quick-terminal-shortcut.json" ;;
        d) printf '{"type":"doubleShift"}' > "$app_support/quick-terminal-shortcut.json" ;;
        e)
            printf '{"type":"keyCombo","keyCombo":{"key":"space","modifiers":1048576},"virtualKeyCode":49}' \
                > "$app_support/quick-terminal-shortcut.json"
            ;;
        f)
            printf '{"type":"keyCombo","keyCombo":{"key":"space","modifiers":1048576}}' \
                > "$app_support/quick-terminal-shortcut.json"
            ;;
        g)
            printf '{"type":"doubleShift"}' > "$app_support/quick-terminal-shortcut.json"
            staged_case="backend-failure"
            before="$(shasum -a 256 "$app_support/quick-terminal-shortcut.json")"
            ;;
        h)
            printf '{"type":"doubleShift"}' > "$app_support/quick-terminal-shortcut.json"
            staged_case="persistence-failure"
            before="$(shasum -a 256 "$app_support/quick-terminal-shortcut.json")"
            ;;
        *) fail "unknown staged fixture: $fixture" ;;
    esac
    executable_name="$(plutil -extract CFBundleExecutable raw -o - "$app/Contents/Info.plist")"
    executable="$app/Contents/MacOS/$executable_name"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P6_SHORTCUT_CASE="$staged_case" \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp" \
        XDG_CONFIG_HOME="$app_support/xdg" \
        "$executable" > "$log" 2>&1 &
    app_pid=$!
    for _ in $(jot 400); do
        [[ -S "$socket" ]] && break
        kill -0 "$app_pid" 2>/dev/null || {
            cat "$log"
            fail "staged app exited before binding for fixture $fixture"
        }
        sleep 0.05
    done
    [[ -S "$socket" ]] || fail "staged socket was not created for fixture $fixture"
    MUXY_SOCKET_PATH="$socket" "$(command -v muxy)" list-projects >/dev/null
    if [[ -n "$staged_case" ]]; then
        status_file="$app_support/.muxy-p6-shortcut-status.json"
        for _ in $(jot 200); do
            [[ -f "$status_file" ]] && break
            kill -0 "$app_pid" 2>/dev/null || break
            sleep 0.05
        done
        [[ -f "$status_file" ]] || fail "staged shortcut status was not written for $staged_case"
        [[ "$(plutil -extract case raw -o - "$status_file")" == "$staged_case" ]] || {
            fail "staged shortcut status case did not match"
        }
        [[ "$(plutil -extract result raw -o - "$status_file")" == error ]] || {
            fail "forced staged shortcut update unexpectedly succeeded"
        }
        [[ "$(plutil -extract shortcut raw -o - "$status_file")" == doubleShift ]] || {
            fail "failed staged shortcut update replaced the active shortcut"
        }
        case "$(plutil -extract monitoring raw -o - "$status_file")" in
            localOnly|systemWide) ;;
            *) fail "failed staged shortcut update lost the previous backend" ;;
        esac
        if [[ "$staged_case" == backend-failure ]]; then
            rg -q 'forced staged backend failure' "$status_file" || {
                fail "forced backend failure was not reported"
            }
            [[ "$(plutil -extract candidateStops raw -o - "$status_file")" == 0 ]] || {
                fail "failed-to-start staged backend was stopped unexpectedly"
            }
        else
            rg -q 'forced staged persistence failure' "$status_file" || {
                fail "forced persistence failure was not reported"
            }
            [[ "$(plutil -extract candidateStops raw -o - "$status_file")" == 1 ]] || {
                fail "staged persistence rollback did not stop the candidate backend"
            }
        fi
    fi
    osascript -e 'tell application id "com.muxy.tests" to quit'
    for _ in $(jot 400); do
        ! kill -0 "$app_pid" 2>/dev/null && break
        sleep 0.05
    done
    ! kill -0 "$app_pid" 2>/dev/null || fail "staged app did not quit normally for fixture $fixture"
    wait "$app_pid"
    [[ ! -S "$socket" ]] || fail "staged socket remained after fixture $fixture"
    if [[ -n "$before" ]]; then
        [[ "$before" == "$(shasum -a 256 "$app_support/quick-terminal-shortcut.json")" ]] || {
            fail "shortcut fixture changed unexpectedly for $fixture"
        }
    fi
}

spike_status_raw() {
    plutil -extract "$2" raw -o - "$1" 2>/dev/null || true
}

send_spike_control() {
    local status_file="$1" control_file="$2" app_pid="$3" id="$4" body="$5" expected="${6:-success}"
    local temporary="$control_file.tmp"
    printf '%s' "$body" > "$temporary"
    chmod 0600 "$temporary"
    mv "$temporary" "$control_file"
    for _ in $(jot 400); do
        [[ "$(spike_status_raw "$status_file" controlId)" == "$id" ]] && break
        kill -0 "$app_pid" 2>/dev/null || fail "staged spike app exited while handling control $id"
        sleep 0.025
    done
    [[ "$(spike_status_raw "$status_file" controlId)" == "$id" ]] || fail "staged spike control $id timed out"
    [[ "$(spike_status_raw "$status_file" result)" == "$expected" ]] || fail "staged spike control $id did not report $expected"
}

wait_for_spike_value() {
    local status_file="$1" control_file="$2" app_pid="$3" first_id="$4" key="$5" expected="$6"
    local id
    for id in $(jot 80 "$first_id"); do
        send_spike_control "$status_file" "$control_file" "$app_pid" "$id" \
            "{\"id\":$id,\"command\":\"status\"}"
        [[ "$(spike_status_raw "$status_file" "$key")" == "$expected" ]] && return
        sleep 0.025
    done
    fail "staged spike value $key did not become $expected"
}

wait_for_spike_screen() {
    local status_file="$1" control_file="$2" app_pid="$3" first_id="$4" needle="$5"
    local id screen
    for id in $(jot 80 "$first_id"); do
        send_spike_control "$status_file" "$control_file" "$app_pid" "$id" \
            "{\"id\":$id,\"command\":\"status\",\"lastLines\":200}"
        screen="$(spike_status_raw "$status_file" screenText)"
        printf '%s\n' "$screen" | grep -Fxq "$needle" && return
        sleep 0.05
    done
    fail "staged spike screen did not contain $needle"
}

process_identity_token() {
    local pid="$1" started
    started="$(ps -p "$pid" -o lstart= 2>/dev/null)" || return 1
    started="$(printf '%s' "$started" | awk '{$1=$1; print}')"
    [[ -n "$started" ]] || return 1
    printf '%s' "$started" | shasum -a 256 | awk '{print $1}'
}

record_process_identity() {
    local identities="$1" pid="$2" token
    token="$(process_identity_token "$pid")" || return 1
    if ! awk -v pid="$pid" -v token="$token" '$1 == pid && $2 == token { found=1 } END { exit !found }' "$identities"; then
        printf '%s %s\n' "$pid" "$token" >> "$identities"
    fi
}

process_identity_is_live() {
    local pid="$1" expected="$2" current
    current="$(process_identity_token "$pid")" || return 1
    [[ "$current" == "$expected" ]]
}

start_process_tree_monitor() {
    local root="$1" identities="$2" stop="$3" owner="$$"
    [[ "$root" =~ ^[1-9][0-9]*$ ]] || fail "invalid process tree root: $root"
    : > "$identities"
    rm -f "$stop"
    (
        local current child visited
        local -a queue
        record_process_identity "$identities" "$root" || exit 1
        while [[ ! -e "$stop" ]] && kill -0 "$owner" 2>/dev/null; do
            queue=("$root")
            while IFS=' ' read -r current _; do
                [[ -n "$current" ]] && queue+=("$current")
            done < "$identities"
            visited=" "
            while ((${#queue[@]} > 0)); do
                current="${queue[0]}"
                queue=("${queue[@]:1}")
                [[ "$visited" == *" $current "* ]] && continue
                visited+="$current "
                while IFS= read -r child; do
                    [[ "$child" =~ ^[1-9][0-9]*$ ]] || continue
                    record_process_identity "$identities" "$child" || continue
                    queue+=("$child")
                done < <(pgrep -P "$current" 2>/dev/null || true)
            done
            sleep 0.01
        done
    ) &
}

stop_process_tree_monitor() {
    local monitor_pid="$1" stop="$2"
    : > "$stop"
    wait "$monitor_pid"
}

wait_for_process_tree_exit() {
    local identities="$1" label="$2" pid token live
    for _ in $(jot 240); do
        live=false
        while IFS=' ' read -r pid token; do
            [[ -n "$pid" && -n "$token" ]] || continue
            if process_identity_is_live "$pid" "$token"; then
                live=true
                break
            fi
        done < "$identities"
        [[ "$live" == false ]] && return
        sleep 0.025
    done
    while IFS=' ' read -r pid token; do
        [[ -n "$pid" && -n "$token" ]] || continue
        process_identity_is_live "$pid" "$token" && printf 'surviving process identity: %s %s\n' "$pid" "$token" >&2
    done < "$identities"
    fail "$label"
}

run_staged_spike() {
    local mode="$1" app="$2" case_root="$3"
    local app_support="$case_root/s" socket_name socket log executable_name executable app_pid
    local status_file control_file first_generation first_pid second_generation second_pid screen normalized_screen exit_status
    local second_identities second_stop second_monitor
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    socket_name="muxy.sock"
    [[ "$mode" == debug ]] && socket_name="muxy-dev.sock"
    socket="$app_support/$socket_name"
    ((${#socket} < 104)) || fail "staged spike socket path exceeds the macOS limit"
    log="$app_support/app.log"
    status_file="$app_support/.muxy-p6-spike-status.json"
    control_file="$app_support/.muxy-p6-spike-control.json"
    executable_name="$(plutil -extract CFBundleExecutable raw -o - "$app/Contents/Info.plist")"
    executable="$app/Contents/MacOS/$executable_name"
    MUXY_PANE_ID=STALE-PANE \
        MUXY_PROJECT_ID=STALE-PROJECT \
        MUXY_WORKTREE_ID=STALE-WORKTREE \
        MUXY_SOCKET_PATH=/tmp/stale.socket \
        MUXY_HOOK_BIN=/tmp/stale-hook \
        MUXY_HOOK_SCRIPT=/tmp/stale-hook-script \
        MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P6_SPIKE_CASE=spike \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp" \
        XDG_CONFIG_HOME="$app_support/xdg" \
        "$executable" > "$log" 2>&1 &
    app_pid=$!
    for _ in $(jot 600); do
        [[ -S "$socket" && -f "$status_file" ]] && break
        kill -0 "$app_pid" 2>/dev/null || {
            cat "$log"
            fail "staged spike app exited before becoming ready"
        }
        sleep 0.05
    done
    [[ -S "$socket" ]] || fail "staged spike socket was not created"
    [[ -f "$status_file" ]] || fail "staged spike status was not written"
    [[ "$(spike_status_raw "$status_file" result)" == success ]] || {
        cat "$status_file"
        fail "staged spike initialization failed"
    }
    for key in visible nativeVisible borderless nonactivating statusLevel joinsAllSpaces fullScreenAuxiliary ignoresCycle floating visibleOnDeactivate transparent keyCapable hasSurface; do
        [[ "$(spike_status_raw "$status_file" "$key")" == true ]] || fail "staged spike property $key was not true"
    done
    [[ "$(spike_status_raw "$status_file" movable)" == false ]] || fail "staged spike panel remained movable"
    [[ "$(spike_status_raw "$status_file" mainCapable)" == false ]] || fail "staged spike panel could become main"
    first_generation="$(spike_status_raw "$status_file" surfaceGeneration)"
    [[ "$first_generation" =~ ^[1-9][0-9]*$ ]] || fail "staged spike surface generation was invalid"
    MUXY_SOCKET_PATH="$socket" "$(command -v muxy)" list-projects >/dev/null

    send_spike_control "$status_file" "$control_file" "$app_pid" 1 \
        '{"id":1,"command":"sendLine","text":"echo QT_PHASE3_OUTPUT; cd /tmp; pwd; env | grep -E '\''^(MUXY_PANE_ID|MUXY_PROJECT_ID|MUXY_WORKTREE_ID|MUXY_SOCKET_PATH|MUXY_HOOK_BIN|MUXY_HOOK_SCRIPT)='\'' | sort; echo QT_PHASE3_ENV_DONE"}'
    wait_for_spike_screen "$status_file" "$control_file" "$app_pid" 2 QT_PHASE3_ENV_DONE
    screen="$(spike_status_raw "$status_file" screenText)"
    normalized_screen="$(printf '%s' "$screen" | tr -d '\r\n')"
    printf '%s\n' "$screen" | grep -Fxq QT_PHASE3_OUTPUT || fail "staged spike shell marker was not emitted"
    printf '%s\n' "$screen" | grep -Fxq /tmp || fail "staged spike shell did not change to /tmp"
    [[ "$normalized_screen" == *"MUXY_SOCKET_PATH=$socket"* ]] || fail "standalone shell did not receive the selected socket"
    for key in MUXY_PANE_ID MUXY_PROJECT_ID MUXY_WORKTREE_ID MUXY_HOOK_BIN MUXY_HOOK_SCRIPT; do
        ! printf '%s\n' "$screen" | grep -Fq "$key=" || fail "standalone shell inherited $key"
    done
    first_pid="$(spike_status_raw "$status_file" foregroundPid)"
    [[ "$first_pid" =~ ^[1-9][0-9]*$ ]] || fail "staged spike foreground PID was invalid"
    [[ "$(spike_status_raw "$status_file" foregroundProcessIdentity)" == "$first_generation:$first_pid" ]] \
        || fail "staged spike process identity was not generation-owned"

    send_spike_control "$status_file" "$control_file" "$app_pid" 90 '{"id":90,"command":"hide"}'
    [[ "$(spike_status_raw "$status_file" visible)" == false ]] || fail "staged spike did not hide"
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 91 nativeVisible false
    send_spike_control "$status_file" "$control_file" "$app_pid" 180 '{"id":180,"command":"show"}'
    [[ "$(spike_status_raw "$status_file" surfaceGeneration)" == "$first_generation" ]] || fail "hide/show replaced the standalone surface"
    [[ "$(spike_status_raw "$status_file" foregroundPid)" == "$first_pid" ]] || fail "hide/show replaced the standalone process"
    wait_for_spike_screen "$status_file" "$control_file" "$app_pid" 181 QT_PHASE3_OUTPUT
    screen="$(spike_status_raw "$status_file" screenText)"
    printf '%s\n' "$screen" | grep -Fxq /tmp || fail "hide/show lost the retained shell CWD"
    send_spike_control "$status_file" "$control_file" "$app_pid" 270 '{"id":270,"command":"reload"}'
    wait_for_spike_screen "$status_file" "$control_file" "$app_pid" 271 QT_PHASE3_OUTPUT

    send_spike_control "$status_file" "$control_file" "$app_pid" 360 '{"id":360,"command":"sendLine","text":"exit"}'
    for id in $(jot 100 361); do
        send_spike_control "$status_file" "$control_file" "$app_pid" "$id" \
            "{\"id\":$id,\"command\":\"status\"}"
        [[ "$(spike_status_raw "$status_file" hasSurface)" == false ]] && break
        sleep 0.05
    done
    [[ "$(spike_status_raw "$status_file" hasSurface)" == false ]] || fail "exited shell surface remained retained"
    ! kill -0 "$first_pid" 2>/dev/null || fail "exited Quick Terminal process survived"
    send_spike_control "$status_file" "$control_file" "$app_pid" 500 '{"id":500,"command":"show"}'
    second_generation="$(spike_status_raw "$status_file" surfaceGeneration)"
    second_pid="$(spike_status_raw "$status_file" foregroundPid)"
    [[ "$second_generation" -gt "$first_generation" ]] || fail "show after exit did not create a new generation"
    [[ "$second_pid" =~ ^[1-9][0-9]*$ && "$second_pid" != "$first_pid" ]] || fail "show after exit did not create a new process"
    [[ "$(spike_status_raw "$status_file" foregroundProcessIdentity)" == "$second_generation:$second_pid" ]] \
        || fail "replacement process identity was not generation-owned"
    second_identities="$app_support/second-process-identities"
    second_stop="$app_support/second-process-monitor.stop"
    start_process_tree_monitor "$second_pid" "$second_identities" "$second_stop"
    second_monitor=$!
    send_spike_control "$status_file" "$control_file" "$app_pid" 510 \
        '{"id":510,"command":"sendLine","text":"sleep 300 & echo QT_PHASE3_TREE_READY"}'
    wait_for_spike_screen "$status_file" "$control_file" "$app_pid" 511 QT_PHASE3_TREE_READY
    for _ in $(jot 100); do
        [[ "$(wc -l < "$second_identities" | tr -d ' ')" -ge 2 ]] && break
        sleep 0.01
    done
    [[ "$(wc -l < "$second_identities" | tr -d ' ')" -ge 2 ]] || fail "staged spike did not create a descendant process"

    printf '%s\n' close > "$app_support/.muxy-test-close-main-window"
    for _ in $(jot 400); do
        ! kill -0 "$app_pid" 2>/dev/null && break
        sleep 0.05
    done
    ! kill -0 "$app_pid" 2>/dev/null || fail "main-window close did not quit the staged app"
    set +e
    wait "$app_pid"
    exit_status=$?
    set -e
    [[ "$exit_status" == 0 ]] || {
        cat "$log"
        fail "staged spike app exited with status $exit_status"
    }
    [[ ! -S "$socket" ]] || fail "staged spike socket remained after main-window close"
    stop_process_tree_monitor "$second_monitor" "$second_stop"
    wait_for_process_tree_exit "$second_identities" "Quick Terminal process tree survived app shutdown"
}

run_staged_panel_lifecycle() {
    local mode="$1" app="$2" case_root="$3"
    local app_support="$case_root/l" socket_name socket log executable_name executable app_pid
    local status_file control_file first_generation first_panel_generation first_pid second_panel_generation second_pid third_generation third_pid fourth_generation fourth_pid width height exit_status
    local first_identities first_stop first_monitor third_identities third_stop third_monitor fourth_identities fourth_stop fourth_monitor
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    printf '%s' '{"muxy.quickTerminal.enabled":true,"muxy.quickTerminal.width":900,"muxy.quickTerminal.height":520,"muxy.quickTerminal.transparency":22,"muxy.quickTerminal.blur":80}' > "$app_support/settings.json"
    printf '%s\n' 'clipboard-write = ask' > "$app_support/ghostty.conf"
    socket_name="muxy.sock"
    [[ "$mode" == debug ]] && socket_name="muxy-dev.sock"
    socket="$app_support/$socket_name"
    ((${#socket} < 104)) || fail "staged panel lifecycle socket path exceeds the macOS limit"
    log="$app_support/app.log"
    status_file="$app_support/.muxy-p6-spike-status.json"
    control_file="$app_support/.muxy-p6-spike-control.json"
    executable_name="$(plutil -extract CFBundleExecutable raw -o - "$app/Contents/Info.plist")"
    executable="$app/Contents/MacOS/$executable_name"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P6_SPIKE_CASE=panel-lifecycle \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp" \
        XDG_CONFIG_HOME="$app_support/xdg" \
        "$executable" > "$log" 2>&1 &
    app_pid=$!
    for _ in $(jot 600); do
        [[ -S "$socket" && -f "$status_file" ]] && break
        kill -0 "$app_pid" 2>/dev/null || {
            cat "$log"
            fail "staged panel lifecycle app exited before becoming ready"
        }
        sleep 0.05
    done
    [[ -S "$socket" ]] || fail "staged panel lifecycle socket was not created"
    [[ "$(spike_status_raw "$status_file" result)" == success ]] || {
        cat "$status_file"
        fail "staged panel lifecycle initialization failed"
    }
    for key in enabled visible nativeVisible hasPanel hasSurface activeSpaceIntent; do
        [[ "$(spike_status_raw "$status_file" "$key")" == true ]] || fail "staged panel lifecycle property $key was not true"
    done
    [[ "$(spike_status_raw "$status_file" configuredWidth)" == 900 ]] || fail "configured panel width was not loaded"
    [[ "$(spike_status_raw "$status_file" configuredHeight)" == 520 ]] || fail "configured panel height was not loaded"
    [[ "$(spike_status_raw "$status_file" storedTransparency)" == 22 ]] || fail "stored transparency was not loaded"
    [[ "$(spike_status_raw "$status_file" storedBlur)" == 80 ]] || fail "stored blur was not loaded"
    [[ "$(spike_status_raw "$status_file" appearance)" == blurred ]] || fail "nonzero transparency and blur did not select blurred appearance"
    [[ "$(spike_status_raw "$status_file" accessibilityNodeCount)" == 5 ]] || fail "native accessibility model was incomplete"
    [[ -n "$(spike_status_raw "$status_file" screenName)" ]] || fail "pointer display telemetry was missing"
    width="$(spike_status_raw "$status_file" frame.width)"
    height="$(spike_status_raw "$status_file" frame.height)"
    awk -v value="$width" 'BEGIN { exit !(value > 0 && value <= 900) }' || fail "panel width was not constrained"
    awk -v value="$height" 'BEGIN { exit !(value > 0 && value <= 520) }' || fail "panel height was not constrained"
    [[ "$(spike_status_raw "$status_file" nativeFrame.width)" == "$width" ]] || fail "terminal surface was not created at target width"
    [[ "$(spike_status_raw "$status_file" nativeFrame.height)" == "$height" ]] || fail "terminal surface was not created at target height"
    first_generation="$(spike_status_raw "$status_file" surfaceGeneration)"
    first_panel_generation="$(spike_status_raw "$status_file" panelGeneration)"

    send_spike_control "$status_file" "$control_file" "$app_pid" 1 '{"id":1,"command":"hide"}'
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 2 nativeVisible false
    send_spike_control "$status_file" "$control_file" "$app_pid" 100 '{"id":100,"command":"show"}'
    [[ "$(spike_status_raw "$status_file" surfaceGeneration)" == "$first_generation" ]] || fail "panel hide/show replaced the shell"
    send_spike_control "$status_file" "$control_file" "$app_pid" 110 '{"id":110,"command":"rapidShowHideShow"}'
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 111 nativeVisible true
    [[ "$(spike_status_raw "$status_file" surfaceGeneration)" == "$first_generation" ]] || fail "rapid reversal replaced the shell"
    first_pid="$(spike_status_raw "$status_file" foregroundPid)"
    [[ "$first_pid" =~ ^[1-9][0-9]*$ ]] || fail "panel shell PID was invalid after startup"
    [[ "$(spike_status_raw "$status_file" foregroundProcessIdentity)" == "$first_generation:$first_pid" ]] \
        || fail "panel process identity was not generation-owned"
    first_identities="$app_support/first-process-identities"
    first_stop="$app_support/first-process-monitor.stop"
    start_process_tree_monitor "$first_pid" "$first_identities" "$first_stop"
    first_monitor=$!

    send_spike_control "$status_file" "$control_file" "$app_pid" 200 '{"id":200,"command":"close"}'
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 201 nativeVisible false
    [[ "$(spike_status_raw "$status_file" nativeFrame.width)" == "$width" ]] || fail "hide animation resized the terminal viewport width"
    [[ "$(spike_status_raw "$status_file" nativeFrame.height)" == "$height" ]] || fail "hide animation resized the terminal viewport height"
    [[ "$(spike_status_raw "$status_file" hasSurface)" == true ]] || fail "close action destroyed the shell"
    [[ "$(spike_status_raw "$status_file" foregroundPid)" == "$first_pid" ]] || fail "close action replaced the shell"
    send_spike_control "$status_file" "$control_file" "$app_pid" 290 '{"id":290,"command":"show"}'

    send_spike_control "$status_file" "$control_file" "$app_pid" 300 '{"id":300,"command":"setAccessibilityOpaque","text":"true"}'
    [[ "$(spike_status_raw "$status_file" appearance)" == opaque ]] || fail "Reduce Transparency did not force opaque appearance"
    [[ "$(spike_status_raw "$status_file" effectiveTransparency)" == 0 ]] || fail "opaque accessibility override retained effective transparency"
    [[ "$(spike_status_raw "$status_file" effectiveBlur)" == 0 ]] || fail "opaque accessibility override retained effective blur"
    [[ "$(spike_status_raw "$status_file" storedTransparency)" == 22 ]] || fail "accessibility override rewrote stored transparency"
    [[ "$(spike_status_raw "$status_file" storedBlur)" == 80 ]] || fail "accessibility override rewrote stored blur"
    send_spike_control "$status_file" "$control_file" "$app_pid" 305 \
        '{"id":305,"command":"sendLine","text":"sleep 300 & echo QT_PHASE4_DISABLE_TREE_READY"}'
    wait_for_spike_screen "$status_file" "$control_file" "$app_pid" 306 QT_PHASE4_DISABLE_TREE_READY
    for _ in $(jot 100); do
        [[ "$(wc -l < "$first_identities" | tr -d ' ')" -ge 2 ]] && break
        sleep 0.01
    done
    [[ "$(wc -l < "$first_identities" | tr -d ' ')" -ge 2 ]] || fail "disable fixture did not create a descendant process"

    send_spike_control "$status_file" "$control_file" "$app_pid" 310 '{"id":310,"command":"disable"}'
    for key in enabled visible nativeVisible hasPanel hasSurface hasWakeupTask hasEventTask; do
        [[ "$(spike_status_raw "$status_file" "$key")" == false ]] || fail "disable teardown left $key active"
    done
    stop_process_tree_monitor "$first_monitor" "$first_stop"
    wait_for_process_tree_exit "$first_identities" "disable teardown left the Quick Terminal process tree alive"
    rg -q '^  "muxy\.quickTerminal\.enabled" : false,$' "$app_support/settings.json" \
        || fail "staged disable bypassed transactional persistence"
    send_spike_control "$status_file" "$control_file" "$app_pid" 320 '{"id":320,"command":"enable"}'
    [[ "$(spike_status_raw "$status_file" enabled)" == true ]] || fail "Quick Terminal did not re-enable"
    [[ "$(spike_status_raw "$status_file" hasPanel)" == false ]] || fail "re-enable eagerly created the panel"
    [[ "$(spike_status_raw "$status_file" hasSurface)" == false ]] || fail "re-enable eagerly created the shell"
    rg -q '^  "muxy\.quickTerminal\.enabled" : true,$' "$app_support/settings.json" \
        || fail "staged enable bypassed transactional persistence"
    send_spike_control "$status_file" "$control_file" "$app_pid" 330 '{"id":330,"command":"show"}'
    second_panel_generation="$(spike_status_raw "$status_file" panelGeneration)"
    second_pid="$(spike_status_raw "$status_file" foregroundPid)"
    [[ "$second_panel_generation" -gt "$first_panel_generation" ]] || fail "re-enable did not create a fresh panel generation"
    [[ "$second_pid" =~ ^[1-9][0-9]*$ && "$second_pid" != "$first_pid" ]] || fail "re-enable did not create a fresh shell"

    send_spike_control "$status_file" "$control_file" "$app_pid" 340 '{"id":340,"command":"sendLine","text":"echo QT_PHASE4_RECREATED_READY"}'
    wait_for_spike_screen "$status_file" "$control_file" "$app_pid" 341 QT_PHASE4_RECREATED_READY
    sleep 1
    send_spike_control "$status_file" "$control_file" "$app_pid" 430 '{"id":430,"command":"sendLine","text":"exit"}'
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 431 hasSurface false
    send_spike_control "$status_file" "$control_file" "$app_pid" 520 '{"id":520,"command":"show"}'
    third_generation="$(spike_status_raw "$status_file" surfaceGeneration)"
    third_pid="$(spike_status_raw "$status_file" foregroundPid)"
    [[ "$third_generation" -gt 1 ]] || fail "shell exit did not advance the surface generation"
    [[ "$third_pid" =~ ^[1-9][0-9]*$ && "$third_pid" != "$second_pid" ]] || fail "shell exit did not create a fresh process"

    [[ "$(spike_status_raw "$status_file" foregroundProcessIdentity)" == "$third_generation:$third_pid" ]] \
        || fail "destructive-close process identity was not generation-owned"
    third_identities="$app_support/third-process-identities"
    third_stop="$app_support/third-process-monitor.stop"
    start_process_tree_monitor "$third_pid" "$third_identities" "$third_stop"
    third_monitor=$!
    send_spike_control "$status_file" "$control_file" "$app_pid" 530 \
        '{"id":530,"command":"sendLine","text":"sleep 300"}'
    for _ in $(jot 100); do
        [[ "$(wc -l < "$third_identities" | tr -d ' ')" -ge 2 ]] && break
        sleep 0.01
    done
    [[ "$(wc -l < "$third_identities" | tr -d ' ')" -ge 2 ]] || fail "destructive-close fixture did not create a descendant process"
    send_spike_control "$status_file" "$control_file" "$app_pid" 620 '{"id":620,"command":"closeSurface"}'
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 621 pendingConfirmations 1
    [[ "$(spike_status_raw "$status_file" hasSurface)" == true ]] || fail "destructive close bypassed active-process confirmation"
    send_spike_control "$status_file" "$control_file" "$app_pid" 710 '{"id":710,"command":"approveConfirmation"}'
    for key in visible nativeVisible hasPanel hasSurface hasWakeupTask hasEventTask; do
        [[ "$(spike_status_raw "$status_file" "$key")" == false ]] || fail "destructive close left $key active"
    done
    stop_process_tree_monitor "$third_monitor" "$third_stop"
    wait_for_process_tree_exit "$third_identities" "destructive close left the Quick Terminal process tree alive"
    send_spike_control "$status_file" "$control_file" "$app_pid" 630 '{"id":630,"command":"show"}'
    fourth_generation="$(spike_status_raw "$status_file" surfaceGeneration)"
    fourth_pid="$(spike_status_raw "$status_file" foregroundPid)"
    [[ "$fourth_pid" =~ ^[1-9][0-9]*$ && "$fourth_pid" != "$third_pid" ]] || fail "show after destructive close did not create a fresh process"
    [[ "$(spike_status_raw "$status_file" foregroundProcessIdentity)" == "$fourth_generation:$fourth_pid" ]] \
        || fail "app-quit process identity was not generation-owned"
    fourth_identities="$app_support/fourth-process-identities"
    fourth_stop="$app_support/fourth-process-monitor.stop"
    start_process_tree_monitor "$fourth_pid" "$fourth_identities" "$fourth_stop"
    fourth_monitor=$!
    send_spike_control "$status_file" "$control_file" "$app_pid" 632 \
        '{"id":632,"command":"sendLine","text":"sleep 300 & echo QT_PHASE4_QUIT_TREE_READY"}'
    wait_for_spike_screen "$status_file" "$control_file" "$app_pid" 633 QT_PHASE4_QUIT_TREE_READY
    for _ in $(jot 100); do
        [[ "$(wc -l < "$fourth_identities" | tr -d ' ')" -ge 2 ]] && break
        sleep 0.01
    done
    [[ "$(wc -l < "$fourth_identities" | tr -d ' ')" -ge 2 ]] || fail "app-quit fixture did not create a descendant process"

    send_spike_control "$status_file" "$control_file" "$app_pid" 635 \
        '{"id":635,"command":"sendLine","text":"printf '\''\\033]9;QT_NOTIFICATION\\007'\''"}'
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 636 notificationGeneration 1
    send_spike_control "$status_file" "$control_file" "$app_pid" 720 \
        '{"id":720,"command":"sendLine","text":"printf '\''\\033]52;c;UVRfQ09ORklSTQ==\\007'\''"}'
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 721 pendingConfirmations 1
    send_spike_control "$status_file" "$control_file" "$app_pid" 810 '{"id":810,"command":"hide"}'
    [[ "$(spike_status_raw "$status_file" pendingConfirmations)" == 0 ]] || fail "hide did not deny the visible standalone confirmation"
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 811 nativeVisible false
    send_spike_control "$status_file" "$control_file" "$app_pid" 900 \
        '{"id":900,"command":"sendLine","text":"printf '\''\\033]52;c;UVRfSElEREVO\\007'\''"}'
    sleep 0.2
    send_spike_control "$status_file" "$control_file" "$app_pid" 901 '{"id":901,"command":"status"}'
    [[ "$(spike_status_raw "$status_file" pendingConfirmations)" == 0 ]] || fail "hidden standalone confirmation was not denied"
    send_spike_control "$status_file" "$control_file" "$app_pid" 910 '{"id":910,"command":"show"}'
    send_spike_control "$status_file" "$control_file" "$app_pid" 920 \
        '{"id":920,"command":"sendLine","text":"printf '\''\\033]52;c;UVRfU0hVVERPV04=\\007'\''"}'
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 921 pendingConfirmations 1

    osascript -e 'tell application id "com.muxy.tests" to quit'
    for _ in $(jot 400); do
        ! kill -0 "$app_pid" 2>/dev/null && break
        sleep 0.05
    done
    ! kill -0 "$app_pid" 2>/dev/null || fail "staged panel lifecycle app did not quit normally"
    set +e
    wait "$app_pid"
    exit_status=$?
    set -e
    [[ "$exit_status" == 0 ]] || {
        cat "$log"
        fail "staged panel lifecycle app exited with status $exit_status"
    }
    [[ ! -S "$socket" ]] || fail "staged panel lifecycle socket remained after quit"
    stop_process_tree_monitor "$fourth_monitor" "$fourth_stop"
    wait_for_process_tree_exit "$fourth_identities" "Quick Terminal process tree survived normal app quit"
}

run_staged_live_settings() {
    local mode="$1" app="$2" case_root="$3" case_name="$4"
    local app_support="$case_root/live" socket_name socket log executable_name executable app_pid
    local status_file control_file panel_generation surface_generation shell_pid before_settings before_shortcut exit_status
    local shell_identities shell_stop shell_monitor
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    printf '%s' '{"muxy.quickTerminal.enabled":true,"muxy.quickTerminal.width":720,"muxy.quickTerminal.height":430,"muxy.quickTerminal.transparency":18,"muxy.quickTerminal.blur":70}' > "$app_support/settings.json"
    printf '%s' '{"type":"doubleShift"}' > "$app_support/quick-terminal-shortcut.json"
    socket_name="muxy.sock"
    [[ "$mode" == debug ]] && socket_name="muxy-dev.sock"
    socket="$app_support/$socket_name"
    ((${#socket} < 104)) || fail "staged live settings socket path exceeds the macOS limit"
    log="$app_support/app.log"
    status_file="$app_support/.muxy-p6-spike-status.json"
    control_file="$app_support/.muxy-p6-spike-control.json"
    executable_name="$(plutil -extract CFBundleExecutable raw -o - "$app/Contents/Info.plist")"
    executable="$app/Contents/MacOS/$executable_name"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_P6_SPIKE_CASE="$case_name" \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp" \
        XDG_CONFIG_HOME="$app_support/xdg" \
        "$executable" > "$log" 2>&1 &
    app_pid=$!
    for _ in $(jot 600); do
        [[ -S "$socket" && -f "$status_file" ]] && break
        kill -0 "$app_pid" 2>/dev/null || {
            cat "$log"
            fail "staged live settings app exited before becoming ready"
        }
        sleep 0.05
    done
    [[ -S "$socket" ]] || fail "staged live settings socket was not created"
    [[ "$(spike_status_raw "$status_file" result)" == success ]] || fail "staged live settings initialization failed"
    MUXY_SOCKET_PATH="$socket" "$(command -v muxy)" list-projects >/dev/null
    send_spike_control "$status_file" "$control_file" "$app_pid" 1 \
        '{"id":1,"command":"sendLine","text":"echo QT_PHASE5_READY"}'
    wait_for_spike_screen "$status_file" "$control_file" "$app_pid" 2 QT_PHASE5_READY
    sleep 1
    send_spike_control "$status_file" "$control_file" "$app_pid" 90 '{"id":90,"command":"status"}'
    panel_generation="$(spike_status_raw "$status_file" panelGeneration)"
    surface_generation="$(spike_status_raw "$status_file" surfaceGeneration)"
    shell_pid="$(spike_status_raw "$status_file" foregroundPid)"
    [[ "$shell_pid" =~ ^[1-9][0-9]*$ ]] || fail "staged live settings shell PID was invalid"
    [[ "$(spike_status_raw "$status_file" foregroundProcessIdentity)" == "$surface_generation:$shell_pid" ]] \
        || fail "live-settings process identity was not generation-owned"
    shell_identities="$app_support/shell-process-identities"
    shell_stop="$app_support/shell-process-monitor.stop"
    start_process_tree_monitor "$shell_pid" "$shell_identities" "$shell_stop"
    shell_monitor=$!

    send_spike_control "$status_file" "$control_file" "$app_pid" 100 \
        '{"id":100,"command":"setWidth","text":"840"}'
    [[ "$(spike_status_raw "$status_file" configuredWidth)" == 720 ]] || fail "ordinary settings path changed the visible panel width"
    rg -q '^  "muxy\.quickTerminal\.width" : 840,$' "$app_support/settings.json" || fail "ordinary settings path did not persist panel width"
    [[ "$(spike_status_raw "$status_file" panelGeneration)" == "$panel_generation" ]] || fail "ordinary settings path replaced the panel"
    [[ "$(spike_status_raw "$status_file" surfaceGeneration)" == "$surface_generation" ]] || fail "ordinary settings path replaced the terminal surface"

    send_spike_control "$status_file" "$control_file" "$app_pid" 110 \
        '{"id":110,"command":"applyJson","text":"{\"muxy.quickTerminal.enabled\":true,\"muxy.quickTerminal.width\":840,\"muxy.quickTerminal.height\":500,\"muxy.quickTerminal.transparency\":28,\"muxy.quickTerminal.blur\":90,\"shortcuts.quickTerminal\":{\"type\":\"unassigned\"}}"}'
    [[ "$(spike_status_raw "$status_file" configuredHeight)" == 430 ]] || fail "JSON settings path changed the visible panel height"
    [[ "$(spike_status_raw "$status_file" storedTransparency)" == 18 ]] || fail "JSON settings path changed visible transparency"
    [[ "$(spike_status_raw "$status_file" storedBlur)" == 70 ]] || fail "JSON settings path changed visible blur"
    [[ "$(spike_status_raw "$status_file" shortcut)" == Unassigned ]] || fail "JSON settings path did not publish the shortcut transaction"
    [[ "$(spike_status_raw "$status_file" panelGeneration)" == "$panel_generation" ]] || fail "JSON settings path replaced the panel"
    [[ "$(spike_status_raw "$status_file" surfaceGeneration)" == "$surface_generation" ]] || fail "JSON settings path replaced the terminal surface"
    [[ "$(spike_status_raw "$status_file" foregroundPid)" == "$shell_pid" ]] || fail "settings persistence replaced the retained shell"

    send_spike_control "$status_file" "$control_file" "$app_pid" 115 '{"id":115,"command":"hide"}'
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 116 nativeVisible false
    send_spike_control "$status_file" "$control_file" "$app_pid" 117 '{"id":117,"command":"show"}'
    [[ "$(spike_status_raw "$status_file" configuredWidth)" == 840 ]] || fail "next opening did not apply the persisted width"
    [[ "$(spike_status_raw "$status_file" configuredHeight)" == 500 ]] || fail "next opening did not apply the persisted height"
    [[ "$(spike_status_raw "$status_file" storedTransparency)" == 28 ]] || fail "next opening did not apply persisted transparency"
    [[ "$(spike_status_raw "$status_file" storedBlur)" == 90 ]] || fail "next opening did not apply persisted blur"
    [[ "$(spike_status_raw "$status_file" panelGeneration)" == "$panel_generation" ]] || fail "next opening replaced the panel"
    [[ "$(spike_status_raw "$status_file" surfaceGeneration)" == "$surface_generation" ]] || fail "next opening replaced the terminal surface"
    [[ "$(spike_status_raw "$status_file" foregroundPid)" == "$shell_pid" ]] || fail "next opening replaced the retained shell"

    before_settings="$(shasum -a 256 "$app_support/settings.json")"
    before_shortcut="$(shasum -a 256 "$app_support/quick-terminal-shortcut.json")"
    send_spike_control "$status_file" "$control_file" "$app_pid" 120 \
        '{"id":120,"command":"applyJson","text":"{\"shortcuts.app\":{\"openProject\":{\"key\":\"space\",\"modifiers\":1048576}},\"shortcuts.quickTerminal\":{\"type\":\"keyCombo\",\"keyCombo\":{\"key\":\"space\",\"modifiers\":1048576},\"virtualKeyCode\":49}}"}' error
    rg -q 'conflicts with Open Project' "$status_file" || fail "reverse JSON shortcut conflict was not reported"
    [[ "$before_settings" == "$(shasum -a 256 "$app_support/settings.json")" ]] || fail "rejected JSON conflict changed settings.json"
    [[ "$before_shortcut" == "$(shasum -a 256 "$app_support/quick-terminal-shortcut.json")" ]] || fail "rejected JSON conflict changed the shortcut file"
    [[ "$(spike_status_raw "$status_file" shortcut)" == Unassigned ]] || fail "rejected JSON conflict changed the active shortcut"

    send_spike_control "$status_file" "$control_file" "$app_pid" 125 '{"id":125,"command":"reset"}'
    [[ "$(spike_status_raw "$status_file" configuredWidth)" == 840 ]] || fail "Reset changed the visible panel width"
    [[ "$(spike_status_raw "$status_file" configuredHeight)" == 500 ]] || fail "Reset changed the visible panel height"
    [[ "$(spike_status_raw "$status_file" storedTransparency)" == 28 ]] || fail "Reset changed visible transparency"
    [[ "$(spike_status_raw "$status_file" storedBlur)" == 90 ]] || fail "Reset changed visible blur"
    rg -q '^  "muxy\.quickTerminal\.width" : 720,$' "$app_support/settings.json" || fail "Reset did not persist defaults"
    send_spike_control "$status_file" "$control_file" "$app_pid" 126 '{"id":126,"command":"hide"}'
    wait_for_spike_value "$status_file" "$control_file" "$app_pid" 127 nativeVisible false
    send_spike_control "$status_file" "$control_file" "$app_pid" 128 '{"id":128,"command":"show"}'
    [[ "$(spike_status_raw "$status_file" configuredWidth)" == 720 ]] || fail "next opening did not apply the reset width"
    [[ "$(spike_status_raw "$status_file" configuredHeight)" == 430 ]] || fail "next opening did not apply the reset height"
    [[ "$(spike_status_raw "$status_file" storedTransparency)" == 18 ]] || fail "next opening did not apply reset transparency"
    [[ "$(spike_status_raw "$status_file" storedBlur)" == 70 ]] || fail "next opening did not apply reset blur"
    [[ "$(spike_status_raw "$status_file" panelGeneration)" == "$panel_generation" ]] || fail "Reset reopening replaced the panel"
    [[ "$(spike_status_raw "$status_file" surfaceGeneration)" == "$surface_generation" ]] || fail "Reset reopening replaced the terminal surface"

    send_spike_control "$status_file" "$control_file" "$app_pid" 130 \
        '{"id":130,"command":"sendLine","text":"sleep 300 & echo QT_PHASE6_LIVE_TREE_READY"}'
    wait_for_spike_screen "$status_file" "$control_file" "$app_pid" 131 QT_PHASE6_LIVE_TREE_READY
    for _ in $(jot 100); do
        [[ "$(wc -l < "$shell_identities" | tr -d ' ')" -ge 2 ]] && break
        sleep 0.01
    done
    [[ "$(wc -l < "$shell_identities" | tr -d ' ')" -ge 2 ]] || fail "live-settings fixture did not create a descendant process"

    send_spike_control "$status_file" "$control_file" "$app_pid" 140 '{"id":140,"command":"quit"}'
    for _ in $(jot 400); do
        ! kill -0 "$app_pid" 2>/dev/null && break
        sleep 0.05
    done
    ! kill -0 "$app_pid" 2>/dev/null || fail "staged live settings app did not quit normally"
    set +e
    wait "$app_pid"
    exit_status=$?
    set -e
    [[ "$exit_status" == 0 ]] || {
        cat "$log"
        fail "staged live settings app exited with status $exit_status"
    }
    [[ ! -S "$socket" ]] || fail "staged live settings socket remained after quit"
    stop_process_tree_monitor "$shell_monitor" "$shell_stop"
    wait_for_process_tree_exit "$shell_identities" "staged live settings process tree survived app quit"
}

staged_case() {
    local mode="$1" app="$2" case_name="$3" case_root production_socket production_identity
    local production_owner
    [[ "$(uname -s)" == Darwin ]] || fail "staged verification requires macOS"
    [[ "$mode" == debug || "$mode" == release ]] || fail "invalid staged profile: $mode"
    [[ "$app" == /* ]] || fail "staged app path must be absolute"
    reject_symlink_ancestors "$app"
    [[ "$app" == "$APPS_ROOT/"*/MuxyTests.app ]] || fail "staged app is outside the owned apps root"
    [[ -d "$app" ]] || fail "staged app does not exist: $app"
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$app/Contents/Info.plist")" == com.muxy.tests ]] || {
        fail "staged app identity is not com.muxy.tests"
    }
    cmp -s \
        "$PROJECT_ROOT/Muxy/Resources/scripts/muxy-cli" \
        "$app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli" \
        || fail "staged app does not contain the retained byte-identical CLI"
    case "$case_name" in
        phase-1|phase-2|spike|phase-3|panel-lifecycle|phase-4|live-settings|phase-5|phase-6|final-debug|final-release) ;;
        *) fail "unknown staged case: $case_name" ;;
    esac
    prepare_root
    case_root="$P6_ROOT/p1"
    [[ "$case_name" == phase-2 ]] && case_root="$P6_ROOT/p2"
    [[ "$case_name" == spike || "$case_name" == phase-3 ]] && case_root="$P6_ROOT/p3"
    [[ "$case_name" == panel-lifecycle || "$case_name" == phase-4 ]] && case_root="$P6_ROOT/p4"
    [[ "$case_name" == live-settings || "$case_name" == phase-5 ]] && case_root="$P6_ROOT/p5"
    [[ "$case_name" == phase-6 || "$case_name" == final-debug || "$case_name" == final-release ]] && case_root="$P6_ROOT/p6"
    prepare_case_root "$case_root"
    production_socket="$HOME/Library/Application Support/Muxy/muxy.sock"
    [[ -S "$production_socket" ]] || fail "production socket is not live"
    production_owner="$(lsof -t "$production_socket" | head -n 1)"
    [[ -n "$production_owner" ]] || fail "production socket has no owner"
    production_identity="$(stat -f '%d:%i' "$production_socket")"
    if [[ "$case_name" == phase-1 ]]; then
        for fixture in a b c d e f; do
            run_staged_fixture "$mode" "$app" "$fixture" "$case_root"
        done
    elif [[ "$case_name" == phase-2 ]]; then
        for fixture in c d e b g h; do
            run_staged_fixture "$mode" "$app" "$fixture" "$case_root"
        done
    elif [[ "$case_name" == panel-lifecycle || "$case_name" == phase-4 ]]; then
        panel_policy_source_checks
        run_staged_panel_lifecycle "$mode" "$app" "$case_root"
    elif [[ "$case_name" == live-settings || "$case_name" == phase-5 ]]; then
        settings_transaction_source_checks
        run_staged_live_settings "$mode" "$app" "$case_root" "$case_name"
    elif [[ "$case_name" == phase-6 || "$case_name" == final-debug || "$case_name" == final-release ]]; then
        phase_6_guardrails
        run_staged_panel_lifecycle "$mode" "$app" "$case_root"
        run_staged_live_settings "$mode" "$app" "$case_root" "$case_name"
    else
        phase_3_source_checks
        run_staged_spike "$mode" "$app" "$case_root"
    fi
    [[ "$(stat -f '%d:%i' "$production_socket")" == "$production_identity" ]] || {
        fail "production socket identity changed"
    }
    [[ "$(lsof -t "$production_socket" | head -n 1)" == "$production_owner" ]] || {
        fail "production socket owner changed"
    }
    if pgrep -f "$app/Contents/MacOS/MuxyTests$" >/dev/null; then
        fail "staged app process survived verification"
    fi
    printf 'P6 staged %s passed\n' "$case_name"
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
if [[ "${1:-}" == --staged ]]; then
    for command_name in chmod grep head jot lsof muxy mv osascript pgrep plutil shasum stat; do
        require_command "$command_name"
    done
fi

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
        (($# == 4)) || fail "usage: scripts/verify-p6-quick-terminal.sh --staged PROFILE ABSOLUTE_APP CASE"
        staged_case "$2" "$3" "$4"
        ;;
    *) fail "usage: scripts/verify-p6-quick-terminal.sh --self-test | --fixture CASE | --staged PROFILE ABSOLUTE_APP CASE" ;;
esac
