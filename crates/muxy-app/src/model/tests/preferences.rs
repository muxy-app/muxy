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
    let config = boot.state_path.with_file_name("ghostty.conf");
    if !config.exists() {
        muxy_app_core::settings::TerminalSettings::load_with_seed(&config, None)
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
    click_preference(settings, "settings-category-Terminal");
    click_preference(settings, "settings-field-font-size");
    settings.simulate_keystrokes("cmd-a 2 1");
    view.update(settings, AppModel::quit);
    view.read_with(settings, |model, _| {
        assert_eq!(model.terminal.font_size, 21.0);
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
        let settings = muxy_protocol::ServerSettingsDoc { sandbox: None, default_shell: None, history_budget_bytes: 1024 * 1024, shell_integration: true };
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
