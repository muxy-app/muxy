#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Test fixtures and assertions fail immediately"
)]
use super::*;
use muxy_protocol::{
    OperationId, ProjectDescriptor, ProjectId, ProjectIntent, ProjectMutation, WorktreeAction,
    WorktreeIntent,
};
use std::sync::mpsc;
mod worktree_concurrency;

struct Repo {
    path: PathBuf,
    registry: Registry,
    project: ProjectId,
}
impl Repo {
    fn new(commit: bool) -> Self {
        let path = std::env::temp_dir().join(format!("muxy-git-test-{}", OperationId::new()));
        std::fs::create_dir(&path).unwrap();
        run(&path, &["init", "-b", "main"]).unwrap();
        run(&path, &["config", "user.name", "Test"]).unwrap();
        run(&path, &["config", "user.email", "test@example.invalid"]).unwrap();
        run(&path, &["config", "commit.gpgsign", "false"]).unwrap();
        if commit {
            run(&path, &["commit", "--allow-empty", "-m", "initial"]).unwrap();
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
                    directory: server_path(&path),
                    name: "Test".into(),
                    icon: None,
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
    fn git(&self, action: GitAction) -> Result<GitReply> {
        self.registry.git(&GitRequest {
            project: self.project,
            action,
        })
    }
    fn summary(&self) -> muxy_protocol::GitSummary {
        let GitReply::Summary(Some(s)) = self.git(GitAction::Summary).unwrap() else {
            panic!()
        };
        s
    }
    fn create(&self) -> (ProjectId, PathBuf, GitRequest) {
        let id = ProjectId::new();
        let directory = self.path.join("checkout");
        let request = GitRequest {
            project: self.project,
            action: GitAction::Worktree(WorktreeIntent {
                operation: OperationId::new(),
                action: WorktreeAction::Create {
                    project: id,
                    directory: server_path(&directory),
                    branch: "feature".into(),
                    base: Some("main".into()),
                },
            }),
        };
        self.registry.git(&request).unwrap();
        (id, directory, request)
    }
}
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn unborn_stage_unstage_and_literal_unix_paths() {
    let repo = Repo::new(false);
    #[cfg(target_os = "linux")]
    let relative = ServerPath(b"odd\xff\n[*].txt".to_vec());
    #[cfg(not(target_os = "linux"))]
    let relative = ServerPath(b"odd\n[*].txt".to_vec());
    std::fs::write(repo.path.join(path(&relative)), b"one\ntwo\n").unwrap();
    assert!(repo.summary().head.is_none());
    assert_eq!(repo.summary().untracked, 1);
    repo.git(GitAction::Stage(vec![relative.clone()])).unwrap();
    assert_eq!(repo.summary().staged, 1);
    repo.git(GitAction::Unstage(vec![relative.clone()]))
        .unwrap();
    assert_eq!(repo.summary().untracked, 1);
    repo.git(GitAction::Discard(vec![relative.clone()]))
        .unwrap();
    assert!(!repo.path.join(path(&relative)).exists());
}

#[test]
fn discard_validates_the_whole_selection_before_removing_files() {
    let repo = Repo::new(true);
    std::fs::write(repo.path.join("keep"), "untracked\n").unwrap();
    assert!(
        repo.git(GitAction::Discard(vec![
            ServerPath(b"keep".to_vec()),
            ServerPath(b"../outside".to_vec()),
        ]))
        .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(repo.path.join("keep")).unwrap(),
        "untracked\n"
    );
}

#[test]
fn branch_switch_delete_and_detached_head() {
    let repo = Repo::new(true);
    repo.git(GitAction::CreateBranch("feature".into())).unwrap();
    assert_eq!(repo.summary().branch.as_deref(), Some("feature"));
    assert!(repo.git(GitAction::DeleteBranch("feature".into())).is_err());
    repo.git(GitAction::SwitchBranch("main".into())).unwrap();
    repo.git(GitAction::DeleteBranch("feature".into())).unwrap();
    run(&repo.path, &["switch", "--detach", "HEAD"]).unwrap();
    assert!(repo.summary().branch.is_none());
    assert!(repo.summary().head.is_some());
}

#[test]
fn discard_preserves_index_and_rejects_traversal() {
    let repo = Repo::new(true);
    let file = ServerPath(b"file".to_vec());
    std::fs::write(repo.path.join("file"), "staged\n").unwrap();
    repo.git(GitAction::Stage(vec![file.clone()])).unwrap();
    std::fs::write(repo.path.join("file"), "unstaged\n").unwrap();
    assert_eq!(repo.summary().changed, 1);
    assert_eq!(repo.summary().staged, 1);
    assert_eq!(repo.summary().unstaged, 1);
    repo.git(GitAction::Discard(vec![file])).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path.join("file")).unwrap(),
        "staged\n"
    );
    assert!(
        repo.git(GitAction::Discard(vec![ServerPath(b"../outside".to_vec())]))
            .is_err()
    );
}

