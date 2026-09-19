use super::*;
use gpui::{MouseButton, MouseDownEvent, MouseUpEvent};
use muxy_app_core::{
    TabCloseScope,
    settings::{AppLayout, CloseBehavior},
};

fn fixture() -> (AppState, [TabId; 4]) {
    let mut state = AppState::bootstrap().expect("state");
    let ids = std::array::from_fn(|_| state.open_terminal_tab(state.home().id).expect("tab"));
    state.select_tab(state.home().id, ids[2]).expect("select");
    (state, ids)
}

fn click(cx: &mut VisualTestContext, selector: &str, button: MouseButton) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let position = cx
        .debug_bounds(selector.to_owned().leak())
        .expect(selector)
        .center();
    cx.simulate_event(MouseDownEvent {
        position,
        button,
        click_count: 1,
        ..Default::default()
    });
    cx.simulate_event(MouseUpEvent {
        position,
        button,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

fn menu(cx: &mut VisualTestContext, label: &str) {
    click(cx, &format!("menu-label-{label}"), MouseButton::Left);
}

#[gpui::test]
fn tab_context_customization_targets_inactive_tabs_in_both_layouts(cx: &mut TestAppContext) {
    for layout in [AppLayout::ProjectFocused, AppLayout::TabFocused] {
        let (state, ids) = fixture();
        let (mut boot, _requests) = stub_boot(state);
        boot.settings.appearance.layout = layout;
        boot.settings.appearance.sidebar_expanded = true;
        cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
        let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
        cx.simulate_resize(size(px(1000.0), px(600.0)));
        cx.run_until_parked();
        let selector = if layout == AppLayout::ProjectFocused {
            "tab-cell-0".into()
        } else {
            format!("sidebar-tab-{}", ids[0])
        };
        click(cx, &selector, MouseButton::Right);
        view.read_with(cx, |model, _| {
            assert_eq!(model.active_tab(), Some(ids[2]));
            assert!(!model.tab_drag.is_active());
            assert!(matches!(model.overlay, Some(Overlay::Menu(_))));
        });
        menu(cx, "Rename Tab");
        cx.simulate_input("Build 日本語");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert_eq!(
                model.tab(ids[0]).expect("tab").custom_title.as_deref(),
                Some("Build 日本語")
            );
            assert_eq!(model.active_tab(), Some(ids[2]));
            assert_eq!(store::load(&model.path).expect("saved"), model.state);
        });
        click(cx, &selector, MouseButton::Right);
        menu(cx, "Rename Tab");
        cx.simulate_input("Cancelled");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert_eq!(
            view.read_with(cx, |model, _| model
                .tab(ids[0])
                .expect("tab")
                .custom_title
                .clone()),
            Some("Build 日本語".into())
        );
        click(cx, &selector, MouseButton::Right);
        menu(cx, "Set Tab Color…");
        cx.simulate_keystrokes("right enter");
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            assert_eq!(
                model
                    .tab(ids[0])
                    .expect("tab")
                    .color
                    .as_ref()
                    .map(muxy_app_core::Color::as_str),
                Some(muxy_app_core::PROJECT_COLORS[1].1)
            );
            assert_eq!(model.active_tab(), Some(ids[2]));
        });
        click(cx, &selector, MouseButton::Right);
        menu(cx, "Pin Tab");
        click(cx, &selector, MouseButton::Middle);
        view.update(cx, |model, cx| {
            model.close_tab(ids[0], cx);
            model.close_pane(model.tab(ids[0]).expect("tab").panes[0].id, cx);
            assert!(model.tab(ids[0]).expect("pinned tab").pinned);
            assert_eq!(store::load(&model.path).expect("saved"), model.state);
        });
        click(cx, &selector, MouseButton::Right);
        menu(cx, "Unpin Tab");
        for label in ["Reset Title", "Reset Tab Color"] {
            click(cx, &selector, MouseButton::Right);
            menu(cx, label);
        }
        view.read_with(cx, |model, _| {
            let tab = model.tab(ids[0]).expect("tab");
            assert!(tab.custom_title.is_none());
            assert!(tab.color.is_none());
            assert!(!tab.pinned);
        });
        click(cx, &selector, MouseButton::Right);
        menu(cx, "Close Tabs to the Left");
        assert!(view.read_with(cx, |model, _| matches!(
            model.overlay,
            Some(Overlay::Menu(_))
        )));
        menu(cx, "Close Tab");
        view.read_with(cx, |model, _| {
            assert!(model.tab(ids[0]).is_none());
            assert_eq!(model.active_tab(), Some(ids[2]));
        });
    }
}

