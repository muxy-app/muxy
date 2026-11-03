#![allow(clippy::float_cmp)]

use super::*;
use crate::model::banners::BannerKind;
use crate::model::preferences::Change;

fn settings_view(model: &AppModel) -> Entity<crate::views::settings::SettingsView> {
    model
        .settings_tab
        .as_ref()
        .expect("settings tab")
        .view
        .clone()
}

pub(super) fn settings_tab(
    boot: Boot,
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext) {
    let config = boot.state_path.with_file_name("terminal.toml");
    if !config.exists() {
        muxy_app_core::settings::TerminalSettings::load_native(&config)
            .expect("isolated terminal configuration");
    }
    let (model, main) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    main.update(|window, cx| model.update(cx, |model, cx| model.open_settings(window, cx)));
    main.run_until_parked();
    (model, main)
}

pub(super) fn click_preference(cx: &mut VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let position = cx.debug_bounds(selector).expect(selector).center();
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

fn select_terminal(model: &Entity<AppModel>, cx: &mut VisualTestContext) {
    model.update(cx, |model, cx| {
        let tab = model
            .state
            .current_project()
            .tabs
            .iter()
            .find(|tab| {
                tab.panes
                    .iter()
                    .any(|pane| matches!(pane.content, PaneContent::Terminal { .. }))
            })
            .expect("terminal tab")
            .id;
        model.select_tab(tab, cx);
    });
    cx.run_until_parked();
}

#[gpui::test]
fn settings_reuses_its_tab_without_opening_windows_or_stealing_focus(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("terminal");
    let (boot, _) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (model, cx) = settings_tab(boot, cx);
    let original = model.read_with(cx, |model, _| {
        assert_eq!(model.state.home().tabs.len(), 2);
        (model.active_tab().unwrap(), settings_view(model))
    });
    assert_eq!(cx.update(|_, cx| cx.windows().len()), 1);
    click_preference(cx, "settings-field-width");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-a 1 2 3 4"
    } else {
        "ctrl-a 1 2 3 4"
    });
    click_preference(cx, "tab-cell-0");
    model.read_with(cx, |model, cx| {
        assert_ne!(model.active_tab(), Some(original.0));
        assert_eq!(model.settings.window.default_size[0], 1234.0);
        assert_eq!(settings_view(model), original.1);
        assert_eq!(model.grids.len(), 1);
        assert!(
            model
                .state
                .is_settings_pane(model.state.home().tabs[1].panes[0].id)
        );
        assert!(
            model
                .settings_tab
                .as_ref()
                .unwrap()
                .root
                .read(cx)
                .overlay
                .is_none()
        );
    });
    cx.update(|window, cx| assert!(!original.1.read(cx).focus.contains_focused(window, cx)));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-,"
    } else {
        "ctrl-,"
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("settings-view").is_some());
    click_preference(cx, "settings-button");
    model.read_with(cx, |model, _| {
        assert_eq!(model.active_tab(), Some(original.0));
        assert_eq!(model.state.home().tabs.len(), 2);
        assert_eq!(settings_view(model), original.1);
    });
    assert_eq!(cx.update(|_, cx| cx.windows().len()), 1);
    cx.dispatch_action(crate::views::workspace::CloseTab);
    cx.run_until_parked();
    model.read_with(cx, |model, _| {
        assert!(model.settings_tab.is_none());
        assert_eq!(model.state.home().tabs.len(), 1);
        assert_eq!(model.grids.len(), 1);
    });
}

