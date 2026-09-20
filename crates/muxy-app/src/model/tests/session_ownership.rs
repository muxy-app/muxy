use super::*;
use muxy_protocol::{ClientKind, ProjectSession, ProjectSessions, SessionClient, SessionStatus};

fn session(
    project: ProjectId,
    id: u64,
    attached: bool,
    owner: Option<SessionClient>,
) -> ProjectSession {
    ProjectSession {
        info: SessionInfo {
            id: SessionId::new(id).expect("session"),
            project,
            directory: muxy_protocol::ServerPath(b"/tmp/muxy".to_vec()),
        },
        status: SessionStatus::Live,
        attached,
        owner,
    }
}

#[gpui::test]
fn existing_modal_filters_attached_sessions_preserves_owner_search_and_opens_one_tab(
    cx: &mut TestAppContext,
) {
    let mut state = AppState::bootstrap().expect("state");
    let project = state.home().id;
    state.open_terminal_tab(project).expect("tab");
    let pane = state.window().active_pane.expect("pane");
    state
        .set_pane_session(pane, SessionId::new(1))
        .expect("session");
    let (boot, _requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.simulate_resize(size(px(1000.0), px(600.0)));
    let owner = SessionClient {
        kind: ClientKind::Tui,
        ..SessionClient::default()
    };
    let entries = vec![
        session(project, 1, false, None),
        session(project, 2, true, None),
        session(project, 3, false, None),
        session(project, 4, false, Some(owner)),
    ];
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.receive_session_page(project, Ok(page(5, entries.clone())), cx);
        assert_eq!(model.existing_terminal_count(), 2);
    });
    cx.run_until_parked();
    let button = cx
        .debug_bounds("existing-terminals-button")
        .expect("availability button");
    let populated_width = cx.debug_bounds("tabs-scroll").expect("tabs").size.width;
    cx.simulate_click(button.center(), Modifiers::default());
    cx.run_until_parked();
    let picker = view.read_with(cx, |model, _| {
        let Some(Overlay::Sessions(picker)) = &model.overlay else {
            panic!("session modal");
        };
        picker.picker.clone()
    });
    view.update(cx, |model, cx| {
        model.receive_session_page(project, Ok(page(5, entries.clone())), cx);
    });
    cx.run_until_parked();
    let panel = cx
        .debug_bounds("project-terminals")
        .expect("compact terminal picker");
    let row = cx.debug_bounds("picker-row-3").expect("terminal row");
    let (width, row_height) = view.read_with(cx, |model, _| {
        (model.metrics.scaled(480.0), model.metrics.scaled(32.0))
    });
    assert_eq!(panel.size.width, width);
    assert_eq!(row.size.height, row_height);
    assert!(panel.size.height < px(160.0));
    picker.update(cx, |picker, cx| {
        assert!(picker.select_row("1", cx).is_err());
        assert!(picker.select_row("2", cx).is_err());
        assert!(picker.select_row("3", cx).is_ok());
        assert!(picker.select_row("4", cx).is_ok());
    });
    cx.simulate_input("TUI");
    cx.run_until_parked();
    picker.update(cx, |picker, cx| {
        assert!(picker.select_row("3", cx).is_err());
        assert!(picker.select_row("4", cx).is_ok());
    });
    let mut updated = entries;
    updated[3].owner = Some(SessionClient {
        kind: ClientKind::Desktop,
        ..owner
    });
    view.update(cx, |model, cx| {
        model.receive_session_page(project, Ok(page(6, updated)), cx);
    });
    picker.update(cx, |picker, cx| {
        assert_eq!(picker.query(), "TUI");
        assert!(picker.select_row("4", cx).is_err());
    });
    cx.simulate_keystrokes("cmd-a backspace");
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    view.read_with(cx, |model, _| {
        assert!(model.overlay.is_none());
        assert_eq!(model.state.current_project().tabs.len(), 2);
        assert_eq!(model.existing_terminal_count(), 1);
    });
    view.update(cx, |model, cx| {
        model.receive_session_page(project, Ok(page(7, vec![])), cx);
        assert_eq!(model.existing_terminal_count(), 0);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("tabs-scroll").expect("tabs").size.width > populated_width);
}

