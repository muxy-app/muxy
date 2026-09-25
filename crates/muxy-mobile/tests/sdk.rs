use std::net::{SocketAddr, TcpListener};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};
use std::thread;
use std::time::{Duration, Instant};

use muxy_client::Client;
use muxy_mobile::{
    Connection, ConnectionEvent, ConnectionListener, FilesAction, FilesReply, GitAction,
    GitDiffKind, GitDiffRequest, GitReply, Key, Line, MobileError, Modifiers, MouseButton,
    ScrollDirection, ServerCredential,
};
use muxy_protocol::transport::Listener;
use muxy_protocol::transport::tls::TlsListener;
use muxy_protocol::{
    ListenerStatus, OperationId, ProjectDescriptor, ProjectId, ProjectIntent, ProjectMutation,
    RemoteAccessSettings, ServerPath,
};
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

/// Waits until `done` holds, checking now and after each event.
fn wait_until(events: &Receiver<ConnectionEvent>, mut done: impl FnMut() -> bool) -> TestResult {
    if done() {
        return Ok(());
    }
    wait_for(events, |_| done())
}

fn metadata_changed(event: &ConnectionEvent, session: u64) -> bool {
    matches!(event, ConnectionEvent::MetadataChanged { session_id } if *session_id == session)
}

/// A temporary folder, removed when dropped.
struct Folder(PathBuf);

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A new folder that the computer registers as a project; returns it with the project's id.
fn project_folder(local: &Client) -> TestResult<(Folder, String)> {
    let folder = Folder(std::env::temp_dir().join(format!("muxy-sdk-{}", OperationId::new())));
    std::fs::create_dir(&folder.0)?;
    let project = ProjectId::new();
    local.mutate_project(ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Create(ProjectDescriptor {
            id: project,
            home: false,
            directory: ServerPath(folder.0.as_os_str().as_bytes().to_vec()),
            name: "Phone".into(),
            icon: None,
            logo: None,
            color: "#ffffff".into(),
            kind: None,
            parent_id: None,
        }),
    })?;
    Ok((folder, project.to_string()))
}

fn run_git(folder: &Path, args: &[&str]) -> TestResult {
    let output = Command::new("git")
        .args(args)
        .current_dir(folder)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into())
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

#[test]
fn taps_and_scrolling_reach_a_program_that_tracks_the_mouse() -> TestResult {
    let (_server, _local, credential) = paired()?;
    let (recorder, events) = listener();
    let connection = Connection::connect(credential, recorder)?;
    let home = connection
        .projects()?
        .into_iter()
        .find(|project| project.is_home)
        .ok_or("no Home project")?;
    let session = connection.create_session(home.id, 40, 6)?;
    let terminal = connection.attach(session.id, 40, 6)?;
    assert!(!terminal.screen().mouse_tracking);
    // Button tracking with SGR reports, printed as soon as each one arrives.
    terminal
        .send_input(b"stty -icanon -echo; printf '\\033[?1000h\\033[?1006h'; cat -v\r".to_vec())?;
    wait_for(&events, |event| {
        metadata_changed(event, session.id) && terminal.screen().mouse_tracking
    })?;
    terminal.click(MouseButton::Left, 2, 4, Modifiers::default())?;
    terminal.scroll(ScrollDirection::Up, 2, 4)?;
    wait_until(&events, || {
        texts(&terminal.screen().lines)
            .iter()
            .any(|line| line.contains("^[[<0;5;3M^[[<0;5;3m^[[<64;5;3M"))
    })?;

    // Attaching again shows the mode the program already turned on.
    terminal.detach()?;
    let terminal = connection.attach(session.id, 40, 6)?;
    wait_until(&events, || terminal.screen().mouse_tracking)?;
    let control = Modifiers {
        control: true,
        ..Modifiers::default()
    };
    terminal.send_key(Key::Character { text: "c".into() }, control)?;
    terminal.send_input(b"printf '\\033[?1000l'\r".to_vec())?;
    wait_for(&events, |event| {
        metadata_changed(event, session.id) && !terminal.screen().mouse_tracking
    })?;
    connection.end_session(session.id)?;
    Ok(())
}

#[test]
fn scrolling_a_full_screen_program_sends_it_arrow_keys() -> TestResult {
    let (_server, _local, credential) = paired()?;
    let (recorder, events) = listener();
    let connection = Connection::connect(credential, recorder)?;
    let home = connection
        .projects()?
        .into_iter()
        .find(|project| project.is_home)
        .ok_or("no Home project")?;
    let session = connection.create_session(home.id, 40, 6)?;
    let terminal = connection.attach(session.id, 40, 6)?;
    assert!(!terminal.screen().alternate_scroll);
    // The alternate screen scrolls with arrow keys unless the program tracks the mouse.
    terminal.send_input(b"stty -icanon -echo; printf '\\033[?1049h'; cat -v\r".to_vec())?;
    wait_for(&events, |event| {
        metadata_changed(event, session.id) && terminal.screen().alternate_scroll
    })?;
    assert!(!terminal.screen().mouse_tracking);
    terminal.scroll(ScrollDirection::Down, 0, 0)?;
    wait_until(&events, || {
        texts(&terminal.screen().lines)
            .iter()
            .any(|line| line.contains("^[[B^[[B^[[B"))
    })?;
    connection.end_session(session.id)?;
    Ok(())
}

