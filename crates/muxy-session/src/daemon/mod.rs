pub mod session;

use crate::process_tree::current_process_identity;
use crate::runtime_paths::SecureRuntime;
use crate::transport::{
    authenticate_same_user, random_nonce, read_frame, read_structured, server_handshake,
    write_frame, write_structured,
};
use muxy_proto::session::codec::{FrameKind, encode_output};
use muxy_proto::session::{
    BuildMode, ClientKind, ControlRequest, ControlResponse, CreateSessionResolution,
    MAX_CONTROL_CONNECTIONS, MAX_STREAM_CHUNK_BYTES, SessionId, resolve_create_session,
};
use session::{DaemonSession, SessionEvent};
use std::collections::HashMap;
use std::io;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

const MAX_DAEMON_CONNECTIONS: usize = MAX_CONTROL_CONNECTIONS * 2;
const INITIAL_CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug)]
pub struct DaemonConfig {
    pub socket_path: PathBuf,
    pub build_mode: BuildMode,
    pub idle_timeout: Duration,
}

impl DaemonConfig {
    pub fn new(socket_path: impl Into<PathBuf>, build_mode: BuildMode) -> Self {
        Self {
            socket_path: socket_path.into(),
            build_mode,
            idle_timeout: Duration::from_secs(10),
        }
    }
}

struct DaemonState {
    socket_path: PathBuf,
    build_mode: BuildMode,
    daemon_identity: muxy_proto::session::ProcessIdentity,
    daemon_nonce: [u8; 32],
    sessions: Mutex<HashMap<SessionId, Arc<DaemonSession>>>,
    session_creation: Mutex<()>,
    connections: AtomicUsize,
    control_connections: AtomicUsize,
}