#[test]
fn worktree_creation_registration_removal_and_retries() {
    let repo = Repo::new(true);
    let (id, directory, request) = repo.create();
    assert_eq!(
        repo.registry.git(&request).unwrap(),
        repo.registry.git(&request).unwrap()
    );
    assert!(directory.join(".git").is_file());
    let GitReply::Worktrees(entries) = repo.git(GitAction::Worktrees).unwrap() else {
        panic!()
    };
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|w| w.registered == Some(id)));
    let GitReply::Removal(expected) = repo
        .registry
        .git(&GitRequest {
            project: id,
            action: GitAction::InspectRemoval,
        })
        .unwrap()
    else {
        panic!()
    };
    let remove = GitRequest {
        project: id,
        action: GitAction::Worktree(WorktreeIntent {
            operation: OperationId::new(),
            action: WorktreeAction::Remove { expected },
        }),
    };
    repo.registry.git(&remove).unwrap();
    repo.registry.git(&remove).unwrap();
    assert!(!directory.exists());
    assert!(repo.registry.catalog.project(id).is_err());
    assert!(repo.path.join(".git").exists());
}

#[test]
fn stale_dirty_removal_confirmation_preserves_files() {
    let repo = Repo::new(true);
    let (id, directory, _) = repo.create();
    let GitReply::Removal(expected) = repo
        .registry
        .git(&GitRequest {
            project: id,
            action: GitAction::InspectRemoval,
        })
        .unwrap()
    else {
        panic!()
    };
    std::fs::write(directory.join("new"), "keep me").unwrap();
    assert!(
        repo.registry
            .git(&GitRequest {
                project: id,
                action: GitAction::Worktree(WorktreeIntent {
                    operation: OperationId::new(),
                    action: WorktreeAction::Remove { expected }
                })
            })
            .is_err()
    );
    assert!(directory.join("new").exists());
}

#[test]
fn rename_status_and_conflicts_are_parsed_without_quoting() {
    let files = read::files(b"R  new\nname\0old name\0UU conflict\0?? untracked\0").unwrap();
    assert_eq!(files[0].original_path.as_ref().unwrap().0, b"old name");
    assert!(files[0].staged());
    assert!(files[1].conflicted());
    assert!(files[2].untracked());
}

#[test]
fn failed_creation_releases_only_its_empty_reserved_directory() {
    let repo = Repo::new(true);
    let target = repo.path.join("failed");
    let request = GitRequest {
        project: repo.project,
        action: GitAction::Worktree(WorktreeIntent {
            operation: OperationId::new(),
            action: WorktreeAction::Create {
                project: ProjectId::new(),
                directory: server_path(&target),
                branch: "main".into(),
                base: Some("HEAD".into()),
            },
        }),
    };
    let first = repo.registry.git(&request).unwrap_err();
    assert!(!target.exists());
    assert_eq!(repo.registry.git(&request).unwrap_err(), first);
    repo.registry.resume_git();
    assert!(!target.exists());
}

#[test]
fn removal_stops_local_processes_and_preserves_other_registrations() {
    let repo = Repo::new(true);
    let (id, directory, _) = repo.create();
    let other = ProjectId::new();
    let parent = repo.registry.catalog.project(repo.project).unwrap();
    let mut duplicate = parent.clone();
    duplicate.id = other;
    repo.registry
        .mutate_project(&ProjectIntent {
            operation: OperationId::new(),
            mutation: ProjectMutation::Create(duplicate),
        })
        .unwrap();
    let child = ProjectId::new();
    repo.registry
        .git(&GitRequest {
            project: other,
            action: GitAction::Worktree(WorktreeIntent {
                operation: OperationId::new(),
                action: WorktreeAction::Register {
                    project: child,
                    directory: server_path(&directory),
                },
            }),
        })
        .unwrap();
    let mut process = std::process::Command::new("/bin/sleep")
        .arg("30")
        .current_dir(&directory)
        .spawn()
        .unwrap();
    let GitReply::Removal(expected) = repo
        .registry
        .git(&GitRequest {
            project: id,
            action: GitAction::InspectRemoval,
        })
        .unwrap()
    else {
        panic!()
    };
    let result = repo.registry.git(&GitRequest {
        project: id,
        action: GitAction::Worktree(WorktreeIntent {
            operation: OperationId::new(),
            action: WorktreeAction::Remove { expected },
        }),
    });
    let ended = process.try_wait().unwrap().is_some();
    let _ = process.kill();
    let _ = process.wait();
    result.unwrap();
    assert!(ended);
    assert!(repo.registry.catalog.project(child).is_ok());
    assert!(
        repo.registry
            .git(&GitRequest {
                project: child,
                action: GitAction::Summary
            })
            .is_err()
    );
}

