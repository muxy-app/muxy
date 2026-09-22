use muxy_client::Client;
use muxy_protocol::{
    GitAction, GitReply, GitRequest, OperationId, ProjectDescriptor, ProjectId, ProjectIntent,
    ProjectMutation, ServerPath,
};
use muxy_server::{Registry, ServerSettings, connection};
use std::error::Error;
use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt, net::UnixStream};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

type TestResult = Result<(), Box<dyn Error>>;
struct Directory(std::path::PathBuf);
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn git_round_trips_while_slow_hooks_leave_control_requests_responsive() -> TestResult {
    let directory =
        Directory(std::env::temp_dir().join(format!("muxy-git-client-{}", OperationId::new())));
    std::fs::create_dir(&directory.0)?;
    for args in [
        vec!["init", "-b", "main"],
        vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "initial",
        ],
    ] {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&directory.0)
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let hook = directory.0.join(".git/hooks/post-checkout");
    std::fs::write(&hook, "#!/bin/sh\ntouch hook-started\nsleep 1\n")?;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o700))?;
    let (send, events) = mpsc::channel();
    let registry = Arc::new(Registry::new(ServerSettings::default(), send));
    let (local, remote) = UnixStream::pair()?;
    let serving_registry = registry.clone();
    let serving =
        std::thread::spawn(move || connection::serve(Box::new(remote), serving_registry, events));
    let client = Client::from_stream(Box::new(local))?;
    let project = ProjectId::new();
    client.mutate_project(ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Create(ProjectDescriptor {
            id: project,
            home: false,
            directory: ServerPath(directory.0.as_os_str().as_bytes().to_vec()),
            name: "Test".into(),
            icon: None,
            logo: None,
            color: "#ffffff".into(),
            kind: None,
            parent_id: None,
        }),
    })?;
    let worker = client.clone();
    let mutation = std::thread::spawn(move || {
        worker.git(GitRequest {
            project,
            action: GitAction::CreateBranch("feature".into()),
        })
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !directory.0.join("hook-started").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(directory.0.join("hook-started").exists());
    let started = Instant::now();
    client.ping()?;
    client.catalog()?;
    assert!(started.elapsed() < Duration::from_millis(750));
    assert_eq!(
        mutation.join().map_err(|_| "Git worker panicked")??,
        GitReply::Done
    );
    let reply = client.git(GitRequest {
        project,
        action: GitAction::Summary,
    })?;
    assert!(
        matches!(reply, GitReply::Summary(Some(summary)) if summary.branch.as_deref() == Some("feature"))
    );
    client.disconnect();
    registry.shutdown();
    serving.join().map_err(|_| "server panicked")??;
    Ok(())
}

fn register_repository(
    client: &Client,
    directory: &std::path::Path,
) -> Result<ProjectId, Box<dyn Error>> {
    let output = std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(directory)
        .output()?;
    assert!(output.status.success());
    let project = ProjectId::new();
    client.mutate_project(ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Create(ProjectDescriptor {
            id: project,
            home: false,
            directory: ServerPath(directory.as_os_str().as_bytes().to_vec()),
            name: "Watch".into(),
            icon: None,
            logo: None,
            color: "#ffffff".into(),
            kind: None,
            parent_id: None,
        }),
    })?;
    Ok(project)
}

fn next_git_event(
    events: &mpsc::Receiver<muxy_client::ClientEvent>,
    timeout: Duration,
) -> Option<ProjectId> {
    let deadline = Instant::now() + timeout;
    while let Ok(event) = events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        if let muxy_client::ClientEvent::GitChanged { project } = event {
            return Some(project);
        }
    }
    None
}

