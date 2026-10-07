use super::*;
use muxy_protocol::{ClientKind, ProjectSession, SessionClient, SessionId};

#[test]
fn availability_search_skips_attached_pages_and_restarts_after_invalidation() -> TestResult {
    use muxy_protocol::{
        ProjectId, ProjectSessions, ReplyBody, RequestBody, ServerPath, SessionInfo, SessionStatus,
    };
    let project = ProjectId::new();
    let owner = SessionClient {
        kind: ClientKind::Tui,
        ..SessionClient::default()
    };
    let (socket, server) = UnixStream::pair()?;
    let serving = thread::spawn(move || -> Result<(), String> {
        let mut decoder = Decoder::new(server.try_clone().map_err(|e| e.to_string())?);
        let mut encoder = Encoder::new(server);
        assert!(matches!(
            decoder.next().map_err(|e| e.to_string())?.1,
            Message::Hello { .. }
        ));
        encoder
            .send(
                CONTROL,
                &Message::HelloReply {
                    versions: muxy_protocol::SUPPORTED.to_vec(),
                    server: muxy_protocol::ServerInfo::current(),
                    features: Vec::new(),
                },
            )
            .map_err(|e| e.to_string())?;
        for (index, expected_after) in [None, SessionId::new(128), None, SessionId::new(128)]
            .into_iter()
            .enumerate()
        {
            let (_, message) = decoder.next().map_err(|e| e.to_string())?;
            let Message::Request {
                id,
                body:
                    RequestBody::ListProjectSessions {
                        project: received,
                        after,
                        revision,
                    },
            } = message
            else {
                return Err("expected listing".into());
            };
            assert_eq!(received, project);
            assert_eq!(after, expected_after);
            assert_eq!(revision, after.map(|_| if index == 1 { 1 } else { 2 }));
            let body = if index == 1 {
                ReplyBody::Error(muxy_protocol::ErrorReply {
                    code: ErrorCode::CatalogChanged,
                    message: "changed".into(),
                })
            } else {
                let attached = after.is_none();
                let range = if attached { 1..=128 } else { 129..=130 };
                ReplyBody::ProjectSessions(ProjectSessions {
                    revision: if index == 0 { 1 } else { 2 },
                    sessions: range
                        .map(|id| ProjectSession {
                            info: SessionInfo {
                                id: SessionId::new(id).expect("id"),
                                project,
                                directory: ServerPath(b"/tmp".to_vec()),
                            },
                            status: SessionStatus::Live,
                            attached,
                            owner: (id != 130).then_some(owner),
                        })
                        .collect(),
                    next: attached.then(|| SessionId::new(128).expect("id")),
                })
            };
            encoder
                .send(CONTROL, &Message::Reply { id, body })
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    });
    let client = Client::from_stream(Box::new(socket))?;
    let page = client.available_project_sessions(project)?;
    assert_eq!(page.revision, 2);
    assert_eq!(
        page.sessions
            .iter()
            .map(|entry| entry.info.id.get())
            .collect::<Vec<_>>(),
        [129, 130]
    );
    assert_eq!(page.sessions[0].owner, Some(owner));
    assert_eq!(page.sessions[1].owner, None);
    assert!(page.next.is_none());
    serving.join().map_err(|_| "server panicked")??;
    Ok(())
}
