use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use muxy_core::worker::WorkerPool;
use std::thread;

use muxy_app_core::{AppState, PaneId, ServerId, TabId, store};
use muxy_client::{Attachment, Client, ClientError, ClientEvent, SshTarget, Start};
use muxy_protocol::{
    ChannelId, ErrorCode, ForegroundProcess, HistoryPage, MouseEvent, SavedScreen, SearchPage,
    SearchSource, SessionId, SessionInfo, Size, TerminalColors,
};

use crate::server::{ServerUpdate, UpdateMode, prepare_update};
use crate::views::terminal::find::SearchRequest;
use crate::views::terminal::scroll::HistoryRequest;

pub(crate) type Worker = Sender<(u64, Work)>;
/// What every server's worker sends back, tagged with its server and generation.
pub(crate) type Updates = async_channel::Receiver<(ServerId, u64, Update)>;

#[derive(Debug)]
pub(crate) struct Boot {
    pub(crate) import_error: Option<String>,
    pub(crate) composer: muxy_app_core::composer::ComposerStore,
    pub(crate) state: AppState,
    pub(crate) state_path: PathBuf,
    pub(crate) settings: muxy_app_core::settings::Settings,
    pub(crate) terminal: muxy_app_core::settings::TerminalSettings,
    /// This computer's server, already connecting.
    pub(crate) work: Worker,
    pub(crate) workers: Workers,
    pub(crate) updates: Updates,
}

impl Boot {
    pub(crate) fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let state_path = store::default_path()?;
        crate::diagnostics::init(
            state_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
        );
        let import_error =
            crate::backup::apply_pending(state_path.parent().ok_or("Missing profile directory")?)?;
        let state = store::load(&state_path)?;
        let settings =
            muxy_app_core::settings::Settings::load(&state_path.with_file_name("settings.toml"))?;
        let terminal = muxy_app_core::settings::TerminalSettings::load(
            &state_path.with_file_name("ghostty.conf"),
        )?;
        settings.validate_command_shortcuts(&terminal)?;
        let (sender, updates) = async_channel::unbounded();
        let workers = Workers::new(sender);
        let work = workers.start(
            ServerId::local(),
            Target::Local(state_path.with_file_name("server.sock")),
        )?;
        work.send((1, Work::Connect))?;
        Ok(Self {
            import_error,
            composer: muxy_app_core::composer::ComposerStore::load_from(
                state_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(".")),
            ),
            state,
            state_path,
            settings,
            terminal,
            work,
            workers,
            updates,
        })
    }
}

/// How a worker reaches its server.
#[derive(Clone, Debug)]
pub(crate) enum Target {
    /// This computer's server, at the socket in the profile.
    Local(PathBuf),
    /// The server on another computer, reached over SSH.
    Ssh(SshTarget),
}

impl Target {
    /// Another connection to the server the worker reached. It never starts one.
    fn connect(&self) -> Result<Client, ClientError> {
        match self {
            Self::Local(socket) => Client::connect(socket),
            Self::Ssh(host) => Client::connect_ssh(host, Start::Never),
        }
    }

    fn socket(&self) -> Result<&std::path::Path, ClientError> {
        match self {
            Self::Local(socket) => Ok(socket),
            Self::Ssh(_) => {
                Err(std::io::Error::other("Updates apply only to this computer's server").into())
            }
        }
    }
}

/// Checks that another computer's server answers over SSH, and reports its
/// version or why it doesn't.
pub(crate) type Probe = Arc<dyn Fn(&SshTarget) -> Result<String, ProbeFailure> + Send + Sync>;

#[derive(Debug)]
pub(crate) enum ProbeFailure {
    NotInstalled(String),
    Other(String),
}

impl From<String> for ProbeFailure {
    fn from(message: String) -> Self {
        Self::Other(message)
    }
}

impl From<&str> for ProbeFailure {
    fn from(message: &str) -> Self {
        Self::Other(message.into())
    }
}

/// Runs a command on another computer over SSH, such as an installer, and
/// returns what it printed, or why it failed.
pub(crate) type Run =
    Arc<dyn Fn(&SshTarget, &str, Option<&std::path::Path>) -> Result<String, String> + Send + Sync>;

/// Starts one worker per server.
pub(crate) struct Workers {
    start: Box<dyn Fn(ServerId, Target) -> std::io::Result<Worker>>,
    probe: Probe,
    run: Run,
}

