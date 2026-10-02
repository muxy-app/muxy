use super::*;
use crate::views::settings::Change;
use muxy_app_core::settings::{CustomCommand, Settings};

fn command() -> CustomCommand {
    CustomCommand {
        id: "tests".into(),
        name: "Run project tests".into(),
        command: "printf 'hello world' && echo done".into(),
    }
}

fn configured_boot(state: AppState) -> (Boot, std::sync::mpsc::Receiver<(u64, Work)>) {
    let (mut boot, requests) = stub_boot(state);
    boot.settings.commands.push(command());
    boot.settings.keymap = boot
        .settings
        .keymap
        .with_binding("command.tests", Some("cmd-alt-y".parse().unwrap()))
        .unwrap();
    (boot, requests)
}

#[gpui::test]
fn custom_command_shortcut_creates_named_tab_and_submits_once_after_attach(
    cx: &mut TestAppContext,
) {
    let project_dir = tempfile::tempdir().unwrap();
    let mut state = AppState::bootstrap().unwrap();
    let project = state.add_project(project_dir.path().to_owned()).unwrap();
    state.select_project(project).unwrap();
    let (boot, requests) = configured_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        acknowledge_catalog(model, cx);
    });
    requests.try_iter().for_each(drop);
    cx.simulate_keystrokes("cmd-alt-y");
    cx.run_until_parked();
    let pane = view.read_with(cx, |model, _| {
        assert_eq!(model.state.current_project().id, project);
        let tab = &model.state.current_project().tabs[0];
        assert_eq!(tab.title(None), "Run project tests");
        let pane = model.active_pane().unwrap();
        assert_eq!(
            model.state.startup_command(pane),
            Some(command().command.as_str())
        );
        pane
    });
    let work: Vec<_> = requests.try_iter().map(|(_, work)| work).collect();
    assert!(!work.iter().any(|work| matches!(work, Work::Input(..))));
    assert!(work.iter().any(|work| matches!(work, Work::Attach { pane: target, directory, .. } if *target == pane && directory == project_dir.path())));
    let session = SessionId::new(7).unwrap();
    view.update(cx, |model, cx| {
        model.receive_attached(pane, session, attachment(), true, cx);
        model.receive_attached(pane, session, attachment(), false, cx);
    });
    let typed: Vec<_> = requests
        .try_iter()
        .filter_map(|(_, work)| match work {
            Work::Input(_, bytes) => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(typed, [format!("{}\r", command().command).into_bytes()]);
    view.read_with(cx, |model, _| {
        assert!(model.state.startup_command(pane).is_none());
        let saved = store::load(&model.path).unwrap();
        assert!(saved.startup_command(pane).is_none());
    });
}

#[gpui::test]
fn custom_command_palette_rebinding_unassign_and_delete_take_effect_immediately(
    cx: &mut TestAppContext,
) {
    let (boot, _requests) = configured_boot(AppState::bootstrap().unwrap());
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_keystrokes("cmd-alt-y");
    view.update(cx, |model, cx| {
        assert_eq!(model.state.home().tabs.len(), 1);
        model.change_preference(
            Change::Binding("command.tests".into(), Some("cmd-alt-u".parse().unwrap())),
            cx,
        );
    });
    cx.simulate_keystrokes("cmd-alt-y");
    view.read_with(cx, |model, _| assert_eq!(model.state.home().tabs.len(), 1));
    cx.simulate_keystrokes("cmd-alt-u");
    view.read_with(cx, |model, _| assert_eq!(model.state.home().tabs.len(), 2));
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("Run project tests");
    cx.simulate_keystrokes("cmd-alt-u");
    view.read_with(cx, |model, _| assert_eq!(model.state.home().tabs.len(), 2));
    cx.simulate_keystrokes("enter");
    view.update(cx, |model, cx| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.home().tabs.len(), 3);
        model.change_preference(Change::Binding("command.tests".into(), None), cx);
    });
    cx.simulate_keystrokes("cmd-alt-u");
    view.update(cx, |model, cx| {
        assert_eq!(model.state.home().tabs.len(), 3);
        model.change_preference(Change::RemoveCommand("tests".into()), cx);
        model.run_custom_command("tests", cx);
        assert_eq!(model.state.home().tabs.len(), 3);
        assert!(model.settings.commands.is_empty());
    });
}