#[gpui::test]
fn quick_terminal_settings_and_pickers_use_the_workspace_tab(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("terminal");
    let (boot, _) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (model, cx) = settings_tab(boot, cx);
    model.update(cx, AppModel::open_quick_terminal_settings);
    cx.run_until_parked();
    assert!(cx.debug_bounds("settings-quick-record").is_some());
    click_preference(cx, "settings-search");
    cx.simulate_input("font family");
    click_preference(cx, "settings-picker-font-family");
    assert!(cx.debug_bounds("settings-dropdown").is_some());
    model.read_with(cx, |model, cx| {
        assert!(
            model
                .settings_tab
                .as_ref()
                .unwrap()
                .root
                .read(cx)
                .overlay
                .is_some()
        );
    });
    select_terminal(&model, cx);
    model.read_with(cx, |model, cx| {
        assert!(
            model
                .settings_tab
                .as_ref()
                .unwrap()
                .root
                .read(cx)
                .overlay
                .is_none()
        );
    });
    cx.update(|window, cx| model.update(cx, |model, cx| model.show_browser_settings(window, cx)));
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| cx.windows().len()), 1);
    model.read_with(cx, |model, _| {
        assert_eq!(model.state.home().tabs.len(), 2);
        assert_eq!(
            model.active_tab(),
            model.state.settings_tab().map(|(_, tab)| tab)
        );
    });
}

#[gpui::test]
fn restored_settings_tab_renders_and_accepts_shortcuts(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let tab = state
        .show_settings_tab(state.home().id, "Settings")
        .expect("settings");
    let state = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    let (boot, _) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("settings-view").is_some());
    model.read_with(cx, |model, _| assert_eq!(model.active_tab(), Some(tab)));
    click_preference(cx, "settings-field-width");
    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-f"
    } else {
        "ctrl-shift-f"
    });
    cx.simulate_input("font family");
    cx.run_until_parked();
    assert!(cx.debug_bounds("settings-picker-font-family").is_some());
}

fn restored_settings_tab_with_keys(
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext) {
    let mut state = AppState::bootstrap().expect("state");
    state
        .show_settings_tab(state.home().id, "Settings")
        .expect("settings");
    let state = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    let (boot, _) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    (model, cx)
}

fn settings_holds_focus(model: &Entity<AppModel>, cx: &mut VisualTestContext) -> bool {
    let view = model.read_with(cx, |model, _| settings_view(model));
    cx.update(|window, cx| view.read(cx).focus.contains_focused(window, cx))
}

#[gpui::test]
fn tab_stays_inside_settings_in_both_directions(cx: &mut TestAppContext) {
    let (model, cx) = restored_settings_tab_with_keys(cx);
    assert!(cx.debug_bounds("settings-view").is_some());
    let mut stops = Vec::new();
    for key in ["tab", "shift-tab"] {
        for press in 0..40 {
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            assert!(
                settings_holds_focus(&model, cx),
                "{key} {press} left Settings"
            );
            stops.push(cx.update(|window, cx| window.focused(cx)));
        }
    }
    assert!(
        stops.iter().any(|stop| *stop != stops[0]),
        "Tab should move between controls"
    );
}

#[gpui::test]
fn settings_takes_the_keyboard_back_when_the_focused_control_goes_away(cx: &mut TestAppContext) {
    let (model, cx) = restored_settings_tab_with_keys(cx);
    click_preference(cx, "settings-field-width");
    let view = model.read_with(cx, |model, _| settings_view(model));
    view.update(cx, crate::views::settings::SettingsView::show_browser);
    cx.run_until_parked();
    assert!(settings_holds_focus(&model, cx));
}

/// Presses the empty top bar. Says whether the window move reached the
/// workspace: an action no window handler takes falls through to the app.
fn press_top_bar_reaches_window_move(cx: &mut VisualTestContext) -> bool {
    let unhandled = Rc::new(std::cell::Cell::new(false));
    let flag = unhandled.clone();
    cx.update(|_, cx| {
        cx.on_action(move |_: &crate::views::titlebar::BeginWindowMove, _| flag.set(true));
    });
    let strip = cx.debug_bounds("tab-strip").expect("tab strip");
    let position = gpui::point(
        strip.origin.x + strip.size.width - px(700.0),
        strip.origin.y + px(16.0),
    );
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
    !unhandled.get()
}

#[gpui::test]
fn the_top_bar_moves_the_window_while_settings_is_selected(cx: &mut TestAppContext) {
    let (model, cx) = restored_settings_tab_with_keys(cx);
    assert!(press_top_bar_reaches_window_move(cx));
    assert!(settings_holds_focus(&model, cx));

    click_preference(cx, "settings-field-width");
    assert!(press_top_bar_reaches_window_move(cx));
    assert!(settings_holds_focus(&model, cx));
}

