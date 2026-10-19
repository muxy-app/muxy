use super::*;
use muxy_protocol::{WorktreeHook, WorktreeOptions};

/// A hook as `sh` runs it on Unix, or as `cmd.exe` runs it on Windows. There
/// `<nul set /p =text` writes text without a newline, and fails, so a hook
/// that ends with it exits explicitly.
fn hook(unix: &'static str, windows: &'static str) -> &'static str {
    if cfg!(windows) { windows } else { unix }
}

fn configure(repo: &Repo, setup: &[&str], teardown: &[&str]) {
    std::fs::create_dir_all(repo.path.join(".muxy")).unwrap();
    std::fs::write(
        repo.path.join(".muxy/worktree.json"),
        serde_json::to_vec(&serde_json::json!({"setup": setup, "teardown": teardown})).unwrap(),
    )
    .unwrap();
}

fn preview(repo: &Repo, teardown: bool) -> Vec<WorktreeHook> {
    let GitReply::WorktreeHooks(hooks) = repo.git(GitAction::WorktreeHooks { teardown }).unwrap()
    else {
        panic!("hooks")
    };
    hooks
}

fn create_request(repo: &Repo, hooks: Option<Vec<WorktreeHook>>) -> GitRequest {
    GitRequest {
        project: repo.project,
        action: GitAction::Worktree(WorktreeIntent {
            operation: OperationId::new(),
            options: Some(WorktreeOptions {
                name: Some("Friendly name".into()),
                hooks,
            }),
            action: WorktreeAction::Create {
                project: ProjectId::new(),
                directory: server_path(&repo.path.join("nested/trees/feature")).unwrap(),
                branch: "feature".into(),
                base: Some("main".into()),
            },
        }),
    }
}

fn remove_request(repo: &Repo, project: ProjectId, hooks: Option<Vec<WorktreeHook>>) -> GitRequest {
    let GitReply::Removal(expected) = repo
        .registry
        .git(&GitRequest {
            project,
            action: GitAction::InspectRemoval,
        })
        .unwrap()
    else {
        panic!("inspection")
    };
    GitRequest {
        project,
        action: GitAction::Worktree(WorktreeIntent {
            operation: OperationId::new(),
            options: Some(WorktreeOptions { name: None, hooks }),
            action: WorktreeAction::Remove { expected },
        }),
    }
}

#[test]
fn worktree_hooks_use_the_source_config_environment_order_and_run_once() {
    let repo = Repo::new(true);
    std::fs::write(
        repo.path.join("machine-worktree.json"),
        serde_json::to_vec(&serde_json::json!({
            "setup": [hook("printf global > order", "<nul set /p =global> order& exit /b 0")],
            "teardown": [hook(
                "printf global >> \"$MUXY_PROJECT_PATH/removed\"",
                r#"<nul set /p =global>> "%MUXY_PROJECT_PATH%\removed"& exit /b 0"#,
            )],
        }))
        .unwrap(),
    )
    .unwrap();
    configure(
        &repo,
        &[
            hook(
                "printf project >> order",
                "<nul set /p =project>> order& exit /b 0",
            ),
            hook(
                "printf '%s\n' \"$MUXY_PROJECT_PATH\" \"$MUXY_WORKTREE_PATH\" \"$MUXY_WORKTREE_NAME\" \"$MUXY_WORKTREE_BRANCH\" \"$MUXY_WORKTREE_ID\" > environment",
                "(echo %MUXY_PROJECT_PATH%& echo %MUXY_WORKTREE_PATH%& echo %MUXY_WORKTREE_NAME%& echo %MUXY_WORKTREE_BRANCH%& echo %MUXY_WORKTREE_ID%)> environment",
            ),
        ],
        &[
            hook(
                "printf project > \"$MUXY_PROJECT_PATH/removed\"",
                r#"<nul set /p =project> "%MUXY_PROJECT_PATH%\removed"& exit /b 0"#,
            ),
            hook("rm order", "del order"),
        ],
    );
    let request = create_request(&repo, Some(preview(&repo, false)));
    let GitReply::Project(project) = repo.registry.git(&request).unwrap() else {
        panic!("project")
    };
    assert_eq!(project.name, "Friendly name");
    let target = path(&project.directory).unwrap();
    assert_eq!(
        std::fs::read_to_string(target.join("order")).unwrap(),
        "globalproject"
    );
    let environment = std::fs::read_to_string(target.join("environment")).unwrap();
    let values: Vec<_> = environment.lines().collect();
    assert_eq!(Path::new(values[0]), repo.path);
    assert_eq!(Path::new(values[1]), target);
    assert_eq!(
        &values[2..],
        ["Friendly name", "feature", &project.id.to_string()]
    );
    assert_eq!(
        repo.registry.git(&request).unwrap(),
        GitReply::Project(project.clone())
    );
    assert_eq!(
        std::fs::read_to_string(target.join("order")).unwrap(),
        "globalproject"
    );
    // The source project config is used, even when it wasn't committed into the worktree.
    assert!(!target.join(".muxy/worktree.json").exists());
    let removal = remove_request(&repo, project.id, Some(preview(&repo, true)));
    repo.registry.git(&removal).unwrap();
    repo.registry.git(&removal).unwrap();
    assert!(!target.exists());
    assert_eq!(
        std::fs::read_to_string(repo.path.join("removed")).unwrap(),
        "projectglobal"
    );
}

