use super::*;
use crate::views::menu::{Command, Item};
use gpui::Focusable;
use muxy_app_core::Direction;

/// Two attached panes with the first one active, so focus going back to the
/// active pane can't be mistaken for focus going to the newest pane.
fn split_workspace(cx: &mut TestAppContext) -> (Entity<AppModel>, &mut VisualTestContext, PaneId) {
    let mut state = AppState::bootstrap().expect("state");
    state.open_terminal_tab(state.home().id).expect("tab");
    let first = state.home().tabs[0].panes[0].id;
    let second = state.split_pane(first, Direction::Right).expect("split");
    for (index, pane) in [first, second].into_iter().enumerate() {
        state
            .set_pane_session(pane, SessionId::new(100 + index as u64))
            .expect("session");
    }
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        for (index, pane) in [first, second].into_iter().enumerate() {
            let session = model.state.pane_session(pane).expect("session").1;
            model.receive(
                (
                    ServerId::local(),
                    1,
                    Update::Attached {
                        pane,
                        session,
                        attachment: attachment_on(u32::try_from(index + 1).expect("channel")),
                        created: false,
                    },
                ),
                cx,
            );
        }
        model.focus_pane(first, cx);
    });
    cx.run_until_parked();
    (view, cx, first)
}

fn pane_focused(view: &Entity<AppModel>, pane: PaneId, cx: &mut VisualTestContext) -> bool {
    view.update_in(cx, |model, window, cx| {
        assert_eq!(model.active_pane(), Some(pane));
        model
            .terminal(&pane)
            .expect("terminal")
            .view
            .read(cx)
            .focus
            .is_focused(window)
    })
}

#[gpui::test]
fn each_overlay_hands_the_keyboard_back_to_the_active_pane(cx: &mut TestAppContext) {
    type Open = fn(&mut AppModel, &mut Window, &mut Context<AppModel>);
    let overlays: [(&str, Open); 5] = [
        ("command palette", |model, window, cx| {
            model.toggle_command_palette(window, cx);
        }),
        ("menu", |model, window, cx| {
            let items = vec![Item::action("Agents", Command::AgentsPanel(true))];
            model.open_menu(items, gpui::point(px(40.0), px(40.0)), window, cx);
        }),
        ("remote popover", AppModel::toggle_remote_popover),
        ("server form", |model, _, cx| {
            model.open_server_form(None, cx);
        }),
        ("password prompt", |model, _, cx| {
            model.ask_password(ServerId::local(), cx);
        }),
    ];
    let (view, cx, pane) = split_workspace(cx);
    assert!(pane_focused(&view, pane, cx));
    for (name, open) in overlays {
        view.update_in(cx, open);
        cx.run_until_parked();
        view.read_with(cx, |model, _| assert!(model.overlay.is_some(), "{name}"));
        assert!(!pane_focused(&view, pane, cx), "{name} took the keyboard");
        view.update(cx, AppModel::dismiss_overlay);
        cx.run_until_parked();
        assert!(
            pane_focused(&view, pane, cx),
            "{name} gave the keyboard back"
        );
    }
}

#[gpui::test]
fn session_providers_hand_the_keyboard_back_to_the_search_unless_it_hides(cx: &mut TestAppContext) {
    let (view, cx, pane) = split_workspace(cx);
    let search_focused = |view: &Entity<AppModel>, cx: &mut VisualTestContext| {
        view.update_in(cx, |model, window, cx| {
            let input = model.agent_sessions.input.as_ref().expect("input");
            input.focus_handle(cx).is_focused(window)
        })
    };
    view.update_in(cx, AppModel::toggle_agent_sessions);
    cx.run_until_parked();
    assert!(search_focused(&view, cx));
    view.update_in(cx, AppModel::toggle_session_providers);
    cx.run_until_parked();
    assert!(!search_focused(&view, cx));
    view.update(cx, AppModel::dismiss_overlay);
    cx.run_until_parked();
    assert!(search_focused(&view, cx));

    view.update_in(cx, AppModel::toggle_session_providers);
    view.update(cx, |model, cx| {
        model.appearance.agent_sessions_visible = false;
        model.sync_agent_sessions(cx);
        assert!(model.overlay.is_none());
    });
    cx.run_until_parked();
    assert!(pane_focused(&view, pane, cx));
}

#[gpui::test]
fn closing_find_hands_the_keyboard_back_to_its_pane(cx: &mut TestAppContext) {
    let (view, cx, pane) = split_workspace(cx);
    view.update_in(cx, |model, window, cx| {
        let (theme, metrics) = (model.theme.clone(), model.metrics);
        let terminal = model.terminal(&pane).expect("terminal").view.clone();
        terminal.update(cx, |terminal, cx| {
            terminal.open_find(&theme, metrics, window, cx);
        });
    });
    cx.run_until_parked();
    assert!(!pane_focused(&view, pane, cx));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(pane_focused(&view, pane, cx));
}

#[gpui::test]
fn with_no_pane_the_workspace_keeps_the_keyboard_but_leaves_other_surfaces_alone(
    cx: &mut TestAppContext,
) {
    let (boot, _requests) = stub_boot(AppState::bootstrap().expect("state"));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    view.update_in(cx, |model, window, cx| {
        assert!(model.active_pane().is_none());
        assert!(model.focus.is_focused(window));
        model.focus_agents_panel(window);
        cx.notify();
    });
    cx.run_until_parked();
    view.update_in(cx, |model, window, cx| {
        assert!(model.agents_panel.focus.is_focused(window));
        window.blur();
        cx.notify();
    });
    cx.run_until_parked();
    view.update_in(cx, |model, window, _| {
        assert!(model.focus.is_focused(window));
    });
}
