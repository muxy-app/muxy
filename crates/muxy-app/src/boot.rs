use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use muxy_core::worker::WorkerPool;
use std::thread;

use muxy_app_core::{AppState, PaneId, TabId, store};
use muxy_client::{Attachment, Client, ClientError, ClientEvent};
use muxy_protocol::{
    ChannelId, ErrorCode, ForegroundProcess, HistoryPage, MouseEvent, SavedScreen, SearchPage,
    SearchSource, SessionId, SessionInfo, Size, TerminalColors,
};

use crate::server::{ServerUpdate, UpdateMode, prepare_update};
use crate::views::terminal::find::SearchRequest;
use crate::views::terminal::scroll::HistoryRequest;

pub(crate) type Worker = Sender<(u64, Work)>;

#[derive(Debug)]
pub(crate) struct Boot {
    pub(crate) state: AppState,
    pub(crate) state_path: PathBuf,
    pub(crate) settings: muxy_app_core::settings::Settings,
    pub(crate) terminal: muxy_app_core::settings::TerminalSettings,
    pub(crate) work: Worker,
    pub(crate) updates: async_channel::Receiver<(u64, Update)>,
}

impl Boot {
    pub(crate) fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let state_path = store::default_path()?;
        let state = store::load(&state_path)?;
        let settings =
            muxy_app_core::settings::Settings::load(&state_path.with_file_name("settings.toml"))?;
        let terminal = muxy_app_core::settings::TerminalSettings::load(
            &state_path.with_file_name("ghostty.conf"),
        )?;
        let (work, updates) = bridge(state_path.with_file_name("server.sock"))?;
        work.send((1, Work::Connect))?;
        Ok(Self {
            state,
            state_path,
            settings,
            terminal,
            work,
            updates,
        })
    }
}

#[derive(Debug)]
pub(crate) enum Work {
    ReadActivity,
    OpenActivitySession {
        project: muxy_protocol::ProjectId,
        session: SessionId,
    },
    AcknowledgeActivity(Vec<u64>),
    ClaimActivity(Vec<u64>),
    Git(muxy_protocol::GitRequest),
    ProjectSessions {
        project: muxy_protocol::ProjectId,
    },
    ReadCatalog,
    CancelCreation(muxy_protocol::OperationId),
    MutateProject(muxy_protocol::ProjectIntent),
    Search {
        pane: PaneId,
        source: SearchSource,
        request: SearchRequest,
    },
    Connect,
    ReconnectAfterUpdate(u64),
    ReadServerSettings,
    WriteServerSettings(muxy_protocol::ServerSettingsDoc),
    StopServer {
        socket: PathBuf,
        restart: bool,
    },
    PrepareUpdate {
        socket: PathBuf,
        update: crate::updater::PreparedUpdate,
        mode: UpdateMode,
        server: muxy_protocol::ServerInfo,
    },
    CheckServerUpdate {
        socket: PathBuf,
        replace: bool,
    },
    Attach {
        pane: PaneId,
        project: muxy_protocol::ProjectId,
        session: Option<SessionId>,
        directory: PathBuf,
        size: Size,
    },
    References(Vec<SessionId>),
    Discard(SessionId, muxy_protocol::OperationId),
    CheckClose {
        tab: TabId,
        session: SessionId,
        size: Size,
    },
    ReadSaved {
        pane: PaneId,
        session: SessionId,
    },
    History {
        pane: PaneId,
        session: SessionId,
        channel: Option<ChannelId>,
        request: HistoryRequest,
    },
    EndAll(Vec<SessionId>),
    Flush,
    Completed(Box<Option<Update>>),
    Detach(ChannelId),
    Resize(ChannelId, Size),
    Colors(TerminalColors),
    Input(ChannelId, Vec<u8>),
    Mouse(ChannelId, MouseEvent),
    CellSize(ChannelId, muxy_protocol::CellSize),
    Ack(ChannelId, u64),
    Event(ClientEvent),
    Stop,
}

