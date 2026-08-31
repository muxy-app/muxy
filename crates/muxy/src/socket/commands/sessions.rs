use super::CommandResult;
use crate::sessions::SessionCoordinator;
use muxy_core::workspace_store::WorkspaceStore;
use muxy_proto::session::{SessionDescriptor, SessionId, SessionOwner};

trait SessionCommandTarget {
    fn list(&mut self) -> Result<Vec<SessionDescriptor>, String>;
    fn end(&mut self, session_id: SessionId, expected_owner: &SessionOwner) -> Result<(), String>;
}

struct CoordinatorTarget<'a> {
    coordinator: &'a mut SessionCoordinator,
    store: &'a mut WorkspaceStore,
}

impl SessionCommandTarget for CoordinatorTarget<'_> {
    fn list(&mut self) -> Result<Vec<SessionDescriptor>, String> {
        self.coordinator.live_session_descriptors()
    }

    fn end(&mut self, session_id: SessionId, expected_owner: &SessionOwner) -> Result<(), String> {
        self.coordinator
            .end_session(session_id, expected_owner, self.store)
    }
}

pub fn handle(
    head: &str,
    parts: &[&str],
    coordinator: &mut SessionCoordinator,
    store: &mut WorkspaceStore,
) -> Option<CommandResult> {
    let mut target = CoordinatorTarget { coordinator, store };
    handle_target(head, parts, &mut target)
}

fn handle_target(
    head: &str,
    parts: &[&str],
    target: &mut impl SessionCommandTarget,
) -> Option<CommandResult> {
    let result = match head {
        "list-sessions" => list(target).map(CommandResult::reply),
        "kill-session" => kill(parts, target).map(CommandResult::changed),
        _ => return None,
    };
    Some(result.unwrap_or_else(|error| CommandResult::reply(format!("error:{error}"))))
}