impl Workers {
    fn new(updates: async_channel::Sender<(ServerId, u64, Update)>) -> Self {
        Self {
            start: Box::new(move |server, target| worker(server, target, updates.clone())),
            probe: Arc::new(probe),
            run: Arc::new(run),
        }
    }

    /// Workers from `start`, with a probe that never reaches a network.
    #[cfg(test)]
    pub(crate) fn with(
        start: impl Fn(ServerId, Target) -> std::io::Result<Worker> + 'static,
    ) -> Self {
        Self {
            start: Box::new(start),
            probe: Arc::new(|_| Err("SSH is unavailable in tests".into())),
            run: Arc::new(|_, _, _| Err("SSH is unavailable in tests".into())),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_run(
        self,
        run: impl Fn(&SshTarget, &str) -> Result<String, String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            run: Arc::new(move |target, command, _| run(target, command)),
            ..self
        }
    }

    #[cfg(test)]
    pub(crate) fn with_run_input(
        self,
        run: impl Fn(&SshTarget, &str, Option<&std::path::Path>) -> Result<String, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            run: Arc::new(run),
            ..self
        }
    }

    #[cfg(test)]
    pub(crate) fn with_probe(
        self,
        probe: impl Fn(&SshTarget) -> Result<String, ProbeFailure> + Send + Sync + 'static,
    ) -> Self {
        Self {
            probe: Arc::new(probe),
            ..self
        }
    }

    pub(crate) fn start(&self, server: ServerId, target: Target) -> std::io::Result<Worker> {
        (self.start)(server, target)
    }

    pub(crate) fn probe(&self) -> Probe {
        self.probe.clone()
    }

    pub(crate) fn run(&self) -> Run {
        self.run.clone()
    }
}

/// Runs `remote` there, waits for it, and returns its output. On failure,
/// the last line of its error output, else of its output, says why.
fn run(
    host: &SshTarget,
    remote: &str,
    archive: Option<&std::path::Path>,
) -> Result<String, String> {
    let input = match archive {
        Some(path) => std::fs::File::open(path)
            .map(std::process::Stdio::from)
            .map_err(|error| {
                format!(
                    "Cannot read the development build at {}: {error}. Run scripts/build-linux-dev.sh in this checkout, then retry installation.",
                    path.display()
                )
            })?,
        None => std::process::Stdio::null(),
    };
    let output = host
        .command(remote)
        .and_then(|mut command| command.stdin(input).output())
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    let last = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .lines()
            .map(str::trim)
            .rfind(|line| !line.is_empty())
            .map(str::to_owned)
    };
    Err(last(&output.stderr)
        .or_else(|| last(&output.stdout))
        .unwrap_or_else(|| format!("{host} exited with {}", output.status)))
}

/// Connects once, starting the server if needed, as a worker would. It
/// blocks for as long as SSH takes, so it runs off the UI thread.
pub(crate) fn probe(host: &SshTarget) -> Result<String, ProbeFailure> {
    let client = Client::connect_ssh(host, Start::IfNeeded).map_err(|error| {
        let message = explain(&Target::Ssh(host.clone()), &error);
        match error {
            ClientError::Remote {
                reason: muxy_client::RemoteReason::NotInstalled,
                ..
            } => ProbeFailure::NotInstalled(message),
            _ => ProbeFailure::Other(message),
        }
    })?;
    let version = client.server_info().build.version.clone();
    client.disconnect();
    Ok(version)
}

