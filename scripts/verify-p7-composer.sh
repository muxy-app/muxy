#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly SCRIPT_DIR
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd -P)"
readonly PROJECT_ROOT
readonly VERIFICATION_PARENT="$PROJECT_ROOT/target/test-verification"
readonly VERIFICATION_ROOT="$VERIFICATION_PARENT/p7"
readonly ROOT_MARKER="$VERIFICATION_ROOT/.muxy-p7-verifier"
readonly ROOT_OWNER="muxy-p7-composer-verifier-v1"
readonly CASE_MARKER=".muxy-p7-case"
readonly CASE_OWNER="muxy-p7-composer-case-v1"
readonly SOURCE_CLI="$PROJECT_ROOT/Muxy/Resources/scripts/muxy-cli"
readonly STAGED_APPS_ROOT="$PROJECT_ROOT/target/test-verification/apps"
readonly STAGED_OWNER_FILE=".muxy-stage-owner"
readonly PRODUCTION_SOCKET="$HOME/Library/Application Support/Muxy/muxy.sock"
APP_PID=""
ACTIVE_SOCKET=""
ACTIVE_CASE_ROOT=""
PRODUCTION_SOCKET_IDENTITY=""
PRODUCTION_SOCKET_OWNER=""

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

root_ancestors_are_safe() {
    local path
    for path in "$PROJECT_ROOT/target" "$VERIFICATION_PARENT" "$VERIFICATION_ROOT"; do
        [[ ! -L "$path" ]] || return 1
        [[ ! -e "$path" || -d "$path" ]] || return 1
    done
}

root_is_owned() {
    [[ -d "$VERIFICATION_ROOT" && ! -L "$VERIFICATION_ROOT" ]] || return 1
    [[ -f "$ROOT_MARKER" && ! -L "$ROOT_MARKER" ]] || return 1
    [[ "$(<"$ROOT_MARKER")" == "$ROOT_OWNER" ]]
}

prepare_root() {
    root_ancestors_are_safe || fail "verification root has an unsafe ancestor"
    mkdir -p "$VERIFICATION_PARENT"
    if [[ -e "$VERIFICATION_ROOT" ]]; then
        root_is_owned || fail "verification root is not owned by the P7 verifier"
    else
        mkdir "$VERIFICATION_ROOT"
        printf '%s\n' "$ROOT_OWNER" > "$ROOT_MARKER"
    fi
    root_ancestors_are_safe || fail "verification root changed while preparing it"
}

