use super::*;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

struct Gate(PathBuf);

impl Drop for Gate {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "release");
    }
}

#[test]
fn pending_worktree_keeps_unrelated_lifecycle_responsive_and_replays_once() {
    let repo = Repo::new(true);
    let other = Repo::new(true);
    let hook = repo.path.join(".git/hooks/post-checkout");
    std::fs::write(&hook, "#!/bin/sh\ncommon=$(git rev-parse --path-format=absolute --git-common-dir)\ntouch \"$common/test-started\"\nattempt=0\nwhile [ ! -e \"$common/test-release\" ] && [ $attempt -lt 400 ]; do\n  sleep 0.025\n  attempt=$((attempt + 1))\ndone\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o700)).unwrap();
    let project = ProjectId::new();
    let request = GitRequest {
        project: repo.project,
        action: GitAction::Worktree(WorktreeIntent {
            operation: OperationId::new(),
            action: WorktreeAction::Create {
                project,
                directory: server_path(&repo.path.join("pending")),
                branch: "pending".into(),
                base: Some("main".into()),
            },
        }),
    };
    std::thread::scope(|scope| {
        let gate = Gate(repo.path.join(".git/test-release"));
        let first = scope.spawn(|| repo.registry.git(&request));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !repo.path.join(".git/test-started").exists() {
            assert!(Instant::now() < deadline, "checkout hook did not start");
            std::thread::sleep(Duration::from_millis(5));
        }
        let duplicate = scope.spawn(|| repo.registry.git(&request));
        let (send, receive) = mpsc::channel();
        let registry = &repo.registry;
        let parent = repo.project;
        scope.spawn(move || {
            let result = exercise_unrelated_lifecycle(registry, &other).map(|()| {
                let conflict = registry.mutate_project(&ProjectIntent {
                    operation: OperationId::new(),
                    mutation: ProjectMutation::Delete(parent),
                });
                assert!(
                    conflict
                        .unwrap_err()
                        .message()
                        .contains("active worktree operation")
                );
            });
            let _ = send.send(result);
        });
        let responsive = receive.recv_timeout(Duration::from_secs(3));
        drop(gate);
        let first = first.join().unwrap().unwrap();
        assert_eq!(first, duplicate.join().unwrap().unwrap());
        responsive
            .expect("worktree blocked unrelated lifecycle work")
            .unwrap();
        assert!(matches!(first, GitReply::Project(ref created) if created.id == project));
        assert_eq!(read::worktrees(&repo.path).unwrap().len(), 2);
    });
}

fn exercise_unrelated_lifecycle(registry: &Registry, other: &Repo) -> Result<()> {
    let project = other.registry.catalog.project(other.project)?;
    registry.mutate_project(&ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Create(project.clone()),
    })?;
    registry.read_catalog(None, None)?;
    registry.git(&GitRequest {
        project: project.id,
        action: GitAction::Summary,
    })?;
    registry.list_project_sessions(project.id, None, None)?;
    let operation = OperationId::new();
    registry.create_project_session(
        project.id,
        operation,
        &other.path,
        muxy_protocol::Size { cols: 80, rows: 24 },
    )?;
    registry.cancel_creation(operation)?;
    assert!(
        registry
            .list_project_sessions(project.id, None, None)?
            .sessions
            .is_empty()
    );
    registry.mutate_project(&ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Delete(project.id),
    })?;
    assert!(registry.stop_if_idle());
    Ok(())
}