#[gpui::test]
fn clicking_from_one_settings_field_to_another_keeps_the_keyboard(cx: &mut TestAppContext) {
    let (_, cx) = restored_settings_tab_with_keys(cx);
    click_preference(cx, "settings-field-width");
    let width = cx.update(|window, cx| window.focused(cx)).expect("width");
    click_preference(cx, "settings-field-height");
    let height = cx.update(|window, cx| window.focused(cx)).expect("height");
    assert_ne!(width, height);
}

#[gpui::test]
fn settings_fields_save_on_blur_tab_close_and_quit(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, settings) = settings_tab(boot, cx);
    click_preference(settings, "settings-field-width");
    settings.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-a 1 2 0 0"
    } else {
        "ctrl-a 1 2 0 0"
    });
    click_preference(settings, "settings-field-height");
    view.read_with(settings, |model, _| {
        assert_eq!(model.settings.window.default_size[0], 1200.0);
    });
    settings.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-a 8 0 0"
    } else {
        "ctrl-a 8 0 0"
    });
    settings.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-w"
    } else {
        "ctrl-shift-w"
    });
    view.read_with(settings, |model, _| {
        assert!(model.settings_tab.is_none());
        assert_eq!(model.settings.window.default_size[1], 800.0);
    });
    settings.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-,"
    } else {
        "ctrl-,"
    });
    click_preference(settings, "settings-field-width");
    settings.simulate_keystrokes(if cfg!(target_os = "macos") {
        "cmd-a 1 3 0 0"
    } else {
        "ctrl-a 1 3 0 0"
    });
    view.update(settings, AppModel::quit);
    view.read_with(settings, |model, _| {
        assert_eq!(model.settings.window.default_size[0], 1300.0);
    });
}

#[gpui::test]
fn terminal_preferences_apply_to_live_panes_preserve_zoom_and_reject_invalid_edits(
    cx: &mut TestAppContext,
) {
    use muxy_app_core::settings::{CellHeight, TerminalSettings};

    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _) = stub_boot(state);
    let path = boot.state_path.with_file_name("terminal.toml");
    let (model, cx) = settings_tab(boot, cx);
    select_terminal(&model, cx);
    model.update(cx, |model, cx| {
        assert_eq!(model.grids.len(), 1);
        let pane = model.grids.values().next().expect("pane").view.clone();
        pane.update(cx, |pane, _| pane.terminal.font_size = 20.0);
        model.change_preference(Change::Field("background-opacity", "65".into()), cx);
        model.change_preference(Change::Field("adjust-cursor-thickness", "2".into()), cx);
        model.change_preference(Change::Terminal("cursor-style", "bar".into()), cx);
        model.change_preference(Change::Terminal("copy-on-select", "true".into()), cx);
        assert_eq!(pane.read(cx).terminal.font_size, 20.0);
        assert_eq!(
            pane.read(cx).terminal.options.background_opacity,
            Some(0.65)
        );
        assert_eq!(
            pane.read(cx).terminal.options.cursor_thickness,
            CellHeight::Pixels(2)
        );
        assert_eq!(pane.read(cx).terminal.options.copy_on_select, Some(true));
        assert_eq!(
            model.palette.cursor_style,
            Some(muxy_protocol::CursorShape::Bar)
        );
        let saved = TerminalSettings::load_native(&path).expect("saved preferences");
        assert_eq!(saved, model.terminal);
        model.change_preference(Change::Field("background-opacity", "invalid".into()), cx);
        assert!(
            settings_view(model)
                .read(cx)
                .errors
                .contains_key("background-opacity")
        );
        assert_eq!(
            TerminalSettings::load_native(&path).expect("unchanged preferences"),
            saved
        );
        assert_eq!(pane.read(cx).terminal.options, saved.options);
        model.reload_configuration(cx);
        assert_eq!(pane.read(cx).terminal.font_size, 20.0);
        assert_eq!(model.terminal, saved);
    });
}

