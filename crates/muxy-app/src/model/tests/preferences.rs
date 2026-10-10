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

#[gpui::test]
fn terminal_reset_asks_first_then_restores_every_default_on_the_page(cx: &mut TestAppContext) {
    use muxy_app_core::settings::{NewPaneDirectory, TerminalSettings};
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let (boot, _) = stub_boot(state);
    let path = boot.state_path.with_file_name("terminal.toml");
    let app_settings = boot.state_path.with_file_name("settings.toml");
    let (model, settings) = settings_window(boot, cx);
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
        let pane = model.grids.values().next().expect("pane").view.read(cx);
        assert_eq!(pane.terminal, model.terminal);
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
    let (model, cx) = settings_window(boot, cx);
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
    let (model, settings) = settings_window(boot, cx);
    let main = model.read_with(settings, |model, _| model.window);
    let focus_main = |settings: &mut VisualTestContext| {
        settings.update(|_, cx| {
            let _ = main.update(cx, |_, window, _| window.activate_window());
        });
        settings.run_until_parked();
    };
    let focus_settings = |settings: &mut VisualTestContext| {
        settings.update(|window, _| window.activate_window());
        settings.run_until_parked();
    };

    focus_main(settings);
    let mut edited = TerminalSettings::load_native(&path).expect("stored");
    edited.set_preference("font-size", "19").expect("size");
    edited
        .set_preference("background", "#102030")
        .expect("color");
    edited.save_native(&path).expect("hand edit");
    focus_settings(settings);
    model.read_with(settings, |model, cx| {
        assert_eq!(model.terminal, edited);
        let pane = model.grids.values().next().expect("pane").view.read(cx);
        assert_eq!(pane.terminal.font_size, 19.0);
        assert_eq!(model.palette.background, 0x10_2030);
    });

    focus_main(settings);
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
