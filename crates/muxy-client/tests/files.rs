use muxy_client::{Client, ClientEvent};
use muxy_protocol::{
    FileChanges, FilesAction, FilesReply, FilesRequest, OperationId, ProjectDescriptor, ProjectId,
    ProjectIntent, ProjectMutation, ServerPath,
};
use muxy_server::{Registry, ServerSettings, connection};
use std::error::Error;
use std::os::unix::{ffi::OsStrExt, net::UnixStream};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

type TestResult = Result<(), Box<dyn Error>>;

fn p(value: &str) -> ServerPath {
    ServerPath(value.as_bytes().to_vec())
}

fn register(client: &Client, path: &std::path::Path) -> Result<ProjectId, Box<dyn Error>> {
    let project = ProjectId::new();
    client.mutate_project(ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Create(ProjectDescriptor {
            id: project,
            home: false,
            directory: ServerPath(path.as_os_str().as_bytes().to_vec()),
            name: "Files".into(),
            icon: None,
            logo: None,
            color: "#ffffff".into(),
            kind: None,
            parent_id: None,
        }),
    })?;
    Ok(project)
}

fn next_change(
    events: &mpsc::Receiver<ClientEvent>,
    timeout: Duration,
) -> Option<(ProjectId, FileChanges)> {
    let deadline = Instant::now() + timeout;
    while let Ok(event) = events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        if let ClientEvent::FilesChanged { project, changes } = event {
            return Some((project, changes));
        }
    }
    None
}

#[test]
fn files_round_trip_and_bad_requests_leave_connection_usable() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (send, events) = mpsc::channel();
    let registry = Arc::new(Registry::new(ServerSettings::default(), send));
    let (local, remote) = UnixStream::pair()?;
    let serving_registry = Arc::clone(&registry);
    let serving =
        std::thread::spawn(move || connection::serve(Box::new(remote), serving_registry, events));
    let client = Client::from_stream(Box::new(local))?;
    let project = register(&client, directory.path())?;
    let files = |action| client.files(FilesRequest { project, action });
    assert_eq!(
        files(FilesAction::Mkdir(p("folder")))?,
        FilesReply::Path(p("folder"))
    );
    files(FilesAction::Write {
        path: p("folder/file"),
        content: "hello".into(),
    })?;
    assert!(
        matches!(files(FilesAction::Read(p("folder/file")))?, FilesReply::Content(content) if content.content == "hello")
    );
    assert!(
        matches!(files(FilesAction::Stat(p("folder/file")))?, FilesReply::Info(info) if info.size == 5)
    );
    assert_eq!(
        files(FilesAction::Rename {
            path: p("folder/file"),
            name: p("new")
        })?,
        FilesReply::Path(p("folder/new"))
    );
    assert_eq!(
        files(FilesAction::Move {
            paths: vec![p("folder/new")],
            into: p("")
        })?,
        FilesReply::Paths(vec![p("new")])
    );
    assert!(
        matches!(files(FilesAction::List(p("")))?, FilesReply::Entries(entries) if entries.len() == 2)
    );
    assert_eq!(files(FilesAction::Delete(vec![]))?, FilesReply::Done);
    for action in [
        FilesAction::Read(p("/etc/passwd")),
        FilesAction::Read(p("../outside")),
        FilesAction::Delete(vec![p("")]),
        FilesAction::Rename {
            path: p("new"),
            name: p("../escape"),
        },
    ] {
        assert!(files(action).is_err());
    }
    client.ping()?;
    client.catalog()?;
    client.disconnect();
    registry.shutdown();
    serving.join().map_err(|_| "server panicked")??;
    Ok(())
}

#[test]
fn subscriptions_report_external_changes_and_unwatch_projects_independently() -> TestResult {
    let first = tempfile::tempdir()?;
    let second = tempfile::tempdir()?;
    let (send, server_events) = mpsc::channel();
    let registry = Arc::new(Registry::new(ServerSettings::default(), send));
    let (local, remote) = UnixStream::pair()?;
    let serving_registry = Arc::clone(&registry);
    let serving = std::thread::spawn(move || {
        connection::serve(Box::new(remote), serving_registry, server_events)
    });
    let client = Client::from_stream(Box::new(local))?;
    let events = client.events().ok_or("events already taken")?;
    let a = register(&client, first.path())?;
    let b = register(&client, second.path())?;
    for project in [a, b] {
        client.files(FilesRequest {
            project,
            action: FilesAction::Watch,
        })?;
    }
    std::fs::write(first.path().join("external"), "changed")?;
    let (project, changes) =
        next_change(&events, Duration::from_secs(8)).ok_or("missing file event")?;
    assert_eq!(project, a);
    assert!(changes.rescan || changes.paths.contains(&p("external")));
    client.files(FilesRequest {
        project: a,
        action: FilesAction::Unwatch,
    })?;
    while next_change(&events, Duration::from_millis(500)).is_some() {}
    std::fs::write(first.path().join("unwatched"), "changed")?;
    assert!(next_change(&events, Duration::from_millis(700)).is_none());
    std::fs::write(second.path().join("still-watched"), "changed")?;
    assert_eq!(
        next_change(&events, Duration::from_secs(8)).map(|event| event.0),
        Some(b)
    );
    client.disconnect();
    registry.shutdown();
    serving.join().map_err(|_| "server panicked")??;
    Ok(())
}

