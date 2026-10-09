use std::error::Error;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;

#[path = "client/attachment_timeout.rs"]
mod attachment_timeout;
#[path = "client/clear.rs"]
mod clear;
#[path = "client/close.rs"]
mod close;
#[path = "client/colors.rs"]
mod colors;
#[path = "client/input.rs"]
mod input;
#[path = "client/observe.rs"]
mod observe;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use muxy_client::{Attachment, Client, ClientError, ClientEvent, RunGrid};
use muxy_protocol::transport::{ByteStream, StreamCancellation};
use muxy_protocol::wire::{Decoder, Encoder, WireError};
use muxy_protocol::{CONTROL, ChannelId, ErrorCode, ExitReason, Message, ScreenFrame, Size};
use muxy_server::{Registry, ServerEvent, ServerSettings, connection::serve};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const TIMEOUT: Duration = Duration::from_secs(10);
const QUIET: Duration = Duration::from_millis(150);
const SIZE: Size = Size { cols: 80, rows: 24 };
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    registry: Arc<Registry>,
    subscribers: Arc<Mutex<Vec<Sender<ServerEvent>>>>,
    running: Arc<AtomicBool>,
    directory: PathBuf,
}

struct Connection {
    client: Client,
    events: Receiver<ClientEvent>,
    server: Box<dyn StreamCancellation>,
    finished: Receiver<Result<(), WireError>>,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        Self::with_startup("")
    }

    fn with_startup(script: &str) -> TestResult<Self> {
        Self::with_storage(script, false)
    }

    fn with_storage(script: &str, persistent: bool) -> TestResult<Self> {
        let directory = std::env::temp_dir().join(format!(
            "muxy-client-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory)?;
        let shell = if script.is_empty() {
            PathBuf::from("/bin/sh")
        } else {
            let shell = directory.join("shell");
            fs::write(
                &shell,
                format!(
                    "#!/bin/sh
{script}
exec /bin/sh -l
"
                ),
            )?;
            fs::set_permissions(&shell, fs::Permissions::from_mode(0o700))?;
            shell
        };
        let (sender, events) = mpsc::channel();
        let settings = ServerSettings {
            default_shell: Some(shell),
            ..ServerSettings::default()
        };
        let registry = Arc::new(if persistent {
            Registry::persistent(settings, sender, &directory.join("sessions"))?
        } else {
            Registry::new(settings, sender)
        });
        let subscribers = Arc::new(Mutex::new(Vec::<Sender<ServerEvent>>::new()));
        let sinks = Arc::clone(&subscribers);
        let running = Arc::new(AtomicBool::new(true));
        let active = Arc::clone(&running);
        thread::spawn(move || {
            while active.load(Ordering::Relaxed) {
                match events.recv_timeout(QUIET) {
                    Ok(event) => {
                        if let Ok(mut sinks) = sinks.lock() {
                            sinks.retain(|sink| sink.send(event.clone()).is_ok());
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Ok(Self {
            registry,
            subscribers,
            running,
            directory,
        })
    }

    fn connect(&self) -> TestResult<Connection> {
        self.connect_stream(|socket| Box::new(socket))
    }

    fn connect_stream(
        &self,
        stream: impl FnOnce(UnixStream) -> Box<dyn ByteStream>,
    ) -> TestResult<Connection> {
        let (socket, server) = UnixStream::pair()?;
        let server: Box<dyn ByteStream> = Box::new(server);
        let cancellation = server.cancellation()?;
        let (sender, events) = mpsc::channel();
        self.subscribers
            .lock()
            .map_err(|_| "subscriber lock poisoned")?
            .push(sender);
        let registry = Arc::clone(&self.registry);
        let (done, finished) = mpsc::channel();
        thread::spawn(move || {
            let _ = done.send(serve(server, registry, events));
        });
        let client = Client::from_stream(stream(socket))?;
        let events = client.events().ok_or("events already taken")?;
        Ok(Connection {
            client,
            events,
            server: cancellation,
            finished,
        })
    }

    fn create(&self, client: &Client) -> TestResult<muxy_protocol::SessionInfo> {
        Ok(client.create_session(&self.directory, SIZE)?)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for session in self.registry.list() {
            let _ = self.registry.end(session.id);
        }
        let deadline = Instant::now() + TIMEOUT;
        while !self.registry.list().is_empty() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        self.running.store(false, Ordering::Relaxed);
        let _ = fs::remove_dir_all(&self.directory);
    }
}

impl Connection {
    fn next_frame(&self, channel: ChannelId) -> TestResult<ScreenFrame> {
        loop {
            match self.events.recv_timeout(TIMEOUT)? {
                ClientEvent::Frame {
                    channel: received,
                    frame,
                } if received == channel => return Ok(frame),
                ClientEvent::Metadata { .. }
                | ClientEvent::SessionsChanged { .. }
                | ClientEvent::SessionMetadata { .. }
                | ClientEvent::ActivityChanged { .. }
                | ClientEvent::Progress { .. }
                | ClientEvent::FilesChanged { .. }
                | ClientEvent::GitChanged { .. }
                | ClientEvent::CatalogChanged { .. } => {}
                other => return Err(format!("expected frame, got {other:?}").into()),
            }
        }
    }

    fn frame_containing(
        &self,
        attachment: &mut Attachment,
        needle: &str,
    ) -> TestResult<ScreenFrame> {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if Instant::now() > deadline {
                return Err("frame never contained expected text".into());
            }
            let frame = self.next_frame(attachment.channel)?;
            attachment.grid.apply(&frame);
            if text(&attachment.grid).contains(needle) {
                return Ok(frame);
            }
            self.client.ack(attachment.channel, frame.seq)?;
        }
    }

    fn quiet(&self, attachment: &mut Attachment) -> TestResult {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            match self.events.recv_timeout(QUIET) {
                Ok(ClientEvent::Frame { channel, frame })
                    if channel == attachment.channel && Instant::now() < deadline =>
                {
                    attachment.grid.apply(&frame);
                    self.client.ack(channel, frame.seq)?;
                }
                Ok(
                    ClientEvent::Metadata { .. }
                    | ClientEvent::SessionsChanged { .. }
                    | ClientEvent::SessionMetadata { .. }
                    | ClientEvent::ActivityChanged { .. }
                    | ClientEvent::Progress { .. }
                    | ClientEvent::FilesChanged { .. }
                    | ClientEvent::GitChanged { .. }
                    | ClientEvent::CatalogChanged { .. },
                ) => {}
                Err(RecvTimeoutError::Timeout) => return Ok(()),
                other => return Err(format!("connection did not become quiet: {other:?}").into()),
            }
        }
    }

    fn expect_ended(&self, session: muxy_protocol::SessionId) -> TestResult<ExitReason> {
        loop {
            match self.events.recv_timeout(TIMEOUT)? {
                ClientEvent::SessionEnded {
                    session: ended,
                    reason,
                } if ended == session => return Ok(reason),
                ClientEvent::Frame { .. }
                | ClientEvent::Metadata { .. }
                | ClientEvent::SessionsChanged { .. }
                | ClientEvent::SessionMetadata { .. }
                | ClientEvent::ActivityChanged { .. }
                | ClientEvent::Progress { .. }
                | ClientEvent::FilesChanged { .. }
                | ClientEvent::GitChanged { .. }
                | ClientEvent::CatalogChanged { .. } => {}
                other => return Err(format!("expected session ended, got {other:?}").into()),
            }
        }
    }

    /// Disconnects and waits for the server to let go of the connection. A
    /// write the server had in flight may fail on the closed socket first.
    fn disconnect(&self) -> TestResult {
        self.client.disconnect();
        match self.finished.recv_timeout(TIMEOUT)? {
            Ok(()) => Ok(()),
            Err(WireError::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn text(grid: &RunGrid) -> String {
    (0..grid.rows.len())
        .map(|index| grid.row_text(index))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn connect_list_create_attach_input_and_end() -> TestResult {
    let fixture = Fixture::new()?;
    let connection = fixture.connect()?;
    let client = &connection.client;
    assert!(client.is_connected());
    assert_eq!(client.list_sessions()?, vec![]);
    let info = fixture.create(client)?;
    assert_eq!(client.list_sessions()?, vec![info.clone()]);
    let mut attachment = client.attach(info.id, SIZE)?;
    assert_ne!(attachment.channel, CONTROL);
    assert_eq!(attachment.grid.size, SIZE);
    assert_eq!(attachment.grid.rows.len(), usize::from(SIZE.rows));
    assert_eq!(
        attachment.directory.0,
        fixture.directory.canonicalize()?.as_os_str().as_bytes()
    );
    client.send_input(attachment.channel, b"echo hel\"\"lo\n")?;
    connection.frame_containing(&mut attachment, "hello")?;
    client.end_session(info.id)?;
    assert_eq!(connection.expect_ended(info.id)?, ExitReason::Ended);
    assert_eq!(client.list_sessions()?, vec![]);
    Ok(())
}

#[test]
fn request_errors_are_correlated_and_invalid_requests_never_leave_the_client() -> TestResult {
    let fixture = Fixture::new()?;
    let connection = fixture.connect()?;
    let client = &connection.client;
    let unknown = muxy_protocol::SessionId::new(1).ok_or("zero session")?;
    assert!(matches!(
        client.attach(unknown, SIZE),
        Err(ClientError::Server(error)) if error.code == ErrorCode::UnknownSession
    ));
    assert!(matches!(
        client.create_session(&fixture.directory, Size { cols: 0, rows: 1 }),
        Err(ClientError::Invalid(ErrorCode::BadSize))
    ));
    assert!(matches!(
        client.send_input(ChannelId(1), &vec![0; muxy_protocol::MAX_INPUT + 1]),
        Err(ClientError::Invalid(ErrorCode::BadRequest))
    ));
    assert!(matches!(
        client.send_input(CONTROL, b"x"),
        Err(ClientError::Invalid(ErrorCode::UnknownChannel))
    ));
    assert!(matches!(
        client.ack(CONTROL, 1),
        Err(ClientError::Invalid(ErrorCode::UnknownChannel))
    ));
    client.ping()?;
    Ok(())
}

#[test]
fn server_exit_disconnects_the_client() -> TestResult {
    let fixture = Fixture::new()?;
    let connection = fixture.connect()?;
    let client = connection.client.clone();
    let info = fixture.create(&client)?;
    let mut attachment = client.attach(info.id, SIZE)?;
    connection.quiet(&mut attachment)?;
    connection.server.cancel();
    connection.finished.recv_timeout(TIMEOUT)??;
    loop {
        match connection.events.recv_timeout(TIMEOUT)? {
            ClientEvent::ServerRestarting | ClientEvent::Disconnected => break,
            ClientEvent::Frame { .. }
            | ClientEvent::Metadata { .. }
            | ClientEvent::SessionsChanged { .. }
            | ClientEvent::SessionMetadata { .. }
            | ClientEvent::ActivityChanged { .. }
            | ClientEvent::Progress { .. }
            | ClientEvent::FilesChanged { .. }
            | ClientEvent::GitChanged { .. }
            | ClientEvent::CatalogChanged { .. }
            | ClientEvent::RemoteAccessChanged { .. } => {}
            other @ ClientEvent::SessionEnded { .. } => {
                return Err(format!("expected disconnect, got {other:?}").into());
            }
        }
    }
    assert!(!client.is_connected());
    assert!(matches!(client.ping(), Err(ClientError::Disconnected)));
    assert!(matches!(
        connection.client.ack(attachment.channel, 1),
        Err(ClientError::Disconnected)
    ));
    Ok(())
}

#[test]
fn history_reads_refresh_a_coherent_boundary_and_merge_all_older_pages() -> TestResult {
    use muxy_protocol::HistoryCursor;
    let fixture = Fixture::new()?;
    let connection = fixture.connect()?;
    let session = fixture.create(&connection.client)?;
    let mut attachment = connection.client.attach(session.id, SIZE)?;
    connection.quiet(&mut attachment)?;
    connection.client.send_input(
        attachment.channel,
        // The shell's own job-control warnings would land in the history.
        b"exec 2>/dev/null; stty -echo; PS1=''; printf '\\033[2J\\033[H\\033[3J'; seq 1 5000; printf HISTORY_READY\n",
    )?;
    let deadline = Instant::now() + TIMEOUT;
    let page = loop {
        let page = connection
            .client
            .history_page(attachment.channel, HistoryCursor(0), 200)?;
        if page.screen.as_ref().is_some_and(|screen| {
            screen.rows.iter().any(|row| {
                row.runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>()
                    .trim_end()
                    == "HISTORY_READY"
            })
        }) {
            break page;
        }
        if Instant::now() >= deadline {
            return Err("history output never completed".into());
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(page.rows.len(), 200);
    attachment.grid.replace_history(page);
    assert_eq!(attachment.grid.row_text(0).trim_end(), "4978");
    while let Some(before) = attachment.grid.history_cursor {
        let page = connection
            .client
            .history_page(attachment.channel, before, 500)?;
        attachment.grid.fetch_older(page);
    }
    let history = attachment
        .grid
        .history
        .iter()
        .map(|row| {
            row.runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        history,
        (1..=4977)
            .map(|number| number.to_string())
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        connection
            .client
            .history_page(CONTROL, HistoryCursor(0), 200),
        Err(ClientError::Invalid(ErrorCode::UnknownChannel))
    ));
    assert!(matches!(
        connection
            .client
            .history_page(attachment.channel, HistoryCursor(0), 501),
        Err(ClientError::Invalid(ErrorCode::BadRequest))
    ));
    connection.client.ping()?;
    Ok(())
}

#[path = "client/activity.rs"]
mod activity;

#[test]
fn asynchronous_session_presence_includes_saved_history_without_mutating_projects() -> TestResult {
    let fixture = Fixture::new()?;
    let connection = fixture.connect()?;
    let project = fixture.registry.home_project();
    let has_sessions = || -> TestResult<bool> {
        let (send, receive) = mpsc::channel();
        connection
            .client
            .project_has_sessions_async(project)
            .on_complete(move |result| {
                let _ = send.send(result);
            });
        Ok(receive.recv_timeout(TIMEOUT)??)
    };
    let revision = connection.client.catalog()?.revision;
    for _ in 0..3 {
        assert!(!has_sessions()?);
    }
    assert_eq!(connection.client.catalog()?.revision, revision);
    let session = fixture.create(&connection.client)?;
    assert!(has_sessions()?);
    fixture.registry.end(session.id)?;
    assert!(has_sessions()?);
    fixture.registry.discard(session.id)?;
    assert!(!has_sessions()?);
    Ok(())
}