#[gpui::test]
fn adjacent_menu_creation_uses_the_clicked_project_and_position(cx: &mut TestAppContext) {
    let (mut state, ids) = fixture();
    let home = state.home().id;
    let other = state.add_project(std::env::temp_dir()).expect("project");
    state.open_terminal_tab(other).expect("other tab");
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.layout = AppLayout::TabFocused;
    boot.settings.appearance.sidebar_expanded = true;
    boot.settings
        .appearance
        .tab_focused_expanded
        .insert(home, true);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    cx.run_until_parked();
    for (label, expected_index) in [("New Tab to the Left", 1), ("New Tab to the Right", 3)] {
        click(cx, &format!("sidebar-tab-{}", ids[1]), MouseButton::Right);
        menu(cx, label);
        view.read_with(cx, |model, _| {
            assert_eq!(model.state.current_project().id, home);
            assert_eq!(
                model.active_tab(),
                Some(model.state.home().tabs[expected_index].id)
            );
            assert_eq!(model.state.project(other).expect("other").tabs.len(), 1);
        });
    }
}

#[gpui::test]
fn bulk_close_is_project_scoped_and_honors_pins_focus_and_detach(cx: &mut TestAppContext) {
    for (scope, remaining) in [
        (TabCloseScope::Left, vec![0, 2, 3]),
        (TabCloseScope::Right, vec![0, 1, 2]),
        (TabCloseScope::Other, vec![0, 2]),
    ] {
        for behavior in [CloseBehavior::CloseSession, CloseBehavior::Detach] {
            let (mut state, ids) = fixture();
            state.toggle_tab_pin(ids[0]).expect("pin");
            let other = state.add_project(std::env::temp_dir()).expect("project");
            let other_tab = state.open_terminal_tab(other).expect("other tab");
            let (mut boot, _requests) = stub_boot(state);
            boot.settings.window.close_behavior = behavior;
            let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
            view.update(cx, |model, cx| {
                model.close_tabs(ids[2], scope, cx);
                assert_eq!(
                    model
                        .state
                        .home()
                        .tabs
                        .iter()
                        .map(|tab| tab.id)
                        .collect::<Vec<_>>(),
                    remaining
                        .iter()
                        .map(|index| ids[*index])
                        .collect::<Vec<_>>()
                );
                assert_eq!(model.active_tab(), Some(other_tab));
                assert!(model.close_request.is_none());
                assert_eq!(store::load(&model.path).expect("saved"), model.state);
            });
        }
    }
}

#[gpui::test]
fn bulk_close_confirms_once_and_cancellation_preserves_every_target(cx: &mut TestAppContext) {
    let (mut state, ids) = fixture();
    for index in [0, 1, 3] {
        let pane = state.home().tabs[index].panes[0].id;
        state
            .set_pane_session(pane, SessionId::new(index as u64 + 10))
            .expect("session");
    }
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    for answer in ["Cancel", "Close"] {
        view.update(cx, |model, cx| {
            model.connection = ConnectionState::Ready;
            model.close_tabs(ids[2], TabCloseScope::Other, cx);
            let request = model.close_request.as_ref().expect("request");
            let tab = request.tab;
            let session = model.pane_session(request.panes[0]).expect("session");
            model.receive_close_checked(
                tab,
                session,
                Ok(Some(muxy_protocol::ForegroundProcess {
                    name: "vim".into(),
                    is_shell: false,
                })),
                cx,
            );
        });
        cx.run_until_parked();
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer(answer);
        cx.run_until_parked();
        assert!(!cx.has_pending_prompt());
        view.read_with(cx, |model, _| {
            assert_eq!(
                model.state.home().tabs.len(),
                if answer == "Cancel" { 4 } else { 1 }
            );
            assert_eq!(model.active_tab(), Some(ids[2]));
            assert!(model.close_request.is_none());
        });
    }
}

#[gpui::test]
fn tab_edits_and_bulk_close_roll_back_on_save_failure(cx: &mut TestAppContext) {
    let (state, ids) = fixture();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let previous = model.state.clone();
        let path = model.path.clone();
        let blocked = path.with_file_name("blocked");
        std::fs::create_dir_all(blocked.parent().expect("parent")).expect("directory");
        std::fs::write(&blocked, "file").expect("blocked");
        model.path = blocked.join("state.json");
        assert!(!model.edit_tab(|state| state.toggle_tab_pin(ids[0]), cx));
        assert_eq!(model.state, previous);
        model.close_tabs(ids[2], TabCloseScope::Other, cx);
        assert_eq!(model.state, previous);
        model.path = path;
    });
}
