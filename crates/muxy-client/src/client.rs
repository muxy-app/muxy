use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use muxy_protocol::transport::{ByteStream, StreamCancellation};
use muxy_protocol::wire::{Decoder, Encoder, message_version};
use muxy_protocol::{
    CONTROL, ChannelId, ErrorCode, ErrorReply, ForegroundProcess, HistoryCursor, HistoryPage,
    Message, MouseEvent, ReplyBody, RequestBody, SavedScreen, SearchPage, SearchSource, ServerPath,
    SessionId, SessionInfo, Size, TerminalColors, Version,
};

use crate::events::{self, ClientEvent};
use crate::handshake;
use crate::requests::Pending;
use crate::{ClientError, RunGrid};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attachment {
    pub channel: ChannelId,
    pub grid: RunGrid,
    pub title: String,
    pub directory: ServerPath,
    pub process: Option<ForegroundProcess>,
}

struct Shared {
    writer: Arc<Mutex<Encoder<Box<dyn Write + Send>>>>,
    asynchronous: mpsc::SyncSender<(muxy_protocol::RequestId, RequestBody)>,
    pending: Arc<Pending>,
    events: Mutex<Option<Receiver<ClientEvent>>>,
    cancellation: Arc<dyn StreamCancellation>,
    version: Version,
    server: muxy_protocol::ServerInfo,
}

impl Drop for Shared {
    fn drop(&mut self) {
        self.pending.close();
        self.cancellation.cancel();
    }
}

#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
    timeout: Duration,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Client")
            .field("timeout", &self.timeout)
            .field("connected", &!self.shared.pending.is_closed())
            .finish_non_exhaustive()
    }
}

impl Client {
    pub fn connect(socket: &Path) -> Result<Self, ClientError> {
        Self::connect_with_timeout(socket, DEFAULT_TIMEOUT)
    }

    pub fn connect_with_timeout(socket: &Path, timeout: Duration) -> Result<Self, ClientError> {
        if timeout.is_zero() {
            return Err(ClientError::Timeout);
        }
        Self::from_stream_with_timeout(muxy_protocol::transport::connect(socket)?, timeout)
    }

    pub fn from_stream(stream: Box<dyn ByteStream>) -> Result<Self, ClientError> {
        Self::from_stream_with_timeout(stream, DEFAULT_TIMEOUT)
    }