#[test]
fn recovery_reconciles_git_success_before_catalog_commit() {
    let repo = Repo::new(true);
    let (id, directory, request) = repo.create();
    let GitAction::Worktree(intent) = &request.action else {
        panic!()
    };
    let mut receipt = repo.registry.catalog.git_receipt(intent.operation).unwrap();
    repo.registry
        .mutate_project(&ProjectIntent {
            operation: OperationId::new(),
            mutation: ProjectMutation::Delete(id),
        })
        .unwrap();
    receipt.applied = false;
    receipt.reply = None;
    repo.registry.catalog.save_git(receipt).unwrap();
    repo.registry.resume_git();
    assert!(repo.registry.catalog.project(id).is_ok());
    assert!(directory.exists());
    assert_eq!(read::worktrees(&repo.path).unwrap().len(), 2);
}

#[test]
fn untracked_directory_and_symlink_traversal_are_never_recursively_deleted() {
    let repo = Repo::new(true);
    let outside = repo.path.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep"), "keep").unwrap();
    std::os::unix::fs::symlink(&outside, repo.path.join("link")).unwrap();
    assert!(safe_path(&repo.path, &ServerPath(b"link/keep".to_vec())).is_err());
    assert!(
        repo.git(GitAction::Discard(vec![ServerPath(b"outside".to_vec())]))
            .is_err()
    );
    assert!(outside.join("keep").is_file());
}

#[test]
fn checkout_hook_failure_reconciles_the_created_worktree() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new(true);
    let hook = repo.path.join(".git/hooks/post-checkout");
    std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o700)).unwrap();
    let (id, directory, request) = repo.create();
    assert!(repo.registry.catalog.project(id).is_ok());
    assert!(directory.join(".git").exists());
    assert!(matches!(
        repo.registry.git(&request).unwrap(),
        GitReply::Project(_)
    ));
}

#[test]
fn generic_registration_rejects_duplicates_and_preserves_receipt_retries() {
    let repo = Repo::new(true);
    let (id, _, _) = repo.create();
    let mut duplicate = repo.registry.catalog.project(id).unwrap();
    duplicate.id = ProjectId::new();
    assert!(
        repo.registry
            .mutate_project(&ProjectIntent {
                operation: OperationId::new(),
                mutation: ProjectMutation::Create(duplicate)
            })
            .is_err()
    );
    let external = repo.path.join("external");
    run(
        &repo.path,
        &[
            "worktree",
            "add",
            "-b",
            "external",
            external.to_str().unwrap(),
        ],
    )
    .unwrap();
    let mut record = repo.registry.catalog.project(id).unwrap();
    record.id = ProjectId::new();
    record.directory = server_path(&external);
    let intent = ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::Create(record.clone()),
    };
    repo.registry.mutate_project(&intent).unwrap();
    repo.registry.mutate_project(&intent).unwrap();
    repo.registry
        .mutate_project(&ProjectIntent {
            operation: OperationId::new(),
            mutation: ProjectMutation::Delete(record.id),
        })
        .unwrap();
    run(
        &repo.path,
        &["worktree", "remove", external.to_str().unwrap()],
    )
    .unwrap();
    repo.registry.mutate_project(&intent).unwrap();
    assert!(repo.registry.catalog.project(record.id).is_err());
}

#[test]
fn removal_recovery_cleans_up_the_exact_missing_worktree_registration() {
    let repo = Repo::new(true);
    let (id, directory, _) = repo.create();
    let GitReply::Removal(expected) = repo
        .registry
        .git(&GitRequest {
            project: id,
            action: GitAction::InspectRemoval,
        })
        .unwrap()
    else {
        panic!()
    };
    let intent = WorktreeIntent {
        operation: OperationId::new(),
        action: WorktreeAction::Remove {
            expected: expected.clone(),
        },
    };
    let receipt = crate::catalog::GitReceipt {
        owner: id,
        intent: intent.clone(),
        project: repo.registry.catalog.project(id).unwrap(),
        device: expected.device,
        inode: expected.inode,
        applied: false,
        failed: None,
        reply: None,
    };
    repo.registry.catalog.save_git(receipt).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    repo.registry.resume_git();
    assert_eq!(read::worktrees(&repo.path).unwrap().len(), 1);
    assert!(repo.registry.catalog.project(id).is_err());
    assert_eq!(
        repo.registry
            .git(&GitRequest {
                project: id,
                action: GitAction::Worktree(intent)
            })
            .unwrap(),
        GitReply::Done
    );
}

