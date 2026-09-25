use std::net::{SocketAddr, TcpListener};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};
use std::thread;
use std::time::{Duration, Instant};

use muxy_client::Client;
use muxy_mobile::{
    Connection, ConnectionEvent, ConnectionListener, Key, Line, MobileError, Modifiers,
    ServerCredential,
};
use muxy_protocol::transport::Listener;
use muxy_protocol::transport::tls::TlsListener;
use muxy_protocol::{ListenerStatus, RemoteAccessSettings};
use muxy_server::connection::{serve, serve_remote};
use muxy_server::{Registry, ServerEvent, ServerSettings};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const TIMEOUT: Duration = Duration::from_secs(10);

type Subscribers = Arc<Mutex<Vec<Sender<ServerEvent>>>>;

/// An in-process server whose network listener is a real TLS listener on loopback.
struct Server {
    registry: Arc<Registry>,
    subscribers: Subscribers,
}

/// Forwards server events such as session ends to every connection, as the executable does.
fn subscribe(subscribers: &Subscribers) -> Receiver<ServerEvent> {
    let (sender, events) = mpsc::channel();
    subscribers
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(sender);
    events
}

impl Server {
    fn new() -> Self {
        let (sender, events) = mpsc::channel::<ServerEvent>();
        let subscribers = Subscribers::default();
        let sinks = Arc::clone(&subscribers);
        thread::spawn(move || {
            for event in events {
                sinks
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .retain(|sink| sink.send(event.clone()).is_ok());
            }
        });
        let slot = Arc::new(OnceLock::<Weak<Registry>>::new());
        let owner = Arc::clone(&slot);
        let remote_subscribers = Arc::clone(&subscribers);
        let running = Mutex::new(None::<Arc<TlsListener>>);
        let registry = Arc::new(
            Registry::new(
                ServerSettings {
                    default_shell: Some(PathBuf::from("/bin/sh")),
                    ..ServerSettings::default()
                },
                sender,
            )
            .with_remote_listener(move |listening| {
                let mut running = running.lock().unwrap_or_else(PoisonError::into_inner);
                if let Some(previous) = running.take() {
                    previous.close();
                }
                let Some((port, identity)) = listening else {
                    return ListenerStatus::Disabled;
                };
                match TlsListener::bind(SocketAddr::from(([127, 0, 0, 1], port)), &identity) {
                    Ok(listener) => {
                        let listener = Arc::new(listener);
                        *running = Some(Arc::clone(&listener));
                        let registry = owner.get().cloned().unwrap_or_default();
                        let subscribers = Arc::clone(&remote_subscribers);
                        thread::spawn(move || accept(&listener, &registry, &subscribers));
                        ListenerStatus::Listening
                    }
                    Err(error) => ListenerStatus::Failed(error.to_string()),
                }
            }),
        );
        let _ = slot.set(Arc::downgrade(&registry));
        Self {
            registry,
            subscribers,
        }
    }

    fn local(&self) -> TestResult<Client> {
        let (client, server) = UnixStream::pair()?;
        let registry = Arc::clone(&self.registry);
        let events = subscribe(&self.subscribers);
        thread::spawn(move || {
            let _ = serve(Box::new(server), registry, events);
        });
        Ok(Client::from_stream(Box::new(client))?)
    }
}

fn accept(listener: &TlsListener, registry: &Weak<Registry>, subscribers: &Subscribers) {
    while let Ok(stream) = listener.accept() {
        let Some(registry) = registry.upgrade() else {
            return;
        };
        let Some(admission) = registry.admit_remote() else {
            continue;
        };
        let events = subscribe(subscribers);
        thread::spawn(move || {
            let _ = serve_remote(stream, registry, events, admission);
        });
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

/// Pairs through the SDK and returns the saved credential plus the local admin client.
fn paired() -> TestResult<(Server, Client, ServerCredential)> {
    let server = Server::new();
    let local = server.local()?;
    let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
    local.write_remote_access(RemoteAccessSettings {
        enabled: true,
        port,
    })?;
    let mut offer = local.start_pairing()?;
    offer.invite.hosts = vec!["127.0.0.1".into()];
    let link = offer.invite.to_link();
    let preview = muxy_mobile::parse_pairing_link(link.clone())?;
    assert_eq!(preview.port, port);
    let credential = muxy_mobile::pair(link, "  Test phone\n".into())?;
    assert_eq!(credential.fingerprint.len(), 32);
    Ok((server, local, credential))
}

#[test]
fn a_phone_pairs_opens_a_shell_and_types() -> TestResult {
    let (_server, local, credential) = paired()?;
    assert_eq!(local.read_remote_access()?.devices[0].name, "Test phone");
    let (recorder, events) = listener();
    let connection = Connection::connect(credential, recorder)?;
    let projects = connection.projects()?;
    let home = projects
        .iter()
        .find(|project| project.is_home)
        .ok_or("no Home project")?;
    let session = connection.create_session(home.id.clone(), 40, 6)?;
    let terminal = connection.attach(session.id, 40, 6)?;
    assert_eq!(terminal.screen().columns, 40);
    terminal.send_input(b"echo mobile-$((40+2))".to_vec())?;
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
            && text().iter().any(|line| line.trim() == "mobile-42")
    })
    .map_err(|error| format!("{error}; screen: {:?}", text()))?;
    let sessions = connection.sessions(home.id.clone())?;
    assert!(
        sessions
            .iter()
            .any(|listed| listed.id == session.id && listed.attached)
    );
    terminal.detach()?;
    connection.end_session(session.id)?;
    wait_for(
        &events,
        |event| matches!(event, ConnectionEvent::SessionEnded { session_id } if *session_id == session.id),
    )?;
    Ok(())
}