#[derive(Debug)]
pub(crate) enum Update {
    ActivitySession(Result<Option<muxy_protocol::ProjectSession>, ClientError>),
    Activity(Result<muxy_protocol::ActivitySnapshot, ClientError>),
    ActivityAcknowledged {
        ids: Vec<u64>,
        result: Result<(), ClientError>,
    },
    ActivityClaimed(Result<Vec<u64>, ClientError>),
    Git {
        request: muxy_protocol::GitRequest,
        result: Result<muxy_protocol::GitReply, ClientError>,
    },
    ProjectSessions {
        project: muxy_protocol::ProjectId,
        result: Result<muxy_protocol::ProjectSessions, ClientError>,
    },
    CreationCancelled {
        operation: muxy_protocol::OperationId,
        result: Result<(), ClientError>,
    },
    Catalog(Result<muxy_protocol::CatalogPage, ClientError>),
    ProjectMutated {
        operation: muxy_protocol::OperationId,
        result: Result<u64, ClientError>,
    },
    Search {
        pane: PaneId,
        request: SearchRequest,
        result: Result<SearchPage, ClientError>,
    },
    Connected(Vec<SessionInfo>),
    ServerSettings(Result<muxy_protocol::ServerSettingsDoc, ClientError>),
    ServerStopped {
        restart: bool,
        result: Result<(), ClientError>,
    },
    PreparedForInstall(Result<Option<std::fs::File>, ClientError>),
    ServerInfo(muxy_protocol::ServerInfo),
    ServerChecked(Result<ServerUpdate, ClientError>),
    ConnectFailed(String),
    Attached {
        pane: PaneId,
        session: SessionId,
        attachment: Attachment,
        created: bool,
    },
    AttachFailed {
        pane: PaneId,
        session: Option<SessionId>,
        created: bool,
        error: ClientError,
    },
    Saved {
        pane: PaneId,
        result: Result<SavedScreen, ClientError>,
    },
    ReferencesSynced(Result<(), ClientError>),
    Discarded {
        session: SessionId,
        result: Result<(), ClientError>,
    },
    CloseChecked {
        tab: TabId,
        session: SessionId,
        result: Result<Option<ForegroundProcess>, ClientError>,
    },
    History {
        pane: PaneId,
        request: HistoryRequest,
        result: Result<HistoryPage, ClientError>,
    },
    EndedAll(Result<(), ClientError>),
    Flushed,
    CloseSessionPanes(SessionId),
    Event(ClientEvent),
    Error(String),
}

fn bridge(socket: PathBuf) -> std::io::Result<(Worker, async_channel::Receiver<(u64, Update)>)> {
    let (sender, work) = mpsc::channel();
    let (updates, receiver) = async_channel::unbounded();
    let event_sender = sender.clone();
    let mut requests = requests::Requests::new()?;
    thread::Builder::new()
        .name("muxy-app-client".into())
        .spawn(move || {
            let mut client = None;
            let mut generation = 0;
            let mut delivery = delivery::Delivery::default();
            for (requested, work) in work {
                if matches!(work, Work::Stop) {
                    if let Some(client) = &client {
                        Client::disconnect(client);
                    }
                    break;
                }
                if matches!(work, Work::Completed(_)) {
                    requests.running = false;
                }
                let mut ready = if matches!(work, Work::Connect | Work::ReconnectAfterUpdate(_)) {
                    generation = requested;
                    delivery = delivery::Delivery::default();
                    requests.reset();
                    if let Some(client) = client.take() {
                        Client::disconnect(&client);
                    }
                    match connect(
                        &socket,
                        generation,
                        &event_sender,
                        match work {
                            Work::ReconnectAfterUpdate(instance) => Some(instance),
                            _ => None,
                        },
                    ) {
                        Ok((connected, sessions)) => {
                            let info = connected.server_info().clone();
                            client = Some(connected);
                            vec![Update::ServerInfo(info), Update::Connected(sessions)]
                        }
                        Err(error) => vec![Update::ConnectFailed(error.to_string())],
                    }
                } else if requested != generation {
                    Vec::new()
                } else {
                    match work {
                        Work::Event(event) => delivery.event(event).unwrap_or_else(|error| {
                            if let Some(client) = &client {
                                client.disconnect();
                            }
                            vec![Update::Error(error.into())]
                        }),
                        Work::Input(_, _)
                        | Work::Mouse(_, _)
                        | Work::CellSize(_, _)
                        | Work::Ack(_, _) => client
                            .as_ref()
                            .and_then(|client| perform(work, client))
                            .into_iter()
                            .collect(),
                        Work::Flush => delivery.flush(),
                        Work::Completed(update) => delivery.complete(*update),
                        work => client
                            .as_ref()
                            .and_then(|_| requests.push(work, &mut delivery))
                            .into_iter()
                            .collect(),
                    }
                };
                if ready
                    .iter()
                    .any(|update| matches!(update, Update::ReferencesSynced(Err(_))))
                    && let Some(client) = &client
                {
                    client.disconnect();
                }
                if let Some(client) = &client {
                    ready.extend(requests.start(client, generation, &event_sender, &mut delivery));
                }
                if ready
                    .into_iter()
                    .any(|update| updates.send_blocking((generation, update)).is_err())
                {
                    break;
                }
            }
        })?;
    Ok((sender, receiver))
}

