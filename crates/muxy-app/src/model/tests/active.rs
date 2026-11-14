use super::*;
use crate::views::command_palette::Palette;
use crate::views::workspace::{
    CopyProjectPath, FocusNextPane, FocusPreviousPane, MoveTabLeft, MoveTabRight, RenameTab,
    ResetTabTitle, ResizePaneDown, ResizePaneLeft, ResizePaneRight, SwitchProject, ToggleStatusBar,
    ToggleTabPin,
};
use muxy_app_core::Layout;
use muxy_core::shortcuts::ShortcutId;

fn tabs(count: usize) -> (AppState, Vec<TabId>) {
    let mut state = AppState::bootstrap().expect("state");
    let ids: Vec<_> = (0..count)
        .map(|_| state.open_terminal_tab(state.home().id).expect("tab"))
        .collect();
    (state, ids)
}

fn order(model: &AppModel) -> Vec<TabId> {
    model.state.home().tabs.iter().map(|tab| tab.id).collect()
}

fn root_ratio(model: &AppModel) -> Option<f32> {
    match &model.tab(model.active_tab()?)?.layout {
        Layout::Split { ratio, .. } => Some(*ratio),
        Layout::Leaf(_) => None,
    }
}

fn disabled(
    model: &mut AppModel,
    ids: &[ShortcutId],
    cx: &mut Context<AppModel>,
) -> Vec<Option<bool>> {
    let registry = model.command_registry(true, cx);
    ids.iter()
        .map(|id| {
            registry
                .command(id.name())
                .map(muxy_ui::command_palette::Command::is_disabled)
        })
        .collect()
}

#[gpui::test]
fn moving_the_active_tab_reorders_it_without_wrapping_or_crossing_pins(cx: &mut TestAppContext) {
    let (mut state, ids) = tabs(3);
    state.select_tab(state.home().id, ids[1]).expect("select");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.dispatch_action(MoveTabRight);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(order(model), [ids[0], ids[2], ids[1]]);
        assert_eq!(model.active_tab(), Some(ids[1]));
    });
    cx.dispatch_action(MoveTabRight);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(order(model), [ids[0], ids[2], ids[1]]);
    });
    cx.dispatch_action(MoveTabLeft);
    cx.dispatch_action(MoveTabLeft);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(order(model), [ids[1], ids[0], ids[2]]);
    });

    cx.dispatch_action(ToggleTabPin);
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        assert!(model.state.home().tabs[0].pinned);
        assert_eq!(
            disabled(
                model,
                &[ShortcutId::MoveTabLeft, ShortcutId::MoveTabRight],
                cx
            ),
            [Some(true), Some(true)]
        );
    });
    cx.dispatch_action(MoveTabRight);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(order(model), [ids[1], ids[0], ids[2]]);
    });
}

#[gpui::test]
fn resizing_and_cycling_panes_from_the_keyboard(cx: &mut TestAppContext) {
    let (state, _) = tabs(1);
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.dispatch_action(ResizePaneLeft);
    cx.run_until_parked();
    let first = view.update(cx, |model, cx| {
        assert_eq!(root_ratio(model), None);
        let first = model.active_pane().expect("pane");
        model.split_pane(Direction::Right, cx);
        first
    });
    let second = view.read_with(cx, |model, _| model.active_pane().expect("pane"));
    assert_ne!(first, second);

    cx.dispatch_action(ResizePaneLeft);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!((root_ratio(model).expect("split") - 0.45).abs() < 1e-4);
    });
    cx.dispatch_action(ResizePaneDown);
    cx.dispatch_action(ResizePaneRight);
    cx.dispatch_action(ResizePaneRight);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!((root_ratio(model).expect("split") - 0.55).abs() < 1e-4);
        let saved = store::load(&model.path).expect("saved");
        assert_eq!(saved, model.state);
    });

    for (expected, forward) in [
        (first, true),
        (second, true),
        (first, false),
        (second, false),
    ] {
        if forward {
            cx.dispatch_action(FocusNextPane);
        } else {
            cx.dispatch_action(FocusPreviousPane);
        }
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert_eq!(model.active_pane(), Some(expected));
        });
    }

    let resize = [
        ShortcutId::ResizePaneLeft,
        ShortcutId::ResizePaneRight,
        ShortcutId::ResizePaneUp,
        ShortcutId::ResizePaneDown,
    ];
    view.update(cx, |model, cx| {
        assert_eq!(
            disabled(model, &resize, cx),
            [Some(false), Some(false), Some(true), Some(true)]
        );
        model.toggle_zoom_pane(cx);
        assert_eq!(disabled(model, &resize, cx), [Some(true); 4]);
    });
    cx.dispatch_action(ResizePaneLeft);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!((root_ratio(model).expect("split") - 0.55).abs() < 1e-4);
    });
}

