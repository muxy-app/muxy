//! Connecting through an SSH channel that the app opens itself. The fake
//! channel's command is a bridge in this process: it prints the shell's
//! noise and the ready line, then relays to an in-process server.

use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use muxy_client::Client;
use muxy_mobile::{
    BridgeChannel, ChannelWriter, ClientKind, Connection, ConnectionEvent, ConnectionListener, Key,
    MobileError, Modifiers,
};
use muxy_protocol::transport::relay;
use muxy_server::connection::serve;
use muxy_server::{Registry, ServerEvent, ServerSettings};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const TIMEOUT: Duration = Duration::from_secs(10);

/// The app's side of the channel: what the SDK writes goes to the remote
/// command's stdin.
struct Writer {
    stdin: UnixStream,
    closed: Mutex<Sender<()>>,
}

impl ChannelWriter for Writer {
    fn write(&self, bytes: Vec<u8>) -> Result<(), MobileError> {
        (&self.stdin)
            .write_all(&bytes)
            .map_err(|_| MobileError::Disconnected)
    }

    fn close(&self) -> Result<(), MobileError> {
        let _ = self.stdin.shutdown(Shutdown::Write);
        let _ = self
            .closed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(());
        Ok(())
    }
}

/// A channel the test feeds by hand, and what it learns from the SDK.
struct Fed {
    channel: Arc<BridgeChannel>,
    /// Receives once the SDK closes the channel.
    closed: Receiver<()>,
    /// The remote command's end of stdin.
    stdin: UnixStream,
}

fn fed() -> TestResult<Fed> {
    let (stdin, remote_stdin) = UnixStream::pair()?;
    let (sender, closed) = mpsc::channel();
    let channel = BridgeChannel::new(Box::new(Writer {
        stdin,
        closed: Mutex::new(sender),
    }))?;
    Ok(Fed {
        channel,
        closed,
        stdin: remote_stdin,
    })
}

/// A computer whose server runs in this process, reached through a channel
/// whose command prints `noise` before starting the bridge.
struct Remote {
    channel: Arc<BridgeChannel>,
    closed: Receiver<()>,
    registry: Arc<Registry>,
    _events: Receiver<ServerEvent>,
}

impl Remote {
    fn open(noise: Vec<u8>) -> TestResult<Self> {
        let (sender, events) = mpsc::channel();
        let registry = Arc::new(Registry::new(
            ServerSettings {
                default_shell: Some(PathBuf::from("/bin/sh")),
                ..ServerSettings::default()
            },
            sender,
        ));
        let Fed {
            channel,
            closed,
            stdin,
        } = fed()?;
        let (mut stdout, mut remote_stdout) = UnixStream::pair()?;
        let (bridge, server) = UnixStream::pair()?;
        let serving = Arc::clone(&registry);
        thread::spawn(move || {
            let (_keep, events) = mpsc::channel();
            let _ = serve(Box::new(server), serving, events);
        });
        thread::spawn(move || -> std::io::Result<()> {
            remote_stdout.write_all(&noise)?;
            remote_stdout.write_all(b"MUXY-STDIO/1\n")?;
            relay(Box::new(bridge), stdin, remote_stdout)
        });
        let feeding = Arc::clone(&channel);
        thread::spawn(move || {
            let mut chunk = [0; 4096];
            while let Ok(read @ 1..) = stdout.read(&mut chunk) {
                feeding.receive(chunk[..read].to_vec());
            }
            feeding.finish(Some(0));
        });
        Ok(Self {
            channel,
            closed,
            registry,
            _events: events,
        })
    }

    fn connect(&self) -> Result<(Arc<Connection>, Receiver<ConnectionEvent>), MobileError> {
        let (recorder, events) = listener();
        let connection =
            Connection::connect_channel(Arc::clone(&self.channel), "dev@box".into(), recorder)?;
        Ok((connection, events))
    }

    /// A client on the computer itself.
    fn local(&self) -> TestResult<Client> {
        let (client, server) = UnixStream::pair()?;
        let registry = Arc::clone(&self.registry);
        thread::spawn(move || {
            let (_keep, events) = mpsc::channel();
            let _ = serve(Box::new(server), registry, events);
        });
        Ok(Client::from_stream(Box::new(client))?)
    }
}

struct Recorder(Mutex<Sender<ConnectionEvent>>);

impl ConnectionListener for Recorder {
    fn on_event(&self, event: ConnectionEvent) {
        let _ = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send(event);
    }
}

fn listener() -> (Arc<Recorder>, Receiver<ConnectionEvent>) {
    let (sender, events) = mpsc::channel();
    (Arc::new(Recorder(Mutex::new(sender))), events)
}