fn connect(
    socket: &std::path::Path,
    generation: u64,
    sender: &Worker,
    after_update: Option<u64>,
) -> Result<(Client, Vec<SessionInfo>), ClientError> {
    let client = if let Some(instance) = after_update {
        crate::server::reconnect_after_update(socket, instance)?
    } else {
        crate::server::ensure_server_running(socket)?
    };
    client.identify(muxy_protocol::ClientKind::Desktop)?;
    let events = client
        .events()
        .ok_or_else(|| std::io::Error::other("client events already taken"))?;
    let sender = sender.clone();
    thread::Builder::new()
        .name("muxy-app-events".into())
        .spawn(move || {
            for event in events {
                if sender.send((generation, Work::Event(event))).is_err() {
                    break;
                }
            }
        })?;
    let sessions = client.list_sessions()?;
    Ok((client, sessions))
}

fn schedule(
    requests: &WorkerPool,
    work: Work,
    client: Client,
    generation: u64,
    completed: Worker,
) -> Option<Update> {
    let work = Arc::new(Mutex::new(Some(work)));
    let queued = Arc::clone(&work);
    requests
        .try_spawn(move || {
            let work = queued
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            let update = work.and_then(|work| perform(work, &client));
            let _ = completed.send((generation, Work::Completed(Box::new(update))));
        })
        .err()
        .and_then(|error| {
            work.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .map(|work| rejected(work, error.into()))
        })
}

