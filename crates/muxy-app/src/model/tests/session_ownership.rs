use super::*;
use muxy_protocol::{ProjectSession, ProjectSessions, SessionStatus};

fn session(project: ProjectId, id: u64) -> ProjectSession {
    ProjectSession {
        info: SessionInfo {
            id: SessionId::new(id).expect("session"),
            project,
            directory: muxy_protocol::ServerPath(b"/tmp/muxy".to_vec()),
        },
        status: SessionStatus::Live,
        attached: false,
        owner: None,
    }
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
        model.servers.local.connection = ConnectionState::Ready;
        model.refresh_existing_sessions(cx);
        model.receive_event(
            ServerId::local(),
            ClientEvent::SessionsChanged { revision: 5 },
            cx,
        );
        model.receive_event(
            ServerId::local(),
            ClientEvent::SessionsChanged { revision: 6 },
            cx,
        );
        assert_eq!(
            requests
                .try_iter()
                .filter(|(_, work)| matches!(work, Work::ProjectSessions { .. }))
                .count(),
            1
        );
        model.receive_session_page(project, Ok(page(5, vec![session(project, 1)])), cx);
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
        model.receive_session_page(ProjectId::new(), Ok(page(6, vec![session(project, 2)])), cx);
        assert_eq!(model.existing_terminal_count(), 0);
        model.disconnect(ServerId::local(), cx);
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
