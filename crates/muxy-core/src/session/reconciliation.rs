use muxy_proto::session::SessionId;
use muxy_proto::session::messages::{
    CreateSessionRequest, CreateSessionResolution, MessageValidationError, SessionOwner,
    resolve_create_session,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkedSessionState {
    Present,
    MissingOrEnded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionReconciliationError {
    SessionOwnerMismatch,
    DuplicateSessionId,
    Protocol(MessageValidationError),
}

pub fn reconcile_linked_session(
    session_id: SessionId,
    owner: &SessionOwner,
    current: &[CreateSessionRequest],
) -> Result<LinkedSessionState, SessionReconciliationError> {
    let matches: Vec<_> = current
        .iter()
        .filter(|request| request.session_id == session_id)
        .collect();
    match matches.as_slice() {
        [] => Ok(LinkedSessionState::MissingOrEnded),
        [request] if &request.owner == owner => Ok(LinkedSessionState::Present),
        [_] => Err(SessionReconciliationError::SessionOwnerMismatch),
        _ => Err(SessionReconciliationError::DuplicateSessionId),
    }
}

pub fn reconcile_unlinked_session(
    proposed: &CreateSessionRequest,
    current: &[CreateSessionRequest],
) -> Result<CreateSessionResolution, SessionReconciliationError> {
    resolve_create_session(current, proposed).map_err(SessionReconciliationError::Protocol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_proto::session::messages::{EnvironmentEntry, WorkspacePlacement};
    use muxy_proto::session::window_size::WindowSize;

    fn request(session_id: &str, owner_tab: &str) -> CreateSessionRequest {
        CreateSessionRequest {
            session_id: SessionId::parse(session_id).unwrap(),
            owner: SessionOwner {
                project_id: "project".into(),
                worktree_id: "worktree".into(),
                original_tab_id: owner_tab.into(),
            },
            placement: Some(WorkspacePlacement {
                project_id: "project".into(),
                worktree_id: "worktree".into(),
                tab_id: owner_tab.into(),
                area_id: "area".into(),
            }),
            working_directory: "/workspace".into(),
            initial_size: WindowSize::new(80, 24),
            shell_executable: "/bin/zsh".into(),
            argv: vec!["-l".into()],
            startup_command: None,
            keep_shell_open: false,
            environment: vec![EnvironmentEntry {
                key: "TERM".into(),
                value: "xterm-ghostty".into(),
            }],
            ghostty_resources: "/resources".into(),
            terminfo: "/terminfo".into(),
            terminal_type: "xterm-ghostty".into(),
            color_terminal: "truecolor".into(),
            title: "Terminal".into(),
        }
    }

    #[test]
    fn linked_reconciliation_uses_exact_session_and_owner_ids() {
        let held = request("123E4567-E89B-12D3-A456-426614174000", "tab");
        assert_eq!(
            reconcile_linked_session(held.session_id, &held.owner, std::slice::from_ref(&held)),
            Ok(LinkedSessionState::Present)
        );
        assert_eq!(
            reconcile_linked_session(held.session_id, &held.owner, &[]),
            Ok(LinkedSessionState::MissingOrEnded)
        );
        let wrong = request("123E4567-E89B-12D3-A456-426614174000", "other");
        assert_eq!(
            reconcile_linked_session(held.session_id, &held.owner, &[wrong]),
            Err(SessionReconciliationError::SessionOwnerMismatch)
        );
    }

    #[test]
    fn unlinked_reconciliation_recovers_only_current_exact_owner_contract() {
        let held = request("123E4567-E89B-12D3-A456-426614174000", "tab");
        let proposed = request("223E4567-E89B-12D3-A456-426614174000", "tab");
        assert_eq!(
            reconcile_unlinked_session(&proposed, std::slice::from_ref(&held)),
            Ok(CreateSessionResolution::Existing(held.session_id))
        );
        let other = request("323E4567-E89B-12D3-A456-426614174000", "other");
        assert_eq!(
            reconcile_unlinked_session(&proposed, &[other]),
            Ok(CreateSessionResolution::Create)
        );
        let mut conflict = held.clone();
        conflict.argv.push("different".into());
        assert_eq!(
            reconcile_unlinked_session(&conflict, &[held]),
            Ok(CreateSessionResolution::DuplicateOwnerConflict)
        );
    }
}