impl std::fmt::Debug for Workers {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Workers").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub(crate) enum Work {
    /// Lends the worker's client.
    ExtensionClient(async_channel::Sender<Option<Client>>),
    /// Opens a separate connection to the same server, so extension work
    /// never queues behind the app's requests or terminal frames.
    ExtensionConnection(async_channel::Sender<Result<Client, String>>),
    WriteInput {
        channel: ChannelId,
        bytes: Vec<u8>,
        completion: async_channel::Sender<Result<(), String>>,
    },
    ReadActivity,
    AcknowledgeActivity(Vec<u64>),
    ClaimActivity(Vec<u64>),
    Git(muxy_protocol::GitRequest),
    ProjectSessions {
        project: muxy_protocol::ProjectId,
    },
    ProjectLayouts {
        project: muxy_protocol::ProjectId,
        request: u64,
    },
    LoadProjectLayout {
        project: muxy_protocol::ProjectId,
        request: u64,
        layout: muxy_app_core::project_layouts::Descriptor,
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
    /// A connection attempt finished on its own thread.
    Connected(Box<Result<(Client, Vec<SessionInfo>), ClientError>>),
    ReadServerSettings,
    WriteServerSettings(muxy_protocol::ServerSettingsDoc),
    ReadRemoteAccess,
    WriteRemoteAccess(muxy_protocol::RemoteAccessSettings),
    StartPairing,
    CancelPairing,
    RevokeDevice(muxy_protocol::DeviceId),
    StopServer {
        restart: bool,
    },
    PrepareUpdate {
        update: crate::updater::PreparedUpdate,
        mode: UpdateMode,
        server: muxy_protocol::ServerInfo,
    },
    CheckServerUpdate {
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
    AsyncCompleted(Box<Update>),
    Detach(ChannelId),
    Resize(ChannelId, Size),
    Colors(TerminalColors),
    Input(ChannelId, Vec<u8>),
    TerminalInput(ChannelId, muxy_protocol::TerminalInput),
    ClearScreen(ChannelId),
    Mouse(ChannelId, MouseEvent),
    CellSize(ChannelId, muxy_protocol::CellSize),
    Ack(ChannelId, u64),
    Event(ClientEvent),
    Stop,
}

impl Work {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::ExtensionClient(..) => "ExtensionClient",
            Self::ExtensionConnection(..) => "ExtensionConnection",
            Self::WriteInput { .. } => "WriteInput",
            Self::ReadActivity => "ReadActivity",
            Self::AcknowledgeActivity(..) => "AcknowledgeActivity",
            Self::ClaimActivity(..) => "ClaimActivity",
            Self::Git(..) => "Git",
            Self::ProjectSessions { .. } => "ProjectSessions",
            Self::ProjectLayouts { .. } => "ProjectLayouts",
            Self::LoadProjectLayout { .. } => "LoadProjectLayout",
            Self::ReadCatalog => "ReadCatalog",
            Self::CancelCreation(..) => "CancelCreation",
            Self::MutateProject(..) => "MutateProject",
            Self::Search { .. } => "Search",
            Self::Connect => "Connect",
            Self::ReconnectAfterUpdate(..) => "ReconnectAfterUpdate",
            Self::Connected(..) => "Connected",
            Self::ReadServerSettings => "ReadServerSettings",
            Self::WriteServerSettings(..) => "WriteServerSettings",
            Self::ReadRemoteAccess => "ReadRemoteAccess",
            Self::WriteRemoteAccess(..) => "WriteRemoteAccess",
            Self::StartPairing => "StartPairing",
            Self::CancelPairing => "CancelPairing",
            Self::RevokeDevice(..) => "RevokeDevice",
            Self::StopServer { .. } => "StopServer",
            Self::PrepareUpdate { .. } => "PrepareUpdate",
            Self::CheckServerUpdate { .. } => "CheckServerUpdate",
            Self::Attach { .. } => "Attach",
            Self::References(..) => "References",
            Self::Discard(..) => "Discard",
            Self::CheckClose { .. } => "CheckClose",
            Self::ReadSaved { .. } => "ReadSaved",
            Self::History { .. } => "History",
            Self::EndAll(..) => "EndAll",
            Self::Flush => "Flush",
            Self::Completed(..) => "Completed",
            Self::AsyncCompleted(..) => "AsyncCompleted",
            Self::Detach(..) => "Detach",
            Self::Resize(..) => "Resize",
            Self::Colors(..) => "Colors",
            Self::Input(..) => "Input",
            Self::TerminalInput(..) => "TerminalInput",
            Self::ClearScreen(..) => "ClearScreen",
            Self::Mouse(..) => "Mouse",
            Self::CellSize(..) => "CellSize",
            Self::Ack(..) => "Ack",
            Self::Event(..) => "Event",
            Self::Stop => "Stop",
        }
    }
}

