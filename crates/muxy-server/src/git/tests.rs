#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Test fixtures and assertions fail immediately"
)]

use super::*;
use crate::connection::Connection;
use muxy_protocol::{Message, OperationId, ProjectDescriptor, ProjectIntent, ProjectMutation};
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub(super) struct Repository {
    pub(super) path: PathBuf,
    registry: Registry,
    project: ProjectId,
}

impl Repository {
    /// A committed repository that ignores `target/`, with an `origin` that
    /// can't be reached.
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("muxy-git-{}", OperationId::new()));
        std::fs::create_dir(&path).unwrap();
        let path = path.canonicalize().unwrap();
        std::fs::write(path.join(".gitignore"), "target/\n").unwrap();
        for args in [
            &["init", "--quiet", "--initial-branch=main"][..],
            &[
                "-c",
                "user.name=Muxy",
                "-c",
                "user.email=muxy@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "Start",
            ],
            &["remote", "add", "origin", "/nonexistent/muxy/origin.git"],
        ] {
            run(&path, args).unwrap();
        }
        let (send, _) = mpsc::channel();
        let registry = Registry::new(crate::ServerSettings::default(), send);
        let project = ProjectId::new();
        registry
            .mutate_project(&ProjectIntent {
                operation: OperationId::new(),
                mutation: ProjectMutation::Create(ProjectDescriptor {
                    id: project,
                    home: false,
                    directory: ServerPath::from(&path),
                    name: "Test".into(),
                    icon: None,
                    logo: None,
                    color: "#ffffff".into(),
                    kind: None,
                    parent_id: None,
                }),
            })
            .unwrap();
        Self {
            path,
            registry,
            project,
        }
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn every_subscriber_shares_one_watch_until_the_last_leaves() {
    let repository = Repository::new();
    let registry = &repository.registry;
    let connections = [
        Arc::new(Connection::new(Arc::default())),
        Arc::new(Connection::new(Arc::default())),
    ];
    let mut subscriptions: Vec<_> = connections
        .iter()
        .map(|connection| {
            registry
                .watch_git(repository.project, connection)
                .unwrap()
                .unwrap()
        })
        .collect();
    assert_eq!(registry.git.watches.watching(), 1);

    std::fs::write(repository.path.join("notes.txt"), "changed").unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut notified = [false; 2];
    while notified.contains(&false) && Instant::now() < deadline {
        for (connection, notified) in connections.iter().zip(&mut notified) {
            *notified |= connection
                .take_control()
                .iter()
                .any(|message| matches!(message, Message::GitChanged { .. }));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(notified, [true; 2]);

    subscriptions.pop();
    assert_eq!(registry.git.watches.watching(), 1);
    subscriptions.clear();
    assert_eq!(registry.git.watches.watching(), 0);
}

#[test]
fn a_failed_remote_lookup_waits_before_asking_again() {
    let mut repository = Repository::new();
    let calls = repository.path.join("gh-calls");
    let cli = repository.path.join("gh");
    std::fs::write(
        &cli,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 1\n", calls.display()),
    )
    .unwrap();
    std::fs::set_permissions(&cli, std::fs::Permissions::from_mode(0o755)).unwrap();
    repository.registry.git.github = github::Github::with_cli(cli);
    let status = |fresh| {
        let GitReply::Status(status) = repository
            .registry
            .git(&GitRequest {
                project: repository.project,
                action: GitAction::Status {
                    local: false,
                    fresh,
                },
            })
            .unwrap()
        else {
            panic!("expected a status");
        };
        assert!(status.pull_request.is_none());
        assert!(status.default_branch.is_none());
    };
    let asked = || {
        std::fs::read_to_string(&calls)
            .unwrap_or_default()
            .lines()
            .count()
    };

    status(false);
    let first = asked();
    assert!(first > 0);
    status(false);
    assert_eq!(asked(), first);
    status(true);
    assert!(asked() > first);
}

#[test]
fn output_past_the_limit_fails_unless_it_may_be_cut_short() {
    let flood = || {
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", "head -c 5000000 /dev/zero"]);
        command
    };
    let failed = command::capture(flood()).unwrap_err();
    assert_eq!(failed.message(), "Git output exceeds the limit");
    let (kept, truncated) =
        command::capture_with(flood(), Duration::from_secs(20), Some(10), false).unwrap();
    assert_eq!((kept, truncated), (vec![0; 10], true));
}

#[test]
fn a_failing_hook_reports_the_hook_not_git() {
    let repository = Repository::new();
    let project = ProjectDescriptor {
        id: ProjectId::new(),
        home: false,
        directory: ServerPath::from(&repository.path),
        name: "Test".into(),
        icon: None,
        logo: None,
        color: "#ffffff".into(),
        kind: None,
        parent_id: None,
    };
    let failure = |command: &str| {
        let hook = muxy_protocol::WorktreeHook {
            command: command.into(),
            name: Some("Setup".into()),
            project: true,
        };
        hooks::run(&repository.path, &project, "main", &[hook])
            .unwrap_err()
            .message()
            .to_owned()
    };
    assert_eq!(
        failure("echo broken >&2; exit 3"),
        "Worktree hook failed (Setup): broken"
    );
    assert_eq!(
        failure("exit 3"),
        "Worktree hook failed (Setup): it exited with code 3"
    );
    assert_eq!(
        failure("head -c 5000000 /dev/zero"),
        "Worktree hook failed (Setup): its output exceeds 4 MiB"
    );
}