#[gpui::test]
fn invalid_queued_server_fields_do_not_stall_later_changes_or_allow_early_connect(
    cx: &mut TestAppContext,
) {
    let state = AppState::bootstrap().expect("state");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = settings_tab(boot, cx);
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        let settings = muxy_protocol::ServerSettingsDoc { default_shell: None, history_budget_bytes: 1024 * 1024, shell_integration: true };
        model.server_preferences.document = Some(settings.clone());
        model.server_preferences.busy = true;
        model.change_preference(Change::Field("history-budget", "invalid".into()), cx);
        model.change_preference(Change::ShellIntegration(false), cx);
        model.receive_server_settings(Ok(settings), cx);
        assert!(model.server_preferences.busy);
        assert!(model.server_preferences.pending.is_empty());
        assert!(settings_view(model).read(cx).errors.contains_key("history-budget"));
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::WriteServerSettings(settings) if !settings.shell_integration)));
        model.server_preferences.control_busy = true;
        model.disconnect(ServerId::local(), cx);
        model.connect(cx);
        assert_eq!(model.servers.local.generation, 1);
        assert!(requests.try_iter().all(|(_, work)| !matches!(work, Work::Connect)));
    });
}

#[gpui::test]
fn terminal_slider_previews_without_writing_and_saves_when_settings_close(cx: &mut TestAppContext) {
    use muxy_app_core::settings::TerminalSettings;
    let (boot, _) = stub_boot(AppState::bootstrap().expect("state"));
    let path = boot.state_path.with_file_name("terminal.toml");
    let (model, settings) = settings_tab(boot, cx);
    let original = TerminalSettings::load_native(&path).expect("original");
    click_preference(settings, "settings-category-Terminal");
    let position = settings
        .debug_bounds("terminal-slider-font-size")
        .expect("font slider")
        .center();
    settings.simulate_event(gpui::MouseDownEvent {
        position,
        button: gpui::MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    settings.run_until_parked();
    let preview = model.read_with(settings, |model, _| model.terminal.clone());
    assert_ne!(preview.font_size, original.font_size);
    assert_eq!(
        TerminalSettings::load_native(&path).expect("unwritten preview"),
        original
    );
    settings.dispatch_action(crate::views::settings::CloseSettings);
    settings.run_until_parked();
    model.read_with(cx, |model, _| {
        assert!(model.terminal_preview.is_none());
        assert_eq!(
            TerminalSettings::load_native(&path).expect("saved slider"),
            preview
        );
    });
}

#[gpui::test]
fn terminal_preview_preserves_concurrent_edits_and_rolls_back_failed_saves(
    cx: &mut TestAppContext,
) {
    use muxy_app_core::settings::TerminalSettings;
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _) = stub_boot(state);
    let path = boot.state_path.with_file_name("terminal.toml");
    let (model, cx) = settings_tab(boot, cx);
    select_terminal(&model, cx);
    model.update(cx, |model, cx| {
        let pane = model.grids.values().next().expect("pane").view.clone();
        pane.update(cx, |pane, _| pane.terminal.font_size = 20.0);
        model.preview_terminal_preference("background-transparency", "30", cx);
        model.preview_terminal_preference("background-transparency", "40", cx);
        TerminalSettings::edit_native(&path, |external| {
            external.set_preference("padding-right", "31")
        })
        .expect("external edit");
        model.change_preference(Change::Terminal("background-transparency", "40".into()), cx);
        assert_eq!(model.terminal.options.padding_x[1], 31.0);
        assert_eq!(model.terminal.options.background_opacity, Some(0.6));
        assert_eq!(pane.read(cx).terminal.font_size, 20.0);
        let saved = model.terminal.clone();
        model.preview_terminal_preference("font-size", "32", cx);
        model.preview_terminal_preference("font-size", &saved.font_size.to_string(), cx);
        model.change_preference(
            Change::Terminal("font-size", saved.font_size.to_string()),
            cx,
        );
        assert_eq!(pane.read(cx).terminal.font_size, 20.0);
        model.preview_terminal_preference("font-size", "32", cx);
        model.reload_configuration(cx);
        assert_eq!(pane.read(cx).terminal.font_size, 32.0);
        std::fs::write(&path, "invalid native config").expect("corrupt file");
        model.change_preference(Change::Terminal("font-size", "32".into()), cx);
        assert_eq!(model.terminal, saved);
        assert_eq!(pane.read(cx).terminal.font_size, 20.0);
        assert!(model.terminal_preview.is_none());
        assert_eq!(
            std::fs::read_to_string(&path).expect("preserved file"),
            "invalid native config"
        );
        assert!(
            settings_view(model)
                .read(cx)
                .errors
                .contains_key("font-size")
        );
    });
}

#[gpui::test]
fn terminal_reset_asks_first_then_restores_every_default_on_the_page(cx: &mut TestAppContext) {
    use muxy_app_core::settings::{NewPaneDirectory, TerminalSettings};
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _) = stub_boot(state);
    let path = boot.state_path.with_file_name("terminal.toml");
    let app_settings = boot.state_path.with_file_name("settings.toml");
    let (model, settings) = settings_tab(boot, cx);
    model.update(settings, |model, cx| {
        model.change_preference(Change::Terminal("scroll-discrete", "4.5".into()), cx);
        model.change_preference(Change::Terminal("cursor-style", "bar".into()), cx);
        model.change_preference(Change::Field("background", "not a color".into()), cx);
        model.change_preference(Change::Directory(NewPaneDirectory::Current), cx);
        assert!(
            settings_view(model)
                .read(cx)
                .errors
                .contains_key("background")
        );
    });
    click_preference(settings, "settings-search");
    settings.simulate_input("reset terminal settings");
    click_preference(settings, "settings-button-terminal-reset");
    assert!(settings.has_pending_prompt());
    settings.simulate_prompt_answer("Cancel");
    settings.run_until_parked();
    model.read_with(settings, |model, _| {
        assert_eq!(model.terminal.options.scroll_discrete, 4.5);
    });
    click_preference(settings, "settings-button-terminal-reset");
    settings.simulate_prompt_answer("Reset");
    settings.run_until_parked();
    model.read_with(settings, |model, cx| {
        assert_eq!(model.terminal, TerminalSettings::default());
        assert_eq!(
            TerminalSettings::load_native(&path).expect("saved defaults"),
            model.terminal
        );
        assert_eq!(
            model.settings.panes.new_pane_directory,
            NewPaneDirectory::Project
        );
        assert!(model.palette.cursor_style.is_none());
        assert!(settings_view(model).read(cx).errors.is_empty());
    });
    assert_eq!(
        muxy_app_core::settings::Settings::load(&app_settings)
            .expect("saved app settings")
            .panes
            .new_pane_directory,
        NewPaneDirectory::Project
    );
    click_preference(settings, "settings-button-terminal-reset");
    assert!(!settings.has_pending_prompt());
    select_terminal(&model, settings);
    model.read_with(settings, |model, cx| {
        let pane = model.grids.values().next().expect("pane").view.read(cx);
        assert_eq!(pane.terminal, model.terminal);
    });
}