#[derive(Debug)]
pub(crate) enum Update {
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
    ProjectLayouts {
        project: muxy_protocol::ProjectId,
        request: u64,
        result: Result<Vec<muxy_app_core::project_layouts::Descriptor>, String>,
    },
    ProjectLayout {
        project: muxy_protocol::ProjectId,
        request: u64,
        layout: muxy_app_core::project_layouts::Descriptor,
        result: Result<muxy_app_core::project_layouts::Config, String>,
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
    RemoteAccess(Result<muxy_protocol::RemoteAccessState, ClientError>),
    Pairing(Result<muxy_protocol::PairingOffer, ClientError>),
    ServerStopped {
        restart: bool,
        result: Result<(), ClientError>,
    },
    PreparedForInstall(Result<Option<std::fs::File>, ClientError>),
    ServerInfo(muxy_protocol::ServerInfo),
    ServerChecked(Result<ServerUpdate, ClientError>),
    /// Why connecting failed, and for another computer, the kind of failure.
    ConnectFailed(String, Option<muxy_client::RemoteReason>),
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

/// Runs one server's connection and requests on a thread of its own, so a
/// slow or unreachable server never holds up another. Work sent while it
/// connects waits for the connection, then runs in order.
#[allow(
    clippy::too_many_lines,
    reason = "Keep connection and request routing together"
)]
pub(crate) fn worker(
    server: ServerId,
    target: Target,
    updates: async_channel::Sender<(ServerId, u64, Update)>,
) -> std::io::Result<Worker> {
    let (sender, work) = mpsc::channel();
    let event_sender = sender.clone();
    let mut requests = requests::Requests::new()?;
    thread::Builder::new()
        .name("muxy-app-client".into())
        .spawn(move || {
            let mut client: Option<Client> = None;
            let mut generation = 0;
            let mut delivery = delivery::Delivery::default();
            let mut connecting = false;
            let mut held = VecDeque::new();
            let mut released = VecDeque::new();
            while let Some((requested, work)) = released.pop_front().or_else(|| work.recv().ok()) {
                if matches!(work, Work::Stop) {
                    if let Some(client) = &client {
                        Client::disconnect(client);
                    }
                    break;
                }
                if connecting
                    && requested == generation
                    && !matches!(
                        work,
                        Work::Connect | Work::ReconnectAfterUpdate(_) | Work::Connected(_)
                    )
                {
                    held.push_back((requested, work));
                    continue;
                }
                if matches!(work, Work::Completed(_)) {
                    requests.running = false;
                }
                let mut ready = match work {
                    Work::Connect | Work::ReconnectAfterUpdate(_) => {
                        generation = requested;
                        delivery = delivery::Delivery::default();
                        requests.reset();
                        held.clear();
                        if let Some(client) = client.take() {
                            Client::disconnect(&client);
                        }
                        let after_update = match work {
                            Work::ReconnectAfterUpdate(instance) => Some(instance),
                            _ => None,
                        };
                        let failed =
                            start_connect(&target, generation, &event_sender, after_update);
                        connecting = failed.is_empty();
                        failed
                    }
                    Work::Connected(connection) if requested != generation => {
                        if let Ok((stale, _)) = *connection {
                            stale.disconnect();
                        }
                        Vec::new()
                    }
                    Work::Connected(connection) => {
                        connecting = false;
                        released.extend(held.drain(..));
                        match *connection {
                            Ok((connected, sessions)) => {
                                let info = connected.server_info().clone();
                                crate::diagnostics::event(
                                    "connected",
                                    format_args!(
                                        "server={server} generation={generation} instance={}",
                                        info.instance
                                    ),
                                );
                                client = Some(connected);
                                vec![Update::ServerInfo(info), Update::Connected(sessions)]
                            }
                            Err(error) => {
                                let reason = match &error {
                                    ClientError::Remote { reason, .. } => Some(*reason),
                                    _ => None,
                                };
                                vec![Update::ConnectFailed(explain(&target, &error), reason)]
                            }
                        }
                    }
                    _ if requested != generation => Vec::new(),
                    Work::ExtensionClient(reply) => {
                        let _ = reply.try_send(client.clone());
                        Vec::new()
                    }
                    Work::ExtensionConnection(reply) => {
                        if let Some(client) = &client {
                            open_connection(&target, client.server_info().instance, reply);
                        }
                        Vec::new()
                    }
                    Work::Event(event) => delivery.event(event).unwrap_or_else(|error| {
                        if let Some(client) = &client {
                            client.disconnect();
                        }
                        vec![Update::Error(error.into())]
                    }),
                    Work::Input(_, _)
                    | Work::TerminalInput(_, _)
                    | Work::Mouse(_, _)
                    | Work::CellSize(_, _)
                    | Work::Ack(_, _) => client
                        .as_ref()
                        .and_then(|client| perform(work, client, &target))
                        .into_iter()
                        .collect(),
                    Work::Git(request) => {
                        if let Some(client) = &client {
                            dispatch_git(client, request, requested, event_sender.clone());
                        }
                        Vec::new()
                    }
                    Work::AsyncCompleted(update) => vec![*update],
                    Work::Flush => delivery.flush(),
                    Work::Completed(update) => delivery.complete(*update),
                    work => client
                        .as_ref()
                        .and_then(|_| requests.push(work, &mut delivery))
                        .into_iter()
                        .collect(),
                };
                if ready
                    .iter()
                    .any(|update| matches!(update, Update::ReferencesSynced(Err(_))))
                    && let Some(client) = &client
                {
                    client.disconnect();
                }
                if let Some(client) = &client {
                    ready.extend(requests.start(
                        client,
                        &target,
                        generation,
                        &event_sender,
                        &mut delivery,
                    ));
                }
                if ready
                    .into_iter()
                    .any(|update| updates.send_blocking((server, generation, update)).is_err())
                {
                    break;
                }
            }
        })?;
    Ok(sender)
}