pub fn run(config: DaemonConfig) -> io::Result<()> {
    let runtime = SecureRuntime::bind(&config.socket_path)?;
    runtime.listener().set_nonblocking(true)?;
    let state = Arc::new(DaemonState {
        socket_path: config.socket_path,
        build_mode: config.build_mode,
        daemon_identity: current_process_identity()?,
        daemon_nonce: random_nonce()?,
        sessions: Mutex::new(HashMap::new()),
        session_creation: Mutex::new(()),
        connections: AtomicUsize::new(0),
        control_connections: AtomicUsize::new(0),
    });
    let mut idle_since = Some(Instant::now());
    loop {
        match runtime.listener().accept() {
            Ok((stream, _)) => {
                idle_since = None;
                stream.set_nonblocking(false)?;
                let Some(permit) = DaemonConnectionPermit::try_acquire(&state) else {
                    let _ = stream.shutdown(Shutdown::Both);
                    continue;
                };
                let held = Arc::clone(&state);
                std::thread::Builder::new()
                    .name("session-connection".into())
                    .spawn(move || {
                        let _permit = permit;
                        let _ = handle_connection(stream, held);
                    })?;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
        let no_running_sessions = !sessions(&state)
            .values()
            .any(|session| session.is_running());
        let no_connections = state.connections.load(Ordering::Acquire) == 0;
        if no_running_sessions && no_connections {
            let since = idle_since.get_or_insert_with(Instant::now);
            if since.elapsed() >= config.idle_timeout {
                return Ok(());
            }
        } else {
            idle_since = None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn handle_connection(mut stream: UnixStream, state: Arc<DaemonState>) -> io::Result<()> {
    stream.set_read_timeout(Some(INITIAL_CONNECTION_TIMEOUT))?;
    let peer = authenticate_same_user(&stream)?;
    let kind = server_handshake(
        &mut stream,
        peer,
        state.daemon_identity,
        state.build_mode,
        state.daemon_nonce,
    )?;
    match kind {
        ClientKind::Control => {
            let _permit = ControlConnectionPermit::try_acquire(&state).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "session control connection limit reached",
                )
            })?;
            handle_control(stream, &state)
        }
        ClientKind::Renderer => handle_renderer(stream, state),
    }
}

fn handle_control(mut stream: UnixStream, state: &DaemonState) -> io::Result<()> {
    let mut awaiting_first_request = true;
    while let Some(frame) = read_frame(&mut stream)? {
        if awaiting_first_request {
            stream.set_read_timeout(None)?;
            awaiting_first_request = false;
        }
        if frame.header.kind != FrameKind::ControlRequest {
            return Err(invalid("control connection received a non-control frame"));
        }
        let response = match read_structured::<ControlRequest>(&frame) {
            Ok(request) => apply_control(state, request),
            Err(error) => ControlResponse::Error {
                code: "invalidRequest".into(),
                message: error.to_string(),
            },
        };
        write_structured(
            &mut stream,
            FrameKind::ControlResponse,
            frame.header.request_id,
            &response,
        )?;
    }
    Ok(())
}

fn apply_control(state: &DaemonState, request: ControlRequest) -> ControlResponse {
    match request {
        ControlRequest::ListSessions => ControlResponse::Sessions(
            sessions(state)
                .values()
                .map(|session| session.descriptor())
                .collect(),
        ),
        ControlRequest::GetSession { session_id } => ControlResponse::Session(
            sessions(state)
                .get(&session_id)
                .map(|session| session.descriptor()),
        ),
        ControlRequest::CreateSession(request) => create_session(state, *request),
        ControlRequest::EndSession { session_id } => end_session(state, session_id),
        ControlRequest::EndSessionsByOwner { owner } => {
            let _mutation = state
                .session_creation
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let identifiers: Vec<_> = sessions(state)
                .iter()
                .filter(|(_, session)| session.request().owner == owner)
                .map(|(identifier, _)| *identifier)
                .collect();
            for identifier in identifiers {
                let response = end_session(state, identifier);
                if !matches!(response, ControlResponse::Acknowledged) {
                    return response;
                }
            }
            ControlResponse::Acknowledged
        }
        ControlRequest::EndAllSessions => {
            let _mutation = state
                .session_creation
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let identifiers: Vec<_> = sessions(state).keys().copied().collect();
            for identifier in identifiers {
                let response = end_session(state, identifier);
                if !matches!(response, ControlResponse::Acknowledged) {
                    return response;
                }
            }
            ControlResponse::Acknowledged
        }
        ControlRequest::SetWorkspacePlacement {
            session_id,
            placement,
        } => match sessions(state).get(&session_id) {
            Some(session) => match session.set_placement(placement) {
                Ok(_) => ControlResponse::Acknowledged,
                Err(error) => control_error("invalidPlacement", error),
            },
            None => control_error("notFound", "session was not found"),
        },
        ControlRequest::Ping => ControlResponse::Pong,
    }
}

fn create_session(
    state: &DaemonState,
    request: muxy_proto::session::CreateSessionRequest,
) -> ControlResponse {
    if let Err(error) = request.validate() {
        return control_error("invalidCreateRequest", error);
    }
    let _creation = state
        .session_creation
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let resolution = {
        let held = sessions(state);
        if let Some(existing) = held.get(&request.session_id) {
            if existing.request().owner == request.owner
                && existing.request().same_launch_contract(&request)
            {
                return ControlResponse::Created(existing.descriptor());
            }
            return ControlResponse::DuplicateOwnerConflict;
        }
        resolve_create_session(held.values().map(|session| session.request()), &request)
    };
    match resolution {
        Ok(CreateSessionResolution::Existing(identifier)) => sessions(state)
            .get(&identifier)
            .map(|session| ControlResponse::Created(session.descriptor()))
            .unwrap_or_else(|| control_error("invariant", "resolved session is absent")),
        Ok(CreateSessionResolution::DuplicateOwnerConflict) => {
            ControlResponse::DuplicateOwnerConflict
        }
        Ok(CreateSessionResolution::Create) => {
            match DaemonSession::spawn(request, &state.socket_path) {
                Ok(session) => {
                    let descriptor = session.descriptor();
                    sessions(state).insert(descriptor.session_id, session);
                    ControlResponse::Created(descriptor)
                }
                Err(error) => control_error("spawnFailed", error),
            }
        }
        Err(error) => control_error("invariant", error),
    }
}

fn end_session(state: &DaemonState, identifier: SessionId) -> ControlResponse {
    let session = match sessions(state).get(&identifier).cloned() {
        Some(session) => session,
        None => return control_error("notFound", "session was not found"),
    };
    match session.terminate() {
        Ok(()) => {
            sessions(state).remove(&identifier);
            ControlResponse::Acknowledged
        }
        Err(error) => control_error("cleanupFailed", error),
    }
}

fn handle_renderer(mut stream: UnixStream, state: Arc<DaemonState>) -> io::Result<()> {
    let frame = read_frame(&mut stream)?.ok_or_else(|| eof("renderer closed before attach"))?;
    stream.set_read_timeout(None)?;
    if frame.header.kind != FrameKind::Attach {
        return Err(invalid("renderer first frame must be attach"));
    }
    let request: muxy_proto::session::AttachRequest = read_structured(&frame)?;
    let session = sessions(&state)
        .get(&request.session_id)
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "session was not found"))?;
    let shutdown = stream.try_clone()?;
    let registration = session.attach(request.size, shutdown)?;
    write_structured(&mut stream, FrameKind::Attached, 0, &registration.attached)?;
    if !registration.replay.bytes.is_empty() {
        let chunks = registration
            .replay
            .bytes
            .chunks(MAX_STREAM_CHUNK_BYTES)
            .collect::<Vec<_>>();
        let chunk_count =
            u64::try_from(chunks.len()).map_err(|_| invalid("replay is too large"))?;
        let last_sequence = registration
            .replay
            .last_sequence
            .ok_or_else(|| invalid("replay bytes are present without an output sequence"))?;
        let first_sequence = last_sequence
            .checked_sub(chunk_count.saturating_sub(1))
            .ok_or_else(|| invalid("replay sequence range is invalid"))?;
        for (offset, chunk) in chunks.into_iter().enumerate() {
            let sequence = first_sequence
                .checked_add(u64::try_from(offset).map_err(|_| invalid("replay is too large"))?)
                .ok_or_else(|| invalid("replay sequence range overflowed"))?;
            let payload = encode_output(sequence, chunk).map_err(protocol_error)?;
            write_frame(&mut stream, FrameKind::Output, 0, &payload)?;
        }
    }
    let mut writer = stream.try_clone()?;
    let generation = registration.attached.attachment_generation;
    let receiver = registration.receiver;
    let writer_thread = std::thread::Builder::new()
        .name("session-renderer-output".into())
        .spawn(move || -> io::Result<()> {
            while let Ok(event) = receiver.recv() {
                match event {
                    SessionEvent::Output {
                        sequence,
                        bytes,
                        pending_bytes,
                    } => {
                        let result = encode_output(sequence, &bytes)
                            .map_err(protocol_error)
                            .and_then(|payload| {
                                write_frame(&mut writer, FrameKind::Output, 0, &payload)
                            });
                        pending_bytes.fetch_sub(bytes.len(), Ordering::AcqRel);
                        result?;
                    }
                    SessionEvent::Exited(exited) => {
                        write_structured(&mut writer, FrameKind::Exited, 0, &exited)?;
                        return Ok(());
                    }
                }
            }
            Ok(())
        })?;
    let result = renderer_input_loop(&mut stream, &session, generation);
    session.detach(generation);
    let _ = stream.shutdown(Shutdown::Both);
    let _ = writer_thread.join();
    result
}

