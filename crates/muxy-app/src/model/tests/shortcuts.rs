use super::*;
use gpui::ModifiersChangedEvent;
use muxy_app_core::settings::{AppLayout, ProjectOrder};
use muxy_core::shortcuts::ShortcutId;

fn modifiers(cx: &mut VisualTestContext, modifiers: Modifiers) {
    cx.simulate_event(ModifiersChangedEvent {
        modifiers,
        ..Default::default()
    });
    cx.run_until_parked();
}

#[gpui::test]
fn numbered_shortcuts_and_cycles_work_from_terminal_focus(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let projects: Vec<_> = std::iter::once(home)
        .chain((1..9).map(|_| {
            state
                .add_project(ServerId::local(), std::env::temp_dir())
                .expect("project")
        }))
        .collect();
    let tabs: Vec<_> = (0..9)
        .map(|_| state.open_terminal_tab(home).expect("tab"))
        .collect();
    for project in &projects[1..] {
        state.open_terminal_tab(*project).expect("project tab");
    }
    state.select_tab(home, tabs[0]).expect("home");
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    for (index, project) in projects.iter().enumerate() {
        cx.simulate_keystrokes(&format!("ctrl-{}", index + 1));
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |model, _| model.state.current_project().id),
            *project
        );
        assert!(cx.update(|window, cx| {
            let model = view.read(cx);
            model
                .terminal(&model.active_pane().expect("pane"))
                .expect("terminal")
                .view
                .read(cx)
                .focus
                .is_focused(window)
        }));
    }
    for (key, project) in [
        ("ctrl-]", home),
        ("ctrl-[", projects[8]),
        ("cmd-alt-]", home),
        ("cmd-alt-[", projects[8]),
        ("ctrl-1", home),
    ] {
        cx.simulate_keystrokes(key);
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |model, _| model.state.current_project().id),
            project
        );
    }
    for (index, tab) in tabs.iter().enumerate() {
        cx.simulate_keystrokes(&format!("cmd-{}", index + 1));
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |model, _| model.active_tab()),
            Some(*tab)
        );
    }
    for (key, tab) in [
        ("cmd-]", tabs[0]),
        ("cmd-[", tabs[8]),
        ("ctrl-tab", tabs[0]),
        ("ctrl-shift-tab", tabs[8]),
    ] {
        cx.simulate_keystrokes(key);
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |model, _| model.active_tab()), Some(tab));
    }
}

#[gpui::test]
fn project_shortcuts_follow_sort_and_workspace_filter_even_when_sidebar_is_focused(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let zulu = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("zulu");
    state.rename_project(zulu, "Zulu").expect("name");
    let alpha = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("alpha");
    state.rename_project(alpha, "Alpha").expect("name");
    state.select_project(home).expect("home");
    let workspace = state.create_workspace("Work").expect("workspace");
    state
        .set_workspace_member(workspace, zulu, true)
        .expect("member");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.sidebar_project_order = ProjectOrder::Name;
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-2");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |model, _| model.state.current_project().id),
        alpha
    );
    view.update(cx, |model, cx| {
        model.set_project_focus(true, cx);
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-]");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |model, _| model.state.current_project().id),
        zulu
    );
    view.update(cx, |model, cx| model.select_workspace(Some(workspace), cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-1 ctrl-2 ctrl-3 ctrl-9");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.navigation_projects(), [home, zulu]);
        assert_eq!(model.state.current_project().id, zulu);
        assert!(
            model
                .state
                .projects()
                .iter()
                .all(|project| project.tabs.is_empty())
        );
    });
}