fn dispatch_git(
    client: &Client,
    request: muxy_protocol::GitRequest,
    generation: u64,
    completed: Worker,
) {
    let trace = crate::diagnostics::Span::new(
        "app.git",
        format_args!("generation={generation} project={:?}", request.project),
    );
    client
        .git_async(request.clone())
        .on_complete(move |result| {
            trace.stage(format_args!("phase=reply success={}", result.is_ok()));
            let _ = completed.send((
                generation,
                Work::AsyncCompleted(Box::new(Update::Git { request, result })),
            ));
        });
}

/// Connects on a thread of its own, so Stop and a newer Connect never wait
/// for SSH. A connection that arrives for an older generation is closed.
fn start_connect(
    target: &Target,
    generation: u64,
    worker: &Worker,
    after_update: Option<u64>,
) -> Vec<Update> {
    let (target, worker) = (target.clone(), worker.clone());
    let spawned = thread::Builder::new()
        .name("muxy-app-connect".into())
        .spawn(move || {
            let connection = connect(&target, generation, &worker, after_update);
            if let Err(mpsc::SendError((_, Work::Connected(connection)))) =
                worker.send((generation, Work::Connected(Box::new(connection))))
                && let Ok((client, _)) = *connection
            {
                client.disconnect();
            }
        });
    match spawned {
        Ok(_) => Vec::new(),
        Err(error) => vec![Update::ConnectFailed(error.to_string(), None)],
    }
}

