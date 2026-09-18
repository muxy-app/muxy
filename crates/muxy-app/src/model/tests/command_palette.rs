use super::*;

#[gpui::test]
fn command_palette_clicks_execute_once_and_outside_click_dismisses(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("new tab");
    cx.run_until_parked();
    let bounds = cx.debug_bounds("command-palette").expect("palette");
    cx.simulate_click(
        bounds.origin + gpui::point(px(60.0), px(48.0)),
        Modifiers::default(),
    );
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.current_project().tabs.len(), 1);
    });
    cx.simulate_keystrokes("cmd-shift-p");
    cx.run_until_parked();
    cx.simulate_click(gpui::point(px(900.0), px(500.0)), Modifiers::default());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let model = view.read(cx);
        assert!(model.overlay.is_none());
        let pane = model
            .terminal(&model.active_pane().expect("active pane"))
            .expect("terminal");
        assert!(pane.view.read(cx).focus.is_focused(window));
    });
}

#[gpui::test]
fn command_palette_executes_workspace_actions_and_restores_terminal_focus(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("new tab");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.current_project().tabs.len(), 1);
    });
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("split right");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.current_project().tabs[0].panes.len(), 2);
    });
    cx.simulate_keystrokes("cmd-shift-p escape");
    cx.run_until_parked();
    cx.update(|window, cx| {
        let model = view.read(cx);
        let pane = model
            .terminal(&model.active_pane().expect("active pane"))
            .expect("terminal");
        assert!(pane.view.read(cx).focus.is_focused(window));
    });
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("change theme");
    let commands = cx.debug_bounds("command-palette").expect("command palette");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::Commands { .. })));
    });
    let themes = cx.debug_bounds("command-palette").expect("theme palette");
    assert_eq!(themes.origin, commands.origin);
    assert_eq!(themes.size.width, commands.size.width);
    assert!(cx.debug_bounds("picker-back").is_some());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.debug_bounds("picker-back").is_none());
    assert!(cx.debug_bounds("command-palette").is_some());
    cx.simulate_keystrokes("escape");
    cx.update(|window, cx| view.update(cx, |model, cx| model.open_theme_picker(window, cx)));
    cx.run_until_parked();
    assert_eq!(cx.debug_bounds("command-palette"), Some(themes));
    assert!(cx.debug_bounds("picker-back").is_none());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    cx.simulate_keystrokes("cmd-shift-k");
    cx.run_until_parked();
    assert!(cx.debug_bounds("picker-back").is_none());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    cx.simulate_keystrokes("cmd-shift-k");
    let first = view.read_with(cx, |model, _| {
        model
            .themes
            .entries
            .iter()
            .min_by_key(|entry| (entry.name.to_lowercase(), entry.name.clone()))
            .expect("available themes")
            .name
            .clone()
    });
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(
            model.themes.active_name(&model.appearance, model.dark),
            first
        );
    });
}

#[gpui::test]
fn theme_choices_keep_the_palette_selection_scroll_and_focus(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    for nested in [false, true] {
        if nested {
            cx.simulate_keystrokes("cmd-shift-p");
            cx.simulate_input("change theme");
            cx.simulate_keystrokes("enter");
        } else {
            cx.simulate_keystrokes("cmd-shift-k");
        }
        cx.run_until_parked();
        let mut names = view.read_with(cx, |model, _| {
            model
                .themes
                .entries
                .iter()
                .map(|entry| entry.name.clone())
                .collect::<Vec<_>>()
        });
        names.sort_by_cached_key(|name| (name.to_lowercase(), name.clone()));
        for _ in 0..12 {
            cx.simulate_keystrokes("down");
        }
        cx.run_until_parked();
        let selector = format!("picker-row-{}", names[12]).leak();
        let before = cx.debug_bounds(selector).expect("scrolled theme row");
        cx.simulate_click(before.center(), Modifiers::default());
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds(selector), Some(before));
        view.read_with(cx, |model, _| {
            assert!(matches!(model.overlay, Some(Overlay::Commands { .. })));
            assert_eq!(
                model.themes.active_name(&model.appearance, model.dark),
                names[12]
            );
        });
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert!(model.overlay.is_some());
            assert_eq!(
                model.themes.active_name(&model.appearance, model.dark),
                names[13]
            );
        });
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        view.read_with(cx, |model, _| assert_eq!(model.overlay.is_some(), nested));
        if nested {
            cx.simulate_keystrokes("escape");
        }
    }
}

