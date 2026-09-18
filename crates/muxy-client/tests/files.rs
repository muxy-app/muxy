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