#[gpui::test]
fn terminal_key_bindings_apply_atomically_and_never_take_a_command_shortcut(
    cx: &mut TestAppContext,
) {
    use muxy_app_core::settings::{CustomCommand, TerminalAction, TerminalEdit, TerminalSettings};
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _) = stub_boot(state);
    let path = boot.state_path.with_file_name("terminal.toml");
    let (model, cx) = settings_tab(boot, cx);
    select_terminal(&model, cx);
    model.update(cx, |model, cx| {
        let pane = model.grids.values().next().expect("pane").view.clone();
        let command = CustomCommand::new("Build".into(), "cargo build".into());
        let id = command.shortcut_id();
        model.change_preference(Change::Command(command), cx);
        model.change_preference(
            Change::Binding(id, Some("ctrl-alt-c".parse().expect("chord"))),
            cx,
        );
        let chord = |value: &str| value.parse().expect("chord");
        let edit = |previous: Option<&str>, binding: Option<(&str, &str)>| {
            Change::TerminalEdit(TerminalEdit::Binding {
                previous: previous.map(chord),
                binding: binding.map(|(key, action)| (chord(key), action.to_owned())),
            })
        };
        model.change_preference(edit(None, Some(("ctrl-alt-x", "text:\\e[A"))), cx);
        let saved = TerminalSettings::load_native(&path).expect("saved");
        assert_eq!(saved, model.terminal);
        assert_eq!(
            pane.read(cx)
                .terminal
                .keybindings
                .action(&chord("ctrl-alt-x")),
            Some(&TerminalAction::Text(b"\x1b[A".to_vec()))
        );

        model.change_preference(
            edit(
                Some("ctrl-alt-x"),
                Some(("ctrl-alt-c", "copy_to_clipboard")),
            ),
            cx,
        );
        assert!(
            settings_view(model)
                .read(cx)
                .errors
                .contains_key("terminal-keybindings")
        );
        assert_eq!(model.terminal, saved);
        assert_eq!(TerminalSettings::load_native(&path).expect("kept"), saved);

        model.change_preference(
            edit(
                Some("ctrl-alt-x"),
                Some(("ctrl-alt-y", "paste_from_clipboard")),
            ),
            cx,
        );
        let bindings = &model.terminal.keybindings;
        assert_eq!(bindings.action(&chord("ctrl-alt-x")), None);
        assert_eq!(
            bindings.action(&chord("ctrl-alt-y")),
            Some(&TerminalAction::Paste)
        );
        model.change_preference(edit(Some("ctrl-alt-y"), None), cx);
        assert!(model.terminal.keybindings.bindings.is_empty());
        assert_eq!(
            TerminalSettings::load_native(&path).expect("saved"),
            model.terminal
        );
    });
}

