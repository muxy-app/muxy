use super::*;
use gpui::{MouseButton, Point, point, px};
use muxy_app_core::{Direction, TabPlace};

use crate::model::tab_windows::Surface;

fn listed(model: &AppModel, project: ProjectId) -> Vec<TabId> {
    model
        .state
        .project(project)
        .map(|project| project.listed_tabs().map(|tab| tab.id).collect())
        .unwrap_or_default()
}

fn windows(cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| cx.windows().len())
}

#[gpui::test]
fn a_tab_moves_into_its_own_window_and_back(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let a = state.open_terminal_tab(home).expect("tab");
    let b = state.open_terminal_tab(home).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    assert_eq!(windows(cx), 1);
    view.update(cx, |model, cx| model.move_tab_to_window(b, None, None, cx));
    cx.run_until_parked();
    assert_eq!(windows(cx), 2);
    let id = view.read_with(cx, |model, _| {
        assert_eq!(listed(model, home), [a]);
        assert_eq!(model.main_visible_tabs(), [a]);
        assert_eq!(model.active_tab(), Some(b));
        let (_, tab, window) = model.state.tab_windows().next().expect("tab window");
        assert_eq!(tab.id, b);
        assert!(model.tab_windows.handle(window.id).is_some());
        window.id
    });
    view.update(cx, |model, cx| model.return_tab_window(id, None, cx));
    cx.run_until_parked();
    assert_eq!(windows(cx), 1);
    view.read_with(cx, |model, _| {
        assert_eq!(listed(model, home), [a, b]);
        assert_eq!(model.active_tab(), Some(b));
        assert!(model.tab_windows.handle(id).is_none());
    });
}

#[gpui::test]
fn a_tab_window_answers_for_its_own_tab_and_pane(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let a = state.open_terminal_tab(home).expect("tab");
    let main_pane = state.window().active_pane.expect("pane");
    let b = state.open_terminal_tab(home).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| model.move_tab_to_window(b, None, None, cx));
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        let id = model.state.tab_windows().next().expect("window").2.id;
        let pane = model.tab(b).expect("tab").panes[0].id;
        model.enter_surface(Surface::Tab(id));
        assert_eq!(model.active_tab(), Some(b));
        assert_eq!(model.active_pane(), Some(pane));
        assert_eq!(model.visible_tabs(), [b]);
        model.split_pane(Direction::Right, cx);
        let split = model.active_pane().expect("split");
        assert_ne!(split, pane);
        assert_eq!(model.tab(b).expect("tab").panes.len(), 2);
        model.enter_surface(Surface::Main);
        assert_eq!(model.active_tab(), Some(a));
        assert_eq!(model.active_pane(), Some(main_pane));
        assert!(model.attached_panes().contains(&split));
        assert!(model.attached_panes().contains(&main_pane));
    });
}

#[gpui::test]
fn closing_a_tab_window_closes_its_tab(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let a = state.open_terminal_tab(home).expect("tab");
    let b = state.open_terminal_tab(home).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| model.move_tab_to_window(b, None, None, cx));
    cx.run_until_parked();
    view.update(cx, |model, cx| {
        let id = model.state.tab_windows().next().expect("window").2.id;
        model.close_tab_window(id, cx);
    });
    cx.run_until_parked();
    assert_eq!(windows(cx), 1);
    view.read_with(cx, |model, _| {
        assert!(model.tab(b).is_none());
        assert_eq!(listed(model, home), [a]);
    });
}

#[gpui::test]
fn terminals_change_project_only_on_servers_that_can_follow(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let other = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.select_project(home).expect("select");
    let a = state.open_terminal_tab(home).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    let session = SessionId::new(91).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        let before = model.state.clone();
        model.move_tab_to_place(a, TabPlace::end(other), cx);
        assert_eq!(model.state, before);
        assert!(model.error.is_some());
        model.servers.local.features = vec![muxy_protocol::Feature::SESSION_MOVE];
        model.move_tab_to_place(a, TabPlace::end(other), cx);
        assert_eq!(listed(model, other), [a]);
        assert!(listed(model, home).is_empty());
        assert_eq!(model.state.current_project().id, other);
        assert!(requests.try_iter().any(|(_, work)| matches!(
            work,
            Work::MoveSession(moved, project) if moved == session && project == other
        )));
    });
}

