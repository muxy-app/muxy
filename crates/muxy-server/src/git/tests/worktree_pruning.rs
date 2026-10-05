use super::*;

fn prune(repo: &Repo, project: ProjectId) -> ProjectIntent {
    let intent = ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::PruneWorktree(project),
    };
    repo.registry.mutate_project(&intent).unwrap();
    intent
}

#[test]
fn pruning_removes_deleted_worktrees_and_replays_without_changing_the_catalog() {
    for remove_with_git in [true, false] {
        let repo = Repo::new(true);
        let (child, directory, _) = repo.create();
        if remove_with_git {
            run(
                &repo.path,
                &["worktree", "remove", directory.to_str().unwrap()],
            )
            .unwrap();
        } else {
            std::fs::remove_dir_all(&directory).unwrap();
            assert!(
                read::worktrees(&repo.path)
                    .unwrap()
                    .iter()
                    .any(|worktree| worktree.prunable)
            );
        }
        let intent = prune(&repo, child);
        assert!(repo.registry.catalog.project(child).is_err());
        let revision = repo.registry.catalog.revision();
        repo.registry.mutate_project(&intent).unwrap();
        assert_eq!(repo.registry.catalog.revision(), revision);
        assert!(repo.registry.catalog.project(repo.project).is_ok());
        assert!(repo.path.is_dir());
    }
}

#[test]
fn pruning_preserves_existing_folders_home_and_regular_projects() {
    let repo = Repo::new(true);
    let (child, directory, _) = repo.create();
    for id in [child, repo.project, repo.registry.home_project()] {
        prune(&repo, id);
        assert!(repo.registry.catalog.project(id).is_ok());
    }
    run(
        &repo.path,
        &["worktree", "remove", directory.to_str().unwrap()],
    )
    .unwrap();
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("keep"), "replacement folder").unwrap();
    prune(&repo, child);
    assert!(repo.registry.catalog.project(child).is_ok());
    assert_eq!(
        std::fs::read_to_string(directory.join("keep")).unwrap(),
        "replacement folder"
    );
}

#[test]
fn pruning_preserves_worktrees_when_the_parent_is_unavailable() {
    let repo = Repo::new(true);
    let (child, directory, _) = repo.create();
    std::fs::remove_dir_all(&directory).unwrap();
    let moved = repo.path.with_extension("unavailable");
    std::fs::rename(&repo.path, &moved).unwrap();
    let intent = ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::PruneWorktree(child),
    };
    let result = repo.registry.mutate_project(&intent);
    std::fs::rename(moved, &repo.path).unwrap();
    result.unwrap();
    assert!(repo.registry.catalog.project(child).is_ok());
}

#[test]
fn pruning_preserves_sessions_until_their_history_is_discarded() {
    let repo = Repo::new(true);
    let (child, directory, _) = repo.create();
    let session = repo
        .registry
        .create_project_session(
            child,
            OperationId::new(),
            &directory,
            muxy_protocol::Size { cols: 80, rows: 24 },
        )
        .unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    prune(&repo, child);
    assert!(repo.registry.catalog.project(child).is_ok());
    assert_eq!(
        repo.registry
            .list_project_sessions(child, None, None)
            .unwrap()
            .sessions
            .len(),
        1
    );
    repo.registry.end(session.id).unwrap();
    prune(&repo, child);
    assert!(repo.registry.catalog.project(child).is_ok());
    repo.registry.discard(session.id).unwrap();
    prune(&repo, child);
    assert!(repo.registry.catalog.project(child).is_err());
}

#[test]
fn pruning_waits_for_active_worktree_operations() {
    let repo = Repo::new(true);
    let (child, directory, _) = repo.create();
    std::fs::remove_dir_all(&directory).unwrap();
    let reservation = repo
        .registry
        .git
        .operations
        .reserve(vec![child], None)
        .unwrap();
    let intent = ProjectIntent {
        operation: OperationId::new(),
        mutation: ProjectMutation::PruneWorktree(child),
    };
    assert!(
        repo.registry
            .mutate_project(&intent)
            .unwrap_err()
            .message()
            .contains("active worktree operation")
    );
    assert!(repo.registry.catalog.project(child).is_ok());
    drop(reservation);
    repo.registry.mutate_project(&intent).unwrap();
    assert!(repo.registry.catalog.project(child).is_err());
}
