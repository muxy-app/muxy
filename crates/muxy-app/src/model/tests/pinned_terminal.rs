use super::*;

fn pinned_state() -> (AppState, ProjectId, PaneId, SessionId) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state
        .add_project(ServerId::local(), std::env::temp_dir())
        .expect("project");
    state.select_project(state.home().id).expect("home");
    state.open_terminal_tab(state.home().id).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    let session = SessionId::new(801).expect("session");
    state
        .set_pane_session(pane, Some(session))
        .expect("session");
    (state, project, pane, session)
}

fn connect(model: &mut AppModel, cx: &mut Context<AppModel>) {
    let home = model.state.home().id;
    let sessions = model
        .state
        .session_references(ServerId::local())
        .into_iter()
        .map(|id| SessionInfo {
            project: home,
            id,
            directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
        })
        .collect();
    model.receive((ServerId::local(), 1, Update::Connected(sessions)), cx);
    acknowledge_catalog(model, cx);
}

#[gpui::test]
fn pinned_terminal_leaves_its_tab_and_stays_attached_in_every_project(cx: &mut TestAppContext) {
    let (state, project, pane, session) = pinned_state();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect(model, cx);
        assert!(model.can_pin_terminal(pane));
        model.pin_terminal(pane, cx);
        assert!(model.state.home().tabs.is_empty());
        assert_eq!(model.state.pinned_terminal().map(|pinned| pinned.id), Some(pane));
        assert_eq!(model.pane_session(pane), Some(session));
        assert!(model.attached_panes().contains(&pane));
        assert!(model.grids.contains_key(&pane));

        model.select_project(project, cx);
        assert!(model.attached_panes().contains(&pane));
        assert!(model.grids.contains_key(&pane));
        assert!(!model.visible_panes().contains(&pane));
        model.start_attach(pane, Size { cols: 60, rows: 30 }, cx);
        assert!(requests.try_iter().any(|(_, work)| matches!(work, Work::Attach { pane: id, session: Some(attached), .. } if id == pane && attached == session)));
        assert!(model.state.pending_discards(ServerId::local()).is_empty());
    });
}

#[gpui::test]
fn hiding_the_pinned_terminal_detaches_its_view_and_keeps_the_session(cx: &mut TestAppContext) {
    let (state, _, pane, session) = pinned_state();
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect(model, cx);
        model.pin_terminal(pane, cx);
    });
    view.update_in(cx, |model, window, cx| {
        model.toggle_pinned_terminal(window, cx);
        assert!(model.appearance.pinned_terminal_hidden);
        assert!(!model.attached_panes().contains(&pane));
        assert!(!model.grids.contains_key(&pane));
        assert_eq!(model.pane_session(pane), Some(session));
        assert!(
            !requests
                .try_iter()
                .any(|(_, work)| matches!(work, Work::Discard(_, _)))
        );

        model.toggle_pinned_terminal(window, cx);
        assert!(!model.appearance.pinned_terminal_hidden);
        assert!(model.grids.contains_key(&pane));
        assert!(model.pinned.focused);
    });
}

#[gpui::test]
fn unpinning_moves_the_terminal_into_a_new_tab_of_the_current_project(cx: &mut TestAppContext) {
    let (state, project, pane, session) = pinned_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect(model, cx);
        model.pin_terminal(pane, cx);
        model.select_project(project, cx);
        model.unpin_terminal(cx);
        assert!(model.state.pinned_terminal().is_none());
        assert_eq!(model.state.current_project().id, project);
        assert_eq!(model.active_pane(), Some(pane));
        assert_eq!(model.pane_session(pane), Some(session));
        assert!(model.state.pending_discards(ServerId::local()).is_empty());
    });
}

#[gpui::test]
fn pinning_another_terminal_returns_the_first_one_to_a_tab(cx: &mut TestAppContext) {
    let (mut state, _, first, _) = pinned_state();
    state.open_terminal_tab(state.home().id).expect("tab");
    let second = state.window().active_pane.expect("pane");
    let session = SessionId::new(802).expect("session");
    state
        .set_pane_session(second, Some(session))
        .expect("session");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect(model, cx);
        model.pin_terminal(first, cx);
        model.pin_terminal(second, cx);
        assert_eq!(
            model.state.pinned_terminal().map(|pinned| pinned.id),
            Some(second)
        );
        assert!(
            model
                .state
                .home()
                .tabs
                .iter()
                .any(|tab| tab.layout.contains(first))
        );
        assert!(model.state.pending_discards(ServerId::local()).is_empty());
    });
}

#[gpui::test]
fn an_ended_session_closes_the_pinned_terminal(cx: &mut TestAppContext) {
    let (state, _, pane, session) = pinned_state();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        connect(model, cx);
        model.pin_terminal(pane, cx);
        model.close_ended_session(ServerId::local(), session, cx);
        assert!(model.state.pinned_terminal().is_none());
        assert!(!model.pinned_terminal_shown());
        assert!(!model.grids.contains_key(&pane));
    });
}

#[test]
fn pinned_terminal_round_trips_through_saved_state() {
    let (mut state, _, pane, session) = pinned_state();
    state.pin_pane(pane).expect("pin");
    let saved = serde_json::to_string(&state).expect("save");
    let restored: AppState = serde_json::from_str(&saved).expect("restore");
    assert_eq!(restored, state);
    assert_eq!(restored.pane_server(pane), Some(ServerId::local()));
    assert_eq!(
        restored.session_references(ServerId::local()),
        vec![session]
    );
}

#[gpui::test]
fn a_restart_without_the_pinned_session_closes_the_pinned_terminal(cx: &mut TestAppContext) {
    let (mut state, _, pane, _) = pinned_state();
    state.pin_pane(pane).expect("pin");
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(Vec::new())), cx);
        acknowledge_catalog(model, cx);
        assert!(model.state.pinned_terminal().is_none());
        assert!(!model.grids.contains_key(&pane));
    });
}
