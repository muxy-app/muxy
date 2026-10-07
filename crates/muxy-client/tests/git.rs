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