#[test]
fn a_phone_reads_and_changes_a_repository() -> TestResult {
    let (_server, local, credential) = paired()?;
    let (folder, project) = project_folder(&local)?;
    for args in [
        &["init", "-b", "main"][..],
        &["config", "user.name", "Test"],
        &["config", "user.email", "test@example.invalid"],
        &["config", "commit.gpgsign", "false"],
    ] {
        run_git(&folder.0, args)?;
    }
    let (recorder, events) = listener();
    let connection = Connection::connect(credential, recorder)?;
    let git = |action| connection.git(project.clone(), action);
    let write = |content: &str| {
        connection.files(
            project.clone(),
            FilesAction::Write {
                path: "notes.txt".into(),
                content: content.into(),
            },
        )
    };
    assert_eq!(git(GitAction::Watch)?, GitReply::Done);
    write("one\n")?;
    wait_for(&events, |event| {
        *event
            == ConnectionEvent::GitChanged {
                project_id: project.clone(),
            }
    })?;
    let GitReply::Changes { files } = git(GitAction::Changes)? else {
        return Err("expected changes".into());
    };
    assert_eq!(
        (files[0].path.as_str(), files[0].index.as_str()),
        ("notes.txt", "?")
    );
    git(GitAction::Stage {
        paths: vec!["notes.txt".into()],
    })?;
    let GitReply::Commit { hash } = git(GitAction::Commit {
        message: "Add notes".into(),
        stage_all: false,
    })?
    else {
        return Err("expected a commit".into());
    };
    write("one\ntwo\n")?;
    let request = GitDiffRequest {
        path: Some("notes.txt".into()),
        ..GitDiffRequest::default()
    };
    let GitReply::Diff { diff } = git(GitAction::Diff { request })? else {
        return Err("expected a diff".into());
    };
    assert_eq!((diff.additions, diff.deletions), (1, 0));
    assert!(diff.rows.iter().any(|row| {
        row.kind == GitDiffKind::Addition && row.new_text.as_deref() == Some("two")
    }));
    let GitReply::Log { commits } = git(GitAction::Log {
        max_count: 10,
        skip: 0,
    })?
    else {
        return Err("expected a log".into());
    };
    assert_eq!(
        (commits[0].hash.as_str(), commits[0].subject.as_str()),
        (hash.as_str(), "Add notes")
    );
    git(GitAction::CreateBranch {
        name: "feature".into(),
    })?;
    let GitReply::Summary {
        summary: Some(summary),
    } = git(GitAction::Summary)?
    else {
        return Err("expected a summary".into());
    };
    assert_eq!(summary.branch.as_deref(), Some("feature"));
    assert_eq!(summary.unstaged, 1);
    assert!(matches!(
        git(GitAction::Checkout {
            hash: "not a hash".into()
        }),
        Err(MobileError::Server { .. })
    ));
    Ok(())
}

#[test]
fn a_phone_browses_and_edits_project_files() -> TestResult {
    let (_server, local, credential) = paired()?;
    let (_folder, project) = project_folder(&local)?;
    let (recorder, events) = listener();
    let connection = Connection::connect(credential, recorder)?;
    let files = |action| connection.files(project.clone(), action);
    assert_eq!(files(FilesAction::Watch)?, FilesReply::Done);
    assert_eq!(
        files(FilesAction::Mkdir {
            path: "docs".into()
        })?,
        FilesReply::Path {
            path: "docs".into()
        }
    );
    files(FilesAction::Write {
        path: "docs/a.md".into(),
        content: "hello".into(),
    })?;
    wait_for(&events, |event| {
        matches!(event, ConnectionEvent::FilesChanged { project_id, paths }
            if *project_id == project
                && (paths.is_empty() || paths.iter().any(|path| path.starts_with("docs"))))
    })?;
    let FilesReply::Entries { entries } = files(FilesAction::ListDirectory {
        path: "docs".into(),
    })?
    else {
        return Err("expected entries".into());
    };
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        ["docs/a.md"]
    );
    let FilesReply::Content { file } = files(FilesAction::Read {
        path: "docs/a.md".into(),
    })?
    else {
        return Err("expected content".into());
    };
    assert_eq!((file.content.as_str(), file.size), ("hello", 5));
    let FilesReply::Info { info } = files(FilesAction::Stat {
        path: "docs/a.md".into(),
    })?
    else {
        return Err("expected info".into());
    };
    assert!(!info.is_directory && info.size == 5);
    assert_eq!(
        files(FilesAction::Rename {
            path: "docs/a.md".into(),
            name: "b.md".into(),
        })?,
        FilesReply::Path {
            path: "docs/b.md".into()
        }
    );
    assert_eq!(
        files(FilesAction::Move {
            paths: vec!["docs/b.md".into()],
            into: String::new(),
        })?,
        FilesReply::Paths {
            paths: vec!["b.md".into()]
        }
    );
    // An empty delete moves nothing to the computer's Trash.
    assert_eq!(
        files(FilesAction::Delete { paths: Vec::new() })?,
        FilesReply::Done
    );
    assert!(matches!(
        files(FilesAction::Read {
            path: "../outside".into()
        }),
        Err(MobileError::Server { .. })
    ));
    assert_eq!(files(FilesAction::Unwatch)?, FilesReply::Done);
    Ok(())
}