#[test]
fn worktree_hooks_are_opt_in_and_changed_approval_is_rejected_before_creation() {
    let repo = Repo::new(true);
    configure(
        &repo,
        &[hook("touch setup-ran", "type nul > setup-ran")],
        &[hook("touch teardown-ran", "type nul > teardown-ran")],
    );
    let approved = preview(&repo, false);
    configure(&repo, &[hook("touch changed", "type nul > changed")], &[]);
    let request = create_request(&repo, Some(approved));
    assert!(
        repo.registry
            .git(&request)
            .unwrap_err()
            .message()
            .contains("changed after approval")
    );
    assert!(!repo.path.join("nested").exists());
    // Even invalid repository-owned configuration cannot force execution or block an opt-out.
    std::fs::write(repo.path.join(".muxy/worktree.json"), "invalid").unwrap();
    let GitReply::Project(project) = repo.registry.git(&create_request(&repo, None)).unwrap()
    else {
        panic!("project")
    };
    assert!(!path(&project.directory).unwrap().join("setup-ran").exists());
    repo.registry
        .git(&remove_request(&repo, project.id, None))
        .unwrap();
}

#[test]
fn worktree_setup_failure_keeps_a_registered_checkout_and_reports_it_on_replay() {
    let repo = Repo::new(true);
    configure(
        &repo,
        &[
            hook(
                "printf once >> attempted; echo setup-failure >&2; exit 1",
                "<nul set /p =once>> attempted& echo setup-failure 1>&2 & exit /b 1",
            ),
            hook("touch should-not-run", "type nul > should-not-run"),
        ],
        &[],
    );
    let request = create_request(&repo, Some(preview(&repo, false)));
    let reply = repo.registry.git(&request).unwrap();
    let GitReply::WorktreeSetupFailed { project, message } = &reply else {
        panic!("setup failure")
    };
    assert!(message.contains("setup-failure"));
    assert_eq!(repo.registry.catalog.project(project.id).unwrap(), *project);
    assert!(path(&project.directory).unwrap().join(".git").is_file());
    assert!(
        !path(&project.directory)
            .unwrap()
            .join("should-not-run")
            .exists()
    );
    assert_eq!(repo.registry.git(&request).unwrap(), reply);
    assert_eq!(
        std::fs::read_to_string(path(&project.directory).unwrap().join("attempted")).unwrap(),
        "once"
    );
}

#[test]
fn worktree_teardown_failure_preserves_files_and_changed_hooks_require_new_approval() {
    let repo = Repo::new(true);
    let (id, target, _) = repo.create();
    std::fs::write(target.join("keep"), "keep").unwrap();
    configure(
        &repo,
        &[],
        &[hook(
            "echo teardown-failure >&2; exit 1",
            "echo teardown-failure 1>&2 & exit /b 1",
        )],
    );
    let failed = remove_request(&repo, id, Some(preview(&repo, true)));
    assert!(
        repo.registry
            .git(&failed)
            .unwrap_err()
            .message()
            .contains("teardown-failure")
    );
    assert!(target.join("keep").exists());
    assert!(repo.registry.catalog.project(id).is_ok());
    let changed = remove_request(&repo, id, Some(preview(&repo, true)));
    configure(
        &repo,
        &[],
        &[hook("touch unexpected", "type nul > unexpected")],
    );
    assert!(
        repo.registry
            .git(&changed)
            .unwrap_err()
            .message()
            .contains("changed after approval")
    );
    assert!(!target.join("unexpected").exists());
    repo.registry.git(&remove_request(&repo, id, None)).unwrap();
    assert!(!target.exists());
}

#[test]
fn worktree_recovery_does_not_repeat_an_interrupted_hook() {
    let repo = Repo::new(true);
    configure(
        &repo,
        &[hook(
            "printf x >> attempts",
            "<nul set /p =x>> attempts& exit /b 0",
        )],
        &[],
    );
    let request = create_request(&repo, Some(preview(&repo, false)));
    let GitReply::Project(project) = repo.registry.git(&request).unwrap() else {
        panic!("project")
    };
    let GitAction::Worktree(intent) = &request.action else {
        panic!("intent")
    };
    let mut receipt = repo.registry.catalog.git_receipt(intent.operation).unwrap();
    receipt.reply = None;
    receipt.hooks_finished = false;
    repo.registry.catalog.save_git(receipt).unwrap();
    let GitReply::WorktreeSetupFailed { message, .. } = repo.registry.git(&request).unwrap() else {
        panic!("interrupted")
    };
    assert!(message.contains("interrupted"));
    assert_eq!(
        std::fs::read_to_string(path(&project.directory).unwrap().join("attempts")).unwrap(),
        "x"
    );
}