fn renderer_input_loop(
    stream: &mut UnixStream,
    session: &DaemonSession,
    generation: u64,
) -> io::Result<()> {
    while let Some(frame) = read_frame(stream)? {
        match frame.header.kind {
            FrameKind::Input => session.input(generation, &frame.payload)?,
            FrameKind::Resize => {
                let resize: muxy_proto::session::Resize = read_structured(&frame)?;
                if resize.attachment_generation != generation {
                    return Err(invalid("resize attachment generation differs"));
                }
                session.resize(resize)?;
            }
            _ => return Err(invalid("renderer sent an unsupported frame")),
        }
    }
    Ok(())
}

fn sessions(state: &DaemonState) -> MutexGuard<'_, HashMap<SessionId, Arc<DaemonSession>>> {
    state
        .sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct DaemonConnectionPermit {
    state: Arc<DaemonState>,
}

impl DaemonConnectionPermit {
    fn try_acquire(state: &Arc<DaemonState>) -> Option<Self> {
        state
            .connections
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_DAEMON_CONNECTIONS).then_some(count + 1)
            })
            .ok()
            .map(|_| Self {
                state: Arc::clone(state),
            })
    }
}

impl Drop for DaemonConnectionPermit {
    fn drop(&mut self) {
        self.state.connections.fetch_sub(1, Ordering::AcqRel);
    }
}

struct ControlConnectionPermit<'a> {
    state: &'a DaemonState,
}

impl<'a> ControlConnectionPermit<'a> {
    fn try_acquire(state: &'a DaemonState) -> Option<Self> {
        state
            .control_connections
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_CONTROL_CONNECTIONS).then_some(count + 1)
            })
            .ok()
            .map(|_| Self { state })
    }
}

impl Drop for ControlConnectionPermit<'_> {
    fn drop(&mut self) {
        self.state
            .control_connections
            .fetch_sub(1, Ordering::AcqRel);
    }
}

fn control_error(code: &str, error: impl std::fmt::Display) -> ControlResponse {
    ControlResponse::Error {
        code: code.into(),
        message: error.to_string(),
    }
}

fn protocol_error(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn eof(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, message)
}