#[gpui::test]
fn pane_menu_moves_a_pane_into_a_new_tab(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let a = state.open_terminal_tab(home).expect("tab");
    let first = state.window().active_pane.expect("pane");
    let second = state.split_pane(first, Direction::Right).expect("split");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.move_pane_to_new_tab(second, TabPlace::end(home), cx);
        let tabs = listed(model, home);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0], a);
        assert_eq!(model.tab(tabs[1]).expect("tab").layout.leaves(), [second]);
        assert_eq!(model.active_pane(), Some(second));
    });
}

fn press(cx: &mut VisualTestContext, position: Point<gpui::Pixels>, command: bool) {
    cx.simulate_event(gpui::MouseDownEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        modifiers: Modifiers {
            platform: command,
            ..Default::default()
        },
        ..Default::default()
    });
    cx.run_until_parked();
}

fn pointer(cx: &mut VisualTestContext, position: Point<gpui::Pixels>) {
    cx.simulate_event(gpui::MouseMoveEvent {
        position,
        pressed_button: Some(MouseButton::Left),
        ..Default::default()
    });
    cx.run_until_parked();
}

fn release(cx: &mut VisualTestContext, position: Point<gpui::Pixels>) {
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        button: MouseButton::Left,
        click_count: 1,
        ..Default::default()
    });
    cx.run_until_parked();
}

#[gpui::test]
fn dragging_a_pane_onto_a_tab_cell_makes_it_a_tab_beside_that_tab(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let a = state.open_terminal_tab(home).expect("tab");
    let first = state.window().active_pane.expect("pane");
    let second = state.split_pane(first, Direction::Right).expect("split");
    let b = state.open_terminal_tab(home).expect("tab");
    state.select_tab(home, a).expect("select");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let pane = cx
        .debug_bounds(format!("pane-{second}").leak())
        .expect("pane bounds");
    let cell = cx.debug_bounds("tab-cell-1").expect("tab cell");
    press(cx, pane.center(), true);
    pointer(cx, pane.center() + point(px(12.0), px(0.0)));
    let target = point(cell.left() + px(4.0), cell.center().y);
    pointer(cx, target);
    view.read_with(cx, |model, _| {
        assert!(matches!(
            model.move_drag.target(),
            Some(crate::views::move_drag::Target::Place(TabPlace {
                project,
                before: Some(before),
            })) if project == home && before == b
        ));
    });
    release(cx, target);
    view.read_with(cx, |model, _| {
        let tabs = listed(model, home);
        assert_eq!(tabs.len(), 3);
        assert_eq!([tabs[0], tabs[2]], [a, b]);
        assert_eq!(model.tab(tabs[1]).expect("tab").layout.leaves(), [second]);
        assert_eq!(model.tab(a).expect("tab").layout.leaves(), [first]);
        assert!(model.move_drag.target().is_none());
    });
}

#[gpui::test]
fn dragging_a_tab_out_of_the_window_opens_it_in_its_own(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let a = state.open_terminal_tab(home).expect("tab");
    let b = state.open_terminal_tab(home).expect("tab");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.run_until_parked();
    let cell = cx.debug_bounds("tab-cell-1").expect("tab cell");
    press(cx, cell.center(), false);
    pointer(cx, cell.center() + point(px(0.0), px(12.0)));
    let outside = point(px(-40.0), px(-40.0));
    pointer(cx, outside);
    release(cx, outside);
    assert_eq!(windows(cx), 2);
    view.read_with(cx, |model, _| {
        assert_eq!(listed(model, home), [a]);
        assert!(model.tab(b).is_some_and(muxy_app_core::Tab::detached));
    });
}

