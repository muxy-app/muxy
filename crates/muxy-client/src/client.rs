use std::io::Write;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use muxy_protocol::transport::{ByteStream, StreamCancellation};
use muxy_protocol::wire::{Decoder, Encoder};
use muxy_protocol::{
    CONTROL, ChannelId, ErrorCode, Feature, HistoryCursor, HistoryPage, Message, MouseEvent,
    ReplyBody, RequestBody, SavedScreen, SearchPage, SearchSource, ServerPath, SessionId,
    SessionInfo, Size, TerminalColors,
};

use crate::ClientError;
use crate::events::{self, ClientEvent};
use crate::handshake;
use crate::requests::{Attaching, Pending, Reply};

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

struct Shared {
    writer: Arc<Mutex<Encoder<Box<dyn Write + Send>>>>,
    asynchronous: mpsc::SyncSender<(muxy_protocol::RequestId, RequestBody)>,
    pending: Arc<Pending>,
    events: Mutex<Option<Receiver<ClientEvent>>>,
    cancellation: Arc<dyn StreamCancellation>,
    server: muxy_protocol::ServerInfo,
    features: Vec<Feature>,
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

    pub(crate) fn from_stream_with_timeout(
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
        let (server, features) = match negotiated {
            Ok(accepted) => accepted,
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
            server,
            features,
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

    /// What the connected server offers; older servers lack newer features.
    pub fn features(&self) -> &[Feature] {
        &self.shared.features
    }

    /// Whether the connected server offers `feature`; older servers lack newer features.
    pub fn supports(&self, feature: Feature) -> bool {
        self.shared.features.contains(&feature)
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
        self.create_project_session_with_environment(
            project,
            operation,
            directory,
            size,
            std::collections::BTreeMap::new(),
            None,
        )
    }

    /// Starts the shell with `environment` too, such as a proxy, and gives
    /// the session its own `colors` from the start. Servers without
    /// `Feature::SESSION_ENVIRONMENT` ignore the environment, and servers
    /// from before `SetSessionColors` ignore the colors.
    pub fn create_project_session_with_environment(
        &self,
        project: muxy_protocol::ProjectId,
        operation: muxy_protocol::OperationId,
        directory: &Path,
        size: Size,
        environment: std::collections::BTreeMap<String, String>,
        colors: Option<TerminalColors>,
    ) -> Result<SessionInfo, ClientError> {
        let directory = ServerPath::from(directory);
        match self.request(RequestBody::CreateSession {
            project,
            operation,
            directory,
            size,
            environment,
            colors,
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

    /// Attaches and returns the channel. The terminal arrives on the event
    /// stream as `ClientEvent::Attached` with `tag`, before any of its frames.
    /// After an error the event may still arrive; detach what nobody waits for.
    pub fn attach(&self, tag: u64, id: SessionId, size: Size) -> Result<ChannelId, ClientError> {
        self.attach_request(tag, id, RequestBody::Attach { session: id, size })
    }

    /// Like `attach`, without resizing a session other clients show.
    pub fn attach_without_resize(&self, tag: u64, id: SessionId) -> Result<ChannelId, ClientError> {
        self.attach_request(tag, id, RequestBody::AttachWithoutResize(id))
    }

    fn attach_request(
        &self,
        tag: u64,
        session: SessionId,
        request: RequestBody,
    ) -> Result<ChannelId, ClientError> {
        let (id, reply) = self
            .shared
            .pending
            .register_attach(Attaching { tag, session })?;
        let reply = match self.exchange(id, &reply, request, self.timeout) {
            Err(ClientError::Timeout) if !self.shared.pending.give_up_attach(id) => reply
                .recv()
                .map_err(|_| ClientError::Disconnected)
                .and_then(|reply| match reply {
                    Reply::Body(ReplyBody::Error(error)) => Err(ClientError::Server(error)),
                    reply => Ok(reply),
                }),
            reply => reply,
        };
        match reply? {
            Reply::Attached(channel) => Ok(channel),
            Reply::Body(body) => Err(ClientError::UnexpectedReply(Box::new(body))),
        }
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

    pub fn clear_screen(&self, channel: ChannelId) -> Result<(), ClientError> {
        session_channel(channel)?;
        match self.request(RequestBody::ClearScreen(channel))? {
            ReplyBody::ScreenCleared => Ok(()),
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

    /// Gives one session its own colors on this connection, or with `None`
    /// returns it to the connection's. Servers from before this answer `Unsupported`.
    pub fn set_session_colors(
        &self,
        session: SessionId,
        colors: Option<TerminalColors>,
    ) -> Result<(), ClientError> {
        match self.request(RequestBody::SetSessionColors { session, colors })? {
            ReplyBody::SessionColorsSet => Ok(()),
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

    /// Requires `Feature::TERMINAL_INPUT`; raw input remains available for older servers.
    pub fn send_terminal_input(
        &self,
        channel: ChannelId,
        input: muxy_protocol::TerminalInput,
    ) -> Result<(), ClientError> {
        session_channel(channel)?;
        if !self.supports(Feature::TERMINAL_INPUT) {
            return Err(ClientError::Invalid(ErrorCode::Unsupported));
        }
        self.send(channel, &Message::TerminalInput(input))
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
        .map_err(ClientError::Invalid);
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
        let reply = self.exchange(id, &reply, body, timeout);
        if matches!(reply, Err(ClientError::Timeout)) {
            self.shared.pending.forget(id);
        }
        match reply? {
            Reply::Body(body) => Ok(body),
            Reply::Attached(_) => Err(ClientError::Protocol(
                "an attach reply answered another request".into(),
            )),
        }
    }

    fn exchange(
        &self,
        id: muxy_protocol::RequestId,
        reply: &Receiver<Reply>,
        body: RequestBody,
        timeout: Duration,
    ) -> Result<Reply, ClientError> {
        if let Err(error) = self.send(CONTROL, &Message::Request { id, body }) {
            self.shared.pending.abandon(id);
            return Err(error);
        }
        match reply.recv_timeout(timeout) {
            Ok(Reply::Body(ReplyBody::Error(error))) => Err(ClientError::Server(error)),
            Ok(reply) => Ok(reply),
            Err(RecvTimeoutError::Timeout) => Err(ClientError::Timeout),
            Err(RecvTimeoutError::Disconnected) => Err(ClientError::Disconnected),
        }
    }

    fn send(&self, channel: ChannelId, message: &Message) -> Result<(), ClientError> {
        message.validate().map_err(ClientError::Invalid)?;
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

#[cfg(test)]
mod tests;
