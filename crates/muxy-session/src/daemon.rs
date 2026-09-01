use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use muxy_core::terminal_activity::{
    PersistentCommandActivity, ShellActivitySession, ShellActivityTracker,
};
use muxy_proto::session::{
    AttachExisting, AttachRequest, Attached, CommandActivity, CommandActivityResult,
    ProtocolFailure, QueryResult, SessionDescriptor, SessionMessage, TerminateResult,
    TerminationOutcome,
};
use thiserror::Error;

use crate::pty::{self, PtyError};
use crate::security::{BoundSocket, SecurityError};
use crate::{WireError, read_message, write_message};

pub const MAX_CONNECTIONS: usize = 64;
pub const MAX_CLIENT_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_SESSION_INPUT_BYTES: usize = 1024 * 1024;
pub const REPLAY_BYTES: usize = 256 * 1024;
const OUTPUT_CHUNK_BYTES: usize = 16 * 1024;
const OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
const EMPTY_READ_BACKOFF: Duration = Duration::from_millis(5);
const ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(200);
const MAX_SESSIONS: usize = 128;
const DEFAULT_IDLE: Duration = Duration::from_secs(30);
const OPENER_READ_TIMEOUT: Duration = Duration::from_secs(3);
const CLIENT_WRITE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("daemon requires --socket PATH and optional --idle-ms VALUE")]
    Arguments,
    #[error("daemon idle duration is invalid")]
    IdleDuration,
    #[error(transparent)]
    Security(#[from] SecurityError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Pty(#[from] PtyError),
}

struct DaemonState {
    sessions: HashMap<String, Session>,
    last_activity: Instant,
}

struct Session {
    descriptor: SessionDescriptor,
    session_id: i32,
    master: Arc<Mutex<File>>,
    replay: VecDeque<u8>,
    client: Option<ClientSink>,
    next_generation: u64,
    activity_tracker: ShellActivityTracker,
    activity_session: ShellActivitySession,
    exiting: bool,
}

struct ClientSink {
    generation: u64,
    messages: Arc<ClientQueue>,
    shutdown: UnixStream,
}

struct ClientQueue {
    state: Mutex<ClientQueueState>,
    available: Condvar,
}

struct ClientQueueState {
    messages: VecDeque<(SessionMessage, usize)>,
    bytes: usize,
    closed: bool,
}

impl ClientQueue {
    fn new() -> Self {
        Self {
            state: Mutex::new(ClientQueueState {
                messages: VecDeque::new(),
                bytes: 0,
                closed: false,
            }),
            available: Condvar::new(),
        }
    }

    fn push(&self, message: SessionMessage) -> bool {
        let bytes = message_bytes(&message);
        let mut state = lock(&self.state);
        if state.closed
            || state
                .bytes
                .checked_add(bytes)
                .is_none_or(|total| total > MAX_CLIENT_OUTPUT_BYTES)
        {
            return false;
        }
        state.bytes += bytes;
        state.messages.push_back((message, bytes));
        self.available.notify_one();
        true
    }

    fn pop(&self) -> Option<SessionMessage> {
        let mut state = lock(&self.state);
        loop {
            if let Some((message, bytes)) = state.messages.pop_front() {
                state.bytes -= bytes;
                return Some(message);
            }
            if state.closed {
                return None;
            }
            state = self
                .available
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    fn close(&self) {
        let mut state = lock(&self.state);
        state.closed = true;
        self.available.notify_all();
    }
}

struct ConnectionGuard(Arc<AtomicUsize>);

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub fn run(arguments: Vec<std::ffi::OsString>) -> Result<(), DaemonError> {
    let (socket, idle) = parse_arguments(arguments)?;
    let bound = BoundSocket::bind(&socket)?;
    bound.listener.set_nonblocking(true)?;
    serve(bound, idle)
}

fn parse_arguments(arguments: Vec<std::ffi::OsString>) -> Result<(PathBuf, Duration), DaemonError> {
    let mut socket = None;
    let mut idle = DEFAULT_IDLE;
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].to_str() {
            Some("--socket") if index + 1 < arguments.len() => {
                socket = Some(PathBuf::from(&arguments[index + 1]));
                index += 2;
            }
            Some("--idle-ms") if index + 1 < arguments.len() => {
                let milliseconds = arguments[index + 1]
                    .to_str()
                    .and_then(|value| value.parse::<u64>().ok())
                    .filter(|value| (100..=300_000).contains(value))
                    .ok_or(DaemonError::IdleDuration)?;
                idle = Duration::from_millis(milliseconds);
                index += 2;
            }
            _ => return Err(DaemonError::Arguments),
        }
    }
    Ok((socket.ok_or(DaemonError::Arguments)?, idle))
}