#[test]
fn folders_anywhere_are_listed_for_choosing_a_project() -> TestResult {
    let directory = tempfile::tempdir()?;
    for folder in ["beta", "Alpha", ".hidden", "beta/nested"] {
        std::fs::create_dir(directory.path().join(folder))?;
    }
    std::fs::write(directory.path().join("file.txt"), "not a folder")?;
    std::os::unix::fs::symlink(directory.path().join("beta"), directory.path().join("link"))?;
    let (send, events) = mpsc::channel();
    let registry = Arc::new(Registry::new(ServerSettings::default(), send));
    let (local, remote) = UnixStream::pair()?;
    let serving = std::thread::spawn(move || connection::serve(Box::new(remote), registry, events));
    let client = Client::from_stream(Box::new(local))?;
    let root = directory.path().canonicalize()?;
    let listed = client.list_folders(ServerPath(root.as_os_str().as_bytes().to_vec()))?;
    let names: Vec<_> = listed
        .iter()
        .map(|name| String::from_utf8_lossy(&name.0).into_owned())
        .collect();
    assert_eq!(names, [".hidden", "Alpha", "beta", "link"]);
    for bad in ["relative", "/does/not/exist"] {
        let error = client
            .list_folders(p(bad))
            .err()
            .map(|error| error.to_string());
        assert!(error.is_some(), "{bad}");
    }
    let file = root.join("file.txt");
    assert!(
        client
            .list_folders(ServerPath(file.as_os_str().as_bytes().to_vec()))
            .is_err()
    );
    assert_eq!(
        client
            .list_folders(ServerPath(root.as_os_str().as_bytes().to_vec()))?
            .len(),
        4,
        "the connection stays usable"
    );
    drop(client);
    serving.join().map_err(|_| "server thread panicked")??;
    Ok(())
}

#[test]
fn uploads_arrive_in_chunks_and_keep_their_name_on_the_server() -> TestResult {
    let directory = tempfile::tempdir()?;
    let (send, events) = mpsc::channel();
    let registry = Arc::new(
        Registry::new(ServerSettings::default(), send)
            .with_uploads(directory.path().join("uploads")),
    );
    let (local, remote) = UnixStream::pair()?;
    let serving = std::thread::spawn(move || connection::serve(Box::new(remote), registry, events));
    let client = Client::from_stream(Box::new(local))?;
    let session =
        client.create_session(directory.path(), muxy_protocol::Size { cols: 20, rows: 4 })?;
    let bytes: Vec<u8> = (0..muxy_protocol::MAX_UPLOAD_CHUNK * 5 / 2)
        .map(|index| u8::try_from(index % 251).unwrap_or_default())
        .collect();
    let path = client.upload(session.id, "dir/shot.png", &bytes)?;
    let path = std::path::PathBuf::from(std::ffi::OsStr::from_bytes(&path.0));
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("shot.png")
    );
    assert_eq!(std::fs::read(&path)?, bytes);
    let empty = client.upload(session.id, "empty.txt", &[])?;
    assert_eq!(std::fs::read(std::ffi::OsStr::from_bytes(&empty.0))?, b"");
    let unknown = muxy_protocol::SessionId::new(session.id.get() + 100).ok_or("session")?;
    assert!(client.upload(unknown, "a", b"x").is_err());
    let too_big = vec![0; usize::try_from(muxy_protocol::MAX_UPLOAD_BYTES)? + 1];
    assert!(matches!(
        client.upload(session.id, "big", &too_big),
        Err(muxy_client::ClientError::Invalid(_))
    ));
    client.end_session(session.id)?;
    drop(client);
    serving.join().map_err(|_| "server thread panicked")??;
    Ok(())
}