#[gpui::test]
fn the_switch_project_shortcut_opens_and_closes_its_palette_page(cx: &mut TestAppContext) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let key = ShortcutId::SwitchProject.definition().keys[0];
    let page = |model: &AppModel, cx: &gpui::App| match &model.overlay {
        Some(Overlay::Commands(Palette { palette, .. })) => {
            Some(palette.read(cx).is_page(ShortcutId::SwitchProject.name()))
        }
        _ => None,
    };
    view.update(cx, |model, cx| {
        let chord = model.settings.keymap.chord(ShortcutId::SwitchProject);
        let label = crate::views::command_palette::chord_label(chord.expect("chord"));
        let rows = model.command_registry(true, cx).items("switch project");
        let trailing = rows.iter().find_map(|item| match item {
            muxy_ui::picker::PickerItem::Row(row) => row.trailing.clone(),
            muxy_ui::picker::PickerItem::Section(_) => None,
        });
        let trailing = trailing.expect("row");
        assert!(trailing.starts_with(label.as_str()), "{trailing}");
        assert!(trailing.ends_with('›'), "{trailing}");
    });
    cx.simulate_keystrokes(key);
    cx.run_until_parked();
    view.read_with(cx, |model, cx| assert_eq!(page(model, cx), Some(true)));
    cx.simulate_keystrokes(key);
    cx.run_until_parked();
    view.read_with(cx, |model, cx| assert_eq!(page(model, cx), None));

    cx.simulate_keystrokes(ShortcutId::ToggleCommandPalette.definition().keys[0]);
    cx.run_until_parked();
    view.read_with(cx, |model, cx| assert_eq!(page(model, cx), Some(false)));
    cx.dispatch_action(SwitchProject);
    cx.run_until_parked();
    view.read_with(cx, |model, cx| assert_eq!(page(model, cx), Some(true)));
}

#[gpui::test]
fn tab_and_project_commands_act_on_the_active_ones(cx: &mut TestAppContext) {
    let (state, ids) = tabs(2);
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert_eq!(model.active_tab(), Some(ids[1]));
        assert_eq!(
            disabled(
                model,
                &[
                    ShortcutId::RenameTab,
                    ShortcutId::ResetTabTitle,
                    ShortcutId::CloseOtherTabs,
                    ShortcutId::CloseTabsToTheLeft,
                    ShortcutId::CloseTabsToTheRight,
                    ShortcutId::MoveTabToMainWindow,
                    ShortcutId::RemoveProject,
                    ShortcutId::CopyProjectPath,
                ],
                cx
            ),
            [
                Some(false),
                Some(true),
                Some(false),
                Some(false),
                Some(true),
                Some(true),
                Some(true),
                Some(false),
            ]
        );
    });

    cx.dispatch_action(RenameTab);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(matches!(model.overlay, Some(Overlay::ProjectEditor(_))));
    });
    cx.simulate_input("logs");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        let tab = model.tab(ids[1]).expect("tab");
        assert_eq!(tab.custom_title.as_deref(), Some("logs"));
    });
    cx.dispatch_action(ResetTabTitle);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.tab(ids[1]).expect("tab").custom_title, None);
    });

    cx.dispatch_action(CopyProjectPath);
    cx.run_until_parked();
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    view.read_with(cx, |model, _| {
        let directory = model.state.current_project().directory.to_string_lossy();
        assert_eq!(copied.as_deref(), Some(directory.as_ref()));
    });

    let shown = view.read_with(cx, |model, _| model.appearance.status_bar_visible);
    cx.dispatch_action(ToggleStatusBar);
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert_eq!(model.appearance.status_bar_visible, !shown);
    });
}

#[gpui::test]
fn terminal_and_browser_commands_need_their_pane(cx: &mut TestAppContext) {
    let terminal = [
        ShortcutId::ClearScreen,
        ShortcutId::SelectAll,
        ShortcutId::ScrollToTop,
        ShortcutId::ResetFontSize,
    ];
    let browser = [
        ShortcutId::BrowserReload,
        ShortcutId::BrowserFocusAddress,
        ShortcutId::BrowserOpenExternally,
    ];
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        assert_eq!(disabled(model, &terminal, cx), [Some(true); 4]);
        assert_eq!(disabled(model, &browser, cx), [Some(true); 3]);
        model.new_tab(cx);
        assert_eq!(disabled(model, &terminal, cx), [Some(false); 4]);
        assert_eq!(disabled(model, &browser, cx), [Some(true); 3]);
    });
}