    fn from_stream_with_timeout(
        stream: Box<dyn ByteStream>,
        timeout: Duration,
    ) -> Result<Self, ClientError> {
        let cancellation: Arc<dyn StreamCancellation> = stream.cancellation()?.into();
        let reader_cancellation = stream.cancellation()?;
        let (read, write) = stream.split()?;
        let mut decoder = Decoder::new(read);
        let mut encoder = Encoder::new(write);
        encoder.send(CONTROL, &handshake::hello())?;
        let (events, receiver) = mpsc::channel();
        let (connected, accepted) = mpsc::channel();
        let pending = Arc::new(Pending::default());
        let routed = Arc::clone(&pending);
        thread::Builder::new()
            .name("muxy-client-reader".into())
            .spawn(move || {
                events::route(
                    &mut decoder,
                    &routed,
                    &events,
                    &connected,
                    reader_cancellation.as_ref(),
                );
            })?;
        let negotiated = accepted
            .recv_timeout(timeout)
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => ClientError::Timeout,
                RecvTimeoutError::Disconnected => ClientError::Disconnected,
            })
            .and_then(std::convert::identity);
        let (version, server) = match negotiated {
            Ok(version) => version,
            Err(error) => {
                cancellation.cancel();
                return Err(error);
            }
        };
        let (asynchronous, requests) =
            mpsc::sync_channel::<(muxy_protocol::RequestId, RequestBody)>(128);
        let writer = Arc::new(Mutex::new(encoder));
        let shared = Arc::new(Shared {
            writer: writer.clone(),
            asynchronous,
            pending,
            events: Mutex::new(Some(receiver)),
            cancellation,
            version,
            server,
        });
        let writer_pending = Arc::clone(&shared.pending);
        let writer_cancellation = Arc::clone(&shared.cancellation);
        thread::Builder::new()
            .name("muxy-client-writer".into())
            .spawn(move || {
                for (id, body) in requests {
                    if !writer_pending.contains_async(id) {
                        continue;
                    }
                    let sent = lock(&writer).send(CONTROL, &Message::Request { id, body });
                    if sent.is_err() {
                        writer_pending.close();
                        writer_cancellation.cancel();
                        break;
                    }
                }
            })?;
        let deadlines = Arc::clone(&shared.pending);
        thread::Builder::new()
            .name("muxy-client-deadlines".into())
            .spawn(move || deadlines.watch_deadlines())?;
        Ok(Self {
            shared,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn events(&self) -> Option<Receiver<ClientEvent>> {
        lock(&self.shared.events).take()
    }

    pub fn is_connected(&self) -> bool {
        !self.shared.pending.is_closed()
    }

    pub fn read_server_settings(&self) -> Result<muxy_protocol::ServerSettingsDoc, ClientError> {
        match self.request(RequestBody::ReadServerSettings)? {
            ReplyBody::ServerSettings(settings) => Ok(settings),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn write_server_settings(
        &self,
        settings: muxy_protocol::ServerSettingsDoc,
    ) -> Result<(), ClientError> {
        match self.request(RequestBody::WriteServerSettings(settings))? {
            ReplyBody::ServerSettingsWritten => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn server_info(&self) -> &muxy_protocol::ServerInfo {
        &self.shared.server
    }

    /// Atomically refuses to stop if a terminal is running or being created.
    pub fn stop_server_if_idle(&self) -> Result<bool, ClientError> {
        match self.request(RequestBody::StopServerIfIdle)? {
            ReplyBody::ServerStopping => Ok(true),
            ReplyBody::ServerBusy => Ok(false),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn stop_server(&self) -> Result<(), ClientError> {
        match self.request(RequestBody::StopServer)? {
            ReplyBody::ServerStopping => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn identify(
        &self,
        kind: muxy_protocol::ClientKind,
    ) -> Result<muxy_protocol::SessionClient, ClientError> {
        match self.request(RequestBody::IdentifyClient(kind))? {
            ReplyBody::ClientIdentified(client) => Ok(client),
            body => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionInfo>, ClientError> {
        match self.request(RequestBody::ListSessions)? {
            ReplyBody::Sessions(sessions) => Ok(sessions),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    /// Creates an ad-hoc session in this server's Home project.
    pub fn create_session(&self, directory: &Path, size: Size) -> Result<SessionInfo, ClientError> {
        let home = self.catalog_page(None, None)?.home;
        self.create_project_session(home, muxy_protocol::OperationId::new(), directory, size)
    }

    pub fn create_project_session(
        &self,
        project: muxy_protocol::ProjectId,
        operation: muxy_protocol::OperationId,
        directory: &Path,
        size: Size,
    ) -> Result<SessionInfo, ClientError> {
        let directory = ServerPath(directory.as_os_str().as_bytes().to_vec());
        match self.request(RequestBody::CreateSession {
            project,
            operation,
            directory,
            size,
        })? {
            ReplyBody::SessionCreated(info) => Ok(info),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn end_session(&self, id: SessionId) -> Result<(), ClientError> {
        match self.request(RequestBody::EndSession(id))? {
            ReplyBody::SessionEnded => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn read_saved_screen(&self, id: SessionId) -> Result<SavedScreen, ClientError> {
        match self.request(RequestBody::ReadSavedScreen(id))? {
            ReplyBody::SavedScreen(screen) => Ok(screen),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    /// Registers sessions used by open panes, including panes without an output subscription.
    pub fn sync_session_references(&self, sessions: Vec<SessionId>) -> Result<(), ClientError> {
        self.sync_layout_references(None, 0, sessions)
    }

    /// Publishes a revision of a shared layout, or this connection's independent references.
    pub fn sync_layout_references(
        &self,
        owner: Option<muxy_protocol::OperationId>,
        revision: u64,
        sessions: Vec<SessionId>,
    ) -> Result<(), ClientError> {
        match self.request(RequestBody::SyncSessionReferences {
            owner,
            revision,
            sessions,
        })? {
            ReplyBody::SessionReferencesSynced => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    /// Closes the final local reference; the same operation must be reused after a lost reply.
    pub fn close_session(
        &self,
        session: SessionId,
        operation: muxy_protocol::OperationId,
    ) -> Result<(), ClientError> {
        match self.request(RequestBody::CloseSession { session, operation })? {
            ReplyBody::SessionClosed => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn discard_session(&self, id: SessionId) -> Result<(), ClientError> {
        match self.request(RequestBody::DiscardSession(id))? {
            ReplyBody::SessionDiscarded => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn attach(&self, id: SessionId, size: Size) -> Result<Attachment, ClientError> {
        let reply = self.request(RequestBody::Attach { session: id, size });
        if matches!(&reply, Err(ClientError::Timeout)) {
            self.disconnect();
        }
        let (snapshot, process) = match reply? {
            ReplyBody::Attached { snapshot, process } => (*snapshot, process),
            other => return Err(ClientError::UnexpectedReply(Box::new(other))),
        };
        Ok(Attachment {
            channel: snapshot.channel,
            grid: RunGrid::from_snapshot(&snapshot),
            title: snapshot.title,
            directory: snapshot.directory,
            process,
        })
    }

    pub fn history_page(
        &self,
        channel: ChannelId,
        before: HistoryCursor,
        max_rows: u16,
    ) -> Result<HistoryPage, ClientError> {
        self.read_history(RequestBody::HistoryPage {
            channel,
            before,
            max_rows,
        })
    }

    pub fn saved_history_page(
        &self,
        session: SessionId,
        before: HistoryCursor,
        max_rows: u16,
    ) -> Result<HistoryPage, ClientError> {
        self.read_history(RequestBody::SavedHistoryPage {
            session,
            before,
            max_rows,
        })
    }

    fn read_history(&self, request: RequestBody) -> Result<HistoryPage, ClientError> {
        match self.request(request)? {
            ReplyBody::HistoryPage(page) => Ok(page),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn detach(&self, channel: ChannelId) -> Result<(), ClientError> {
        match self.request(RequestBody::Detach(channel))? {
            ReplyBody::Detached => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn search(
        &self,
        source: SearchSource,
        query: &str,
        ignore_case: bool,
        before: HistoryCursor,
        max_results: u16,
    ) -> Result<SearchPage, ClientError> {
        match self.request(RequestBody::Search {
            source,
            query: query.to_owned(),
            ignore_case,
            before,
            max_results,
        })? {
            ReplyBody::SearchPage(page) => Ok(page),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn resize(&self, channel: ChannelId, size: Size) -> Result<(), ClientError> {
        match self.request(RequestBody::Resize { channel, size })? {
            ReplyBody::Resized => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    /// Sets color defaults for this connection and its attached sessions.
    pub fn set_terminal_colors(&self, colors: TerminalColors) -> Result<(), ClientError> {
        match self.request(RequestBody::SetTerminalColors(colors))? {
            ReplyBody::TerminalColorsSet => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn ping(&self) -> Result<(), ClientError> {
        match self.request(RequestBody::Ping)? {
            ReplyBody::Pong => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn send_input(&self, channel: ChannelId, bytes: &[u8]) -> Result<(), ClientError> {
        session_channel(channel)?;
        self.send(channel, &Message::Input(bytes.to_vec()))
    }

    pub fn write_input(&self, channel: ChannelId, bytes: Vec<u8>) -> Result<(), ClientError> {
        session_channel(channel)?;
        muxy_protocol::validate_input(&bytes).map_err(ClientError::Invalid)?;
        match self.request(RequestBody::WriteInput { channel, bytes })? {
            ReplyBody::InputWritten => Ok(()),
            other => Err(ClientError::UnexpectedReply(Box::new(other))),
        }
    }

    pub fn send_cell_size(
        &self,
        channel: ChannelId,
        cell: muxy_protocol::CellSize,
    ) -> Result<(), ClientError> {
        session_channel(channel)?;
        self.send(channel, &Message::CellSize(cell))
    }

    pub fn send_mouse(&self, channel: ChannelId, event: MouseEvent) -> Result<(), ClientError> {
        session_channel(channel)?;
        self.send(channel, &Message::Mouse(event))
    }

    /// Close this connection and cancel outstanding requests for every client clone.
    pub fn disconnect(&self) {
        self.shared.pending.close();
        self.shared.cancellation.cancel();
    }

    pub fn ack(&self, channel: ChannelId, seq: u64) -> Result<(), ClientError> {
        session_channel(channel)?;
        self.send(CONTROL, &Message::FrameAck { channel, seq })
    }

    pub(crate) fn request_async<T>(
        &self,
        body: RequestBody,
        timeout: Duration,
        decode: fn(ReplyBody) -> Result<T, ClientError>,
    ) -> crate::Request<T> {
        let validation = Message::Request {
            id: muxy_protocol::RequestId(0),
            body: body.clone(),
        }
        .validate()
        .map_err(ClientError::Invalid)
        .and_then(|()| {
            if message_version(&Message::Request {
                id: muxy_protocol::RequestId(0),
                body: body.clone(),
            }) > self.shared.version
            {
                Err(ClientError::Protocol(
                    "request requires a newer server".into(),
                ))
            } else {
                Ok(())
            }
        });
        if let Err(error) = validation {
            return crate::Request::failed(error, decode);
        }
        let (id, response) = match self.shared.pending.register_async(timeout) {
            Ok(value) => value,
            Err(error) => return crate::Request::failed(error, decode),
        };
        let request =
            crate::Request::new(response, Arc::downgrade(&self.shared.pending), id, decode);
        if let Err(error) = self.shared.asynchronous.try_send((id, body)) {
            self.shared.pending.fail_async(
                id,
                std::io::Error::new(std::io::ErrorKind::WouldBlock, error.to_string()).into(),
            );
        }
        request
    }

    pub(crate) fn request(&self, body: RequestBody) -> Result<ReplyBody, ClientError> {
        self.request_with_timeout(body, self.timeout)
    }

    pub(crate) fn request_with_timeout(
        &self,
        body: RequestBody,
        timeout: Duration,
    ) -> Result<ReplyBody, ClientError> {
        let (id, reply) = self.shared.pending.register()?;
        if let Err(error) = self.send(CONTROL, &Message::Request { id, body }) {
            self.shared.pending.forget(id);
            return Err(error);
        }
        match reply.recv_timeout(timeout) {
            Ok(ReplyBody::Error(error)) => Err(ClientError::Server(error)),
            Ok(body) => Ok(body),
            Err(RecvTimeoutError::Timeout) => {
                self.shared.pending.forget(id);
                Err(ClientError::Timeout)
            }
            Err(RecvTimeoutError::Disconnected) => Err(ClientError::Disconnected),
        }
    }

    fn send(&self, channel: ChannelId, message: &Message) -> Result<(), ClientError> {
        message.validate().map_err(ClientError::Invalid)?;
        if message_version(message) > self.shared.version {
            return Err(ClientError::Server(ErrorReply {
                code: ErrorCode::BadRequest,
                message: "The running server does not support this message".into(),
            }));
        }
        if self.shared.pending.is_closed() {
            return Err(ClientError::Disconnected);
        }
        lock(&self.shared.writer).send(channel, message)?;
        Ok(())
    }
}

fn session_channel(channel: ChannelId) -> Result<(), ClientError> {
    if channel == CONTROL {
        Err(ClientError::Invalid(ErrorCode::UnknownChannel))
    } else {
        Ok(())
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