#[test]
fn revoking_disconnects_the_phone_and_refuses_it_afterwards() -> TestResult {
    let (_server, local, credential) = paired()?;
    let (recorder, events) = listener();
    let connection = Connection::connect(credential.clone(), recorder)?;
    let device = credential.device_id.parse()?;
    local.revoke_device(device)?;
    wait_for(&events, |event| *event == ConnectionEvent::Disconnected)?;
    drop(connection);
    let (recorder, _events) = listener();
    assert!(matches!(
        Connection::connect(credential, recorder),
        Err(MobileError::Unauthorized)
    ));
    Ok(())
}

#[test]
fn a_damaged_or_foreign_pairing_is_reported_as_such() -> TestResult {
    let (_server, _local, credential) = paired()?;
    let (recorder, _events) = listener();
    let damaged = ServerCredential {
        token: vec![1, 2, 3],
        ..credential.clone()
    };
    assert!(matches!(
        Connection::connect(
            damaged,
            Arc::clone(&recorder) as Arc<dyn ConnectionListener>
        ),
        Err(MobileError::InvalidCredential)
    ));
    let foreign = ServerCredential {
        fingerprint: vec![0; 32],
        ..credential
    };
    assert!(matches!(
        Connection::connect(foreign, recorder),
        Err(MobileError::IdentityMismatch)
    ));
    assert!(matches!(
        muxy_mobile::pair("muxy://pair?v=1".into(), "Phone".into()),
        Err(MobileError::InvalidLink)
    ));
    Ok(())
}

fn texts(lines: &[Line]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>()
                .trim()
                .to_owned()
        })
        .collect()
}

#[test]
fn scrollback_reads_every_line_while_new_output_arrives() -> TestResult {
    let (_server, _local, credential) = paired()?;
    let (recorder, events) = listener();
    let connection = Connection::connect(credential, recorder)?;
    let home = connection
        .projects()?
        .into_iter()
        .find(|project| project.is_home)
        .ok_or("no Home project")?;
    let session = connection.create_session(home.id, 30, 5)?;
    let terminal = connection.attach(session.id, 30, 5)?;
    let on_screen = |text: &str| {
        texts(&terminal.screen().lines)
            .iter()
            .any(|line| line == text)
    };
    terminal
        .send_input(b"i=1; while [ $i -le 120 ]; do echo line-$i; i=$((i+1)); done\r".to_vec())?;
    wait_for(&events, |_| on_screen("line-120"))?;
    let scrollback = terminal.scrollback(50)?;
    terminal.send_input(b"echo after-$((1+1))\r".to_vec())?;
    wait_for(&events, |_| on_screen("after-2"))?;
    let mut lines = texts(&scrollback.lines());
    loop {
        let older = scrollback.load_older(50)?;
        if older.is_empty() {
            break;
        }
        lines.splice(0..0, texts(&older));
    }
    let numbered: Vec<_> = lines
        .iter()
        .filter_map(|line| line.strip_prefix("line-")?.parse::<u32>().ok())
        .collect();
    assert_eq!(numbered, (1..=120).collect::<Vec<_>>());
    assert!(!lines.iter().any(|line| line == "after-2"));
    connection.end_session(session.id)?;
    Ok(())
}

#[test]
fn terminals_attached_at_the_same_time_all_receive_their_screens() -> TestResult {
    let (_server, _local, credential) = paired()?;
    let (recorder, events) = listener();
    let connection = Connection::connect(credential, recorder)?;
    let home = connection
        .projects()?
        .into_iter()
        .find(|project| project.is_home)
        .ok_or("no Home project")?;
    let sessions: Vec<_> = (0..4)
        .map(|_| connection.create_session(home.id.clone(), 30, 4))
        .collect::<Result<_, _>>()?;
    let terminals: Vec<_> = thread::scope(|scope| {
        let attaching: Vec<_> = sessions
            .iter()
            .map(|session| scope.spawn(|| connection.attach(session.id, 30, 4)))
            .collect();
        attaching
            .into_iter()
            .map(|attach| attach.join().map_err(|_| "attach panicked"))
            .collect::<Result<Vec<_>, _>>()
    })?
    .into_iter()
    .collect::<Result<_, _>>()?;
    for (index, terminal) in terminals.iter().enumerate() {
        terminal.send_input(format!("echo ready-{index}\r").into_bytes())?;
    }
    let mut ready = vec![false; terminals.len()];
    let deadline = Instant::now() + TIMEOUT;
    while ready.contains(&false) {
        events.recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
        for (index, terminal) in terminals.iter().enumerate() {
            ready[index] |= terminal.screen().lines.iter().any(|line| {
                line.spans
                    .iter()
                    .map(|span| span.text.as_str())
                    .collect::<String>()
                    .trim()
                    == format!("ready-{index}")
            });
        }
    }
    for session in sessions {
        connection.end_session(session.id)?;
    }
    Ok(())
}