#[gpui::test]
fn custom_commands_settings_create_edit_record_search_and_delete(cx: &mut TestAppContext) {
    use preferences::click_preference as click;
    let (boot, _requests) = stub_boot(AppState::bootstrap().unwrap());
    let (view, cx) = preferences::settings_window(boot, cx);
    let settings = view.read_with(cx, |model, _| {
        model.settings_window.as_ref().unwrap().view.clone()
    });
    click(cx, "settings-category-Commands");
    click(cx, "settings-add-command");
    click(cx, "settings-save-command");
    settings.read_with(cx, |pane, _| assert!(pane.errors.contains_key("commands")));
    click(cx, "settings-field-command-name");
    cx.simulate_input("Run tests");
    click(cx, "settings-field-command-text");
    cx.simulate_input("cargo test");
    click(cx, "settings-save-command");
    let command = view.read_with(cx, |model, _| {
        assert_eq!(model.settings.commands.len(), 1);
        model.settings.commands[0].clone()
    });
    let id = command.shortcut_id();
    for keys in ["y", "cmd-t", "cmd-alt-y"] {
        cx.update(|window, cx| {
            settings.update(cx, |pane, cx| pane.begin_recording(&id, window, cx));
        });
        cx.simulate_keystrokes(keys);
    }
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.settings.keymap.binding(&id).unwrap().as_str(),
            "cmd-alt-y"
        );
        assert!(model.state.home().tabs.is_empty());
    });
    click_dynamic(cx, &format!("settings-edit-{id}"));
    click(cx, "settings-field-command-text");
    cx.simulate_keystrokes("cmd-a");
    cx.simulate_input("cargo test --all-features");
    click(cx, "settings-save-command");
    view.read_with(cx, |model, _| {
        let saved = Settings::load(&model.path.with_file_name("settings.toml")).unwrap();
        assert_eq!(saved.commands[0].command, "cargo test --all-features");
        assert_eq!(saved.commands[0].id, command.id);
        assert_eq!(saved.keymap.binding(&id).unwrap().as_str(), "cmd-alt-y");
    });
    click(cx, "settings-search");
    cx.simulate_input("all-features");
    cx.run_until_parked();
    assert!(
        cx.debug_bounds(format!("settings-row-{id}").leak())
            .is_some()
    );
    click_dynamic(cx, &format!("settings-delete-{id}"));
    view.read_with(cx, |model, _| {
        assert!(model.settings.commands.is_empty());
        let saved = Settings::load(&model.path.with_file_name("settings.toml")).unwrap();
        assert!(saved.commands.is_empty());
        assert!(saved.keymap.binding(&id).is_none());
    });
}

#[gpui::test]
fn custom_commands_settings_keep_descriptions_with_names_and_actions_compact(
    cx: &mut TestAppContext,
) {
    let (mut boot, _requests) = configured_boot(AppState::bootstrap().unwrap());
    boot.settings.commands[0].name = "list".into();
    boot.settings.commands[0].command = "ls -la".into();
    let (_, cx) = preferences::settings_window(boot, cx);
    preferences::click_preference(cx, "settings-category-Commands");
    for width in [1200.0, 740.0, 600.0] {
        cx.simulate_resize(size(px(width), px(1000.0)));
        cx.run_until_parked();
        let row = cx.debug_bounds("settings-row-command.tests").unwrap();
        let actions = cx
            .debug_bounds("settings-command-actions-command.tests")
            .unwrap();
        let add = cx.debug_bounds("settings-add-command").unwrap();
        assert_eq!(add.left(), row.left());
        assert!(add.size.width < row.size.width / 2.0);
        assert!(actions.left() >= row.left());
        assert!(actions.right() <= row.right());
        if width >= 660.0 {
            assert!((actions.center().y - row.center().y).abs() < px(1.0));
        } else {
            assert_eq!(actions.left(), row.left());
        }
    }
}

fn click_dynamic(cx: &mut VisualTestContext, selector: &str) {
    cx.run_until_parked();
    let position = cx
        .debug_bounds(selector.to_owned().leak())
        .expect(selector)
        .center();
    cx.simulate_click(position, Modifiers::default());
    cx.run_until_parked();
}

#[gpui::test]
fn custom_commands_failed_saves_do_not_change_live_settings_or_launch_tabs(
    cx: &mut TestAppContext,
) {
    let (boot, _requests) = configured_boot(AppState::bootstrap().unwrap());
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let blocker = tempfile::NamedTempFile::new().unwrap();
    view.update(cx, |model, cx| {
        model.path = blocker.path().join("state.json");
        let before = model.settings.clone();
        model.change_preference(Change::RemoveCommand("tests".into()), cx);
        assert_eq!(model.settings, before);
        model.run_custom_command("tests", cx);
        assert!(model.state.home().tabs.is_empty());
    });
}