#[gpui::test]
fn hand_edits_to_terminal_settings_load_when_settings_regains_focus(cx: &mut TestAppContext) {
    use muxy_app_core::settings::TerminalSettings;
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _) = stub_boot(state);
    let path = boot.state_path.with_file_name("terminal.toml");
    let (model, settings) = settings_tab(boot, cx);
    let tab = model.read_with(settings, |model, _| model.state.settings_tab().unwrap().1);
    let focus_main = |settings: &mut VisualTestContext| {
        select_terminal(&model, settings);
    };
    let focus_settings = |settings: &mut VisualTestContext| {
        model.update(settings, |model, cx| model.select_tab(tab, cx));
        settings.run_until_parked();
    };

    focus_main(settings);
    let edited = TerminalSettings::edit_native(&path, |edited| {
        edited.set_preference("font-size", "19")?;
        edited.set_preference("background", "#102030")
    })
    .expect("hand edit");
    focus_settings(settings);
    model.read_with(settings, |model, _| {
        assert_eq!(model.terminal, edited);
        assert_eq!(model.palette.background, 0x10_2030);
    });

    focus_main(settings);
    model.read_with(settings, |model, cx| {
        let pane = model.grids.values().next().expect("pane").view.read(cx);
        assert_eq!(pane.terminal.font_size, 19.0);
    });
    std::fs::write(&path, "font_size = 'broken'").expect("broken hand edit");
    focus_settings(settings);
    model.read_with(settings, |model, cx| {
        assert_eq!(model.terminal, edited);
        assert!(
            settings_view(model)
                .read(cx)
                .errors
                .contains_key("configuration")
        );
    });
}