#[gpui::test]
fn hold_hints_are_delayed_cancelled_and_drawn_on_project_and_tab_icons(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let tab = state.open_terminal_tab(home).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        AppModel::new(boot, window, cx)
    });
    cx.run_until_parked();
    let control = Modifiers {
        control: true,
        ..Default::default()
    };
    let command = Modifiers {
        platform: true,
        ..Default::default()
    };
    let project_hint = format!("project-shortcut-{home}").leak();
    modifiers(cx, control);
    cx.executor().advance_clock(Duration::from_millis(499));
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |model, _| model
            .shortcut_hints
            .label(ShortcutId::SelectProject1, &model.settings.keymap))
            .is_none()
    );
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert!(cx.debug_bounds(project_hint).is_some());
    assert!(
        view.read_with(cx, |model, _| model
            .shortcut_hints
            .label(ShortcutId::SelectTab1, &model.settings.keymap))
            .is_none()
    );
    view.update(cx, |model, cx| {
        model.appearance.sidebar_expanded = true;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds(project_hint).is_some());
    modifiers(cx, Modifiers::default());
    assert!(
        view.read_with(cx, |model, _| model
            .shortcut_hints
            .label(ShortcutId::SelectProject1, &model.settings.keymap))
            .is_none()
    );
    modifiers(cx, command);
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.run_until_parked();
    modifiers(cx, Modifiers::default());
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |model, _| model
            .shortcut_hints
            .label(ShortcutId::SelectTab1, &model.settings.keymap))
            .is_none()
    );
    modifiers(cx, command);
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(cx.debug_bounds("tab-shortcut-0").is_some());
    assert!(
        view.read_with(cx, |model, _| model
            .shortcut_hints
            .label(ShortcutId::SelectProject1, &model.settings.keymap))
            .is_none()
    );
    view.update(cx, |model, cx| model.set_layout(AppLayout::TabFocused, cx));
    cx.run_until_parked();
    let sidebar_hint = format!("sidebar-tab-shortcut-{tab}").leak();
    assert!(cx.debug_bounds(sidebar_hint).is_some());
    modifiers(
        cx,
        Modifiers {
            shift: true,
            ..command
        },
    );
    assert!(
        view.read_with(cx, |model, _| model
            .shortcut_hints
            .label(ShortcutId::SelectTab1, &model.settings.keymap))
            .is_none()
    );
    modifiers(cx, command);
    assert!(cx.debug_bounds(sidebar_hint).is_some());
    cx.deactivate_window();
    cx.run_until_parked();
    assert!(
        view.read_with(cx, |model, _| model
            .shortcut_hints
            .label(ShortcutId::SelectTab1, &model.settings.keymap))
            .is_none()
    );
}

#[gpui::test]
fn modifier_release_after_switching_to_webview_clears_visible_and_pending_hints(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let terminal = state.open_terminal_tab(home).expect("terminal");
    let (editor, _) = state
        .open_webview(
            home,
            muxy_app_core::webview::WebviewDescriptor {
                owner: "not-installed".into(),
                kind: "editor".into(),
                data: serde_json::Value::Null,
            },
            "Editor",
            false,
        )
        .expect("editor");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for (held, shortcut) in [
        (
            Modifiers {
                platform: true,
                ..Default::default()
            },
            ShortcutId::SelectTab1,
        ),
        (
            Modifiers {
                control: true,
                ..Default::default()
            },
            ShortcutId::SelectProject1,
        ),
    ] {
        for delay in [250, 500] {
            view.update(cx, |model, cx| model.select_tab(terminal, cx));
            cx.run_until_parked();
            modifiers(cx, held);
            cx.executor().advance_clock(Duration::from_millis(delay));
            cx.run_until_parked();
            view.update(cx, |model, cx| model.select_tab(editor, cx));
            cx.run_until_parked();
            view.read_with(cx, |model, _| {
                assert_eq!(model.active_tab(), Some(editor));
                assert_eq!(
                    model
                        .shortcut_hints
                        .label(shortcut, &model.settings.keymap)
                        .is_some(),
                    delay == 500,
                );
            });
            modifiers(cx, Modifiers::default());
            cx.executor().advance_clock(Duration::from_millis(500));
            cx.run_until_parked();
            view.read_with(cx, |model, _| {
                assert!(
                    model
                        .shortcut_hints
                        .label(shortcut, &model.settings.keymap)
                        .is_none()
                );
            });
        }
    }
}

#[gpui::test]
fn remapped_number_shortcut_replaces_default_and_tab_cycles_follow_visible_rows(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let first = state.open_terminal_tab(home).expect("first");
    let other = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("other");
    let second = state.open_terminal_tab(other).expect("second");
    state.select_tab(home, first).expect("home");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.layout = AppLayout::TabFocused;
    boot.settings
        .appearance
        .tab_focused_expanded
        .extend([(home, true), (other, true)]);
    boot.settings.keymap = boot
        .settings
        .keymap
        .with_binding("select_project2", Some("ctrl-x".parse().expect("chord")))
        .expect("keymap");
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("ctrl-2");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |model, _| model.state.current_project().id),
        home
    );
    cx.simulate_keystrokes("ctrl-x");
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |model, _| model.state.current_project().id),
        other
    );
    for (key, tab) in [
        ("cmd-]", first),
        ("cmd-[", second),
        ("ctrl-tab", first),
        ("ctrl-shift-tab", second),
        ("cmd-1", first),
        ("cmd-2", second),
    ] {
        cx.simulate_keystrokes(key);
        cx.run_until_parked();
        assert_eq!(view.read_with(cx, |model, _| model.active_tab()), Some(tab));
    }
}
