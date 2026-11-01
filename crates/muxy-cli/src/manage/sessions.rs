use std::io::{self, Write};
use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::{Duration, Instant};

use muxy_app_core::browser::api;
use muxy_client::{Client, ClientError, ClientEvent, RunGrid};
use muxy_protocol::{
    ChannelId, ErrorCode, ExitReason, ProjectDescriptor, ProjectId, ProjectSession, SearchSource,
    SessionId, SessionStatus, Size,
};
use serde_json::{Value, json};

use super::args::{Session, Until};
use super::{Output, Paths, Result, path_text, projects};

const SHOW_WAIT: Duration = Duration::from_secs(10);
/// Each command attaches at most once, so one tag tells its terminal apart.
const ATTACH: u64 = 0;

pub(super) fn run(command: Session, client: &Client, output: &Output, paths: Paths) -> Result {
    match command {
        Session::List { project, all } => {
            list_projects(client, project.as_deref(), all, output, paths)
        }
        Session::Create {
            project,
            directory,
            size,
            command,
        } => create(
            client,
            project.as_deref(),
            directory.as_deref(),
            size,
            command,
            output,
            paths,
        ),
        Session::End(session) => {
            client.end_session(calling(client, session)?)?;
            output.ok()
        }
        Session::Discard(session) => {
            client.discard_session(calling(client, session)?)?;
            output.ok()
        }
        Session::Show(session) => show(client, calling(client, session)?, output, paths),
        Session::Send { session, text } => {
            input(client, calling(client, session)?, text.into_bytes(), output)
        }
        Session::Key { session, bytes } => input(client, calling(client, session)?, bytes, output),
        Session::Wait {
            session,
            until,
            timeout_ms,
        } => wait(
            client,
            calling(client, session)?,
            &until,
            Duration::from_millis(timeout_ms.into()),
            output,
        ),
        Session::Screen {
            session,
            lines,
            saved,
        } => screen(client, calling(client, session)?, lines, saved, output),
        Session::History {
            session,
            before,
            limit,
            saved,
        } => {
            let session = calling(client, session)?;
            let page = if saved {
                client.saved_history_page(session, before, limit)?
            } else {
                attached(client, session, |channel| {
                    Ok(client.history_page(channel, before, limit)?)
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
            let session = calling(client, session)?;
            let page = if saved {
                client.search(
                    SearchSource::Saved(session),
                    &query,
                    ignore_case,
                    before,
                    limit,
                )?
            } else {
                attached(client, session, |channel| {
                    Ok(client.search(
                        SearchSource::Live(channel),
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

/// `session`, or else the terminal this command runs in, when that terminal
/// belongs to the server `client` uses.
fn calling(client: &Client, session: Option<SessionId>) -> Result<SessionId> {
    match session {
        Some(session) => Ok(session),
        None => crate::target::hosting_session(client.catalog_page(None, None)?.server)
            .ok_or_else(|| "give a session ID; this isn't a Muxy terminal on that server".into()),
    }
}

fn show(client: &Client, session: SessionId, output: &Output, paths: Paths) -> Result {
    if paths == Paths::Remote {
        return Err(
            "session show uses the desktop app on this computer; it doesn't take --host".into(),
        );
    }
    let mut found = None;
    for project in client.catalog()?.projects {
        if let Some(entry) = list(client, project.id)?
            .into_iter()
            .find(|entry| entry.info.id == session)
        {
            found = Some(entry);
            break;
        }
    }
    let entry = found.ok_or_else(|| format!("session {} not found", session.get()))?;
    if !matches!(entry.status, SessionStatus::Live | SessionStatus::Starting) {
        return Err(format!("session {} has ended", session.get()).into());
    }
    let mut args = serde_json::Map::new();
    args.insert("session".into(), session.get().to_string().into());
    args.insert("project".into(), entry.info.project.to_string().into());
    show_when_ready(
        || crate::browser::call_app("session.show", args.clone(), true, Duration::ZERO),
        Instant::now,
    )?;
    if output.json {
        Output::json(&json!({"id":session.get().to_string(), "project_id":entry.info.project}))
    } else {
        output.ok()
    }
}

fn show_when_ready(
    mut call: impl FnMut() -> Result<Value>,
    mut now: impl FnMut() -> Instant,
) -> Result {
    let mut deadline = None;
    loop {
        match call() {
            Err(error) if error.to_string() == api::NOT_READY => {
                let now = now();
                if now >= *deadline.get_or_insert(now + SHOW_WAIT) {
                    return Err(error);
                }
                thread::sleep(Duration::from_millis(100));
            }
            reply => return reply.map(|_| ()),
        }
    }
}

fn screen(
    client: &Client,
    session: SessionId,
    lines: usize,
    saved: bool,
    output: &Output,
) -> Result {
    let grid = if saved {
        RunGrid::from_saved(client.read_saved_screen(session)?)
    } else {
        let events = client
            .events()
            .ok_or("this connection can't read a terminal")?;
        attached(client, session, |_| {
            Ok(muxy_client::take_attached(&events, ATTACH)?.terminal.grid)
        })?
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

/// The project of the terminal this command runs in.
fn own_project(client: &Client) -> Result<ProjectDescriptor> {
    let session = crate::target::hosting_session(client.catalog_page(None, None)?.server)
        .ok_or("give a project; this isn't a Muxy terminal on that server")?;
    let project = client
        .list_sessions()?
        .into_iter()
        .find(|info| info.id == session)
        .ok_or("this terminal's session is no longer running")?
        .project;
    Ok(client
        .catalog()?
        .projects
        .into_iter()
        .find(|candidate| candidate.id == project)
        .ok_or("this terminal's project no longer exists")?)
}

fn create(
    client: &Client,
    project: Option<&str>,
    directory: Option<&Path>,
    size: Size,
    command: Option<String>,
    output: &Output,
    paths: Paths,
) -> Result {
    let project = match project {
        Some(project) => projects::resolve(client, project, paths)?,
        None => own_project(client)?,
    };
    let directory = directory.map_or_else(
        || Ok(project.directory.as_path().to_path_buf()),
        |path| paths.directory(path),
    )?;
    let typed = command.map(|command| format!("{command}\r").into_bytes());
    if let Some(bytes) = &typed {
        muxy_protocol::validate_input(bytes)
            .map_err(|code| format!("invalid command: {code:?}"))?;
    }
    let session = client.create_project_session(
        project.id,
        muxy_protocol::OperationId::new(),
        &directory,
        size,
    )?;
    if let Some(bytes) = typed {
        attached(client, session.id, |channel| {
            Ok(client.write_input(channel, bytes)?)
        })
        .map_err(|error| {
            format!(
                "created session {}, but could not type the command: {error}",
                session.id.get()
            )
        })?;
    }
    output.record(
        &json!({"id":session.id.get().to_string(), "project_id":session.project,
        "directory":path_text(&session.directory), "directory_bytes":session.directory.0}),
        &["id"],
    )
}

fn list_projects(
    client: &Client,
    project: Option<&str>,
    all: bool,
    output: &Output,
    paths: Paths,
) -> Result {
    let projects = match project {
        Some(project) => vec![projects::resolve(client, project, paths)?],
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
    attached(client, session, |channel| {
        Ok(client.write_input(channel, bytes)?)
    })?;
    output.ok()
}

fn attached<T>(
    client: &Client,
    session: SessionId,
    action: impl FnOnce(ChannelId) -> Result<T>,
) -> Result<T> {
    let channel = client.attach_without_resize(ATTACH, session)?;
    let result = action(channel);
    let detached = client.detach(channel);
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

enum Waited {
    Line(String),
    Ended(ExitReason),
}

/// Follows the session's screen until `until` happens or `timeout` passes.
fn wait(
    client: &Client,
    session: SessionId,
    until: &Until,
    timeout: Duration,
    output: &Output,
) -> Result {
    let deadline = Instant::now() + timeout;
    let events = client
        .events()
        .ok_or("this connection can't follow a terminal")?;
    let channel = match client.attach_without_resize(ATTACH, session) {
        Err(ClientError::Server(error)) if error.code == ErrorCode::UnknownSession => {
            let saved = client.read_saved_screen(session)?;
            let waited = match until {
                Until::Exit => Waited::Ended(saved.reason.unwrap_or(ExitReason::Ended)),
                Until::Text { text, ignore_case } => saved_line(Some(saved), text, *ignore_case)?,
            };
            return report(session, &waited, output);
        }
        channel => channel?,
    };
    let mut terminal = muxy_client::take_attached(&events, ATTACH)?.terminal;
    let waited = loop {
        if let Until::Text { text, ignore_case } = until
            && let Some(line) = matching_line(&terminal.grid, text, *ignore_case)
        {
            break Ok(Waited::Line(line));
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break Err(format!("timed out after {}ms", timeout.as_millis()).into());
        }
        match events.recv_timeout(left) {
            Ok(ClientEvent::Frame {
                channel: received,
                frame,
            }) if received == channel => {
                let seq = terminal.apply(&frame);
                if let Err(error) = client.ack(channel, seq) {
                    break Err(error.into());
                }
            }
            Ok(ClientEvent::SessionEnded {
                session: ended,
                reason,
            }) if ended == session => {
                // The last output before an exit may never reach a frame.
                break match until {
                    Until::Exit => Ok(Waited::Ended(reason)),
                    Until::Text { text, ignore_case } => {
                        saved_line(client.read_saved_screen(session).ok(), text, *ignore_case)
                    }
                };
            }
            Ok(ClientEvent::Disconnected | ClientEvent::ServerRestarting)
            | Err(RecvTimeoutError::Disconnected) => {
                break Err("lost the connection to the server".into());
            }
            Ok(_) | Err(RecvTimeoutError::Timeout) => (),
        }
    };
    let detached = client.detach(channel);
    report(session, &finish_attachment(waited, detached)?, output)
}

/// The line containing `text` on an ended session's saved screen.
fn saved_line(
    saved: Option<muxy_protocol::SavedScreen>,
    text: &str,
    ignore_case: bool,
) -> Result<Waited> {
    saved
        .and_then(|saved| matching_line(&RunGrid::from_saved(saved), text, ignore_case))
        .map(Waited::Line)
        .ok_or_else(|| "the session ended before the text appeared".into())
}

fn report(session: SessionId, waited: &Waited, output: &Output) -> Result {
    let (record, text) = match waited {
        Waited::Line(line) => (
            json!({"id":session.get().to_string(), "line":line}),
            line.clone(),
        ),
        Waited::Ended(reason) => (
            json!({"id":session.get().to_string(), "ended":reason}),
            match reason {
                ExitReason::Exited(code) => format!("exited {code}"),
                ExitReason::Signaled(signal) => format!("signaled {signal}"),
                ExitReason::ServerStopped => "server stopped".into(),
                ExitReason::Ended | ExitReason::Unrecognized(_) => "ended".into(),
            },
        ),
    };
    if output.json {
        Output::json(&record)
    } else {
        writeln!(io::stdout(), "{text}")?;
        Ok(())
    }
}

fn matching_line(grid: &RunGrid, text: &str, ignore_case: bool) -> Option<String> {
    let needle = if ignore_case {
        text.to_lowercase()
    } else {
        text.to_owned()
    };
    screen_text(grid, grid.rows.len())
        .lines()
        .find(|line| {
            if ignore_case {
                line.to_lowercase().contains(&needle)
            } else {
                line.contains(&needle)
            }
        })
        .map(str::to_owned)
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
    use std::cell::Cell;

    use super::*;

    #[test]
    fn showing_waits_for_the_project_after_a_slow_app_start() -> Result {
        let now = Cell::new(Instant::now());
        let mut calls = 0;
        show_when_ready(
            || {
                calls += 1;
                let delay = if calls == 1 {
                    SHOW_WAIT + Duration::from_secs(1)
                } else {
                    Duration::from_secs(1)
                };
                now.set(now.get() + delay);
                if calls < 3 {
                    Err(api::NOT_READY.into())
                } else {
                    Ok(Value::Null)
                }
            },
            || now.get(),
        )?;
        assert_eq!(calls, 3);
        Ok(())
    }

    #[test]
    fn showing_stops_when_the_project_does_not_load() {
        let now = Cell::new(Instant::now());
        let mut calls = 0;
        let error = show_when_ready(
            || {
                calls += 1;
                assert!(calls <= 3, "project readiness must time out");
                now.set(now.get() + SHOW_WAIT / 2);
                Err(api::NOT_READY.into())
            },
            || now.get(),
        )
        .expect_err("project never loaded");
        assert_eq!(calls, 3);
        assert_eq!(error.to_string(), api::NOT_READY);
    }

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
}