fn serve(bound: BoundSocket, idle: Duration) -> Result<(), DaemonError> {
    let state = Arc::new(Mutex::new(DaemonState {
        sessions: HashMap::new(),
        last_activity: Instant::now(),
    }));
    let connections = Arc::new(AtomicUsize::new(0));
    loop {
        match bound.listener.accept() {
            Ok((stream, _)) => {
                if stream
                    .set_nonblocking(false)
                    .and_then(|()| stream.set_read_timeout(Some(OPENER_READ_TIMEOUT)))
                    .and_then(|()| stream.set_write_timeout(Some(CLIENT_WRITE_TIMEOUT)))
                    .is_err()
                {
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    continue;
                }
                if connections.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
                    connections.fetch_sub(1, Ordering::AcqRel);
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    continue;
                }
                lock(&state).last_activity = Instant::now();
                let state = state.clone();
                let guard = ConnectionGuard(connections.clone());
                let _ = std::thread::Builder::new().spawn(move || {
                    let _guard = guard;
                    let _ = handle_connection(stream, state);
                });
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => {}
        }
        {
            let state = lock(&state);
            if state.sessions.is_empty()
                && connections.load(Ordering::Acquire) == 0
                && state.last_activity.elapsed() >= idle
            {
                return Ok(());
            }
        }
        pty::wait_ready(bound.listener.as_raw_fd(), false, ACCEPT_POLL_INTERVAL);
    }
}

fn handle_connection(
    mut stream: UnixStream,
    state: Arc<Mutex<DaemonState>>,
) -> Result<(), WireError> {
    if crate::security::validate_peer(&stream).is_err() {
        let _ = stream.shutdown(std::net::Shutdown::Both);
        return Ok(());
    }
    let message = match read_message(&mut stream) {
        Ok(message) => message,
        Err(error) => {
            let _ = protocol_error(&mut stream, "invalid_frame", &error.to_string());
            return Err(error);
        }
    };
    match message {
        SessionMessage::AttachCreateOrAttach(request) => {
            handle_attach(stream, state, AttachMode::Create(request))
        }
        SessionMessage::AttachExisting(request) => {
            handle_attach(stream, state, AttachMode::Existing(request))
        }
        SessionMessage::Query(query) => {
            let result = lock(&state)
                .sessions
                .get(&query.session_id)
                .map(descriptor)
                .map_or(QueryResult::Missing, QueryResult::Found);
            write_message(&mut stream, &SessionMessage::QueryResult(result))
        }
        SessionMessage::Recover(_) => {
            let descriptors = lock(&state).sessions.values().map(descriptor).collect();
            write_message(&mut stream, &SessionMessage::Recovered(descriptors))
        }
        SessionMessage::CommandActivity(query) => {
            let Some(activity) = lock(&state)
                .sessions
                .get(&query.session_id)
                .map(|session| command_activity(session.activity_tracker.activity()))
            else {
                return protocol_error(&mut stream, "session_missing", "session does not exist");
            };
            write_message(
                &mut stream,
                &SessionMessage::CommandActivityResult(CommandActivityResult {
                    session_id: query.session_id,
                    activity,
                }),
            )
        }
        SessionMessage::TerminateOne(query) => {
            let target = lock(&state)
                .sessions
                .get(&query.session_id)
                .map(|session| session.session_id);
            let terminated = target.is_none_or(pty::terminate_session);
            if terminated {
                write_message(
                    &mut stream,
                    &SessionMessage::TerminateResult(TerminateResult {
                        outcome: TerminationOutcome::Terminated,
                    }),
                )
            } else {
                protocol_error(
                    &mut stream,
                    "termination_incomplete",
                    "session members remain alive",
                )
            }
        }
        SessionMessage::TerminateAll(_) => {
            let targets = lock(&state)
                .sessions
                .values()
                .map(|session| session.session_id)
                .collect::<Vec<_>>();
            if targets.is_empty() {
                return write_message(
                    &mut stream,
                    &SessionMessage::TerminateResult(TerminateResult {
                        outcome: TerminationOutcome::NoSessions,
                    }),
                );
            }
            let mut complete = true;
            let mut workers = Vec::new();
            for target in targets {
                match std::thread::Builder::new().spawn(move || pty::terminate_session(target)) {
                    Ok(worker) => workers.push(worker),
                    Err(_) => complete &= pty::terminate_session(target),
                }
            }
            for worker in workers {
                complete &= worker.join().unwrap_or(false);
            }
            if complete {
                write_message(
                    &mut stream,
                    &SessionMessage::TerminateResult(TerminateResult {
                        outcome: TerminationOutcome::Terminated,
                    }),
                )
            } else {
                protocol_error(
                    &mut stream,
                    "termination_incomplete",
                    "session members remain alive",
                )
            }
        }
        _ => protocol_error(
            &mut stream,
            "unexpected_message",
            "message is not valid as a connection opener",
        ),
    }
}