#[test]
fn filesystem_watches_debounce_without_polling_and_follow_the_selected_project() -> TestResult {
    let first = tempfile::tempdir()?;
    let second = tempfile::tempdir()?;
    let (send, server_events) = mpsc::channel();
    let registry = Arc::new(Registry::new(ServerSettings::default(), send));
    let (local, remote) = UnixStream::pair()?;
    let serving_registry = registry.clone();
    let serving = std::thread::spawn(move || {
        connection::serve(Box::new(remote), serving_registry, server_events)
    });
    let client = Client::from_stream(Box::new(local))?;
    let events = client.events().ok_or("events already taken")?;
    let a = register_repository(&client, first.path())?;
    let b = register_repository(&client, second.path())?;
    assert_eq!(
        client.git(GitRequest {
            project: a,
            action: GitAction::Watch
        })?,
        GitReply::Done
    );
    for index in 0..8 {
        std::fs::write(first.path().join(format!("file-{index}")), "changed\n")?;
    }
    assert_eq!(next_git_event(&events, Duration::from_secs(8)), Some(a));
    assert_eq!(next_git_event(&events, Duration::from_millis(1200)), None);
    for _ in 0..2 {
        client.git(GitRequest {
            project: a,
            action: GitAction::Summary,
        })?;
    }
    assert_eq!(next_git_event(&events, Duration::from_millis(1200)), None);
    client.git(GitRequest {
        project: b,
        action: GitAction::Watch,
    })?;
    std::fs::write(first.path().join("inactive"), "no refresh\n")?;
    assert_eq!(next_git_event(&events, Duration::from_millis(1200)), None);
    std::fs::write(second.path().join("active"), "refresh\n")?;
    assert_eq!(next_git_event(&events, Duration::from_secs(8)), Some(b));
    client.disconnect();
    registry.shutdown();
    serving.join().map_err(|_| "server panicked")??;
    Ok(())
}

#[test]
fn extension_git_operations_and_validation_round_trip_over_a_live_connection() -> TestResult {
    use muxy_protocol::{GitDiffRequest, GitPullRequestAction};
    let directory = tempfile::tempdir()?;
    let (send, server_events) = mpsc::channel();
    let registry = Arc::new(Registry::new(ServerSettings::default(), send));
    let (local, remote) = UnixStream::pair()?;
    let serving_registry = registry.clone();
    let serving = std::thread::spawn(move || {
        connection::serve(Box::new(remote), serving_registry, server_events)
    });
    let client = Client::from_stream(Box::new(local))?;
    let project = register_repository(&client, directory.path())?;
    for args in [
        vec!["config", "user.name", "Test"],
        vec!["config", "user.email", "test@example.invalid"],
        vec!["config", "commit.gpgsign", "false"],
    ] {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(directory.path())
                .status()?
                .success()
        );
    }
    let git = |action| client.git(GitRequest { project, action });
    std::fs::write(directory.path().join("file"), "before\n")?;
    assert!(matches!(
        git(GitAction::Commit {
            message: "initial".into(),
            stage_all: true
        })?,
        GitReply::Commit(_)
    ));
    std::fs::write(directory.path().join("file"), "after\n")?;
    let GitReply::Status(status) = git(GitAction::Status { local: true })? else {
        return Err("wrong status reply".into());
    };
    assert_eq!(status.files.len(), 1);
    assert_eq!(status.files[0].unstaged.additions, Some(1));
    assert!(matches!(git(GitAction::RepoInfo)?, GitReply::RepoInfo(_)));
    assert!(
        matches!(git(GitAction::Log { max_count: 100, skip: 0 })?, GitReply::Log(log) if log.len() == 1)
    );
    let GitReply::Diff(diff) = git(GitAction::Diff(GitDiffRequest {
        path: Some(ServerPath(b"file".to_vec())),
        ..GitDiffRequest::default()
    }))?
    else {
        return Err("wrong diff reply".into());
    };
    assert_eq!((diff.additions, diff.deletions), (1, 1));
    assert!(
        git(GitAction::PullRequest(GitPullRequestAction::Close {
            number: 0
        }))
        .is_err()
    );
    client.ping()?;
    assert!(matches!(
        git(GitAction::Summary)?,
        GitReply::Summary(Some(_))
    ));
    client.disconnect();
    registry.shutdown();
    serving.join().map_err(|_| "server panicked")??;
    Ok(())
}