#[gpui::test]
fn application_errors_leave_settings_when_dismissed(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (model, settings) = settings_tab(boot, cx);
    let shown = |settings: &mut VisualTestContext| {
        settings.run_until_parked();
        model.read_with(settings, |model, cx| {
            settings_view(model)
                .read(cx)
                .errors
                .get("application")
                .cloned()
        })
    };

    model.update(settings, |model, cx| {
        model.fail("couldn't commit".into(), cx);
    });
    assert_eq!(shown(settings).as_deref(), Some("couldn't commit"));
    click_preference(settings, "settings-dismiss-application-error");
    assert_eq!(shown(settings), None);
    model.update(settings, |model, cx| {
        assert!(model.visible_banner().is_none());
        model.set_configuration_error(Some("broken terminal.toml".into()));
        cx.notify();
    });
    assert_eq!(shown(settings), None);

    model.update(settings, |model, cx| {
        model.fail_detail("couldn't commit".into(), "nothing staged", cx);
    });
    assert_eq!(shown(settings).as_deref(), Some("couldn't commit"));
    model.update(settings, AppModel::dismiss_problem);
    assert_eq!(shown(settings), None);
    model.read_with(settings, |model, _| {
        assert_eq!(
            model.visible_banner(),
            Some((BannerKind::Configuration, "broken terminal.toml"))
        );
    });
}

#[gpui::test]
fn configuration_errors_in_settings_can_be_dismissed(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (model, settings) = settings_tab(boot, cx);
    let view = model.read_with(settings, |model, _| settings_view(model));
    view.update(settings, |view, cx| {
        view.set_error("configuration", Some("Could not open terminal.toml"), cx);
    });
    click_preference(settings, "settings-dismiss-configuration-error");
    view.read_with(settings, |view, _| {
        assert!(!view.errors.contains_key("configuration"));
    });
}

#[gpui::test]
fn a_zoomed_pane_stays_zoomed_through_hiding_and_a_settings_reload(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let first = state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.home().tabs[0].panes[0].id;
    let second = state.open_terminal_tab(state.home().id).expect("tab");
    state.select_tab(state.home().id, first).expect("select");
    let (boot, _) = stub_boot(state);
    let (model, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    model.update(cx, |model, cx| {
        let view = model.terminal(&pane).expect("pane").view.clone();
        view.update(cx, |pane, _| pane.terminal.font_size = 20.0);

        model.select_tab(second, cx);
        assert!(model.terminal(&pane).is_none());
        assert_eq!(model.pane_zoom(pane), Some(20.0));
        model.reload_configuration(cx);

        model.select_tab(first, cx);
        let view = model.terminal(&pane).expect("pane").view.clone();
        assert_eq!(view.read(cx).terminal.font_size, 20.0);
        model.reload_configuration(cx);
        assert_eq!(view.read(cx).terminal.font_size, 20.0);
        assert_eq!(view.read(cx).zoomed_font_size(), Some(20.0));
    });
}

fn settings_picker_outside_split(direction: Direction, cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let tab = state
        .show_settings_tab(state.home().id, "Settings")
        .expect("settings");
    let pane = state.window().active_pane.unwrap();
    state.split_pane(pane, direction).expect("split");
    state.set_ratio(tab, &[], 0.3).expect("ratio");
    state.focus_pane(pane).expect("settings focus");
    let (boot, _) = stub_boot(state);
    let (model, cx) = settings_tab(boot, cx);
    cx.simulate_resize(size(px(1000.0), px(800.0)));
    click_preference(cx, "settings-search");
    cx.simulate_input("cursor-style");
    click_preference(cx, "settings-picker-cursor-style");
    model.read_with(cx, |model, _| {
        let coverage = model.coverage();
        assert!(coverage.hides_terminal_scroll());
        assert!(coverage.hides_webviews());
        assert!(coverage.hides_spinners());
    });
    let pane = cx.debug_bounds("settings-view").expect("settings bounds");
    let row = cx
        .debug_bounds("picker-row-block_hollow")
        .expect("outline cursor");
    let point = gpui::point(row.right() - px(8.0), row.center().y);
    assert!(!pane.contains(&point), "picker extends beyond its split");
    cx.simulate_click(point, Modifiers::default());
    cx.run_until_parked();
    model.read_with(cx, |model, cx| {
        assert_eq!(
            model.terminal.options.cursor_style,
            Some(muxy_protocol::CursorShape::Hollow)
        );
        assert!(
            model
                .settings_tab
                .as_ref()
                .unwrap()
                .root
                .read(cx)
                .overlay
                .is_none()
        );
        assert!(!model.settings_picker_open());
    });
}

#[gpui::test]
fn settings_picker_is_clickable_outside_a_narrow_split(cx: &mut TestAppContext) {
    settings_picker_outside_split(Direction::Right, cx);
}

#[gpui::test]
fn settings_picker_is_clickable_outside_a_short_split(cx: &mut TestAppContext) {
    settings_picker_outside_split(Direction::Down, cx);
}

#[gpui::test]
fn the_interface_size_resizes_the_app_live_and_is_saved(cx: &mut TestAppContext) {
    use muxy_app_core::settings::InterfaceSize;
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _) = stub_boot(state);
    let path = boot.state_path.with_file_name("settings.toml");
    let (model, cx) = settings_tab(boot, cx);
    assert_eq!(
        cx.debug_bounds("tab-strip").expect("strip").size.height,
        px(32.0)
    );
    let view = model.read_with(cx, |model, _| settings_view(model));
    assert_eq!(
        view.read_with(cx, |view, _| view.scale()),
        (1.0_f32.to_bits(), 1.15_f32.to_bits())
    );

    click_preference(cx, "settings-category-Appearance");
    click_preference(cx, "settings-segment-interface-size-huge");

    model.read_with(cx, |model, _| {
        assert_eq!(model.appearance.interface_size, InterfaceSize::Huge);
        assert_eq!(model.metrics, Metrics::new(1.40));
    });
    assert_eq!(
        view.read_with(cx, |view, _| view.scale()),
        (1.40_f32.to_bits(), (1.40_f32 * 1.15).to_bits())
    );
    assert_eq!(
        cx.debug_bounds("tab-strip").expect("strip").size.height,
        px((32.0_f32 * 1.40).round())
    );
    let saved = std::fs::read_to_string(&path).expect("saved settings");
    assert!(saved.contains("interface_size = \"huge\""), "{saved}");

    click_preference(cx, "settings-segment-interface-size-regular");
    model.read_with(cx, |model, _| {
        assert_eq!(model.metrics, Metrics::new(1.0));
    });
    assert_eq!(
        cx.debug_bounds("tab-strip").expect("strip").size.height,
        px(32.0)
    );
}