#[gpui::test]
fn theme_shortcut_replaces_other_command_pages_and_keeps_direct_escape(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for nested in [false, true] {
        cx.simulate_keystrokes("cmd-shift-p");
        if nested {
            cx.simulate_input("switch project");
            cx.simulate_keystrokes("enter");
        }
        cx.simulate_keystrokes("cmd-shift-k");
        cx.run_until_parked();
        view.read_with(cx, |model, cx| {
            let Some(Overlay::Commands { palette, .. }) = &model.overlay else {
                panic!("theme palette")
            };
            assert!(
                palette
                    .read(cx)
                    .is_page(crate::views::theme_picker::PAGE_ID)
            );
        });
        assert!(cx.debug_bounds("picker-back").is_none());
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    }
    cx.simulate_keystrokes("cmd-shift-k cmd-shift-p");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        let Some(Overlay::Commands { palette, .. }) = &model.overlay else {
            panic!("command palette")
        };
        assert!(
            !palette
                .read(cx)
                .is_page(crate::views::theme_picker::PAGE_ID)
        );
    });
}

#[gpui::test]
fn theme_save_failure_is_visible_and_keeps_the_previous_theme(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let path = boot.state_path.with_file_name("settings.toml");
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let before = view.read_with(cx, |model, _| model.appearance.clone());
    if path.exists() {
        std::fs::remove_file(&path).expect("settings file");
    }
    std::fs::create_dir(&path).expect("block settings file");
    cx.simulate_keystrokes("cmd-shift-k");
    cx.simulate_input("Dracula");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert_eq!(model.appearance, before);
        assert!(model.settings_window.is_none());
        assert!(
            model
                .error
                .as_ref()
                .is_some_and(|error| error.contains("Could not save theme"))
        );
        let Some(Overlay::Commands { palette, .. }) = &model.overlay else {
            panic!("theme palette stays open")
        };
        assert_eq!(
            palette.read(cx).current_id(),
            Some(model.themes.active_name(&before, model.dark).as_str())
        );
    });
    std::fs::remove_dir(&path).expect("unblock settings file");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, cx| {
        assert_eq!(
            model.themes.active_name(&model.appearance, model.dark),
            "Dracula"
        );
        let Some(Overlay::Commands { palette, .. }) = &model.overlay else {
            panic!("theme palette stays open")
        };
        assert_eq!(palette.read(cx).current_id(), Some("Dracula"));
    });
}

#[gpui::test]
fn command_palette_navigates_projects_and_opens_settings_with_a_remapped_shortcut(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().expect("project directory");
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(directory.path().to_path_buf())
        .expect("project");
    state
        .rename_project(project, "Palette Project")
        .expect("name");
    state.select_project(state.home().id).expect("home");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.keymap = boot
        .settings
        .keymap
        .with_binding(
            "toggle_command_palette",
            Some("cmd-shift-j".parse().expect("chord")),
        )
        .expect("keymap");
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_keystrokes("cmd-shift-p");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    cx.simulate_keystrokes("cmd-shift-j");
    cx.simulate_input("switch project");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(cx.debug_bounds("picker-back").is_some());
    cx.simulate_input("palette project");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.current_project().id, project);
        assert!(model.state.current_project().tabs.is_empty());
    });
    cx.simulate_keystrokes("cmd-shift-j cmd-shift-j");
    cx.run_until_parked();
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    cx.simulate_keystrokes("cmd-shift-j");
    cx.simulate_input("open settings");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert!(model.settings_window.is_some());
        assert_eq!(model.state.current_project().id, project);
    });
}