fn wait_for(
    events: &Receiver<ConnectionEvent>,
    mut done: impl FnMut(&ConnectionEvent) -> bool,
) -> TestResult {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let event = events.recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
        if done(&event) {
            return Ok(());
        }
    }
}

fn unreachable(error: MobileError) -> TestResult<String> {
    match error {
        MobileError::Unreachable { reason } => Ok(reason),
        other => Err(format!("expected unreachable, got {other:?}").into()),
    }
}

#[test]
fn a_phone_uses_a_server_over_ssh_as_it_would_when_paired() -> TestResult {
    let remote = Remote::open(b"Welcome to box!\nLast login: today\n".to_vec())?;
    let (connection, events) = remote.connect()?;
    assert_eq!(
        connection.server_id()?,
        remote.local()?.catalog()?.server.to_string()
    );
    assert_eq!(connection.server_version(), env!("CARGO_PKG_VERSION"));
    let home = connection
        .projects()?
        .into_iter()
        .find(|project| project.is_home)
        .ok_or("no Home project")?;
    let session = connection.create_session(home.id.clone(), 40, 6)?;
    let terminal = connection.attach(session.id, 40, 6)?;
    terminal.send_input(b"echo ssh-$((40+2))".to_vec())?;
    terminal.send_key(Key::Enter, Modifiers::default())?;
    let text = || -> Vec<String> {
        terminal
            .screen()
            .lines
            .iter()
            .map(|line| line.spans.iter().map(|span| span.text.as_str()).collect())
            .collect()
    };
    wait_for(&events, |event| {
        matches!(event, ConnectionEvent::ScreenChanged { session_id } if *session_id == session.id)
            && text().iter().any(|line| line.trim() == "ssh-42")
    })
    .map_err(|error| format!("{error}; screen: {:?}", text()))?;
    let listed = connection
        .sessions(home.id)?
        .into_iter()
        .find(|listed| listed.id == session.id)
        .ok_or("the session is not listed")?;
    assert!(listed.attached);
    assert_eq!(listed.owner, Some(ClientKind::Mobile));
    connection.end_session(session.id)?;
    Ok(())
}

#[test]
fn a_computer_without_muxy_is_named_and_the_channel_closed() -> TestResult {
    let fed = fed()?;
    fed.channel.receive(b"Welcome to box!\n".to_vec());
    fed.channel
        .receive_error(b"sh: 1: exec: muxy: not found\n".to_vec());
    fed.channel.finish(Some(127));
    let error =
        Connection::connect_channel(Arc::clone(&fed.channel), "dev@box".into(), listener().0)
            .err()
            .ok_or("connected without a bridge")?;
    assert_eq!(
        error.to_string(),
        "The server is unreachable: Muxy isn't installed on dev@box (looked on PATH and in ~/.local/bin)."
    );
    fed.closed.recv_timeout(TIMEOUT)?;

    let again = Connection::connect_channel(fed.channel, "dev@box".into(), listener().0)
        .err()
        .ok_or("a used channel connected again")?;
    assert!(unreachable(again)?.contains("already used"));
    assert!(fed.closed.try_recv().is_err(), "the channel closed twice");
    Ok(())
}

#[test]
fn giving_up_releases_a_delivery_that_waits_for_the_sdk() -> TestResult {
    let fed = fed()?;
    let channel = Arc::clone(&fed.channel);
    let (sender, delivered) = mpsc::channel();
    thread::spawn(move || {
        channel.receive(vec![b'x'; 1024 * 1024]);
        let _ = sender.send(());
    });
    assert!(delivered.recv_timeout(Duration::from_millis(100)).is_err());
    fed.channel.finish(None);
    delivered.recv_timeout(TIMEOUT)?;
    Ok(())
}

#[test]
fn dropping_the_connection_closes_the_channel() -> TestResult {
    let remote = Remote::open(Vec::new())?;
    let (connection, _events) = remote.connect()?;
    assert!(remote.closed.try_recv().is_err());
    drop(connection);
    remote.closed.recv_timeout(TIMEOUT)?;
    assert!(
        remote
            .closed
            .recv_timeout(Duration::from_millis(100))
            .is_err(),
        "the channel closed twice"
    );
    Ok(())
}

#[test]
fn a_channel_that_closes_disconnects_the_phone() -> TestResult {
    let remote = Remote::open(Vec::new())?;
    let (_connection, events) = remote.connect()?;
    remote.channel.finish(None);
    wait_for(&events, |event| *event == ConnectionEvent::Disconnected)?;
    remote.closed.recv_timeout(TIMEOUT)?;
    Ok(())
}
