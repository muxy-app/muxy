use super::coverage::openers;
use super::git;
use super::*;
use crate::model::{PendingRemote, RemoteIntent};
use crate::views::overlays::Overlay;
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

/// The current project, registered so Git overlays open, with one attached
/// pane.
fn project_workspace(
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext, PaneId, ProjectId) {
    let (mut state, project) = git::registered_current_project();
    let tab = state.open_terminal_tab(project).expect("tab");
    let tabs = &state.current_project().tabs;
    let pane = tabs.iter().find(|open| open.id == tab).expect("tab").panes[0].id;
    state
        .set_pane_session(pane, SessionId::new(100))
        .expect("session");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        let session = model.state.pane_session(pane).expect("session").1;
        let attached = Update::Attached {
            pane,
            session,
            attachment: attachment_on(1),
            created: false,
        };
        model.receive((ServerId::local(), 1, attached), cx);
        model.focus_pane(pane, cx);
    });
    cx.run_until_parked();
    (view, cx, pane, project)
}

/// The Agents opener sets its variant directly, and the Git form opens
/// without a window.
const OPEN_WITHOUT_THE_KEYBOARD: [&str; 2] = ["agents", "git form"];

#[gpui::test]
fn each_overlay_hands_the_keyboard_back_to_the_active_pane(cx: &mut TestAppContext) {
    let (view, cx, pane, project) = project_workspace(cx);
    assert!(pane_focused(&view, pane, cx));
    for (name, open) in openers(project) {
        view.update_in(cx, |model, window, cx| {
            model.appearance.status_bar_visible = true;
            open(model, window, cx);
            assert!(model.overlay.is_some(), "{name} opens");
        });
        cx.run_until_parked();
        if !OPEN_WITHOUT_THE_KEYBOARD.contains(&name) {
            assert!(!pane_focused(&view, pane, cx), "{name} took the keyboard");
        }
        while view.read_with(cx, |model, _| model.overlay.is_some()) {
            view.update(cx, AppModel::dismiss_overlay);
            cx.run_until_parked();
        }
        assert!(
            pane_focused(&view, pane, cx),
            "{name} gave the keyboard back"
        );
    }
}

#[gpui::test]
fn each_overlay_dismiss_cancels_only_its_own_work(cx: &mut TestAppContext) {
    let (view, cx, _, project) = project_workspace(cx);
    for (name, open) in openers(project) {
        let (interaction, probe) = view.update_in(cx, |model, window, cx| {
            model.overlay = None;
            model.appearance.status_bar_visible = true;
            open(model, window, cx);
            model.pending_remote =
                Some(PendingRemote::new(ServerId::local(), RemoteIntent::HomeTab));
            (model.git.interaction, model.server_probe_generation)
        });
        view.update(cx, |model, cx| {
            model.dismiss_overlay(cx);
            let git = matches!(name, "git branches" | "git form" | "ai provider");
            assert_eq!(model.git.interaction != interaction, git, "{name}: Git");
            let probed = model.server_probe_generation != probe;
            assert_eq!(probed, name == "server form", "{name}: server probe");
            let dropped = model.pending_remote.is_none();
            assert_eq!(dropped, name == "password", "{name}: pending remote");
            let reopened = matches!(model.overlay, Some(Overlay::RemoteServers));
            let expected = name == "menu over remote servers";
            assert_eq!(reopened, expected, "{name}: Remote popover");
            assert_eq!(model.overlay.is_some(), expected, "{name}: closed");
        });
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
fn a_return_belongs_to_the_overlay_that_closes(cx: &mut TestAppContext) {
    let (view, cx, pane) = split_workspace(cx);
    view.update_in(cx, AppModel::toggle_agent_sessions);
    cx.run_until_parked();

    // Replaced without being dismissed: the next overlay closes to the pane.
    view.update_in(cx, AppModel::toggle_session_providers);
    view.update(cx, |model, cx| model.ask_password(ServerId::local(), cx));
    cx.run_until_parked();
    view.update(cx, AppModel::dismiss_overlay);
    cx.run_until_parked();
    assert!(pane_focused(&view, pane, cx));

    // Closed while another overlay opens: that overlay keeps the keyboard.
    view.update_in(cx, AppModel::toggle_session_providers);
    view.update_in(cx, AppModel::toggle_remote_popover);
    cx.run_until_parked();
    view.update_in(cx, |model, window, _| {
        assert!(matches!(model.overlay, Some(Overlay::RemoteServers)));
        assert!(model.overlay_focus.is_focused(window));
    });

    // Closed without a dismiss: a later dismiss closes to the pane.
    view.update(cx, |model, cx| {
        model.overlay = Some(Overlay::SessionProviders);
        cx.notify();
    });
    view.update(cx, |model, cx| {
        model.overlay = None;
        model.dismiss_overlay(cx);
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