path_is_safe() {
    local path="$1" relative cursor component old_ifs
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

case_is_owned() {
    local path="$1" marker="$1/$CASE_MARKER"
    [[ -d "$path" && ! -L "$path" ]] || return 1
    [[ -f "$marker" && ! -L "$marker" ]] || return 1
    [[ "$(<"$marker")" == "$CASE_OWNER" ]]
}

prepare_case() {
    local path="$1"
    prepare_root
    path_is_safe "$path" || fail "refusing unsafe P7 case path: $path"
    if [[ -e "$path" ]]; then
        case_is_owned "$path" || fail "P7 case path is not verifier-owned: $path"
        rm -rf -- "$path"
    fi
    mkdir -p "$path"
    printf '%s\n' "$CASE_OWNER" > "$path/$CASE_MARKER"
}

source_checks() {
    local changes matches
    changes="$(git status --short --untracked-files=all -- \
        crates/muxy-core/src/migration.rs \
        crates/muxy-proto \
        Muxy/Resources/scripts/muxy-cli \
        crates/muxy/src/socket \
        .github || true)"
    changes="$(printf '%s\n' "$changes" | rg -v 'crates/muxy-proto/src/session/|crates/muxy-proto/src/lib.rs$|crates/muxy/src/socket/catalog.rs$|crates/muxy/src/socket/runtime.rs$|crates/muxy/src/socket/commands/mod.rs$|crates/muxy/src/socket/commands/sessions.rs$' || true)"
    [[ -z "$changes" ]] || {
        printf '%s\n' "$changes"
        fail "a locked migration, protocol, CLI, bundle, staging, or CI path changed"
    }
    matches="$(rg -n 'rich-input-drafts|RichInputImages|richInput' crates/muxy-core/src/migration.rs || true)"
    [[ -z "$matches" ]] || {
        printf '%s\n' "$matches"
        fail "Composer state entered legacy migration"
    }
    rg -q 'PANEL_ID: &str = "builtin:richInput"' crates/muxy-core/src/composer/mod.rs || fail "built-in panel ID differs"
    rg -q 'DRAFTS_FILE_NAME: &str = "rich-input-drafts.json"' crates/muxy-core/src/composer/mod.rs || fail "draft filename differs"
    rg -q 'IMAGES_DIRECTORY_NAME: &str = "RichInputImages"' crates/muxy-core/src/composer/mod.rs || fail "image directory differs"
    for key in \
        'muxy.panel.mode.builtin:richInput' \
        'muxy.richInput.position' \
        'muxy.richInputPanelWidth' \
        'muxy.richInputPanelHeight' \
        'muxy.richInput.broadcast' \
        'muxy.richInput.fontSize'; do
        rg -q -F "$key" crates/muxy-core/src/prefs/mod.rs || fail "missing Composer preference key: $key"
    done
    if rg -n 'muxy\.richInput\.presentationMode' \
        crates/muxy/src/views/settings/categories/composer.rs \
        crates/muxy-core/src/settings_catalog.rs; then
        fail "standalone Composer presentation remains active"
    fi
    if rg -n 'MUXY_TEST_P5_CLOSE_MAIN_WINDOW_REQUEST|\.muxy-p5-close-main-window' \
        crates/muxy/src scripts/verify-p5-notifications.sh scripts/verify-p6-quick-terminal.sh; then
        fail "P5-specific staged close seam remains"
    fi
    rg -q 'MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST' crates/muxy/src/views/window/mod.rs || fail "generic staged close environment key is missing"
    rg -q '\.muxy-test-close-main-window' crates/muxy/src/views/window/mod.rs || fail "generic staged close request file is missing"
    for key in richInputImageStrategy richInputFontFamily richInputLineHeightMultiplier; do
        rg -q "$key" crates/muxy-core/src/prefs/settings.rs || fail "existing editor setting disappeared: $key"
    done
}

persistence_source_checks() {
    local matches
    for path in \
        crates/muxy-core/src/composer/draft.rs \
        crates/muxy-core/src/composer/image_storage.rs; do
        [[ -f "$path" ]] || fail "missing Composer persistence source: $path"
    done
    rg -q 'SAVE_DEBOUNCE: Duration = Duration::from_millis\(400\)' crates/muxy-core/src/composer/draft.rs || fail "Composer save debounce differs"
    rg -q 'rich-input-drafts.json' crates/muxy-core/src/composer/mod.rs || fail "Composer draft filename differs"
    rg -q 'RichInputImages' crates/muxy-core/src/composer/mod.rs || fail "Composer image directory differs"
    rg -q 'ensure_private_directory' crates/muxy-core/src/store/persistence.rs || fail "private directory capability is missing"
    for operation in openat fstatat unlinkat; do
        rg -q "$operation" crates/muxy-core/src/store/persistence.rs || fail "descriptor-relative operation is missing: $operation"
    done
    matches="$(rg -n 'gpui|objc2|AppKit|MainWindow|AppState' crates/muxy-core/src/composer || true)"
    [[ -z "$matches" ]] || {
        printf '%s\n' "$matches"
        fail "portable Composer persistence crossed an app or platform boundary"
    }
    rg -q 'MUXY_TEST_P7_COMPOSER_CASE' crates/muxy/src/state.rs || fail "P7 staged persistence control is missing"
    rg -q 'schedule_composer_save' crates/muxy/src/views/window/mod.rs || fail "Composer debounce scheduling is missing"
    rg -q 'flush_if_revision\(revision\)' crates/muxy/src/views/window/mod.rs || fail "Composer debounce revision guard is missing"
    rg -q 'flush_composer_store' crates/muxy/src/views/window/mod.rs || fail "Composer final flush lifecycle is missing"
}

component_source_checks() {
    local matches
    for path in \
        crates/muxy-ui/src/panel.rs \
        crates/muxy-ui/src/text_input.rs \
        crates/muxy/src/panels/mod.rs; do
        [[ -f "$path" ]] || fail "missing reusable panel source: $path"
    done
    rg -q '^pub mod panel;' crates/muxy-ui/src/lib.rs || fail "muxy-ui panel export is missing"
    rg -q '^(pub )?mod panels;' crates/muxy/src/main.rs || fail "app panel adapter is not included"
    for symbol in PanelId PanelPosition PanelMode PanelPlacement PanelHost PanelChrome PanelFrame PanelResize PanelResizeState PanelSizing; do
        rg -q "pub (struct|enum) $symbol" crates/muxy-ui/src/panel.rs || fail "missing reusable panel symbol: $symbol"
    done
    for method in selected_range selected_text cursor_offset replace_selection insert_at_selection set_font_family set_paste_delegate clear_paste_delegate; do
        rg -q "pub fn $method" crates/muxy-ui/src/text_input.rs || fail "missing reusable TextInput method: $method"
    done
    rg -q 'FnMut\(&mut Window, &mut App\) -> bool' crates/muxy-ui/src/text_input.rs || fail "generic paste delegate cannot access the focused app context"
    rg -q 'window\.defer\(cx' crates/muxy-ui/src/text_input.rs || fail "paste delegate is not deferred outside the TextInput lease"
    rg -q 'delegate\.borrow_mut\(\)\(window, cx\)' crates/muxy-ui/src/text_input.rs || fail "paste delegate is not attempted before fallback"
    rg -q 'input\.paste_from_clipboard\(window, cx\)' crates/muxy-ui/src/text_input.rs || fail "declined paste delegation has no ordinary text fallback"
    rg -q 'with_phase_3_component_proof' crates/muxy/src/views/window/render.rs || fail "staged component construction proof is not mounted"
    rg -q '\.muxy-p7-components-status\.json' crates/muxy/src/panels/mod.rs || fail "staged component paint status is missing"
    matches="$(rg -ni 'composer|draft|terminal|extension' crates/muxy-ui/src/panel.rs || true)"
    [[ -z "$matches" ]] || {
        printf '%s\n' "$matches"
        fail "caller policy entered the reusable panel API"
    }
}

panel_source_checks() {
    local matches
    for path in \
        crates/muxy/src/composer/mod.rs \
        crates/muxy/src/composer/controller.rs \
        crates/muxy/src/composer/view.rs \
        crates/muxy/src/views/window/composer.rs; do
        [[ -f "$path" ]] || fail "missing Composer panel source: $path"
    done
    rg -q '^mod composer;' crates/muxy/src/main.rs || fail "Composer module is not included"
    rg -q 'ToggleRichInput' crates/muxy/src/keymap.rs || fail "Composer toggle action is not bound"
    rg -q 'crate::keymap::ToggleRichInput' crates/muxy/src/views/app.rs || fail "Composer toggle action has no main-window handler"
    rg -q 'window\.toggle_composer\(cx\)' crates/muxy/src/views/app.rs || fail "Composer toggle handler is inert"
    rg -q -F 'terminal_input_overlay_active(' crates/muxy/src/views/window/terminal.rs || fail "terminal input overlay focus policy is missing"
    rg -q -F 'self.composer_input_is_focused(window, cx)' crates/muxy/src/views/window/terminal.rs || fail "Composer editor focus does not suppress native terminal input"
    ! rg -q 'composer_is_open\(\)' crates/muxy/src/views/window/terminal.rs || fail "an open but unfocused Composer still suppresses terminal input"
    [[ "$(rg -c -F 'if window.composer_input_is_focused(window_handle, cx)' crates/muxy/src/views/app.rs)" == 2 ]] || fail "Composer submit shortcuts are not editor-focus gated"
    rg -q 'close_floating_composer_from_outside' crates/muxy/src/views/app.rs || fail "floating Composer outside-click handling is missing"
    rg -q 'PanelFrame::new' crates/muxy/src/composer/view.rs || fail "Composer does not construct the production panel"
    [[ "$(rg -c 'PanelAction::icon\(' crates/muxy/src/composer/view.rs)" == 4 ]] || fail "Composer header does not contain exactly four icon-backed controls"
    rg -q -F 'IconGlyph::new(Icon::Keyboard' crates/muxy/src/composer/view.rs || fail "Rich Input header keyboard icon is missing"
    rg -q -U -F $'return IconButton::new(\n                id,\n                icon,\n                metrics.scaled(13.0),\n                metrics.control_medium(),' crates/muxy-ui/src/panel.rs || fail "panel icon actions do not use the app topbar IconButton sizing"
    rg -q -F 'IconButton::new(' crates/muxy/src/views/titlebar.rs || fail "app topbar actions do not use the shared IconButton component"
    rg -q -F '.focus_handle(focus_handle)' crates/muxy-ui/src/panel.rs || fail "shared panel IconButton actions lost keyboard focus"
    rg -q -F '"Rich Input"' crates/muxy/src/composer/view.rs || fail "Rich Input header title is missing"
    rg -q -F '.with_placeholder("Type…")' crates/muxy/src/views/window/composer.rs || fail "Rich Input placeholder differs"
    rg -q -F '"composer-more-actions"' crates/muxy/src/composer/view.rs || fail "Composer bottom action menu is missing"
    rg -q -F 'Icon::ArrowUp' crates/muxy/src/composer/view.rs || fail "Composer shortcut send control is missing"
    rg -q -F 'shortcuts.combo(ShortcutAction::SubmitRichInput)' crates/muxy/src/composer/view.rs || fail "Composer send control does not read the configured submit shortcut"
    rg -q -F 'fn composer_send_label(combo: &KeyCombo)' crates/muxy/src/composer/view.rs || fail "Composer platform-specific send label is missing"
    rg -q -F '"Cmd+Enter to Send"' crates/muxy/src/composer/view.rs || fail "Composer macOS send label proof is missing"
    rg -q -F '"Ctrl+Enter to Send"' crates/muxy/src/composer/view.rs || fail "Composer non-macOS send label proof is missing"
    rg -q -F 'Icon::PinOff' crates/muxy/src/composer/view.rs || fail "Composer pinned state has no unpin icon"
    rg -q -F 'Self::PinOff => "pin.slash"' crates/muxy-ui/src/icon/mod.rs || fail "Composer unpin SF Symbol mapping is missing"
    [[ -f assets/icons/pin-off.svg ]] || fail "Composer unpin fallback icon is missing"
    for label in 'Send to Active Pane' 'Send to All Split Panes' 'Send Without Enter' 'Clear After Sending' 'Clear on Close'; do
        rg -q -F "\"$label\"" crates/muxy/src/views/window/composer.rs || fail "Composer retained menu action is missing: $label"
    done
    for command in ToggleComposerBroadcast SubmitComposerWithoutReturn ToggleComposerClearAfterSending ToggleComposerClearOnClose; do
        rg -q -F "$command" crates/muxy/src/command.rs crates/muxy/src/views/window/commands.rs || fail "Composer retained menu command is not routed: $command"
    done
    ! rg -qi 'microphone|composer-mic|Icon::Mic' crates/muxy/src/composer/view.rs || fail "P13 microphone UI entered the P7 Composer"
    rg -q -F 'composer_font_shortcut_delta' crates/muxy/src/composer/view.rs crates/muxy/src/views/window/composer.rs || fail "Composer font shortcuts are not wired"
    ! rg -q 'composer-font-(smaller|larger)|composer-send-without-return' crates/muxy/src/composer/view.rs || fail "Composer actions remain crowded into the panel header"
    rg -q '\.h\(self\.metrics\.title_bar_height\(\) \+ px\(1\.0\)\)' crates/muxy-ui/src/panel.rs || fail "panel header does not align with the app topbar separator"
    rg -q -U -F $'.h(metrics.status_bar_height())\n        .when(!merged, |toolbar| {\n            toolbar\n                .w_full()\n                .bg(theme.bg)\n                .border_t(px(1.0))\n                .border_color(theme.border)' crates/muxy/src/composer/view.rs || fail "standalone Composer footer does not match the app status bar frame"
    rg -q -F 'fn composer_footer_separator(theme: &Theme)' crates/muxy/src/composer/view.rs || fail "Composer footer separator style is missing"
    [[ "$(rg -c -F '.child(composer_footer_separator(theme))' crates/muxy/src/composer/view.rs)" == 3 ]] || fail "Composer footer does not contain the expected segmented cells"
    rg -q -F 'action.flex_shrink().min_w(metrics.control_small())' crates/muxy/src/composer/view.rs || fail "Composer shortcut segment cannot shrink in a narrow right panel"
    [[ "$(rg -c -F 'cx.focus_handle().tab_stop(true)' crates/muxy/src/composer/view.rs)" == 3 ]] || fail "Composer footer actions are not keyboard focusable"
    rg -q -F '.on_key_down(move |event, window, cx|' crates/muxy/src/composer/view.rs || fail "Composer footer actions are not keyboard activatable"
    rg -q -F 'merge_composer_footer_with_status_bar' crates/muxy/src/views/app.rs || fail "bottom Composer footer does not merge with the app status bar"
    rg -q -F 'trailing: Vec<AnyElement>' crates/muxy/src/views/status_bar.rs || fail "app status bar cannot host the bottom Composer footer"
    rg -q -F 'Some(footer)' crates/muxy/src/views/app.rs || fail "bottom Composer footer is not mounted in the app status bar"
    [[ "$(rg -U -c -F $'window.dismiss_overlay(cx);\n                        cx.stop_propagation();' crates/muxy/src/views/overlay.rs)" == 2 ]] || fail "overlay backdrop dismissal can reach floating Composer outside-click handling"
    rg -q -U -F $'.min_h(px(0.0))\n                .bg(theme.bg)\n                .child(input)' crates/muxy/src/composer/view.rs || fail "Composer editor is not borderless against the panel background"
    rg -q 'TextInput::new' crates/muxy/src/views/window/composer.rs || fail "Composer does not construct the multiline editor"
    rg -q '\.multiline\(\)' crates/muxy/src/views/window/composer.rs || fail "Composer editor is not multiline"
    for declaration in \
        $'#[serde(rename = "toggleRichInput")]\n    ToggleRichInput,' \
        $'#[serde(rename = "submitRichInput")]\n    SubmitRichInput,' \
        $'#[serde(rename = "submitRichInputWithoutReturn")]\n    SubmitRichInputWithoutReturn,'; do
        rg -q -U -F "$declaration" crates/muxy-core/src/shortcuts.rs || fail "Composer shortcut production declaration differs"
    done
    rg -q -U -F $'fn toggle_rich_input_default() -> KeyCombo {\n    if cfg!(target_os = "macos") {\n        KeyCombo::new("i", COMMAND)\n    } else {\n        KeyCombo::new("i", OPTION)\n    }\n}' crates/muxy-core/src/shortcuts.rs || fail "Composer toggle production defaults differ"
    rg -q -U -F $'(ToggleRichInput, toggle_rich_input_default()),\n        (SubmitRichInput, KeyCombo::new("return", COMMAND)),\n        (\n            SubmitRichInputWithoutReturn,\n            KeyCombo::new("return", COMMAND | SHIFT),\n        ),' crates/muxy-core/src/shortcuts.rs || fail "Composer submit production defaults differ"
    rg -q 'publish_composer_release' crates/muxy/src/views/window/composer.rs || fail "Composer release is not publication-gated"
    rg -q 'block_release' crates/muxy/src/composer/controller.rs crates/muxy/src/views/window/composer.rs || fail "failed Composer publication cannot retain the live editor"
    rg -q 'picker_target_matches' crates/muxy/src/composer/controller.rs crates/muxy/src/views/window/composer.rs || fail "Composer file picker has no draft identity guard"
    if rg -n 'WindowKind|open_window|WindowOptions|Modal' crates/muxy/src/composer crates/muxy/src/views/window/composer.rs; then
        fail "standalone or modal Composer presentation was introduced"
    fi
    rg -q '\.muxy-p7-panel-status\.json' crates/muxy/src/views/window/composer.rs || fail "staged Composer panel status is missing"
}

image_source_checks() {
    local matches
    for path in \
        crates/muxy/src/pasteboard/mod.rs \
        crates/muxy/src/pasteboard/macos.rs \
        crates/muxy/src/pasteboard/unsupported.rs; do
        [[ -f "$path" ]] || fail "missing Composer pasteboard source: $path"
    done
    rg -q 'MAX_ENCODED_IMAGE_BYTES: usize = 25 \* 1024 \* 1024' crates/muxy-core/src/composer/image_storage.rs || fail "Composer encoded image limit differs"
    rg -q 'MAX_DECODED_IMAGE_PIXELS: u64 = 64_000_000' crates/muxy-core/src/composer/image_storage.rs || fail "Composer pixel limit differs"
    rg -q 'normalize_png' crates/muxy-core/src/composer/image_storage.rs crates/muxy/src/views/window/composer.rs || fail "Composer PNG normalization is missing"
    rg -q 'prepare_image_source' crates/muxy-core/src/composer/image_storage.rs crates/muxy/src/views/window/composer.rs || fail "Composer image preparation is missing"
    rg -q 'attach_prepared_image' crates/muxy-core/src/composer/draft.rs crates/muxy/src/views/window/composer.rs || fail "prepared Composer image attachment is missing"
    rg -q 'with_paste_delegate' crates/muxy/src/views/window/composer.rs || fail "Composer native paste delegation is missing"
    rg -q 'PasteboardContent::Files' crates/muxy/src/views/window/composer.rs || fail "Composer clipboard file attachments are missing"
    rg -q 'PasteboardContent::Image' crates/muxy/src/views/window/composer.rs || fail "Composer clipboard image handling is missing"
    rg -q 'pasteboardItems' crates/muxy/src/pasteboard/macos.rs || fail "complete native pasteboard item capture is missing"
    rg -q 'dataForType' crates/muxy/src/pasteboard/macos.rs || fail "complete native pasteboard representation capture is missing"
    rg -q -F 'ok_or(PasteboardError::CaptureFailed)' crates/muxy/src/pasteboard/macos.rs || fail "unmaterialized native pasteboard representations do not abort capture"
    rg -q 'impl Drop for PasteboardReplacement' crates/muxy/src/pasteboard/mod.rs || fail "pasteboard cancellation restoration guard is missing"
    rg -q 'pasteboard_owner' crates/muxy/src/terminal/surfaces.rs crates/muxy/src/terminal/input_queue.rs || fail "app-wide pasteboard mutation serialization is missing"
    rg -q 'PASTE_SHORTCUT: &\[u8\] = b"\\x16"' crates/muxy-terminal/src/input.rs || fail "raw Ctrl-V bytes are missing"
    rg -q 'IMAGE_PASTE_DELAY: Duration = Duration::from_millis\(300\)' crates/muxy/src/terminal/input_queue.rs || fail "image paste delay differs"
    rg -q 'rollback_on_failure' crates/muxy-terminal/src/input.rs crates/muxy/src/terminal/input_queue.rs || fail "partial image rollback is missing"
    rg -q 'input_transaction_cancelled' crates/muxy/src/terminal/input_queue.rs || fail "image-window cancellation guard is missing"
    rg -q 'copied_image_filenames' crates/muxy/src/composer/submission.rs crates/muxy/src/views/window/composer.rs || fail "unique image normalization planning is missing"
    rg -q '"NSData"' Cargo.toml || fail "Objective-C NSData feature is missing"
    rg -q '"NSPasteboardItem"' Cargo.toml || fail "Objective-C NSPasteboardItem feature is missing"
    matches="$(rg -n 'objc2|AppKit|NSPasteboard' crates/muxy-core/src/composer || true)"
    [[ -z "$matches" ]] || {
        printf '%s\n' "$matches"
        fail "portable Composer image code crossed the native pasteboard boundary"
    }
}

drop_source_checks() {
    local matches
    [[ -f crates/muxy/src/views/window/dropped_paths.rs ]] || fail "missing app drop policy source"
    rg -q 'ExternalPaths' crates/muxy/src/composer/view.rs || fail "Composer GPUI external path drop handling is missing"
    rg -q 'handle_composer_drop' crates/muxy/src/composer/view.rs crates/muxy/src/views/window/dropped_paths.rs || fail "Composer drop policy is not connected"
    rg -q 'drag_over::<ExternalPaths>' crates/muxy/src/views/sidebar.rs || fail "sidebar external drag highlight is missing"
    rg -q 'handle_sidebar_drop' crates/muxy/src/views/sidebar.rs crates/muxy/src/views/window/dropped_paths.rs || fail "sidebar directory drop policy is not connected"
    rg -q 'muxy_core::dropped_paths::parse' crates/muxy/src/views/window/dropped_paths.rs || fail "app drop surfaces do not use the shared parser"
    rg -q -F 'Path::new(path).is_dir()' crates/muxy/src/views/window/dropped_paths.rs || fail "sidebar drop policy does not prefilter existing directories"
    if rg -n 'add_project_path|alert\(|ask\(|feedback\(' crates/muxy/src/views/window/dropped_paths.rs; then
        fail "sidebar drop policy can route files through user-visible project mutation"
    fi
    rg -q 'registerForDraggedTypes' crates/muxy-terminal/src/ghostty/host_view.rs || fail "native terminal drag types are not registered"
    rg -q 'public.file-url' crates/muxy-terminal/src/ghostty/host_view.rs || fail "native terminal file URL drag type is missing"
    rg -q 'public.utf8-plain-text' crates/muxy-terminal/src/ghostty/host_view.rs || fail "native terminal plain string drag type is missing"
    rg -q 'HostViewEvent::ExternalDrop' crates/muxy-terminal/src/ghostty/host_view.rs crates/muxy/src/terminal/ghostty/mod.rs || fail "neutral terminal host drop event is not routed"
    rg -q 'external_drop_receiver' crates/muxy/src/terminal/ghostty/mod.rs crates/muxy/src/terminal/surfaces.rs || fail "terminal drop routing does not preserve surface identity"
    rg -q 'inject_staged_external_drop' crates/muxy/src/terminal/ghostty/mod.rs crates/muxy/src/terminal/surfaces.rs crates/muxy/src/views/window/composer.rs || fail "staged terminal drop does not enter the routed event boundary"
    rg -q 'external_drop_adapter_extracts_file_url_and_plain_string_representations' crates/muxy-terminal/src/ghostty/host_view.rs || fail "native terminal drop extraction has no focused adapter test"
    rg -q 'handle_terminal_drop' crates/muxy/src/views/window/lifecycle.rs crates/muxy/src/views/window/dropped_paths.rs || fail "terminal drop event is not applied by the main window"
    rg -q 'shell_escape' crates/muxy/src/views/window/dropped_paths.rs || fail "terminal dropped paths are not shell escaped"
    rg -q '\.join\(" "\)' crates/muxy/src/views/window/dropped_paths.rs || fail "terminal dropped paths are not space joined"
    rg -q 'TerminalInputTransaction::new.*false' crates/muxy/src/views/window/dropped_paths.rs || fail "terminal drop insertion is not explicitly no-Return"
    rg -q 'set_focused_tab\(Some\(tab_id\)\)' crates/muxy/src/views/window/dropped_paths.rs || fail "terminal drop does not focus its target surface"
    rg -q '"NSDragging"' Cargo.toml || fail "Objective-C NSDragging feature is missing"
    matches="$(rg -n 'muxy_core::dropped_paths|shell_escape|TerminalInputTransaction' crates/muxy-terminal/src/ghostty/host_view.rs || true)"
    [[ -z "$matches" ]] || {
        printf '%s\n' "$matches"
        fail "app drop parsing or insertion policy entered the native terminal host"
    }
    rg -q 'external_drop_events' crates/muxy/src/terminal/surfaces.rs || fail "unsupported terminal drop surface is not neutral"
    rg -q '\.muxy-p7-drops-status\.json' crates/muxy/src/views/window/composer.rs scripts/verify-p7-composer.sh || fail "staged drop status proof is missing"
}

documentation_source_checks() {
    rg -q -F 'Composer is panel-only inside the main window.' ARCHITECTURE.md || fail "architecture does not document panel-only Composer ownership"
    rg -q -F 'Composer itself is panel-only.' docs/user-guide/settings.md || fail "settings guide does not document panel-only Composer"
    rg -q -F 'Composer is always an in-window panel.' docs/features/terminal.md || fail "terminal guide does not document panel-only Composer"
    rg -q -F 'IMPLEMENTED PANEL-ONLY; MANUAL NATIVE ACCEPTANCE PENDING' PLAN.md || fail "roadmap Composer status is stale"
    if rg -n 'separate centered modal|Use Floating Composer|Use Composer Panel|Composer dictation' docs/features/terminal.md docs/user-guide/settings.md; then
        fail "standalone or voice Composer documentation remains"
    fi
    for exact in 'builtin:richInput' 'rich-input-drafts.json' 'RichInputImages' 'toggleRichInput' 'submitRichInput' 'submitRichInputWithoutReturn'; do
        rg -q -F "$exact" scripts/check.sh scripts/verify-p7-composer.sh || fail "final exact-name guard is missing: $exact"
    done
}

submission_source_checks() {
    local matches
    for path in \
        crates/muxy-core/src/composer/submission.rs \
        crates/muxy-terminal/src/input.rs \
        crates/muxy/src/composer/submission.rs \
        crates/muxy/src/terminal/input_queue.rs; do
        [[ -f "$path" ]] || fail "missing Composer submission source: $path"
    done
    matches="$(rg -n 'muxy[-_]terminal|TerminalInput' crates/muxy-core/src/composer/submission.rs || true)"
    [[ -z "$matches" ]] || {
        printf '%s\n' "$matches"
        fail "portable Composer submission planning crossed the terminal boundary"
    }
    matches="$(rg -n 'gpui|Composer|MainWindow|broadcast' crates/muxy-terminal/src/input.rs || true)"
    [[ -z "$matches" ]] || {
        printf '%s\n' "$matches"
        fail "neutral terminal input queue contains app policy"
    }
    rg -q 'SubmitRichInput' crates/muxy-core/src/shortcuts.rs crates/muxy/src/keymap.rs || fail "submitRichInput is not modelled and bound"
    rg -q 'SubmitRichInputWithoutReturn' crates/muxy-core/src/shortcuts.rs crates/muxy/src/keymap.rs || fail "submitRichInputWithoutReturn is not modelled and bound"
    rg -q 'crate::keymap::SubmitRichInput' crates/muxy/src/views/app.rs || fail "submitRichInput has no main-window handler"
    rg -q 'crate::keymap::SubmitRichInputWithoutReturn' crates/muxy/src/views/app.rs || fail "submitRichInputWithoutReturn has no main-window handler"
    rg -q 'window\.submit_composer\(true, cx\)' crates/muxy/src/views/app.rs || fail "submitRichInput handler is inert"
    rg -q 'window\.submit_composer\(false, cx\)' crates/muxy/src/views/app.rs || fail "submitRichInputWithoutReturn handler is inert"
    rg -q 'shell_escape' crates/muxy/src/composer/submission.rs || fail "local paths are not shell escaped in the app"
    rg -q 'try_exists' crates/muxy/src/composer/submission.rs || fail "local files are not preflighted"
    rg -q 'INITIAL_INPUT_DELAY: Duration = Duration::from_millis\(50\)' crates/muxy/src/terminal/input_queue.rs || fail "initial terminal input delay differs"
    rg -q 'BRACKETED_PASTE_START' crates/muxy-terminal/src/input.rs || fail "bracketed paste start bytes are missing"
    rg -q 'BRACKETED_PASTE_END' crates/muxy-terminal/src/input.rs || fail "bracketed paste end bytes are missing"
    rg -q 'CARRIAGE_RETURN' crates/muxy-terminal/src/input.rs || fail "raw Return bytes are missing"
    rg -q 'set_input_transaction_active' crates/muxy-terminal/src/backend.rs crates/muxy-terminal/src/ghostty/host_view.rs || fail "native transaction deferral seam is missing"
    rg -q 'DeferredNativeKeyEvent::Down' crates/muxy-terminal/src/ghostty/host_view.rs || fail "native key-down deferral is missing"
    rg -q 'cancel_input_transaction' crates/muxy-terminal/src/backend.rs crates/muxy-terminal/src/ghostty/host_view.rs || fail "native transaction cancellation seam is missing"
    rg -q 'clear_if_revision' crates/muxy/src/views/window/composer.rs || fail "Composer submission clearing is not revision guarded"
    rg -q 'for pane_id in target_pane_ids' crates/muxy/src/views/window/composer.rs || fail "broadcast submission does not preserve captured pane order"
    rg -q 'staged_broadcast_targets_ready' crates/muxy/src/views/window/composer.rs || fail "staged broadcast proof does not wait for both live panes"
    rg -q 'DeferredNativeKeyEvent::MonitoredDown' crates/muxy-terminal/src/ghostty/host_view.rs || fail "monitored native key-down deferral is missing"
    rg -q 'activeBytes' crates/muxy/src/views/window/composer.rs scripts/verify-p7-composer.sh || fail "staged exact-byte proof is missing"
    if rg -n 'send_bytes\(&target\.pane_id, b"\\r"\)' crates/muxy/src/views/window/composer.rs; then
        fail "Composer submit bypasses pane transaction ordering"
    fi
    rg -q -F '"Send Without Enter"' crates/muxy/src/views/window/composer.rs || fail "send-without-Return menu control is missing"
    rg -q -F 'SubmitComposerWithoutReturn' crates/muxy/src/command.rs crates/muxy/src/views/window/commands.rs || fail "send-without-Return menu control is inert"
    rg -q -F '"composer-send"' crates/muxy/src/composer/view.rs || fail "send control is missing"
    if rg -n 'forward_composer_passthrough' crates/muxy/src/composer crates/muxy/src/views/window/composer.rs; then
        fail "modelled Composer submission still uses Phase 4 passthrough"
    fi
}

self_test() {
    local nonce="$$" root missing mismatch linked target outside bad_app mode case_name final_root app_support
    prepare_root
    source_checks
    persistence_source_checks
    component_source_checks
    panel_source_checks
    submission_source_checks
    image_source_checks
    drop_source_checks
    documentation_source_checks
    root="$VERIFICATION_ROOT/self-test-$nonce"
    prepare_case "$root"
    path_is_safe "$root/child" || fail "safe P7 path was rejected"
    outside="$PROJECT_ROOT/target/p7-outside-$nonce"
    if path_is_safe "$outside"; then
        fail "path outside P7 root was accepted"
    fi
    if path_is_safe "$VERIFICATION_ROOT"; then
        fail "P7 root itself was accepted as a disposable case"
    fi
    for production_path in "$HOME/.muxy" "$HOME/.muxy-dev" "$HOME/Library/Application Support/Muxy"; do
        if path_is_safe "$production_path"; then
            fail "production path was accepted: $production_path"
        fi
    done
    staged_case_supported debug phase-7 || fail "supported debug staged case was rejected"
    staged_case_supported release final-release || fail "supported release final case was rejected"
    if staged_case_supported debug final-release || staged_case_supported release final-debug || staged_case_supported debug unsupported; then
        fail "unsupported staged case or profile pairing was accepted"
    fi
    missing="$VERIFICATION_ROOT/missing-$nonce"
    mkdir -p "$missing"
    printf '%s\n' retained > "$missing/sentinel"
    if (prepare_case "$missing") >/dev/null 2>&1; then
        fail "case without ownership marker was accepted"
    fi
    [[ -f "$missing/sentinel" ]] || fail "unowned case data was deleted"
    mismatch="$VERIFICATION_ROOT/mismatch-$nonce"
    mkdir -p "$mismatch"
    printf '%s\n' wrong > "$mismatch/$CASE_MARKER"
    printf '%s\n' retained > "$mismatch/sentinel"
    if (prepare_case "$mismatch") >/dev/null 2>&1; then
        fail "case with wrong ownership marker was accepted"
    fi
    [[ -f "$mismatch/sentinel" ]] || fail "wrong-owner case data was deleted"
    target="$VERIFICATION_ROOT/linked-target-$nonce"
    linked="$VERIFICATION_ROOT/linked-$nonce"
    mkdir -p "$target"
    printf '%s\n' retained > "$target/sentinel"
    ln -s "$target" "$linked"
    if path_is_safe "$linked/child"; then
        fail "symlinked P7 path was accepted"
    fi
    [[ -f "$target/sentinel" ]] || fail "symlink rejection deleted target data"
    for mode in debug release; do
        if [[ "$mode" == debug ]]; then
            case_name="final-debug"
        else
            case_name="final-release"
        fi
        final_root="$VERIFICATION_ROOT/staged/$mode/$case_name"
        for app_support in \
            "$final_root/2/v" \
            "$final_root/2/m" \
            "$final_root/3/s" \
            "$final_root/4/s" \
            "$final_root/5/s" \
            "$final_root/6/s" \
            "$final_root/7/s"; do
            socket_path_is_supported "$mode" "$app_support" || fail "final staged socket path exceeds the macOS limit: $app_support"
        done
    done
    bad_app="$root/invalid/MuxyTests.app"
    mkdir -p "$bad_app/Contents/MacOS"
    cat > "$bad_app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>CFBundleIdentifier</key><string>com.muxy.app</string><key>CFBundleExecutable</key><string>MuxyTests</string></dict></plist>
PLIST
    : > "$bad_app/Contents/MacOS/MuxyTests"
    chmod +x "$bad_app/Contents/MacOS/MuxyTests"
    if (validate_staged_bundle_identity "$bad_app") >/dev/null 2>&1; then
        fail "staged app with production identity was accepted"
    fi
    rm -f -- "$linked"
    rm -rf -- "$root" "$missing" "$mismatch" "$target"
    printf 'P7 Composer verifier self-test passed\n'
}

staged_path_has_symlink() {
    local candidate="$1" current="/" part
    local -a parts
    IFS='/' read -r -a parts <<< "${candidate#/}"
    for part in "${parts[@]}"; do
        [[ -n "$part" ]] || continue
        current="${current%/}/$part"
        [[ ! -L "$current" ]] || return 0
    done
    return 1
}

validate_staged_bundle_identity() {
    local app="$1" executable_name plist="$1/Contents/Info.plist"
    [[ -f "$plist" && ! -L "$plist" ]] || fail "staged bundle plist is missing or symlinked"
    [[ "$(plutil -extract CFBundleIdentifier raw -o - "$plist")" == "com.muxy.tests" ]] || fail "staged bundle identifier is not com.muxy.tests"
    executable_name="$(plutil -extract CFBundleExecutable raw -o - "$plist")"
    [[ "$executable_name" == "MuxyTests" ]] || fail "staged executable identity differs: $executable_name"
    [[ -x "$app/Contents/MacOS/$executable_name" ]] || fail "staged executable is missing"
}

validate_staged_app() {
    local app="$1" parent marker
    [[ "$app" == /* ]] || fail "staged app path must be absolute"
    [[ "$app" != *"/../"* && "$app" != */.. && "$app" != *"/./"* && "$app" != */. ]] || fail "staged app path contains traversal"
    [[ "$app" == "$STAGED_APPS_ROOT/"*/MuxyTests.app ]] || fail "staged app has an invalid shape"
    parent="$(dirname "$app")"
    [[ "$(dirname "$parent")" == "$STAGED_APPS_ROOT" ]] || fail "staged app is outside the staging root"
    staged_path_has_symlink "$app" && fail "staged app has a symlinked path component"
    [[ -d "$app" && ! -L "$app" ]] || fail "staged app is missing or symlinked: $app"
    [[ "$(cd "$STAGED_APPS_ROOT" && pwd -P)" == "$STAGED_APPS_ROOT" ]] || fail "staging root changed identity"
    marker="$parent/$STAGED_OWNER_FILE"
    [[ -f "$marker" && ! -L "$marker" ]] || fail "staged app ownership marker is missing"
    [[ "$(<"$marker")" == "$app" ]] || fail "staged app ownership marker differs"
    validate_staged_bundle_identity "$app"
    codesign --verify --deep --strict "$app"
}

capture_production_socket() {
    PRODUCTION_SOCKET_IDENTITY=""
    PRODUCTION_SOCKET_OWNER=""
    if [[ -e "$PRODUCTION_SOCKET" ]]; then
        [[ -S "$PRODUCTION_SOCKET" && ! -L "$PRODUCTION_SOCKET" ]] || fail "production socket is not a direct socket"
        PRODUCTION_SOCKET_IDENTITY="$(stat -f '%d:%i' "$PRODUCTION_SOCKET")"
        PRODUCTION_SOCKET_OWNER="$(lsof -t "$PRODUCTION_SOCKET" | head -n 1)"
        [[ -n "$PRODUCTION_SOCKET_OWNER" ]] || fail "production socket has no live owner"
        kill -0 "$PRODUCTION_SOCKET_OWNER" 2>/dev/null || fail "production socket owner is not live"
    fi
}

verify_production_socket() {
    [[ -n "$PRODUCTION_SOCKET_IDENTITY" ]] || return
    [[ -S "$PRODUCTION_SOCKET" && ! -L "$PRODUCTION_SOCKET" ]] || fail "production socket disappeared or changed type"
    [[ "$(stat -f '%d:%i' "$PRODUCTION_SOCKET")" == "$PRODUCTION_SOCKET_IDENTITY" ]] || fail "production socket identity changed"
    [[ "$(lsof -t "$PRODUCTION_SOCKET" | head -n 1)" == "$PRODUCTION_SOCKET_OWNER" ]] || fail "production socket owner changed"
    kill -0 "$PRODUCTION_SOCKET_OWNER" 2>/dev/null || fail "production socket owner exited"
}

snapshot_profile() {
    local path="$1" destination="$2" entry relative kind detail
    if [[ ! -e "$path" ]]; then
        printf 'absent\n' > "$destination"
        return
    fi
    [[ -d "$path" && ! -L "$path" ]] || fail "live profile is not a direct directory: $path"
    {
        printf 'present\n'
        while IFS= read -r -d '' entry; do
            relative="${entry#"$path"}"
            [[ -n "$relative" ]] || relative="/"
            if [[ -L "$entry" ]]; then
                kind="symlink"
                detail="$(readlink "$entry")"
            elif [[ -f "$entry" ]]; then
                kind="file"
                detail="$(shasum -a 256 "$entry" | cut -d ' ' -f 1)"
            elif [[ -d "$entry" ]]; then
                kind="directory"
                detail="-"
            elif [[ -S "$entry" ]]; then
                kind="socket"
                detail="-"
            else
                kind="other"
                detail="-"
            fi
            printf '%s|%s|%s|%s\n' "$kind" "$(stat -f '%Lp:%u:%g:%m:%z' "$entry")" "$detail" "$relative"
        done < <(find "$path" -print0 | sort -z)
    } > "$destination"
}

record_descendants() {
    local root="$1" output="$2" current child
    local -a queue
    : > "$output"
    queue=("$root")
    while ((${#queue[@]} > 0)); do
        current="${queue[0]}"
        queue=("${queue[@]:1}")
        while IFS= read -r child; do
            [[ "$child" =~ ^[1-9][0-9]*$ ]] || continue
            if ! grep -Fxq "$child" "$output"; then
                printf '%s\n' "$child" >> "$output"
                queue+=("$child")
            fi
        done < <(pgrep -P "$current" 2>/dev/null || true)
    done
}

cleanup_active() {
    if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" 2>/dev/null; then
        kill -TERM "$APP_PID" 2>/dev/null || true
        wait "$APP_PID" 2>/dev/null || true
    fi
    APP_PID=""
    if [[ -n "$ACTIVE_SOCKET" && -e "$ACTIVE_SOCKET" && -n "$ACTIVE_CASE_ROOT" ]] && path_is_safe "$ACTIVE_SOCKET"; then
        rm -f -- "$ACTIVE_SOCKET"
    fi
}

socket_path_is_supported() {
    local mode="$1" app_support="$2" socket_name="muxy.sock"
    [[ "$mode" == debug ]] && socket_name="muxy-dev.sock"
    ((${#app_support} + ${#socket_name} + 1 < 104))
}

run_lifecycle_once() {
    local mode="$1" executable="$2" staged_cli="$3" lifecycle_root="$4" app_support="$5" control_case="$6"
    local socket_name socket log status_file descendants status pid
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    socket_name="muxy.sock"
    [[ "$mode" == debug ]] && socket_name="muxy-dev.sock"
    socket="$app_support/$socket_name"
    socket_path_is_supported "$mode" "$app_support" || fail "staged socket path exceeds the macOS limit"
    log="$lifecycle_root/app.log"
    status_file="$app_support/.muxy-p7-composer-status.json"
    [[ "$control_case" == phase-3 ]] && status_file="$app_support/.muxy-p7-components-status.json"
    [[ "$control_case" == phase-4 ]] && status_file="$app_support/.muxy-p7-panel-status.json"
    [[ "$control_case" == phase-5 ]] && status_file="$app_support/.muxy-p7-submission-status.json"
    [[ "$control_case" == phase-6 ]] && status_file="$app_support/.muxy-p7-images-status.json"
    [[ "$control_case" == phase-7 ]] && status_file="$app_support/.muxy-p7-drops-status.json"
    descendants="$lifecycle_root/descendants"
    rm -f -- "$status_file"
    MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY="$app_support" \
        MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 \
        MUXY_TEST_P7_COMPOSER_CASE="$control_case" \
        HOME="$app_support/home" \
        CFFIXED_USER_HOME="$app_support/home" \
        TMPDIR="$app_support/tmp/" \
        XDG_CONFIG_HOME="$app_support/xdg" \
        "$executable" > "$log" 2>&1 &
    APP_PID=$!
    ACTIVE_SOCKET="$socket"
    trap cleanup_active EXIT INT TERM
    for _ in $(jot 600); do
        if [[ -S "$socket" ]] && { [[ -z "$control_case" ]] || [[ -f "$status_file" ]]; }; then
            break
        fi
        kill -0 "$APP_PID" 2>/dev/null || {
            cat "$log"
            fail "staged app exited before becoming ready"
        }
        sleep 0.05
    done
    [[ -S "$socket" ]] || fail "staged app did not create its injected socket"
    [[ -z "$control_case" || -f "$status_file" ]] || fail "staged Composer status was not written"
    [[ "$(stat -f '%Lp' "$socket")" == 600 ]] || fail "staged socket mode is not 0600"
    MUXY_SOCKET_PATH="$socket" MUXY_CLI_TIMEOUT=5 "$staged_cli" list-projects > "$lifecycle_root/list-projects.txt"
    verify_production_socket
    record_descendants "$APP_PID" "$descendants"
    printf '%s\n' close > "$app_support/.muxy-test-close-main-window"
    for _ in $(jot 600); do
        ! kill -0 "$APP_PID" 2>/dev/null && break
        sleep 0.05
    done
    kill -0 "$APP_PID" 2>/dev/null && fail "staged app did not close normally"
    set +e
    wait "$APP_PID"
    status=$?
    set -e
    APP_PID=""
    [[ "$status" == 0 ]] || {
        cat "$log"
        fail "staged app exited with status $status"
    }
    [[ ! -e "$socket" ]] || fail "staged socket remained after normal close"
    [[ ! -e "$app_support/.muxy-test-close-main-window" ]] || fail "staged close request remained after close"
    while IFS= read -r pid; do
        [[ -n "$pid" ]] || continue
        for _ in $(jot 100); do
            ! kill -0 "$pid" 2>/dev/null && break
            sleep 0.05
        done
        ! kill -0 "$pid" 2>/dev/null || fail "staged descendant process survived: $pid"
    done < "$descendants"
    if pgrep -f "$executable" >/dev/null; then
        fail "staged app process survived verification"
    fi
    verify_production_socket
    ACTIVE_SOCKET=""
    trap - EXIT INT TERM
}

write_phase_2_valid_fixture() {
    local app_support="$1" images="$1/RichInputImages"
    mkdir -p "$images"
    chmod 0700 "$images"
    printf '%s' referenced > "$images/22222222-3333-4444-8555-666666666666.png"
    printf '%s' orphan > "$images/33333333-4444-4555-8666-777777777777.png"
    chmod 0600 "$images"/*
    cat > "$app_support/rich-input-drafts.json" <<'JSON'
{
  "AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE:11111111-2222-4333-8444-555555555555": {
    "text": "fixture [Image 1]",
    "fileAttachments": ["/tmp/p7-fixture"],
    "imageAttachments": [
      {"number": 1, "filename": "22222222-3333-4444-8555-666666666666.png"}
    ],
    "nextImageNumber": 2
  }
}
JSON
    chmod 0600 "$app_support/rich-input-drafts.json"
}

run_phase_2_staged() {
    local mode="$1" executable="$2" staged_cli="$3" case_root="$4"
    local valid_support malformed_support malformed_hash
    valid_support="$case_root/v"
    mkdir -p "$valid_support"
    write_phase_2_valid_fixture "$valid_support"
    mkdir -p "$case_root/valid-first"
    run_lifecycle_once "$mode" "$executable" "$staged_cli" "$case_root/valid-first" "$valid_support" phase-2
    jq -e '.draftCount == 2 and .overwriteBlocked == false and .malformedKeys == [] and (.imageFiles == ["22222222-3333-4444-8555-666666666666.png"])' \
        "$valid_support/.muxy-p7-composer-status.json" >/dev/null || fail "valid staged Composer status differs"
    jq -e '.["BBBBBBBB-CCCC-4DDD-8EEE-FFFFFFFFFFFF:66666666-7777-4888-8999-AAAAAAAAAAAA"].text == "debounced phase-2 draft"' \
        "$valid_support/rich-input-drafts.json" >/dev/null || fail "debounced staged draft was not published before close"
    [[ -f "$valid_support/RichInputImages/22222222-3333-4444-8555-666666666666.png" ]] || fail "referenced staged image was removed"
    [[ ! -e "$valid_support/RichInputImages/33333333-4444-4555-8666-777777777777.png" ]] || fail "staged orphan image survived"
    mkdir -p "$case_root/valid-relaunch"
    run_lifecycle_once "$mode" "$executable" "$staged_cli" "$case_root/valid-relaunch" "$valid_support" phase-2
    jq -e '.draftCount == 2 and .overwriteBlocked == false' "$valid_support/.muxy-p7-composer-status.json" >/dev/null || fail "relaunch did not restore the valid drafts"
    malformed_support="$case_root/m"
    mkdir -p "$malformed_support/RichInputImages"
    chmod 0700 "$malformed_support/RichInputImages"
    printf '%s' retained > "$malformed_support/RichInputImages/44444444-5555-4666-8777-888888888888.png"
    chmod 0600 "$malformed_support/RichInputImages/44444444-5555-4666-8777-888888888888.png"
    printf '%s' 'not json' > "$malformed_support/rich-input-drafts.json"
    chmod 0600 "$malformed_support/rich-input-drafts.json"
    malformed_hash="$(shasum -a 256 "$malformed_support/rich-input-drafts.json" | cut -d ' ' -f 1)"
    mkdir -p "$case_root/malformed"
    run_lifecycle_once "$mode" "$executable" "$staged_cli" "$case_root/malformed" "$malformed_support" phase-2
    [[ "$(shasum -a 256 "$malformed_support/rich-input-drafts.json" | cut -d ' ' -f 1)" == "$malformed_hash" ]] || fail "malformed draft file was overwritten"
    [[ -f "$malformed_support/RichInputImages/44444444-5555-4666-8777-888888888888.png" ]] || fail "uncertain malformed-state image was swept"
    jq -e '.draftCount == 0 and .overwriteBlocked == true' "$malformed_support/.muxy-p7-composer-status.json" >/dev/null || fail "malformed staged Composer status differs"
}

run_phase_3_staged() {
    local mode="$1" executable="$2" staged_cli="$3" case_root="$4" app_support status_file
    app_support="$case_root/s"
    status_file="$app_support/.muxy-p7-components-status.json"
    mkdir -p "$case_root/components" "$app_support"
    run_lifecycle_once "$mode" "$executable" "$staged_cli" "$case_root/components" "$app_support" phase-3
    jq -e '
        .painted == true and
        .panelId == "phase-3-component-proof" and
        .position == "right" and
        .mode == "floating" and
        .dimension == 320 and
        .overlaysWorkspace == true and
        .chromeActions == ["move", "mode", "close", "custom"] and
        .textInput.multiline == true and
        .textInput.fontFamily == ".SystemUIFontMonospaced" and
        .textInput.fontSize == 14 and
        .textInput.lineHeight == 22 and
        .textInput.pasteDelegate == "deferred"
    ' "$status_file" >/dev/null || fail "staged component construction or paint status differs"
}

write_phase_4_fixture() {
    local app_support="$1" project_path="$1/project"
    local project_id="AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"
    local worktree_id="11111111-2222-4333-8444-555555555555"
    mkdir -p "$project_path" "$app_support/worktrees"
    cat > "$app_support/projects.json" <<JSON
[
  {
    "id": "$project_id",
    "name": "P7 Phase 4",
    "path": "$project_path",
    "sortOrder": 0,
    "createdAt": 1,
    "worktreesEnabled": false,
    "isPinned": false
  }
]
JSON
    cat > "$app_support/worktrees/$project_id.json" <<JSON
[
  {
    "id": "$worktree_id",
    "name": "P7 Phase 4",
    "path": "$project_path",
    "source": "muxy",
    "isPrimary": true,
    "createdAt": 1
  }
]
JSON
    cat > "$app_support/preferences.json" <<JSON
{
  "muxy.activeProjectID": "$project_id",
  "muxy.activeWorktreeIDs": {
    "$project_id": "$worktree_id"
  },
  "muxy.panel.mode.builtin:richInput": "floating",
  "muxy.richInput.position": "right",
  "muxy.richInputPanelWidth": 380,
  "muxy.richInputPanelHeight": 220,
  "muxy.richInput.broadcast": false,
  "muxy.richInput.fontSize": 13,
  "p7.unrelatedPreference": "preserved"
}
JSON
    cat > "$app_support/rich-input-drafts.json" <<JSON
{
  "$project_id:$worktree_id": {
    "text": "restored phase-4 draft",
    "fileAttachments": [],
    "imageAttachments": [],
    "nextImageNumber": 1
  }
}
JSON
    chmod 0600 "$app_support/projects.json" "$app_support/worktrees/$project_id.json" \
        "$app_support/preferences.json" "$app_support/rich-input-drafts.json"
}

run_phase_4_staged() {
    local mode="$1" executable="$2" staged_cli="$3" case_root="$4"
    local app_support="$case_root/s" status_file="$case_root/s/.muxy-p7-panel-status.json"
    mkdir -p "$app_support" "$case_root/first" "$case_root/relaunch"
    write_phase_4_fixture "$app_support"
    run_lifecycle_once "$mode" "$executable" "$staged_cli" "$case_root/first" "$app_support" phase-4
    jq -e '
        .painted == true and
        .panelId == "builtin:richInput" and
        .position == "bottom" and
        .mode == "pinned" and
        .dimension == 260 and
        .overlaysWorkspace == false and
        .broadcast == true and
        .fontSize == 15 and
        .text == "phase-4 draft" and
        .fileAttachments == ["/tmp/p7-phase-4.txt"] and
        .restoredBeforeEdit.text == "restored phase-4 draft" and
        .restoredBeforeEdit.position == "right" and
        .restoredBeforeEdit.mode == "floating" and
        .restoredBeforeEdit.dimension == 380 and
        .restoredBeforeEdit.broadcast == false and
        .restoredBeforeEdit.fontSize == 13
    ' "$status_file" >/dev/null || fail "first staged Composer panel status differs"
    jq -e '
        .["muxy.panel.mode.builtin:richInput"] == "pinned" and
        .["muxy.richInput.position"] == "bottom" and
        .["muxy.richInputPanelWidth"] == 380 and
        .["muxy.richInputPanelHeight"] == 260 and
        .["muxy.richInput.broadcast"] == true and
        .["muxy.richInput.fontSize"] == 15 and
        .["p7.unrelatedPreference"] == "preserved"
    ' "$app_support/preferences.json" >/dev/null || fail "Composer panel preferences did not persist in the injected profile"
    jq -e '
        .["AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE:11111111-2222-4333-8444-555555555555"].text == "phase-4 draft" and
        .["AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE:11111111-2222-4333-8444-555555555555"].fileAttachments == ["/tmp/p7-phase-4.txt"]
    ' "$app_support/rich-input-drafts.json" >/dev/null || fail "Composer draft edit did not persist"
    run_lifecycle_once "$mode" "$executable" "$staged_cli" "$case_root/relaunch" "$app_support" phase-4
    jq -e '
        .painted == true and
        .position == "bottom" and
        .mode == "pinned" and
        .dimension == 260 and
        .broadcast == true and
        .fontSize == 15 and
        .restoredBeforeEdit.text == "phase-4 draft" and
        .restoredBeforeEdit.position == "bottom" and
        .restoredBeforeEdit.mode == "pinned" and
        .restoredBeforeEdit.dimension == 260 and
        .restoredBeforeEdit.broadcast == true and
        .restoredBeforeEdit.fontSize == 15
    ' "$status_file" >/dev/null || fail "Composer relaunch restoration status differs"
}

write_phase_5_fixture() {
    local app_support="$1" project_path="$1/project" attachment="$1/attached file's script.sh"
    local project_id="AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"
    local worktree_id="11111111-2222-4333-8444-555555555555"
    local pane_a="22222222-3333-4444-8555-666666666666"
    local pane_b="77777777-8888-4999-8AAA-BBBBBBBBBBBB"
    local area_a="CCCCCCCC-DDDD-4EEE-8FFF-000000000001"
    local area_b="CCCCCCCC-DDDD-4EEE-8FFF-000000000002"
    mkdir -p "$project_path" "$app_support/worktrees"
    cat > "$app_support/projects.json" <<JSON
[
  {
    "id": "$project_id",
    "name": "P7 Phase 5",
    "path": "$project_path",
    "sortOrder": 0,
    "createdAt": 1,
    "worktreesEnabled": false,
    "isPinned": false
  }
]
JSON
    cat > "$app_support/worktrees/$project_id.json" <<JSON
[
  {
    "id": "$worktree_id",
    "name": "P7 Phase 5",
    "path": "$project_path",
    "source": "muxy",
    "isPrimary": true,
    "createdAt": 1
  }
]
JSON
    cat > "$app_support/preferences.json" <<JSON
{
  "muxy.activeProjectID": "$project_id",
  "muxy.activeWorktreeIDs": {
    "$project_id": "$worktree_id"
  },
  "muxy.richInput.broadcast": false,
  "muxy.richInput.clearAfterSending": false
}
JSON
    cat > "$app_support/workspaces.json" <<JSON
[
  {
    "projectID": "$project_id",
    "worktreeID": "$worktree_id",
    "worktreePath": "$project_path",
    "focusedAreaID": "$area_a",
    "topLevelTabOrder": ["$pane_a"],
    "topLevelTabLayout": {
      "type": "group",
      "group": {
        "tabIDs": ["$pane_a"],
        "activeTabID": "$pane_a"
      }
    },
    "root": {
      "type": "split",
      "split": {
        "direction": "horizontal",
        "ratio": 0.5,
        "first": {
          "type": "tabArea",
          "tabArea": {
            "id": "$area_a",
            "projectPath": "$project_path",
            "tabs": [
              {
                "kind": "terminal",
                "id": "$pane_a",
                "isPinned": false,
                "projectPath": "$project_path"
              }
            ],
            "activeTabIndex": 0
          }
        },
        "second": {
          "type": "tabArea",
          "tabArea": {
            "id": "$area_b",
            "projectPath": "$project_path",
            "tabs": [
              {
                "kind": "terminal",
                "id": "$pane_b",
                "parentTabID": "$pane_a",
                "isPinned": false,
                "projectPath": "$project_path"
              }
            ],
            "activeTabIndex": 0
          }
        }
      }
    }
  }
]
JSON
    cat > "$attachment" <<SCRIPT
printf 'local-path' > "$app_support/phase-5-local.txt"
printf 'LOCAL_PATH_SCREEN\\n'
SCRIPT
    chmod 0600 "$app_support/projects.json" "$app_support/worktrees/$project_id.json" \
        "$app_support/preferences.json" "$app_support/workspaces.json"
    chmod 0700 "$attachment"
}

run_phase_5_staged() {
    local mode="$1" executable="$2" staged_cli="$3" case_root="$4"
    local app_support="$case_root/s" status_file="$case_root/s/.muxy-p7-submission-status.json"
    mkdir -p "$app_support" "$case_root/submission"
    write_phase_5_fixture "$app_support"
    run_lifecycle_once "$mode" "$executable" "$staged_cli" "$case_root/submission" "$app_support" phase-5
    jq -e --arg support "$app_support" --arg attachment "$app_support/attached file's script.sh" '
        def shell_escape:
            if test("^[A-Za-z0-9_./:@%+,\\-]+$") then .
            else "\u0027" + gsub("\u0027"; "\u0027\\\u0027\u0027") + "\u0027"
            end;
        def framed($text; $append_return):
            [[21], ([27, 91, 50, 48, 48, 126] + ($text | explode) + [27, 91, 50, 48, 49, 126])]
            + (if $append_return then [[13]] else [] end);
        ("printf \u0027ACTIVE_SCREEN\\n\u0027 | tee " + (($support + "/phase-5-active.txt") | shell_escape)) as $active |
        ("printf \u0027SELECTED_SCREEN\\n\u0027 | tee " + (($support + "/phase-5-selected.txt") | shell_escape)) as $selected |
        ($attachment | shell_escape) as $local_path |
        ("printf \u0027NO_RETURN_SCREEN\\n\u0027 | tee " + (($support + "/phase-5-no-return.txt") | shell_escape)) as $no_return |
        ("printf \u0027BROADCAST:%s\\n\u0027 \"$MUXY_PANE_ID\" | tee -a " + (($support + "/phase-5-broadcast.txt") | shell_escape)) as $broadcast |
        .activeSucceeded == true and
        .selectedSucceeded == true and
        .localPathSucceeded == true and
        .noReturnSucceeded == true and
        .noReturnBeforeReturn == true and
        .broadcastSucceeded == true and
        (.paneIds | length) == 2 and
        .activeOutput == "ACTIVE_SCREEN\n" and
        .selectedOutput == "SELECTED_SCREEN\n" and
        .localPathOutput == "local-path" and
        .noReturnOutput == "NO_RETURN_SCREEN\n" and
        .activeBytes == framed($active; true) and
        .selectedBytes == framed($selected; true) and
        .localPathBytes == framed($local_path; true) and
        .noReturnBytes == framed($no_return; false) and
        ([.paneIds[] as $id | .broadcastBytes[$id] == framed($broadcast; true)] | all) and
        ([.broadcastOutput | split("\n")[] | select(length > 0)] | length) == 2 and
        ([.paneIds[] as $id | .screens[$id] | contains("BROADCAST:" + $id)] | all)
    ' "$status_file" >/dev/null || fail "staged Composer submission status differs"
}

write_phase_6_fixture() {
    local app_support="$1" image
    local project_id="AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE"
    local worktree_id="11111111-2222-4333-8444-555555555555"
    image="22222222-3333-4444-8555-666666666666.png"
    write_phase_5_fixture "$app_support"
    mkdir -p "$app_support/RichInputImages"
    chmod 0700 "$app_support/RichInputImages"
    printf '%s' 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=' | base64 -D > "$app_support/RichInputImages/$image"
    cat > "$app_support/rich-input-drafts.json" <<JSON
{
  "$project_id:$worktree_id": {
    "text": "[Image 1]",
    "fileAttachments": [],
    "imageAttachments": [
      {"number": 1, "filename": "$image"}
    ],
    "nextImageNumber": 2
  }
}
JSON
    chmod 0600 "$app_support/RichInputImages/$image" "$app_support/rich-input-drafts.json"
}

run_phase_6_staged() {
    local mode="$1" executable="$2" staged_cli="$3" case_root="$4"
    local app_support="$case_root/s" status_file="$case_root/s/.muxy-p7-images-status.json"
    local image_path="$case_root/s/RichInputImages/22222222-3333-4444-8555-666666666666.png"
    mkdir -p "$app_support" "$case_root/images"
    write_phase_6_fixture "$app_support"
    run_lifecycle_once "$mode" "$executable" "$staged_cli" "$case_root/images" "$app_support" phase-6
    jq -e --arg image_path "$image_path" --arg output "$app_support/phase-6-inline.txt" '
        def shell_escape:
            if test("^[A-Za-z0-9_./:@%+,\\-]+$") then .
            else "\u0027" + gsub("\u0027"; "\u0027\\\u0027\u0027") + "\u0027"
            end;
        def framed($text): [27, 91, 50, 48, 48, 126] + ($text | explode) + [27, 91, 50, 48, 49, 126];
        .clipboardSucceeded == true and
        .clipboardRestored == true and
        .clipboardBytes == [[21], [22]] and
        .failureHandled == true and
        .failureClipboardRestored == true and
        (.paneIds | length) == 2 and
        .failureBytes[.paneIds[0]] == [[21], framed("first\nsecond "), [21, 8, 21]] and
        .failureBytes[.paneIds[1]] == [[21], framed("first\nsecond "), [22], [13]] and
        .draftRetainedAfterFailure == true and
        .imageRetainedAfterFailure == true and
        .inlineSucceeded == true and
        .inlineBytes == [[21], framed("printf \u0027%s\u0027 "), framed(($image_path | shell_escape)), framed(" > " + ($output | shell_escape)), [13]] and
        .inlineOutput == $image_path and
        .draftRetained == false and
        .imageRetained == false
    ' "$status_file" >/dev/null || fail "staged Composer image status differs"
    [[ ! -e "$image_path" ]] || fail "successfully cleared staged image remained"
    jq -e 'length == 0' "$app_support/rich-input-drafts.json" >/dev/null || fail "successfully cleared staged image draft remained"
}

write_phase_7_fixture() {
    local app_support="$1"
    write_phase_5_fixture "$app_support"
    mkdir -p "$app_support/drop-project"
    printf 'first\n' > "$app_support/drop first.txt"
    printf 'image\n' > "$app_support/drop-image.png"
    chmod 0700 "$app_support/drop-project"
    chmod 0600 "$app_support/drop first.txt" "$app_support/drop-image.png"
}

run_phase_7_staged() {
    local mode="$1" executable="$2" staged_cli="$3" case_root="$4"
    local app_support="$case_root/s" status_file="$case_root/s/.muxy-p7-drops-status.json"
    mkdir -p "$app_support" "$case_root/drops"
    write_phase_7_fixture "$app_support"
    run_lifecycle_once "$mode" "$executable" "$staged_cli" "$case_root/drops" "$app_support" phase-7
    jq -e --arg support "$app_support" '
        def shell_escape:
            if test("^[A-Za-z0-9_./:@%+,\\-]+$") then .
            else "\u0027" + gsub("\u0027"; "\u0027\\\u0027\u0027") + "\u0027"
            end;
        def framed($text): [27, 91, 50, 48, 48, 126] + ($text | explode) + [27, 91, 50, 48, 49, 126];
        ($support + "/drop first.txt") as $first |
        ($support + "/drop-image.png") as $image |
        (($first | shell_escape) + " " + ($image | shell_escape)) as $terminal |
        .composerAttachments == [$first, $image] and
        .copiedImageCount == 0 and
        .terminalInjected == true and
        .terminalSucceeded == true and
        .terminalFocused == true and
        .terminalBytes == [framed($terminal)] and
        .existingProjectSelected == true and
        .existingProjectCount == .initialProjectCount and
        .projectCount == (.initialProjectCount + 1) and
        .activeProjectPath == ($support + "/drop-project") and
        .newProjectAdded == true and
        .fileAddedAsProject == false
    ' "$status_file" >/dev/null || fail "staged external drop status differs"
    jq -e --arg first "$app_support/drop first.txt" --arg image "$app_support/drop-image.png" '
        .["AAAAAAAA-BBBB-4CCC-8DDD-EEEEEEEEEEEE:11111111-2222-4333-8444-555555555555"].fileAttachments == [$first, $image]
    ' "$app_support/rich-input-drafts.json" >/dev/null || fail "Composer dropped file attachments were not durable"
}

run_final_staged() {
    local mode="$1" executable="$2" staged_cli="$3" case_root="$4"
    run_phase_2_staged "$mode" "$executable" "$staged_cli" "$case_root/2"
    run_phase_3_staged "$mode" "$executable" "$staged_cli" "$case_root/3"
    run_phase_4_staged "$mode" "$executable" "$staged_cli" "$case_root/4"
    run_phase_5_staged "$mode" "$executable" "$staged_cli" "$case_root/5"
    run_phase_6_staged "$mode" "$executable" "$staged_cli" "$case_root/6"
    run_phase_7_staged "$mode" "$executable" "$staged_cli" "$case_root/7"
}

staged_case_supported() {
    local mode="$1" case_name="$2"
    case "$case_name" in
        phase-1 | phase-2 | phase-3 | phase-4 | phase-5 | phase-6 | phase-7) return 0 ;;
        final-debug) [[ "$mode" == debug ]] ;;
        final-release) [[ "$mode" == release ]] ;;
        *) return 1 ;;
    esac
}

run_staged() {
    local mode="$1" app="$2" case_name="$3" case_root executable staged_cli
    local before_debug before_release after_debug after_release
    [[ "$mode" == debug || "$mode" == release ]] || fail "profile must be debug or release"
    [[ "$case_name" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ ]] || fail "invalid staged case name"
    staged_case_supported "$mode" "$case_name" || fail "unsupported staged case for $mode: $case_name"
    validate_staged_app "$app"
    source_checks
    persistence_source_checks
    component_source_checks
    panel_source_checks
    submission_source_checks
    image_source_checks
    drop_source_checks
    documentation_source_checks
    case_root="$VERIFICATION_ROOT/staged/$mode/$case_name"
    prepare_case "$case_root"
    ACTIVE_CASE_ROOT="$case_root"
    executable="$app/Contents/MacOS/MuxyTests"
    staged_cli="$app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    [[ -x "$staged_cli" ]] || fail "legacy staged CLI path is missing"
    cmp -s "$SOURCE_CLI" "$staged_cli" || fail "staged CLI bytes differ from retained source"
    before_debug="$case_root/profile-debug-before"
    before_release="$case_root/profile-release-before"
    after_debug="$case_root/profile-debug-after"
    after_release="$case_root/profile-release-after"
    snapshot_profile "$HOME/.muxy-dev" "$before_debug"
    snapshot_profile "$HOME/.muxy" "$before_release"
    capture_production_socket
    if [[ "$case_name" == phase-2 ]]; then
        run_phase_2_staged "$mode" "$executable" "$staged_cli" "$case_root"
    elif [[ "$case_name" == phase-3 ]]; then
        run_phase_3_staged "$mode" "$executable" "$staged_cli" "$case_root"
    elif [[ "$case_name" == phase-4 ]]; then
        run_phase_4_staged "$mode" "$executable" "$staged_cli" "$case_root"
    elif [[ "$case_name" == phase-5 ]]; then
        run_phase_5_staged "$mode" "$executable" "$staged_cli" "$case_root"
    elif [[ "$case_name" == phase-6 ]]; then
        run_phase_6_staged "$mode" "$executable" "$staged_cli" "$case_root"
    elif [[ "$case_name" == phase-7 ]]; then
        run_phase_7_staged "$mode" "$executable" "$staged_cli" "$case_root"
    elif [[ "$case_name" == final-debug || "$case_name" == final-release ]]; then
        run_final_staged "$mode" "$executable" "$staged_cli" "$case_root"
    else
        mkdir -p "$case_root/generic" "$case_root/s"
        run_lifecycle_once "$mode" "$executable" "$staged_cli" "$case_root/generic" "$case_root/s" ""
    fi
    snapshot_profile "$HOME/.muxy-dev" "$after_debug"
    snapshot_profile "$HOME/.muxy" "$after_release"
    cmp -s "$before_debug" "$after_debug" || fail "debug profile changed during staged verification"
    cmp -s "$before_release" "$after_release" || fail "release profile changed during staged verification"
    ACTIVE_CASE_ROOT=""
    printf 'P7 Composer staged %s passed\n' "$case_name"
}

manual_case() {
    local mode="$1" app="$2" root app_support socket_name socket executable log staged_cli
    [[ "$mode" == debug || "$mode" == release ]] || fail "profile must be debug or release"
    validate_staged_app "$app"
    root="$VERIFICATION_ROOT/manual/$mode"
    prepare_case "$root"
    app_support="$root/s"
    mkdir -p "$app_support/home" "$app_support/tmp" "$app_support/xdg"
    socket_name="muxy.sock"
    [[ "$mode" == debug ]] && socket_name="muxy-dev.sock"
    socket="$app_support/$socket_name"
    executable="$app/Contents/MacOS/MuxyTests"
    log="$root/app.log"
    staged_cli="$app/Contents/Resources/Muxy_Muxy.bundle/scripts/muxy-cli"
    cmp -s "$SOURCE_CLI" "$staged_cli" || fail "staged CLI bytes differ from retained source"
    printf 'Bundle identity: com.muxy.tests\n'
    printf 'Application: %s\n' "$app"
    printf 'Executable: %s\n' "$executable"
    printf 'Application Support: %s\n' "$app_support"
    printf 'Socket: %s\n' "$socket"
    printf 'Log: %s\n' "$log"
    printf 'Launch:\n'
    printf 'env MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY=%q MUXY_TEST_CLOSE_MAIN_WINDOW_REQUEST=1 HOME=%q CFFIXED_USER_HOME=%q TMPDIR=%q XDG_CONFIG_HOME=%q %q >%q 2>&1 &\n' \
        "$app_support" "$app_support/home" "$app_support/home" "$app_support/tmp/" "$app_support/xdg" "$executable" "$log"
    printf 'Read-only CLI check:\n'
    printf 'MUXY_SOCKET_PATH=%q MUXY_CLI_TIMEOUT=5 %q list-projects\n' "$socket" "$staged_cli"
    printf 'Normal close request:\n'
    printf "printf 'close\\n' > %q\n" "$app_support/.muxy-test-close-main-window"
    printf 'Cleanup after the process exits:\n'
    printf 'rm -f %q\n' "$socket"
}

for command_name in cargo cmp cut find git grep ln mkdir pgrep rg rm shasum sort; do
    require_command "$command_name"
done

case "${1:-}" in
    --self-test)
        (($# == 1)) || fail "usage: scripts/verify-p7-composer.sh --self-test"
        require_command plutil
        self_test
        ;;
    --staged)
        (($# == 4)) || fail "usage: scripts/verify-p7-composer.sh --staged PROFILE ABSOLUTE_APP CASE"
        for command_name in base64 codesign jot jq lsof plutil stat; do
            require_command "$command_name"
        done
        run_staged "$2" "$3" "$4"
        ;;
    --manual)
        (($# == 3)) || fail "usage: scripts/verify-p7-composer.sh --manual PROFILE ABSOLUTE_APP"
        for command_name in codesign plutil; do
            require_command "$command_name"
        done
        manual_case "$2" "$3"
        ;;
    --fixture)
        (($# == 2)) || fail "usage: scripts/verify-p7-composer.sh --fixture CASE"
        case "$2" in
            phase-1)
                cargo test -p muxy-core --locked --offline dropped_paths
                cargo test -p muxy-core --locked --offline composer
                source_checks
                ;;
            persistence)
                cargo test -p muxy-core --locked --offline composer
                cargo test -p muxy-core --locked --offline private_directory
                cargo test -p muxy --locked --offline composer_store
                source_checks
                persistence_source_checks
                ;;
            components)
                cargo test -p muxy-ui --locked --offline panel
                cargo test -p muxy-ui --locked --offline text_input
                cargo test -p muxy --locked --offline panels
                source_checks
                persistence_source_checks
                component_source_checks
                ;;
            panel)
                cargo test -p muxy-core --locked --offline shortcuts
                cargo test -p muxy --locked --offline composer
                cargo test -p muxy-ui --locked --offline text_input
                source_checks
                persistence_source_checks
                component_source_checks
                panel_source_checks
                ;;
            submission)
                cargo test -p muxy-core --locked --offline composer
                cargo test -p muxy-terminal --locked --offline input
                cargo test -p muxy --locked --offline composer
                cargo test -p muxy --locked --offline terminal
                source_checks
                persistence_source_checks
                component_source_checks
                panel_source_checks
                submission_source_checks
                ;;
            images)
                cargo test -p muxy-core --locked --offline image
                cargo test -p muxy-terminal --locked --offline input
                cargo test -p muxy --locked --offline composer
                cargo test -p muxy --locked --offline terminal
                cargo test -p muxy --locked --offline pasteboard
                source_checks
                persistence_source_checks
                component_source_checks
                panel_source_checks
                submission_source_checks
                image_source_checks
                ;;
            drops)
                cargo test -p muxy-core --locked --offline dropped_paths
                cargo test -p muxy-terminal --locked --offline drop
                cargo test -p muxy --locked --offline dropped_paths
                source_checks
                persistence_source_checks
                component_source_checks
                panel_source_checks
                submission_source_checks
                image_source_checks
                drop_source_checks
                ;;
            all)
                cargo test -p muxy-core --locked --offline composer
                cargo test -p muxy-core --locked --offline dropped_paths
                cargo test -p muxy-core --locked --offline private_directory
                cargo test -p muxy-ui --locked --offline panel
                cargo test -p muxy-ui --locked --offline text_input
                cargo test -p muxy-terminal --locked --offline input
                cargo test -p muxy-terminal --locked --offline drop
                cargo test -p muxy --locked --offline composer
                cargo test -p muxy --locked --offline panels
                cargo test -p muxy --locked --offline terminal
                cargo test -p muxy --locked --offline pasteboard
                cargo test -p muxy --locked --offline dropped_paths
                source_checks
                persistence_source_checks
                component_source_checks
                panel_source_checks
                submission_source_checks
                image_source_checks
                drop_source_checks
                documentation_source_checks
                ;;
            *) fail "unknown P7 fixture: $2" ;;
        esac
        printf 'P7 Composer fixture passed: %s\n' "$2"
        ;;
    *)
        fail "usage: scripts/verify-p7-composer.sh --self-test | --fixture CASE | --staged PROFILE ABSOLUTE_APP CASE | --manual PROFILE ABSOLUTE_APP"
        ;;
esac