#[gpui::test]
fn tooltips_show_the_key_of_the_command_they_run(cx: &mut TestAppContext) {
    use crate::views::command_palette::chord_label;
    use crate::views::workspace::{FindNext, NewTab};

    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let key = |id: ShortcutId| chord_label(model.settings.keymap.chord(id).expect("chord"));
        let new_tab = key(ShortcutId::NewTab);
        assert_eq!(
            model.tooltip("New Tab", ShortcutId::NewTab),
            format!("New Tab ({new_tab})")
        );
        assert_eq!(
            model.tooltip("Reset Tab Title", ShortcutId::ResetTabTitle),
            "Reset Tab Title"
        );
        let bound = muxy_ui::shortcuts::key_for(&NewTab, cx).expect("bound");
        assert_eq!(bound, new_tab);
        assert_eq!(
            muxy_ui::shortcuts::tooltip_text("Next match", &FindNext, cx),
            format!("Next match ({})", key(ShortcutId::FindNext))
        );
        assert_eq!(
            muxy_ui::shortcuts::tooltip_text("Reset Tab Title", &ResetTabTitle, cx),
            "Reset Tab Title"
        );
    });
}

/// Rests the pointer on the control `selector` names until its tooltip shows,
/// and returns whether the tooltip reads `expected`.
fn hovering_shows(cx: &mut VisualTestContext, selector: String, expected: &str) -> bool {
    let selector: &'static str = selector.leak();
    let expected: &'static str = format!("tooltip:{expected}").leak();
    let position = cx.debug_bounds(selector).expect(selector).center();
    cx.simulate_mouse_move(gpui::point(px(1.0), px(1.0)), None, Modifiers::default());
    cx.simulate_mouse_move(position, None, Modifiers::default());
    cx.executor().advance_clock(Duration::from_millis(700));
    cx.run_until_parked();
    cx.debug_bounds(expected).is_some()
}

#[gpui::test]
fn project_names_only_appear_in_tooltips_when_the_sidebar_is_collapsed(cx: &mut TestAppContext) {
    use muxy_app_core::settings::{AppLayout, SidebarCollapsedStyle};

    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    let name = "Alpha project with a complete display name";
    state.rename_project(project, name).expect("rename");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.layout = AppLayout::ProjectFocused;
    boot.settings.appearance.sidebar_collapsed_style = SidebarCollapsedStyle::Icons;
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));

    for expanded in [true, false] {
        view.update(cx, |model, cx| {
            model.appearance.sidebar_expanded = expanded;
            cx.notify();
        });
        cx.run_until_parked();
        for (id, expected) in [(home, "Home"), (project, name)] {
            assert_eq!(
                hovering_shows(cx, format!("project-tile-{id}"), expected),
                !expanded,
                "{expected}, sidebar expanded: {expanded}"
            );
        }
    }

    view.update(cx, |model, cx| {
        model
            .state
            .rename_project(project, "Renamed project")
            .expect("rename");
        cx.notify();
    });
    cx.run_until_parked();
    assert!(hovering_shows(
        cx,
        format!("project-tile-{project}"),
        "Renamed project"
    ));
}

#[gpui::test]
fn hovering_a_button_shows_its_shortcut_beside_its_name(cx: &mut TestAppContext) {
    use crate::views::command_palette::chord_label;

    let (state, _) = tabs(1);
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1200.0), px(800.0)));
    cx.run_until_parked();
    let (pane, key) = view.read_with(cx, |model, _| {
        let key = |id: ShortcutId| chord_label(model.settings.keymap.chord(id).expect("chord"));
        (
            model.active_pane().expect("pane"),
            [
                key(ShortcutId::SplitRight),
                key(ShortcutId::ClosePane),
                key(ShortcutId::NewTab),
                key(ShortcutId::ToggleSidebar),
                key(ShortcutId::FloatPane),
                key(ShortcutId::NavigateBack),
                key(ShortcutId::ToggleAgentSessions),
            ],
        )
    });
    for (selector, expected) in [
        (
            format!("pane-split-right-{pane}"),
            format!("Split Right ({})", key[0]),
        ),
        (
            format!("pane-close-{pane}"),
            format!("Close Pane ({})", key[1]),
        ),
        (
            format!("pane-float-{pane}"),
            format!("Float Pane ({})", key[4]),
        ),
        ("nav-back".to_owned(), format!("Back ({})", key[5])),
        (
            "toggle-agent-sessions".to_owned(),
            format!("Agent Sessions ({})", key[6]),
        ),
        ("new-tab-button".to_owned(), format!("New Tab ({})", key[2])),
        (
            "sidebar-toggle".to_owned(),
            format!("Toggle Sidebar ({})", key[3]),
        ),
    ] {
        assert!(hovering_shows(cx, selector, &expected), "{expected}");
    }

    assert!(!hovering_shows(
        cx,
        format!("pane-split-right-{pane}"),
        "Split Right"
    ));
    assert!(!hovering_shows(
        cx,
        format!("pane-float-{pane}"),
        "Float Pane"
    ));

    let chord: muxy_app_core::settings::KeyChord = "ctrl-alt-f9".parse().expect("chord");
    let bound = format!("Float Pane ({})", chord_label(&chord));
    view.update(cx, |model, cx| {
        model.settings.keymap = model
            .settings
            .keymap
            .with_binding(ShortcutId::FloatPane.name(), Some(chord))
            .expect("bound");
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        hovering_shows(cx, format!("pane-float-{pane}"), &bound),
        "{bound}"
    );
}