#[test]
fn durable_receipts_recover_after_restart_and_recheck_storage_on_replay() {
    let repo = Repo::new(true);
    let (id, _, request) = repo.create();
    let GitAction::Worktree(intent) = &request.action else {
        panic!()
    };
    let mut receipt = repo.registry.catalog.git_receipt(intent.operation).unwrap();
    receipt.applied = false;
    receipt.reply = None;
    let storage = repo.path.join("server-state");
    std::fs::create_dir(&storage).unwrap();
    let (send, _) = mpsc::channel();
    let persistent =
        Registry::persistent(crate::ServerSettings::default(), send, &storage).unwrap();
    persistent
        .mutate_project(&ProjectIntent {
            operation: OperationId::new(),
            mutation: ProjectMutation::Create(repo.registry.catalog.project(repo.project).unwrap()),
        })
        .unwrap();
    persistent.catalog.save_git(receipt).unwrap();
    drop(persistent);
    let (send, _) = mpsc::channel();
    let restored = Registry::persistent(crate::ServerSettings::default(), send, &storage).unwrap();
    assert!(restored.catalog.project(id).is_ok());
    let moved = repo.path.join("moved-server-state");
    std::fs::rename(&storage, &moved).unwrap();
    std::fs::write(&storage, "not a directory").unwrap();
    assert_eq!(
        restored.git(&request).unwrap_err().code(),
        muxy_protocol::ErrorCode::PersistenceFailed
    );
    std::fs::remove_file(&storage).unwrap();
    std::fs::rename(&moved, &storage).unwrap();
    assert!(matches!(
        restored.git(&request).unwrap(),
        GitReply::Project(_)
    ));
}

#[test]
fn opposing_staged_and_unstaged_edits_keep_their_line_statistics() {
    let repo = Repo::new(true);
    std::fs::write(repo.path.join("file"), "original\n").unwrap();
    run(&repo.path, &["add", "file"]).unwrap();
    run(&repo.path, &["commit", "-m", "file"]).unwrap();
    std::fs::write(repo.path.join("file"), "staged\n").unwrap();
    run(&repo.path, &["add", "file"]).unwrap();
    std::fs::write(repo.path.join("file"), "original\n").unwrap();
    let files = read::changes(&repo.path).unwrap();
    assert_eq!((files[0].added, files[0].removed), (Some(2), Some(2)));
    assert!(files[0].staged() && files[0].unstaged());
}

#[test]
fn linked_worktree_watch_detects_common_refs_and_stops_when_dropped() {
    let repo = Repo::new(true);
    let (child, _, _) = repo.create();
    let (send, events) = mpsc::sync_channel(1);
    let watch = repo
        .registry
        .watch_git(child, move || {
            let _ = send.try_send(());
        })
        .unwrap()
        .unwrap();
    run(&repo.path, &["branch", "shared-ref"]).unwrap();
    events
        .recv_timeout(std::time::Duration::from_secs(8))
        .unwrap();
    drop(watch);
    assert!(
        events
            .recv_timeout(std::time::Duration::from_secs(1))
            .is_err()
    );
}

mod extensions;

#[test]
fn repository_reads_overlap_and_mutations_wait_for_readers() {
    let repo = Repo::new(true);
    let lock = repo.registry.git.lock_for(&repo.path).unwrap();
    let guard = lock.read().unwrap();
    let repo = &repo;
    std::thread::scope(|scope| {
        let (read, completed) = mpsc::channel();
        scope.spawn(move || {
            let _ = read.send(repo.git(GitAction::Summary));
        });
        let result = completed.recv_timeout(std::time::Duration::from_secs(3));
        let (write, written) = mpsc::channel();
        scope.spawn(move || {
            let _ = write.send(repo.git(GitAction::CreateBranch("parallel".into())));
        });
        let early_write = written.recv_timeout(std::time::Duration::from_millis(100));
        drop(guard);
        assert!(matches!(
            result.unwrap().unwrap(),
            GitReply::Summary(Some(_))
        ));
        assert!(early_write.is_err());
        assert!(matches!(
            written
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap()
                .unwrap(),
            GitReply::Done
        ));
    });
}
