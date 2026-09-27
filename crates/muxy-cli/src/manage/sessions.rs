use std::io::{self, Write};

use muxy_client::{Attachment, Client, ClientError, RunGrid};
use muxy_protocol::{ErrorCode, ProjectId, ProjectSession, SearchSource, SessionId, SessionStatus};
use serde_json::{Value, json};

use super::args::Session;
use super::{Output, Result, absolute, local_path, path_text, projects};

pub(super) fn run(command: Session, client: &Client, output: &Output) -> Result {
    match command {
        Session::List { project, all } => list_projects(client, project.as_deref(), all, output),
        Session::Create {
            project,
            directory,
            size,
        } => {
            let project = projects::resolve(client, &project)?;
            let directory = directory.map_or_else(
                || Ok(local_path(&project.directory)),
                |path| absolute(&path),
            )?;
            let session = client.create_project_session(
                project.id,
                muxy_protocol::OperationId::new(),
                &directory,
                size,
            )?;
            output.record(
                &json!({"id":session.id.get().to_string(), "project_id":session.project,
                "directory":path_text(&session.directory), "directory_bytes":session.directory.0}),
                &["id"],
            )
        }
        Session::End(session) => {
            client.end_session(session)?;
            output.ok()
        }
        Session::Discard(session) => {
            client.discard_session(session)?;
            output.ok()
        }
        Session::Send { session, text } => input(client, session, text.into_bytes(), output),
        Session::Key { session, bytes } => input(client, session, bytes, output),
        Session::Screen {
            session,
            lines,
            saved,
        } => {
            let grid = if saved {
                RunGrid::from_saved(client.read_saved_screen(session)?)
            } else {
                attached(client, session, |attachment| Ok(attachment.grid.clone()))?
            };
            let text = screen_text(&grid, lines);
            if output.json {
                Output::json(
                    &json!({"id":session.get().to_string(), "text":text, "size":grid.size, "cursor":grid.cursor, "saved":saved}),
                )
            } else {
                writeln!(io::stdout(), "{text}")?;
                Ok(())
            }
        }
        Session::History {
            session,
            before,
            limit,
            saved,
        } => {
            let page = if saved {
                client.saved_history_page(session, before, limit)?
            } else {
                attached(client, session, |attachment| {
                    Ok(client.history_page(attachment.channel, before, limit)?)
                })?
            };
            Output::json(&super::records::history(&page))
        }
        Session::Search {
            session,
            query,
            before,
            limit,
            ignore_case,
            saved,
        } => {
            let page = if saved {
                client.search(
                    SearchSource::Saved(session),
                    &query,
                    ignore_case,
                    before,
                    limit,
                )?
            } else {
                attached(client, session, |attachment| {
                    Ok(client.search(
                        SearchSource::Live(attachment.channel),
                        &query,
                        ignore_case,
                        before,
                        limit,
                    )?)
                })?
            };
            Output::json(&super::records::search(&page))
        }
    }
}

fn list_projects(client: &Client, project: Option<&str>, all: bool, output: &Output) -> Result {
    let projects = match project {
        Some(project) => vec![projects::resolve(client, project)?],
        None => client.catalog()?.projects,
    };
    let mut records = Vec::new();
    for project in projects {
        records.extend(
            list(client, project.id)?
                .iter()
                .filter(|session| all || session.status == SessionStatus::Live)
                .map(record),
        );
    }
    output.list(
        &records,
        &["id", "project_id", "status", "attached", "directory"],
    )
}

fn record(session: &ProjectSession) -> Value {
    json!({"id":session.info.id.get().to_string(), "project_id":session.info.project,
        "directory":path_text(&session.info.directory), "directory_bytes":session.info.directory.0,
        "status":session.status, "attached":session.owner.is_some(), "owner":session.owner})
}

fn list(client: &Client, project: ProjectId) -> Result<Vec<ProjectSession>> {
    for _ in 0..8 {
        match snapshot(client, project) {
            Err(ClientError::Server(error)) if error.code == ErrorCode::CatalogChanged => (),
            result => return Ok(result?),
        }
    }
    Err("session catalog kept changing; try again".into())
}

fn snapshot(
    client: &Client,
    project: ProjectId,
) -> std::result::Result<Vec<ProjectSession>, ClientError> {
    let mut page = client.project_sessions(project, None, None)?;
    let mut sessions = page.sessions;
    while let Some(after) = page.next {
        page = client.project_sessions(project, Some(after), Some(page.revision))?;
        if page.next.is_some_and(|next| next <= after)
            || sessions.len() + page.sessions.len() > 16_384
        {
            return Err(ClientError::Protocol("invalid session pagination".into()));
        }
        sessions.extend(page.sessions);
    }
    Ok(sessions)
}

fn input(client: &Client, session: SessionId, bytes: Vec<u8>, output: &Output) -> Result {
    muxy_protocol::validate_input(&bytes).map_err(|code| format!("invalid input: {code:?}"))?;
    attached(client, session, |attachment| {
        Ok(client.write_input(attachment.channel, bytes)?)
    })?;
    output.ok()
}

fn attached<T>(
    client: &Client,
    session: SessionId,
    action: impl FnOnce(&Attachment) -> Result<T>,
) -> Result<T> {
    let attachment = client.attach_without_resize(session)?;
    let result = action(&attachment);
    let detached = client.detach(attachment.channel);
    finish_attachment(result, detached)
}

fn finish_attachment<T>(
    result: Result<T>,
    detached: std::result::Result<(), ClientError>,
) -> Result<T> {
    let result = result?;
    match detached {
        Ok(()) => (),
        Err(ClientError::Server(error)) if error.code == ErrorCode::UnknownChannel => (),
        Err(error) => return Err(error.into()),
    }
    Ok(result)
}

fn screen_text(grid: &RunGrid, lines: usize) -> String {
    grid.rows
        .iter()
        .skip(grid.rows.len().saturating_sub(lines))
        .map(|runs| {
            runs.iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_protocol::Size;
    #[test]
    fn acknowledged_success_survives_terminal_exit_during_detach() -> Result {
        let gone = ClientError::Server(muxy_protocol::ErrorReply {
            code: ErrorCode::UnknownChannel,
            message: "terminal exited".into(),
        });
        assert_eq!(finish_attachment(Ok(42), Err(gone))?, 42);
        let error =
            finish_attachment::<()>(Err("input failed".into()), Err(ClientError::Disconnected))
                .expect_err("action error");
        assert_eq!(error.to_string(), "input failed");
        assert!(finish_attachment(Ok(()), Err(ClientError::Disconnected)).is_err());
        assert!(
            finish_attachment(
                Ok(()),
                Err(ClientError::Server(muxy_protocol::ErrorReply {
                    code: ErrorCode::Unauthorized,
                    message: "denied".into(),
                }))
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn screen_does_not_include_scrollback() {
        let mut grid = RunGrid::from_saved(muxy_protocol::SavedScreen {
            graphics: muxy_protocol::Graphics::default(),
            size: Size { cols: 80, rows: 2 },
            rows: Vec::new(),
            cursor: muxy_protocol::Cursor {
                shape: muxy_protocol::CursorShape::Block,
                row: 0,
                col: 0,
                visible: true,
            },
            reason: None,
        });
        grid.history.push_back(muxy_protocol::Row {
            index: 0,
            runs: Vec::new(),
        });
        let text = screen_text(&grid, 1);
        assert!(!text.contains('\n'));
        assert_eq!(
            text,
            grid.rows
                .last()
                .into_iter()
                .flatten()
                .map(|run| run.text.as_str())
                .collect::<String>()
                .trim_end()
        );
    }
}
