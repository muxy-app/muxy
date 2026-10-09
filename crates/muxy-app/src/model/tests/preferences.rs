#![allow(clippy::float_cmp)]

mod mobile;

use super::*;
use crate::views::settings::Change;

fn settings_view(model: &AppModel) -> Entity<crate::views::settings::SettingsView> {
    model
        .settings_window
        .as_ref()
        .expect("settings window")
        .view
        .clone()
}

pub(super) fn settings_window(
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
    let handle = model.read_with(main, |model, _| {
        model.settings_window.as_ref().expect("window").window
    });
    let settings = VisualTestContext::from_window(handle.into(), main).into_mut();
    settings.update(|window, _| window.activate_window());
    settings.run_until_parked();
    (model, settings)
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

#[gpui::test]
fn settings_fields_save_on_blur_window_close_and_quit(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, settings) = settings_window(boot, cx);
    click_preference(settings, "settings-field-width");
    settings.simulate_keystrokes("cmd-a 1 2 0 0");
    click_preference(settings, "settings-field-height");
    view.read_with(settings, |model, _| {
        assert_eq!(model.settings.window.default_size[0], 1200.0);
    });
    settings.simulate_keystrokes("cmd-a 8 0 0");
    let main = view.read_with(settings, |model, _| model.window);
    assert!(settings.simulate_close());
    view.read_with(cx, |model, _| {
        assert!(model.settings_window.is_none());
        assert_eq!(model.settings.window.default_size[1], 800.0);
    });
    let main = VisualTestContext::from_window(main, cx).into_mut();
    main.simulate_keystrokes("cmd-,");
    let handle = view.read_with(main, |model, _| {
        model.settings_window.as_ref().expect("settings").window
    });
    let settings = VisualTestContext::from_window(handle.into(), main).into_mut();
    click_preference(settings, "settings-field-width");
    settings.simulate_keystrokes("cmd-a 1 3 0 0");
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
    let (model, cx) = settings_window(boot, cx);
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
    let (view, cx) = settings_window(boot, cx);
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
    let (model, settings) = settings_window(boot, cx);
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
    assert!(settings.simulate_close());
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
    let (model, cx) = settings_window(boot, cx);
    model.update(cx, |model, cx| {
        let pane = model.grids.values().next().expect("pane").view.clone();
        pane.update(cx, |pane, _| pane.terminal.font_size = 20.0);
        model.preview_terminal_preference("background-transparency", "30", cx);
        model.preview_terminal_preference("background-transparency", "40", cx);
        let mut external = TerminalSettings::load_native(&path).expect("stored");
        external
            .set_preference("padding-right", "31")
            .expect("padding");
        external.save_native(&path).expect("external edit");
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