fn connect(
    target: &Target,
    generation: u64,
    sender: &Worker,
    after_update: Option<u64>,
) -> Result<(Client, Vec<SessionInfo>), ClientError> {
    let client = match target {
        Target::Local(socket) => match after_update {
            Some(instance) => crate::server::reconnect_after_update(socket, instance)?,
            None => crate::server::ensure_server_running(socket)?,
        },
        Target::Ssh(host) => Client::connect_ssh(host, Start::IfNeeded)?,
    };
    client.identify(muxy_protocol::ClientKind::Desktop)?;
    if let Target::Local(socket) = target {
        let profile = socket
            .parent()
            .ok_or_else(|| std::io::Error::other("Missing profile directory"))?;
        crate::backup::apply_mobile_settings(profile, |settings| {
            client.write_remote_access(settings)?;
            Ok(())
        })
        .map_err(std::io::Error::other)?;
    }
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

/// Names the other computer on errors that would not say which one failed.
fn explain(target: &Target, error: &ClientError) -> String {
    match (target, error) {
        (Target::Ssh(host), ClientError::Timeout | ClientError::Disconnected) => {
            format!("{host}: {error}")
        }
        _ => error.to_string(),
    }
}

fn open_connection(
    target: &Target,
    instance: u64,
    reply: async_channel::Sender<Result<Client, String>>,
) {
    let (target, failed) = (target.clone(), reply.clone());
    let spawned = thread::Builder::new()
        .name("muxy-app-connect".into())
        .spawn(move || {
            let connection = target
                .connect()
                .map_err(|error| error.to_string())
                .and_then(|client| {
                    if client.server_info().instance == instance {
                        Ok(client)
                    } else {
                        client.disconnect();
                        Err("server changed while connecting extensions".into())
                    }
                });
            if let Err(error) = reply.try_send(connection)
                && let Ok(client) = error.into_inner()
            {
                client.disconnect();
            }
        });
    if let Err(error) = spawned {
        let _ = failed.try_send(Err(error.to_string()));
    }
}

fn schedule(
    requests: &WorkerPool,
    work: Work,
    client: Client,
    target: Target,
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
            let update = work.and_then(|work| {
                let span = crate::diagnostics::Span::new(
                    "worker",
                    format_args!("generation={generation} kind={}", work.name()),
                );
                let update = perform(work, &client, &target);
                span.stage(format_args!(
                    "phase=reply update={:?}",
                    update.as_ref().map(std::mem::discriminant)
                ));
                update
            });
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
        Work::WriteInput { completion, .. } => {
            let _ = completion.try_send(Err(error.to_string()));
            Update::Error(error.to_string())
        }
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
        Work::ProjectLayouts { project, request } => Update::ProjectLayouts {
            project,
            request,
            result: Err(error.to_string()),
        },
        Work::LoadProjectLayout {
            project,
            request,
            layout,
        } => Update::ProjectLayout {
            project,
            request,
            layout,
            result: Err(error.to_string()),
        },
        Work::ReadServerSettings | Work::WriteServerSettings(_) => {
            Update::ServerSettings(Err(error))
        }
        Work::ReadRemoteAccess
        | Work::WriteRemoteAccess(_)
        | Work::CancelPairing
        | Work::RevokeDevice(_) => Update::RemoteAccess(Err(error)),
        Work::StartPairing => Update::Pairing(Err(error)),
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
fn perform(work: Work, client: &Client, target: &Target) -> Option<Update> {
    let result = match work {
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
        Work::Git(_) => return None,
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
        Work::ProjectLayouts { project, request } => {
            return Some(Update::ProjectLayouts {
                project,
                request,
                result: project_layouts::list(client, project),
            });
        }
        Work::LoadProjectLayout {
            project,
            request,
            layout,
        } => {
            return Some(Update::ProjectLayout {
                project,
                request,
                result: project_layouts::load(client, project, &layout),
                layout,
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
        Work::ReadRemoteAccess => return Some(Update::RemoteAccess(client.read_remote_access())),
        Work::WriteRemoteAccess(settings) => {
            return Some(Update::RemoteAccess(client.write_remote_access(settings)));
        }
        Work::StartPairing => return Some(Update::Pairing(client.start_pairing())),
        Work::CancelPairing => return Some(Update::RemoteAccess(client.cancel_pairing())),
        Work::RevokeDevice(device) => {
            return Some(Update::RemoteAccess(client.revoke_device(device)));
        }
        Work::StopServer { restart } => {
            return Some(Update::ServerStopped {
                restart,
                result: stop_server(client, target),
            });
        }
        Work::PrepareUpdate {
            update,
            mode,
            server,
        } => {
            return Some(Update::PreparedForInstall(target.socket().and_then(
                |socket| prepare_update(client, socket, &update, mode, &server),
            )));
        }
        Work::CheckServerUpdate { replace } => {
            return Some(Update::ServerChecked(target.socket().and_then(|socket| {
                crate::server::check_update(client, socket, replace)
            })));
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
        Work::TerminalInput(channel, input) => client.send_terminal_input(channel, input),
        Work::ClearScreen(channel) => client.clear_screen(channel),
        Work::WriteInput {
            channel,
            bytes,
            completion,
        } => {
            let _ = completion.try_send(
                client
                    .write_input(channel, bytes)
                    .map_err(|error| error.to_string()),
            );
            return None;
        }
        Work::Mouse(channel, event) => client.send_mouse(channel, event),
        Work::CellSize(channel, cell) => client.send_cell_size(channel, cell),
        Work::Ack(channel, seq) => client.ack(channel, seq),
        Work::Connect
        | Work::ReconnectAfterUpdate(_)
        | Work::Connected(_)
        | Work::ExtensionClient(_)
        | Work::ExtensionConnection(_)
        | Work::Event(_)
        | Work::Stop
        | Work::Completed(_)
        | Work::AsyncCompleted(_) => {
            return None;
        }
    };
    result.err().map(|error| Update::Error(error.to_string()))
}

fn stop_server(client: &Client, target: &Target) -> Result<(), ClientError> {
    match target {
        Target::Local(socket) => crate::server::stop_server(client, socket),
        Target::Ssh(_) => match client.stop_server() {
            Err(ClientError::Disconnected) => Ok(()),
            result => result,
        },
    }
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
mod project_layouts;
mod requests;

#[cfg(test)]
pub(crate) mod tests;