#[gpui::test]
fn session_availability_coalesces_refreshes_and_retries_changes_during_a_fetch(
    cx: &mut TestAppContext,
) {
    let state = AppState::bootstrap().expect("state");
    let project = state.home().id;
    let (boot, requests) = stub_boot(state);
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        model.connection = ConnectionState::Ready;
        model.refresh_existing_sessions(cx);
        model.receive_event(ClientEvent::SessionsChanged { revision: 5 }, cx);
        model.receive_event(ClientEvent::SessionsChanged { revision: 6 }, cx);
        assert_eq!(
            requests
                .try_iter()
                .filter(|(_, work)| matches!(work, Work::ProjectSessions { .. }))
                .count(),
            1
        );
        model.receive_session_page(
            project,
            Ok(page(5, vec![session(project, 1, false, None)])),
            cx,
        );
        assert_eq!(
            requests
                .try_iter()
                .filter(|(_, work)| matches!(work, Work::ProjectSessions { .. }))
                .count(),
            1
        );
        model.receive_session_page(project, Ok(page(6, vec![])), cx);
        assert!(
            requests
                .try_iter()
                .all(|(_, work)| !matches!(work, Work::ProjectSessions { .. }))
        );
        model.receive_session_page(
            ProjectId::new(),
            Ok(page(6, vec![session(project, 2, false, None)])),
            cx,
        );
        assert_eq!(model.existing_terminal_count(), 0);
        model.disconnect(cx);
        assert_eq!(model.existing_terminal_count(), 0);
    });
}

fn page(revision: u64, sessions: Vec<ProjectSession>) -> ProjectSessions {
    ProjectSessions {
        revision,
        sessions,
        next: None,
    }
}

#[gpui::test]
fn existing_terminals_shortcut_preserves_save_and_is_configurable_and_resettable(
    cx: &mut TestAppContext,
) {
    let state = AppState::bootstrap().expect("state");
    let project = state.home().id;
    let (boot, _requests) = stub_boot(state);
    assert_eq!(
        boot.settings
            .keymap
            .chord(muxy_core::shortcuts::ShortcutId::ExistingTerminals)
            .map(muxy_app_core::settings::KeyChord::as_str),
        Some("cmd-alt-t")
    );
    cx.update(|cx| crate::views::workspace::bind_keys(&boot.settings.keymap, cx));
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| model.focus_active(window, cx));
    });
    cx.simulate_keystrokes("cmd-s");
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    cx.simulate_keystrokes("cmd-alt-t");
    view.read_with(cx, |model, _| {
        assert!(
            matches!(&model.overlay, Some(Overlay::Sessions(picker)) if picker.project == project)
        );
    });
    cx.simulate_keystrokes("escape");
    view.update(cx, |model, cx| {
        assert!(model.overlay.is_none());
        model.change_preference(
            crate::views::settings::Change::Binding(
                "existing_terminals".into(),
                Some("cmd-shift-e".parse().expect("shortcut")),
            ),
            cx,
        );
    });
    cx.simulate_keystrokes("cmd-alt-t");
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    cx.simulate_keystrokes("cmd-shift-e");
    view.read_with(cx, |model, _| {
        assert!(
            matches!(&model.overlay, Some(Overlay::Sessions(picker)) if picker.project == project)
        );
    });
    cx.simulate_keystrokes("escape");
    view.update(cx, |model, cx| {
        assert!(model.overlay.is_none());
        model.change_preference(
            crate::views::settings::Change::Binding("existing_terminals".into(), None),
            cx,
        );
    });
    cx.simulate_keystrokes("cmd-shift-e");
    view.read_with(cx, |model, _| assert!(model.overlay.is_none()));
    cx.simulate_keystrokes("cmd-alt-t");
    view.read_with(cx, |model, _| {
        assert!(
            matches!(&model.overlay, Some(Overlay::Sessions(picker)) if picker.project == project)
        );
    });
}