#[gpui::test]
fn the_interface_size_applies_at_launch(cx: &mut TestAppContext) {
    use muxy_app_core::settings::InterfaceSize;
    let state = AppState::bootstrap().expect("state");
    let (mut boot, _) = stub_boot(state);
    boot.settings.appearance.interface_size = InterfaceSize::Large;
    let (model, cx) = settings_tab(boot, cx);
    model.read_with(cx, |model, _| {
        assert_eq!(model.metrics, Metrics::new(1.12));
    });
    cx.update(|window, _| assert_eq!(window.rem_size(), px(16.0 * 1.12)));
}

#[gpui::test]
fn the_size_row_stays_on_screen_when_the_interface_grows(cx: &mut TestAppContext) {
    let state = AppState::bootstrap().expect("state");
    let (boot, _) = stub_boot(state);
    let (model, cx) = settings_tab(boot, cx);
    click_preference(cx, "settings-category-Appearance");
    let selector = "settings-segment-interface-size-huge";
    let status_bar = px(40.0);
    let fitted = cx.debug_bounds(selector).expect("size row").bottom() + status_bar;
    cx.simulate_resize(size(px(1200.0), fitted));
    cx.run_until_parked();

    click_preference(cx, selector);
    cx.run_until_parked();

    let row = cx.debug_bounds(selector).expect("size row");
    let visible = fitted - model.read_with(cx, |model, _| model.metrics.status_bar_height());
    assert!(
        row.bottom() <= visible,
        "the Size row ends at {:?}, below the visible {visible:?}",
        row.bottom(),
    );

    cx.simulate_event(gpui::ScrollWheelEvent {
        position: row.center(),
        delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(200.0))),
        ..Default::default()
    });
    cx.run_until_parked();
    let scrolled = cx.debug_bounds(selector).expect("size row");
    assert!(
        scrolled.top() > row.top(),
        "scrolling away brought the Size row back to {:?}",
        scrolled.top(),
    );
}