#[gpui::test]
fn a_move_the_server_refuses_puts_the_terminal_back(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let other = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.select_project(home).expect("select");
    let a = state.open_terminal_tab(home).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    let session = SessionId::new(93).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.servers.local.features = vec![muxy_protocol::Feature::SESSION_MOVE];
        model.move_tab_to_place(a, TabPlace::end(other), cx);
        assert_eq!(model.state.pane_project(pane).map(|p| p.id), Some(other));
        model.session_moved(
            ServerId::local(),
            session,
            Err(muxy_client::ClientError::Disconnected),
            cx,
        );
        assert_eq!(model.state.pane_project(pane).map(|p| p.id), Some(other));
        assert!(
            model
                .state
                .pending_session_move(ServerId::local(), session)
                .is_some()
        );
        model.move_tab_to_place(a, TabPlace::end(home), cx);
        assert_eq!(model.state.pane_project(pane).map(|p| p.id), Some(other));
        model.resend_session_move(
            muxy_app_core::SessionMove {
                server: ServerId::local(),
                session,
                project: other,
            },
            home,
            cx,
        );
        model.session_moved(
            ServerId::local(),
            session,
            Err(muxy_client::ClientError::Invalid(
                muxy_protocol::ErrorCode::UnknownProject,
            )),
            cx,
        );
        assert_eq!(model.state.pane_project(pane).map(|p| p.id), Some(home));
        assert!(listed(model, other).is_empty());
        assert!(
            model
                .state
                .pending_session_move(ServerId::local(), session)
                .is_none()
        );
    });
}

#[gpui::test]
fn a_terminal_still_starting_keeps_its_project(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let other = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.select_project(home).expect("select");
    let a = state.open_terminal_tab(home).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.servers.local.features = vec![muxy_protocol::Feature::SESSION_MOVE];
        model.pending.insert(pane, ServerId::local());
        let before = model.state.clone();
        model.move_tab_to_place(a, TabPlace::end(other), cx);
        assert_eq!(model.state, before);
        assert!(model.error.is_some());
        model.move_tab_to_window(a, None, None, cx);
        assert!(model.tab(a).is_some_and(muxy_app_core::Tab::detached));
    });
}

#[gpui::test]
fn a_terminal_two_panes_show_moves_only_with_both(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let other = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.select_project(home).expect("select");
    let a = state.open_terminal_tab(home).expect("tab");
    let first = state.window().active_pane.expect("pane");
    let second = state.split_pane(first, Direction::Right).expect("split");
    let session = SessionId::new(95).expect("session");
    for pane in [first, second] {
        state
            .set_pane_session(pane, Some(session))
            .expect("session");
    }
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.servers.local.features = vec![muxy_protocol::Feature::SESSION_MOVE];
        let before = model.state.clone();
        model.move_pane_to_new_tab(second, TabPlace::end(other), cx);
        assert_eq!(model.state, before);
        assert!(model.error.is_some());
        model.move_tab_to_place(a, TabPlace::end(other), cx);
        assert_eq!(listed(model, other), [a]);
    });
}

#[gpui::test]
fn a_terminal_moves_again_only_once_its_server_answered(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let other = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.select_project(home).expect("select");
    let a = state.open_terminal_tab(home).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    let session = SessionId::new(97).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.servers.local.features = vec![muxy_protocol::Feature::SESSION_MOVE];
        model.move_tab_to_place(a, TabPlace::end(other), cx);
        let moved = model.state.clone();
        model.move_tab_to_place(a, TabPlace::end(home), cx);
        assert_eq!(model.state, moved);
        model.session_moved(ServerId::local(), session, Ok(()), cx);
        model.move_tab_to_place(a, TabPlace::end(home), cx);
        assert_eq!(listed(model, home), [a]);
    });
}

#[gpui::test]
fn a_move_whose_answer_timed_out_is_sent_again_once(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let home = state.home().id;
    let other = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.select_project(home).expect("select");
    let a = state.open_terminal_tab(home).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    let session = SessionId::new(99).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.servers.local.connection = ConnectionState::Ready;
        model.servers.local.features = vec![muxy_protocol::Feature::SESSION_MOVE];
        model.move_tab_to_place(a, TabPlace::end(other), cx);
        let sent = |requests: &std::sync::mpsc::Receiver<(u64, Work)>| {
            requests
                .try_iter()
                .filter(|(_, work)| matches!(work, Work::MoveSession(id, _) if *id == session))
                .count()
        };
        assert_eq!(sent(&requests), 1);
        model.session_moved(
            ServerId::local(),
            session,
            Err(muxy_client::ClientError::Timeout),
            cx,
        );
        assert_eq!(sent(&requests), 1);
        model.session_moved(
            ServerId::local(),
            session,
            Err(muxy_client::ClientError::Timeout),
            cx,
        );
        assert_eq!(sent(&requests), 0);
        assert_eq!(model.state.pane_project(pane).map(|p| p.id), Some(other));
        assert!(
            model
                .state
                .pending_session_move(ServerId::local(), session)
                .is_some()
        );
    });
}