fn rejected(work: Work, error: ClientError) -> Update {
    match work {
        Work::OpenActivitySession { .. } => Update::ActivitySession(Err(error)),
        Work::ReadActivity => Update::Activity(Err(error)),
        Work::AcknowledgeActivity(ids) => Update::ActivityAcknowledged {
            ids,
            result: Err(error),
        },
        Work::ClaimActivity(_) => Update::ActivityClaimed(Err(error)),
        Work::Git(request) => Update::Git {
            request,
            result: Err(error),
        },
        Work::ProjectSessions { project } => Update::ProjectSessions {
            project,
            result: Err(error),
        },
        Work::CancelCreation(operation) => Update::CreationCancelled {
            operation,
            result: Err(error),
        },
        Work::ReadCatalog => Update::Catalog(Err(error)),
        Work::MutateProject(intent) => Update::ProjectMutated {
            operation: intent.operation,
            result: Err(error),
        },
        Work::ReadServerSettings | Work::WriteServerSettings(_) => {
            Update::ServerSettings(Err(error))
        }
        Work::StopServer { restart, .. } => Update::ServerStopped {
            restart,
            result: Err(error),
        },
        Work::PrepareUpdate { .. } => Update::PreparedForInstall(Err(error)),
        Work::CheckServerUpdate { .. } => Update::ServerChecked(Err(error)),
        Work::Attach { pane, session, .. } => Update::AttachFailed {
            pane,
            session,
            created: false,
            error,
        },
        Work::ReadSaved { pane, .. } => Update::Saved {
            pane,
            result: Err(error),
        },
        Work::History { pane, request, .. } => Update::History {
            pane,
            request,
            result: Err(error),
        },
        Work::Search { pane, request, .. } => Update::Search {
            pane,
            request,
            result: Err(error),
        },
        Work::References(_) => Update::ReferencesSynced(Err(error)),
        Work::Discard(session, _) => Update::Discarded {
            session,
            result: Err(error),
        },
        Work::CheckClose { tab, session, .. } => Update::CloseChecked {
            tab,
            session,
            result: Err(error),
        },
        Work::EndAll(_) => Update::EndedAll(Err(error)),
        _ => Update::Error(error.to_string()),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "Keep work dispatch exhaustive in one place"
)]
fn perform(work: Work, client: &Client) -> Option<Update> {
    let result = match work {
        Work::OpenActivitySession { project, session } => {
            return Some(Update::ActivitySession(
                client.available_project_sessions(project).map(|page| {
                    page.sessions
                        .into_iter()
                        .find(|entry| entry.info.id == session)
                }),
            ));
        }
        Work::ReadActivity => return Some(Update::Activity(client.activity())),
        Work::AcknowledgeActivity(ids) => {
            return Some(Update::ActivityAcknowledged {
                result: client.acknowledge_activity(ids.clone()),
                ids,
            });
        }
        Work::ClaimActivity(ids) => {
            return Some(Update::ActivityClaimed(client.claim_activity(ids)));
        }
        Work::Git(request) => {
            let result = client.git(request.clone());
            return Some(Update::Git { request, result });
        }
        Work::ProjectSessions { project } => {
            return Some(Update::ProjectSessions {
                project,
                result: client.available_project_sessions(project),
            });
        }
        Work::CancelCreation(operation) => {
            return Some(Update::CreationCancelled {
                operation,
                result: client.cancel_creation(operation),
            });
        }
        Work::ReadCatalog => return Some(Update::Catalog(client.catalog())),
        Work::MutateProject(intent) => {
            return Some(Update::ProjectMutated {
                operation: intent.operation,
                result: client.mutate_project(intent),
            });
        }
        Work::ReadServerSettings => {
            return Some(Update::ServerSettings(client.read_server_settings()));
        }
        Work::WriteServerSettings(settings) => {
            return Some(write_server_settings(client, settings));
        }
        Work::StopServer { socket, restart } => {
            return Some(Update::ServerStopped {
                restart,
                result: crate::server::stop_server(client, &socket),
            });
        }
        Work::PrepareUpdate {
            socket,
            update,
            mode,
            server,
        } => {
            return Some(Update::PreparedForInstall(prepare_update(
                client, &socket, &update, mode, &server,
            )));
        }
        Work::CheckServerUpdate { socket, replace } => {
            return Some(Update::ServerChecked(crate::server::check_update(
                client, &socket, replace,
            )));
        }
        Work::Flush => return Some(Update::Flushed),
        Work::Search {
            pane,
            source,
            request,
        } => {
            return Some(search(client, pane, source, request));
        }
        Work::Attach {
            pane,
            project,
            session,
            directory,
            size,
        } => {
            return Some(attach(client, pane, project, session, &directory, size));
        }
        Work::ReadSaved { pane, session } => {
            return Some(Update::Saved {
                pane,
                result: client.read_saved_screen(session),
            });
        }
        Work::History {
            pane,
            session,
            channel,
            request,
        } => {
            return Some(history(client, pane, session, channel, request));
        }
        Work::References(sessions) => {
            let result = client.sync_session_references(sessions);
            if result.is_err() {
                client.disconnect();
            }
            return Some(Update::ReferencesSynced(result));
        }
        Work::Discard(session, operation) => {
            return Some(Update::Discarded {
                session,
                result: client.close_session(session, operation),
            });
        }
        Work::CheckClose { tab, session, size } => {
            return Some(Update::CloseChecked {
                tab,
                session,
                result: check_close(client, session, size),
            });
        }
        Work::EndAll(referenced) => return Some(Update::EndedAll(end_all(client, referenced))),
        Work::Detach(channel) => {
            let result = client.detach(channel);
            if matches!(&result, Err(ClientError::Server(reply)) if reply.code == ErrorCode::UnknownChannel)
            {
                return None;
            }
            result
        }
        Work::Resize(channel, size) => {
            let result = client.resize(channel, size);
            if matches!(&result, Err(ClientError::Server(reply)) if reply.code == ErrorCode::UnknownChannel)
            {
                return None;
            }
            result
        }
        Work::Colors(colors) => client.set_terminal_colors(colors),
        Work::Input(channel, bytes) => client.send_input(channel, &bytes),
        Work::Mouse(channel, event) => client.send_mouse(channel, event),
        Work::CellSize(channel, cell) => client.send_cell_size(channel, cell),
        Work::Ack(channel, seq) => client.ack(channel, seq),
        Work::Connect
        | Work::ReconnectAfterUpdate(_)
        | Work::Event(_)
        | Work::Stop
        | Work::Completed(_) => {
            return None;
        }
    };
    result.err().map(|error| Update::Error(error.to_string()))
}