fn list(target: &mut impl SessionCommandTarget) -> Result<String, String> {
    Ok(target
        .list()?
        .into_iter()
        .map(|session| {
            let placement = session.placement.as_ref();
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                session.session_id,
                session.shell.process_id,
                session.working_directory,
                placement.is_some(),
                session.title,
                session.owner.project_id,
                session.owner.worktree_id,
                placement.map_or("", |placement| placement.tab_id.as_str()),
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn kill(parts: &[&str], target: &mut impl SessionCommandTarget) -> Result<&'static str, String> {
    let Some(value) = parts.get(1).copied().filter(|value| !value.is_empty()) else {
        return Err("usage kill-session|sessionID".to_owned());
    };
    let session_id = SessionId::parse(value).map_err(|_| "invalid pane ID".to_owned())?;
    let descriptor = target
        .list()?
        .into_iter()
        .find(|session| session.session_id == session_id)
        .ok_or_else(|| format!("pane not found {value}"))?;
    target.end(session_id, &descriptor.owner)?;
    Ok("ok")
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_proto::session::{ProcessIdentity, SessionStatus, WorkspacePlacement};

    struct FakeTarget {
        sessions: Result<Vec<SessionDescriptor>, String>,
        end_error: Option<String>,
        ended: Vec<(SessionId, SessionOwner)>,
    }

    impl SessionCommandTarget for FakeTarget {
        fn list(&mut self) -> Result<Vec<SessionDescriptor>, String> {
            self.sessions.clone()
        }

        fn end(
            &mut self,
            session_id: SessionId,
            expected_owner: &SessionOwner,
        ) -> Result<(), String> {
            if let Some(error) = &self.end_error {
                return Err(error.clone());
            }
            self.ended.push((session_id, expected_owner.clone()));
            Ok(())
        }
    }

    fn descriptor(value: &str, attached: bool) -> SessionDescriptor {
        let owner = SessionOwner {
            project_id: "PROJECT".into(),
            worktree_id: "WORKTREE".into(),
            original_tab_id: "ORIGINAL".into(),
        };
        SessionDescriptor {
            session_id: SessionId::parse(value).unwrap(),
            owner: owner.clone(),
            placement: attached.then(|| WorkspacePlacement {
                project_id: owner.project_id.clone(),
                worktree_id: owner.worktree_id.clone(),
                tab_id: "TAB".into(),
                area_id: "AREA".into(),
            }),
            title: "Shell".into(),
            working_directory: "/tmp/project".into(),
            shell: ProcessIdentity {
                process_id: 42,
                start_identity: 99,
            },
            process_session_id: 42,
            process_group_id: 42,
            tty_device: 1,
            created_at_milliseconds: 1,
            renderer_attached: false,
            status: SessionStatus::Running,
        }
    }

    fn target(sessions: Vec<SessionDescriptor>) -> FakeTarget {
        FakeTarget {
            sessions: Ok(sessions),
            end_error: None,
            ended: Vec::new(),
        }
    }

    #[test]
    fn socket_list_sessions_preserves_exact_columns_and_placement_attachment() {
        let first = descriptor("123E4567-E89B-12D3-A456-426614174000", true);
        let second = descriptor("223E4567-E89B-12D3-A456-426614174000", false);
        let mut target = target(vec![first, second]);
        let result = handle_target("list-sessions", &["list-sessions"], &mut target).unwrap();
        assert_eq!(
            result.reply,
            "123E4567-E89B-12D3-A456-426614174000\t42\t/tmp/project\ttrue\tShell\tPROJECT\tWORKTREE\tTAB\n223E4567-E89B-12D3-A456-426614174000\t42\t/tmp/project\tfalse\tShell\tPROJECT\tWORKTREE\t"
        );
        assert!(!result.changed);
    }

    #[test]
    fn socket_kill_session_pins_usage_invalid_not_found_and_success() {
        let id = "123E4567-E89B-12D3-A456-426614174000";
        let mut target = target(vec![descriptor(id, true)]);
        assert_eq!(
            handle_target("kill-session", &["kill-session"], &mut target)
                .unwrap()
                .reply,
            "error:usage kill-session|sessionID"
        );
        assert_eq!(
            handle_target("kill-session", &["kill-session", "nope"], &mut target)
                .unwrap()
                .reply,
            "error:invalid pane ID"
        );
        assert_eq!(
            handle_target(
                "kill-session",
                &["kill-session", "223E4567-E89B-12D3-A456-426614174000"],
                &mut target,
            )
            .unwrap()
            .reply,
            "error:pane not found 223E4567-E89B-12D3-A456-426614174000"
        );
        let result = handle_target("kill-session", &["kill-session", id], &mut target).unwrap();
        assert_eq!(result.reply, "ok");
        assert!(result.changed);
        assert_eq!(target.ended.len(), 1);
        assert_eq!(target.ended[0].0, SessionId::parse(id).unwrap());
    }

    #[test]
    fn socket_kill_session_pins_owner_mismatch_and_daemon_unavailable() {
        let id = "123E4567-E89B-12D3-A456-426614174000";
        let mut mismatch = target(vec![descriptor(id, true)]);
        mismatch.end_error = Some(format!("session owner mismatch {id}"));
        assert_eq!(
            handle_target("kill-session", &["kill-session", id], &mut mismatch)
                .unwrap()
                .reply,
            format!("error:session owner mismatch {id}")
        );
        let mut unavailable = FakeTarget {
            sessions: Err("persistent session runtime is unavailable".into()),
            end_error: None,
            ended: Vec::new(),
        };
        assert_eq!(
            handle_target("list-sessions", &["list-sessions"], &mut unavailable)
                .unwrap()
                .reply,
            "error:persistent session runtime is unavailable"
        );
        assert_eq!(
            handle_target("kill-session", &["kill-session", id], &mut unavailable)
                .unwrap()
                .reply,
            "error:persistent session runtime is unavailable"
        );
    }

    #[test]
    fn socket_kill_session_reports_a_real_missing_runtime() {
        let temp = tempfile::Builder::new()
            .prefix("p8-isolated-test-")
            .tempdir_in("/tmp")
            .unwrap();
        let mut store = WorkspaceStore::load_from(temp.path().join("workspaces.json"));
        let mode = muxy_core::environment::BuildMode::Development;
        let main_socket = temp
            .path()
            .join(muxy_core::environment::RuntimePathPolicy::new(mode).main_socket_filename());
        let mut coordinator = SessionCoordinator::start(
            mode,
            temp.path(),
            &temp.path().join("MuxyTests"),
            &main_socket,
            false,
            &[],
            &mut store,
        );
        let result = handle(
            "kill-session",
            &["kill-session", "123E4567-E89B-12D3-A456-426614174000"],
            &mut coordinator,
            &mut store,
        )
        .unwrap();
        assert_eq!(
            result.reply,
            "error:persistent session runtime is unavailable"
        );
        assert!(!result.changed);
    }
}