enum AttachMode {
    Create(AttachRequest),
    Existing(AttachExisting),
}

fn handle_attach(
    mut stream: UnixStream,
    state: Arc<Mutex<DaemonState>>,
    mode: AttachMode,
) -> Result<(), WireError> {
    stream.set_read_timeout(None)?;
    let (session_id, size) = match &mode {
        AttachMode::Create(request) => (request.session_id.clone(), request.size),
        AttachMode::Existing(request) => (request.session_id.clone(), request.size),
    };
    let created = {
        let mut daemon = lock(&state);
        if !daemon.sessions.contains_key(&session_id) {
            let AttachMode::Create(request) = &mode else {
                return protocol_error(&mut stream, "session_missing", "session does not exist");
            };
            if daemon.sessions.len() >= MAX_SESSIONS {
                return protocol_error(&mut stream, "session_limit", "session capacity is full");
            }
            let child = match pty::spawn(&request.launch, request.size) {
                Ok(child) => child,
                Err(error) => {
                    return protocol_error(&mut stream, "spawn_failed", &error.to_string());
                }
            };
            let reader = child.master.try_clone()?;
            let tracker = ShellActivityTracker::default();
            let activity_session = tracker.begin_session();
            let descriptor = SessionDescriptor {
                session_id: request.session_id.clone(),
                owner: request.owner.clone(),
                working_directory: request.launch.working_directory.clone(),
                shell_pid: child.pid as u32,
                tty_device: child.tty_device,
                command_activity: CommandActivity::Idle,
            };
            daemon.sessions.insert(
                request.session_id.clone(),
                Session {
                    descriptor,
                    session_id: child.pid,
                    master: Arc::new(Mutex::new(child.master)),
                    replay: VecDeque::with_capacity(REPLAY_BYTES),
                    client: None,
                    next_generation: 0,
                    activity_tracker: tracker,
                    activity_session,
                    exiting: false,
                },
            );
            start_session_threads(state.clone(), request.session_id.clone(), child.pid, reader);
            true
        } else {
            false
        }
    };

    let writer_stream = stream.try_clone()?;
    let shutdown = stream.try_clone()?;
    let messages = Arc::new(ClientQueue::new());
    let (generation, master) = {
        let mut daemon = lock(&state);
        let Some(session) = daemon.sessions.get_mut(&session_id) else {
            return protocol_error(
                &mut stream,
                "session_missing",
                "session exited during attach",
            );
        };
        let master = session.master.clone();
        session.next_generation = session.next_generation.wrapping_add(1);
        let generation = session.next_generation;
        if let Some(old) = session.client.take() {
            old.messages.close();
            let _ = old.shutdown.shutdown(std::net::Shutdown::Both);
        }
        let attached = SessionMessage::Attached(Attached {
            created,
            descriptor: descriptor(session),
        });
        if !messages.push(attached)
            || !messages.push(SessionMessage::Replay(
                session.replay.iter().copied().collect(),
            ))
        {
            return Err(queue_wire_error());
        }
        session.client = Some(ClientSink {
            generation,
            messages: messages.clone(),
            shutdown,
        });
        (generation, master)
    };
    let _ = pty::resize(lock(&master).as_raw_fd(), size);
    let writer_messages = messages.clone();
    std::thread::spawn(move || client_writer(writer_stream, writer_messages));

    loop {
        let message = match read_message(&mut stream) {
            Ok(message) => message,
            Err(WireError::Io(error))
                if matches!(
                    error.kind(),
                    io::ErrorKind::UnexpectedEof
                        | io::ErrorKind::ConnectionReset
                        | io::ErrorKind::BrokenPipe
                ) =>
            {
                break;
            }
            Err(error) => {
                queue_protocol_error(&messages, "invalid_frame", &error.to_string());
                break;
            }
        };
        let current = {
            let daemon = lock(&state);
            daemon
                .sessions
                .get(&session_id)
                .and_then(|session| session.client.as_ref())
                .is_some_and(|client| client.generation == generation)
        };
        if !current {
            break;
        }
        match message {
            SessionMessage::Input(bytes) if bytes.len() <= MAX_SESSION_INPUT_BYTES => {
                let master = {
                    let daemon = lock(&state);
                    daemon
                        .sessions
                        .get(&session_id)
                        .map(|session| session.master.clone())
                };
                let Some(master) = master else {
                    break;
                };
                if write_input(&master, &bytes).is_err() {
                    break;
                }
            }
            SessionMessage::Resize(size) => {
                let master = {
                    let daemon = lock(&state);
                    daemon
                        .sessions
                        .get(&session_id)
                        .map(|session| session.master.clone())
                };
                if let Some(master) = master {
                    let _ = pty::resize(lock(&master).as_raw_fd(), size);
                }
            }
            _ => {
                queue_protocol_error(
                    &messages,
                    "unexpected_message",
                    "attach connection accepts only input and resize",
                );
                break;
            }
        }
    }
    clear_client(&state, &session_id, generation);
    Ok(())
}