fn write_server_settings(client: &Client, settings: muxy_protocol::ServerSettingsDoc) -> Update {
    Update::ServerSettings(
        client
            .write_server_settings(settings)
            .and_then(|()| client.read_server_settings()),
    )
}

fn search(client: &Client, pane: PaneId, source: SearchSource, request: SearchRequest) -> Update {
    let result = client.search(
        source,
        &request.query,
        request.ignore_case,
        request.before,
        500,
    );
    Update::Search {
        pane,
        request,
        result,
    }
}

fn history(
    client: &Client,
    pane: PaneId,
    session: SessionId,
    channel: Option<ChannelId>,
    request: HistoryRequest,
) -> Update {
    let result = match channel {
        Some(channel) => client.history_page(channel, request.before, request.max_rows),
        None => client.saved_history_page(session, request.before, request.max_rows),
    };
    Update::History {
        pane,
        request,
        result,
    }
}

fn end_all(client: &Client, referenced: Vec<SessionId>) -> Result<(), ClientError> {
    let mut sessions: HashSet<_> = client
        .list_sessions()?
        .into_iter()
        .map(|session| session.id)
        .collect();
    sessions.extend(referenced);
    for session in sessions {
        client.discard_session(session)?;
    }
    Ok(())
}

fn check_close(
    client: &Client,
    session: SessionId,
    size: Size,
) -> Result<Option<ForegroundProcess>, ClientError> {
    let attachment = client.attach(session, size)?;
    match client.detach(attachment.channel) {
        Err(ClientError::Server(error)) if error.code == ErrorCode::UnknownChannel => {}
        result => result?,
    }
    Ok(attachment.process)
}

fn attach(
    client: &Client,
    pane: PaneId,
    project: muxy_protocol::ProjectId,
    existing: Option<SessionId>,
    directory: &std::path::Path,
    size: Size,
) -> Update {
    let session = match existing.map_or_else(
        || {
            client
                .create_project_session(project, pane.creation_token(), directory, size)
                .map(|info| info.id)
        },
        Ok,
    ) {
        Ok(session) => session,
        Err(error) => {
            return Update::AttachFailed {
                pane,
                session: None,
                created: false,
                error,
            };
        }
    };
    match client.attach(session, size) {
        Ok(attachment) => Update::Attached {
            pane,
            session,
            attachment,
            created: existing.is_none(),
        },
        Err(error) => Update::AttachFailed {
            pane,
            session: Some(session),
            created: existing.is_none(),
            error,
        },
    }
}

pub(crate) fn missing_session(error: &ClientError) -> bool {
    matches!(error, ClientError::Server(reply) if reply.code == ErrorCode::UnknownSession)
}

mod delivery;
mod requests;

#[cfg(test)]
mod tests;