#[gpui::test]
fn custom_commands_respect_extension_terminal_and_quick_terminal_shortcuts(
    cx: &mut TestAppContext,
) {
    use muxy_core::quick_terminal::{
        QuickTerminalShortcut,
        keys::{COMMAND, KeyCombo, OPTION},
    };
    let (boot, _requests) = configured_boot(AppState::bootstrap().unwrap());
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let package = tempfile::tempdir().unwrap();
    std::fs::write(package.path().join("package.json"), r#"{"name":"launcher","version":"1.0.0","muxy":{"commands":[{"id":"open","title":"Open","defaultShortcut":"cmd+alt+u"}]}}"#).unwrap();
    for task in [
        view.update(cx, |model, cx| {
            model.load_unpacked_extension(package.path().to_owned(), cx)
        }),
        view.update(cx, |model, cx| {
            model.set_extension_enabled("launcher", true, cx)
        }),
    ] {
        extensions::finish_extension(task, cx).unwrap();
    }
    view.update(cx, |model, cx| {
        let before = model.settings.keymap.clone();
        model.change_preference(
            Change::Binding("command.tests".into(), Some("cmd-alt-u".parse().unwrap())),
            cx,
        );
        assert_eq!(model.settings.keymap, before);
        model.change_preference(
            Change::Binding(
                "extension.launcher.open".into(),
                Some("cmd-alt-y".parse().unwrap()),
            ),
            cx,
        );
        assert_eq!(model.settings.keymap, before);
        model.terminal.keybindings.bindings.insert(
            "cmd-alt-i".parse().unwrap(),
            muxy_app_core::settings::TerminalAction::Ignore,
        );
        model.change_preference(
            Change::Binding("command.tests".into(), Some("cmd-alt-i".parse().unwrap())),
            cx,
        );
        assert_eq!(model.settings.keymap, before);
        let mut quick = model.settings.quick_terminal.clone();
        quick.shortcut = QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("y", COMMAND | OPTION),
            virtual_key_code: 16,
        };
        assert!(model.apply_quick_settings(quick, cx).is_err());
    });
}

#[gpui::test]
fn custom_commands_do_not_forward_native_webview_editing_keys(cx: &mut TestAppContext) {
    let (boot, _requests) = configured_boot(AppState::bootstrap().unwrap());
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for key in ["ctrl-a", "alt-b", "cmd-alt-y"] {
        view.update(cx, |model, cx| {
            model.change_preference(
                Change::Binding("command.tests".into(), Some(key.parse().unwrap())),
                cx,
            );
            assert_eq!(
                model
                    .settings
                    .keymap
                    .binding("command.tests")
                    .unwrap()
                    .as_str(),
                key
            );
            assert!(
                !model
                    .webview_shortcuts()
                    .iter()
                    .any(|shortcut| shortcut == &Keystroke::parse(key).unwrap())
            );
        });
    }
}

#[gpui::test]
fn custom_commands_reject_conflicting_terminal_reload_and_preference_saves(
    cx: &mut TestAppContext,
) {
    let (boot, _requests) = configured_boot(AppState::bootstrap().unwrap());
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for action in ["ignore", "text:hello", "unbind"] {
        view.update(cx, |model, cx| {
            let path = model.configuration_path("ghostty.conf");
            let before = model.terminal.clone();
            let config = format!("font-size = 22\nkeybind = cmd+alt+y={action}\n");
            std::fs::write(&path, &config).unwrap();
            model.reload_configuration(cx);
            assert_eq!(model.terminal, before);
            let error = model.configuration_error.as_ref().unwrap();
            assert!(error.contains("cmd-alt-y"), "{error}");
            assert!(error.contains("Run project tests"), "{error}");
            assert!(error.contains("ghostty.conf"), "{error}");
            model.change_preference(Change::Field("font-size", "24".into()), cx);
            assert_eq!(model.terminal, before);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), config);
        });
    }
    cx.simulate_keystrokes("cmd-alt-y");
    view.update(cx, |model, cx| {
        assert_eq!(model.state.home().tabs.len(), 1);
        let path = model.configuration_path("ghostty.conf");
        std::fs::write(&path, "keybind = cmd+alt+u=ignore\n").unwrap();
        model.reload_configuration(cx);
        assert!(model.configuration_error.is_none());
        assert!(
            model
                .terminal
                .keybindings
                .bindings
                .contains_key(&"cmd-alt-u".parse().unwrap())
        );
    });
}