fn start_session_threads(
    state: Arc<Mutex<DaemonState>>,
    session_id: String,
    child_pid: i32,
    reader: File,
) {
    let output_state = state.clone();
    let output_id = session_id.clone();
    let (output_finished, output_completion) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        read_output(reader, output_state, output_id);
        let _ = output_finished.send(());
    });
    std::thread::spawn(move || {
        let exit = pty::wait_for_child(child_pid);
        let _ = pty::terminate_session(child_pid);
        if let Some(session) = lock(&state).sessions.get_mut(&session_id) {
            session.exiting = true;
        }
        let _ = output_completion.recv_timeout(OUTPUT_DRAIN_TIMEOUT);
        let client = {
            let mut daemon = lock(&state);
            let client = daemon
                .sessions
                .remove(&session_id)
                .and_then(|session| session.client);
            daemon.last_activity = Instant::now();
            client
        };
        if let Some(client) = client {
            let _ = client.messages.push(SessionMessage::Exited(exit));
            client.messages.close();
        }
    });
}

fn session_finished(state: &Arc<Mutex<DaemonState>>, session_id: &str) -> bool {
    lock(state)
        .sessions
        .get(session_id)
        .is_none_or(|session| session.exiting)
}

fn read_output(mut reader: File, state: Arc<Mutex<DaemonState>>, session_id: String) {
    let mut buffer = [0u8; OUTPUT_CHUNK_BYTES];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                if session_finished(&state, &session_id) {
                    return;
                }
                std::thread::sleep(EMPTY_READ_BACKOFF);
            }
            Ok(length) => broadcast_output(&state, &session_id, &buffer[..length]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if session_finished(&state, &session_id) {
                    return;
                }
                std::thread::sleep(EMPTY_READ_BACKOFF);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

fn broadcast_output(state: &Arc<Mutex<DaemonState>>, session_id: &str, bytes: &[u8]) {
    let mut daemon = lock(state);
    let Some(session) = daemon.sessions.get_mut(session_id) else {
        return;
    };
    session.activity_session.record_output(bytes);
    for byte in bytes {
        if session.replay.len() == REPLAY_BYTES {
            session.replay.pop_front();
        }
        session.replay.push_back(*byte);
    }
    let Some(client) = &session.client else {
        return;
    };
    if !client.messages.push(SessionMessage::Output(bytes.to_vec()))
        && let Some(client) = session.client.take()
    {
        client.messages.close();
        let _ = client.shutdown.shutdown(std::net::Shutdown::Both);
    }
}

fn client_writer(mut stream: UnixStream, messages: Arc<ClientQueue>) {
    while let Some(message) = messages.pop() {
        if write_message(&mut stream, &message).is_err() {
            messages.close();
            let _ = stream.shutdown(std::net::Shutdown::Both);
            return;
        }
    }
}

fn write_input(master: &Arc<Mutex<File>>, bytes: &[u8]) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut master = lock(master);
    let mut offset = 0;
    while offset < bytes.len() {
        match master.write(&bytes[offset..]) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(length) => offset += length,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                pty::wait_ready(master.as_raw_fd(), true, remaining);
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn clear_client(state: &Arc<Mutex<DaemonState>>, session_id: &str, generation: u64) {
    let mut daemon = lock(state);
    let Some(session) = daemon.sessions.get_mut(session_id) else {
        return;
    };
    if session
        .client
        .as_ref()
        .is_some_and(|client| client.generation == generation)
        && let Some(client) = session.client.take()
    {
        client.messages.close();
    }
    daemon.last_activity = Instant::now();
}

fn descriptor(session: &Session) -> SessionDescriptor {
    let mut descriptor = session.descriptor.clone();
    descriptor.command_activity = command_activity(session.activity_tracker.activity());
    descriptor
}

fn command_activity(activity: PersistentCommandActivity) -> CommandActivity {
    match activity {
        PersistentCommandActivity::Idle => CommandActivity::Idle,
        PersistentCommandActivity::Running => CommandActivity::Running,
        PersistentCommandActivity::Unknown => CommandActivity::Unknown,
    }
}

fn protocol_error(stream: &mut UnixStream, code: &str, message: &str) -> Result<(), WireError> {
    write_message(
        stream,
        &SessionMessage::ProtocolError(ProtocolFailure {
            code: code.to_owned(),
            message: message.chars().take(4096).collect(),
        }),
    )
}

fn queue_protocol_error(messages: &ClientQueue, code: &str, message: &str) {
    let _ = messages.push(SessionMessage::ProtocolError(ProtocolFailure {
        code: code.to_owned(),
        message: message.chars().take(4096).collect(),
    }));
    messages.close();
}

fn message_bytes(message: &SessionMessage) -> usize {
    match message {
        SessionMessage::Replay(bytes) | SessionMessage::Output(bytes) => bytes.len(),
        _ => muxy_proto::session::SessionCodec::encode(message).map_or(0, |frame| frame.len()),
    }
}

fn queue_wire_error() -> WireError {
    WireError::Io(io::Error::new(
        io::ErrorKind::BrokenPipe,
        "client output queue rejected message",
    ))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_match_the_locked_memory_contract() {
        assert_eq!(MAX_CONNECTIONS, 64);
        assert_eq!(MAX_CLIENT_OUTPUT_BYTES, 4 * 1024 * 1024);
        assert_eq!(MAX_SESSION_INPUT_BYTES, 1024 * 1024);
        assert_eq!(REPLAY_BYTES, 256 * 1024);
        let queue = ClientQueue::new();
        assert!(queue.push(SessionMessage::Output(vec![0; MAX_CLIENT_OUTPUT_BYTES])));
        assert!(!queue.push(SessionMessage::Output(vec![0])));
    }

    #[test]
    fn daemon_arguments_are_bounded_and_explicit() {
        let (path, idle) = parse_arguments(vec![
            "--socket".into(),
            "/tmp/session-control.test".into(),
            "--idle-ms".into(),
            "500".into(),
        ])
        .unwrap();
        assert_eq!(path, PathBuf::from("/tmp/session-control.test"));
        assert_eq!(idle, Duration::from_millis(500));
        assert!(parse_arguments(vec!["--socket".into()]).is_err());
        assert!(
            parse_arguments(vec!["--socket".into(), "relative".into(), "extra".into()]).is_err()
        );
    }

    #[test]
    fn replay_retains_only_the_newest_bytes() {
        let tracker = ShellActivityTracker::default();
        let session_tracker = tracker.begin_session();
        let descriptor = SessionDescriptor {
            session_id: "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA".to_owned(),
            owner: muxy_proto::session::OwnerMetadata {
                project_id: "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB".to_owned(),
                worktree_id: None,
                title: "test".to_owned(),
            },
            working_directory: "/tmp".to_owned(),
            shell_pid: 1,
            tty_device: 1,
            command_activity: CommandActivity::Idle,
        };
        let temp = tempfile::tempfile().unwrap();
        let state = Arc::new(Mutex::new(DaemonState {
            sessions: HashMap::from([(
                descriptor.session_id.clone(),
                Session {
                    descriptor,
                    session_id: 1,
                    master: Arc::new(Mutex::new(temp)),
                    replay: VecDeque::new(),
                    client: None,
                    next_generation: 0,
                    activity_tracker: tracker,
                    activity_session: session_tracker,
                    exiting: false,
                },
            )]),
            last_activity: Instant::now(),
        }));
        let id = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA";
        broadcast_output(&state, id, &vec![1; REPLAY_BYTES]);
        broadcast_output(&state, id, &[2; 17]);
        let daemon = lock(&state);
        let replay = &daemon.sessions[id].replay;
        assert_eq!(replay.len(), REPLAY_BYTES);
        assert!(replay.iter().rev().take(17).all(|byte| *byte == 2));
    }
}
