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
    local nonce="$$" root missing mismatch linked target outside
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
    rm -f -- "$linked"
    rm -rf -- "$root" "$missing" "$mismatch" "$target"
    printf 'P7 Composer verifier self-test passed\n'
}
for command_name in cargo cmp cut find git grep ln mkdir rg rm shasum sort; do
    require_command "$command_name"
done

case "${1:-}" in
    --self-test)
        (($# == 1)) || fail "usage: scripts/verify-p7-composer.sh --self-test"
        require_command plutil
        self_test
        ;;
    --staged|--manual)
        fail "app-launching E2E verification is disabled; use headless Composer fixtures and ask the user to verify visual behavior"
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
        fail "usage: scripts/verify-p7-composer.sh --self-test | --fixture CASE"
        ;;
esac
