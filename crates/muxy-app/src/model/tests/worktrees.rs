use super::git::registered_current_project;
use super::*;
use muxy_protocol::{GitWorktree, ProjectMutation, ServerPath};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Work the model sent, kept so worktree syncs stay pending until a test answers them.
struct Sent(std::sync::mpsc::Receiver<(u64, Work)>, Vec<Work>);

impl Sent {
    fn take(&mut self) -> &[Work] {
        let start = self.1.len();
        self.1.extend(self.0.try_iter().map(|(_, work)| work));
        &self.1[start..]
    }

    fn syncs(&mut self) -> usize {
        self.take()
            .iter()
            .filter(|work| matches!(work, Work::ExtensionClient(_)))
            .count()
    }
}

fn worktree(directory: &[u8], primary: bool) -> GitWorktree {
    GitWorktree {
        directory: ServerPath(directory.to_vec()),
        head: None,
        branch: None,
        primary,
        locked: false,
        bare: false,
        detached: false,
        prunable: false,
        registered: None,
    }
}

fn connected(
    state: AppState,
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext, Sent) {
    let (boot, requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let mut sent = Sent(requests, Vec::new());
    view.update(cx, |model, _| {
        model.servers.local.connection = ConnectionState::Ready;
    });
    sent.take();
    (view, cx, sent)
}

#[test]
fn listed_worktrees_skip_the_main_checkout_leftovers_and_the_projects_own_folder() {
    let checkout = tempfile::tempdir().expect("checkout");
    let links = tempfile::tempdir().expect("links");
    let link = links.path().join("checkout");
    std::os::unix::fs::symlink(checkout.path(), &link).expect("symlink");
    let bytes = |path: &Path| path.as_os_str().as_bytes().to_vec();
    let own = crate::model::worktrees::resolved(&ServerPath(bytes(&link)), true);
    let listed = |worktree: &GitWorktree| crate::model::worktrees::listed(worktree, &own, true);
    let reported = checkout.path().canonicalize().expect("resolved checkout");
    let mut prunable = worktree(b"/code/app-gone", false);
    prunable.prunable = true;
    assert!(
        !listed(&worktree(&bytes(&reported), false)),
        "a project opened through a symlink still recognizes its own folder"
    );
    assert!(!listed(&worktree(b"/code/app", true)));
    assert!(!listed(&prunable));
    assert!(listed(&worktree(b"/code/app-feature", false)));
    let remote = crate::model::worktrees::resolved(&ServerPath(bytes(&link)), false);
    assert_eq!(remote, link);
    assert!(crate::model::worktrees::listed(
        &worktree(&bytes(&reported), false),
        &remote,
        false
    ));
}

fn pruning_fixture() -> (AppState, ProjectId, [ProjectId; 3], tempfile::TempDir) {
    let (mut state, root) = registered_current_project();
    let directory = tempfile::tempdir().expect("worktrees");
    let children = [(); 3].map(|()| ProjectId::new());
    let mut projects: Vec<_> = state
        .projects()
        .iter()
        .map(muxy_app_core::Project::descriptor)
        .collect();
    for id in children {
        let path = directory.path().join(id.to_string());
        std::fs::create_dir(&path).expect("worktree");
        std::fs::write(path.join(".git"), "gitdir: /tmp/unused").expect("marker");
        let mut descriptor = state.project(root).expect("parent").descriptor();
        descriptor.id = id;
        descriptor.directory = ServerPath(path.as_os_str().as_bytes().to_vec());
        descriptor.parent_id = Some(root);
        descriptor.kind = Some(muxy_app_core::ProjectKind::Worktree);
        projects.push(descriptor);
    }
    state
        .apply_catalog(
            ServerId::local(),
            &muxy_protocol::CatalogPage {
                server: muxy_protocol::ServerIdentity::from_u128(1),
                home: state.home().id,
                revision: 1,
                projects,
                next: None,
                legacy_home: None,
            },
        )
        .expect("catalog");
    (state, root, children, directory)
}

#[gpui::test]
fn sync_prunes_empty_worktrees_without_allowing_tabs_during_reconciliation(
    cx: &mut TestAppContext,
) {
    let (mut state, root, [missing, listed, with_tab], _directory) = pruning_fixture();
    state.open_terminal_tab(with_tab).expect("tab");
    state.select_project(root).expect("parent");
    std::fs::remove_dir_all(&state.project(missing).expect("missing").directory).expect("delete");
    let mut entry = worktree(
        state
            .project(listed)
            .expect("listed")
            .directory
            .as_os_str()
            .as_bytes(),
        false,
    );
    entry.registered = Some(listed);
    let missing_record = state.project(missing).expect("missing").descriptor();
    let (view, cx, mut sent) = connected(state, cx);
    view.update(cx, |model, cx| {
        model.sync_worktrees(root, cx);
        assert_eq!(sent.syncs(), 1);
        model.worktrees_synced(
            root,
            model.generation(ServerId::local()),
            Ok(vec![entry.clone()]),
            cx,
        );
        let intents = model.state.project_intents(ServerId::local());
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].mutation, ProjectMutation::PruneWorktree(missing));
        let operation = intents[0].operation;
        assert!(model.state.project(missing).is_none());
        assert_cannot_open_tabs(&mut model.state, missing);
        let mut page = muxy_protocol::CatalogPage {
            server: muxy_protocol::ServerIdentity::from_u128(1),
            home: model.state.home().id,
            revision: model.state.catalog_revision(ServerId::local()) + 1,
            projects: model
                .state
                .projects()
                .iter()
                .map(muxy_app_core::Project::descriptor)
                .collect(),
            next: None,
            legacy_home: None,
        };
        page.projects.push(missing_record);
        model.receive_catalog(ServerId::local(), Ok(page.clone()), cx);
        assert!(
            model.state.project(missing).is_none(),
            "pending pruning cannot be undone by an older catalog"
        );
        model.sync_worktrees(root, cx);
        model.worktrees_synced(
            root,
            model.generation(ServerId::local()),
            Ok(vec![entry]),
            cx,
        );
        assert_eq!(
            model.state.project_intents(ServerId::local()).len(),
            1,
            "pruning is not queued twice"
        );
        model.receive_project_mutation(ServerId::local(), operation, Ok(page.revision + 1), cx);
        model.receive_catalog(ServerId::local(), Ok(page.clone()), cx);
        assert_cannot_open_tabs(&mut model.state, missing);
        page.revision += 1;
        page.projects.retain(|project| project.id != missing);
        model.receive_catalog(ServerId::local(), Ok(page), cx);
        assert!(model.state.project(missing).is_none());
        assert!(model.state.project(listed).is_some());
        assert_eq!(
            model
                .state
                .project(with_tab)
                .expect("saved worktree")
                .tabs
                .len(),
            1
        );
    });
}

#[gpui::test]
fn failed_and_obsolete_worktree_lists_never_prune(cx: &mut TestAppContext) {
    let (state, root, _, _directory) = pruning_fixture();
    let (view, cx, mut sent) = connected(state, cx);
    view.update(cx, |model, cx| {
        model.sync_worktrees(root, cx);
        assert_eq!(sent.syncs(), 1);
        model.worktrees_synced(
            root,
            model.generation(ServerId::local()),
            Err(muxy_client::ClientError::Disconnected),
            cx,
        );
        assert!(model.state.project_intents(ServerId::local()).is_empty());
        model.sync_worktrees(root, cx);
        assert_eq!(sent.syncs(), 1);
        let previous = model.generation(ServerId::local());
        model.servers.local.generation += 1;
        model.worktrees_synced(root, previous, Ok(Vec::new()), cx);
        assert!(model.state.project_intents(ServerId::local()).is_empty());
    });
}

#[gpui::test]
fn worktree_sync_preserves_tabs_opened_while_it_was_running(cx: &mut TestAppContext) {
    let (state, root, [with_tab, listed, by_path], _directory) = pruning_fixture();
    let mut entries: Vec<_> = [listed, by_path]
        .into_iter()
        .map(|id| {
            worktree(
                state
                    .project(id)
                    .expect("worktree")
                    .directory
                    .as_os_str()
                    .as_bytes(),
                false,
            )
        })
        .collect();
    entries[0].registered = Some(listed);
    entries[1].locked = true;
    let (view, cx, mut sent) = connected(state, cx);
    view.update(cx, |model, cx| {
        model.sync_worktrees(root, cx);
        assert_eq!(sent.syncs(), 1);
        model.state.open_terminal_tab(with_tab).expect("tab");
        model.state.select_project(root).expect("parent");
        model.worktrees_synced(root, model.generation(ServerId::local()), Ok(entries), cx);
        assert!(model.state.project_intents(ServerId::local()).is_empty());
    });
}

#[gpui::test]
fn worktree_sync_never_prunes_projects_added_after_the_listing_started(cx: &mut TestAppContext) {
    let (state, root, children, _directory) = pruning_fixture();
    let (view, cx, mut sent) = connected(state, cx);
    view.update(cx, |model, cx| {
        model.sync_worktrees(root, cx);
        assert_eq!(sent.syncs(), 1);
        let mut projects: Vec<_> = model
            .state
            .projects()
            .iter()
            .map(muxy_app_core::Project::descriptor)
            .collect();
        let mut added = model
            .state
            .project(children[0])
            .expect("child")
            .descriptor();
        added.id = ProjectId::new();
        let new_child = added.id;
        projects.push(added);
        model.receive_catalog(
            ServerId::local(),
            Ok(muxy_protocol::CatalogPage {
                server: muxy_protocol::ServerIdentity::from_u128(1),
                home: model.state.home().id,
                revision: model.state.catalog_revision(ServerId::local()) + 1,
                projects,
                next: None,
                legacy_home: None,
            }),
            cx,
        );
        model.worktrees_synced(
            root,
            model.generation(ServerId::local()),
            Ok(Vec::new()),
            cx,
        );
        assert_eq!(
            model.state.project_intents(ServerId::local()).len(),
            children.len()
        );
        assert!(
            model
                .state
                .project_intents(ServerId::local())
                .iter()
                .all(|intent| intent.mutation != ProjectMutation::PruneWorktree(new_child))
        );
    });
}

#[test]
fn pruning_preflight_skips_existing_folders_locked_worktrees_history_and_failed_reads() {
    let directory = tempfile::tempdir().expect("folders");
    let [empty, history, failed, existing, locked] = [(); 5].map(|()| ProjectId::new());
    let missing_path = |id: ProjectId| directory.path().join(id.to_string());
    let mut entry = worktree(missing_path(locked).as_os_str().as_bytes(), false);
    entry.locked = true;
    let entries = vec![entry];
    let candidates = vec![
        (empty, missing_path(empty)),
        (history, missing_path(history)),
        (failed, missing_path(failed)),
        (existing, directory.path().to_owned()),
        (locked, missing_path(locked)),
    ];
    let mut queried = Vec::new();
    let mut pending = Box::pin(crate::model::worktrees::prune_candidates(
        &entries,
        candidates,
        |id| {
            queried.push(id);
            std::future::ready(if id == failed {
                Err(muxy_client::ClientError::Disconnected)
            } else {
                Ok(id == history)
            })
        },
        |_, folder| std::future::ready(folder.try_exists().is_ok_and(|exists| !exists)),
    ));
    let result = Future::poll(
        pending.as_mut(),
        &mut std::task::Context::from_waker(std::task::Waker::noop()),
    );
    assert_eq!(result, std::task::Poll::Ready(HashSet::from([empty])));
    drop(pending);
    assert_eq!(queried, [empty, history, failed]);
}

#[gpui::test]
fn pruning_uses_remaining_queue_capacity_and_resumes_after_reconciliation(cx: &mut TestAppContext) {
    let (mut state, root, children, _directory) = pruning_fixture();
    while state.project_intent_capacity(ServerId::local()) > 1 {
        state.rename_project(root, "Pending edit").expect("edit");
    }
    let (view, cx, mut sent) = connected(state, cx);
    view.update(cx, |model, cx| {
        model.sync_worktrees(root, cx);
        assert_eq!(sent.syncs(), 1);
        model.worktrees_synced(
            root,
            model.generation(ServerId::local()),
            Ok(Vec::new()),
            cx,
        );
        assert!(model.error.is_none());
        assert_eq!(model.state.project_intent_capacity(ServerId::local()), 0);
        assert_eq!(
            children
                .iter()
                .filter(|id| model.state.project(**id).is_some())
                .count(),
            2
        );
        sent.take();
        acknowledge_catalog(model, cx);
        assert_eq!(
            sent.syncs(),
            1,
            "remaining candidates sync after the batch settles"
        );
        model.worktrees_synced(
            root,
            model.generation(ServerId::local()),
            Ok(Vec::new()),
            cx,
        );
        assert_eq!(model.state.project_intents(ServerId::local()).len(), 2);
        assert!(children.iter().all(|id| model.state.project(*id).is_none()));
    });
}

fn assert_cannot_open_tabs(state: &mut AppState, project: ProjectId) {
    assert!(state.open_terminal_tab(project).is_err());
    assert!(
        state
            .open_webview(
                project,
                muxy_app_core::webview::WebviewDescriptor {
                    owner: "test".into(),
                    kind: "test".into(),
                    data: serde_json::Value::Null,
                },
                "test",
                false
            )
            .is_err()
    );
}

#[test]
fn a_missing_worktree_kept_for_its_tabs_can_still_be_removed() {
    let (mut state, _, [gone, _, _], _directory) = pruning_fixture();
    state.open_terminal_tab(gone).expect("tab");
    std::fs::remove_dir_all(&state.project(gone).expect("worktree").directory).expect("delete");
    state.refresh_project_statuses();
    let project = state.project(gone).expect("kept for its tab");
    let commands: Vec<_> = crate::views::project_menu::worktree_items(project, false)
        .iter()
        .filter_map(crate::views::menu::Item::command)
        .collect();
    assert!(
        matches!(commands.as_slice(), [crate::views::menu::Command::RemoveProject(id)] if *id == gone)
    );
}

fn creation(name: &str) -> muxy_protocol::WorktreeIntent {
    muxy_protocol::WorktreeIntent {
        options: Some(muxy_protocol::WorktreeOptions {
            name: Some(name.into()),
            hooks: None,
        }),
        operation: muxy_protocol::OperationId::new(),
        action: muxy_protocol::WorktreeAction::Create {
            project: ProjectId::new(),
            directory: ServerPath(format!("/tmp/{name}").into_bytes()),
            branch: name.into(),
            base: Some("HEAD".into()),
        },
    }
}

#[gpui::test]
fn a_failed_worktree_creation_is_reported_after_moving_elsewhere_and_clears_its_row(
    cx: &mut TestAppContext,
) {
    let (state, root) = registered_current_project();
    let (view, cx, _sent) = connected(state, cx);
    view.update(cx, |model, cx| {
        let intent = creation("feature");
        model.create_worktree(
            root,
            intent.clone(),
            muxy_app_core::settings::WorktreeLocation::default(),
            cx,
        );
        assert_eq!(model.worktree_creations(root).count(), 1);
        model.select_project(model.state.home().id, cx);
        model.receive_git(
            &muxy_protocol::GitRequest {
                project: root,
                action: muxy_protocol::GitAction::Worktree(intent),
            },
            Err(muxy_client::ClientError::Disconnected),
            cx,
        );
        assert_eq!(model.worktree_creations(root).count(), 0);
        assert!(model.error.is_some());
    });
}

#[gpui::test]
fn removal_after_merge_reaches_its_confirmation_after_unrelated_interactions(
    cx: &mut TestAppContext,
) {
    use muxy_protocol::{GitAction, GitPullRequestAction, GitReply, GitRequest};
    let (mut state, _, [worktree, _, _], _directory) = pruning_fixture();
    state.select_project(worktree).expect("worktree");
    let (view, cx, mut sent) = connected(state, cx);
    let merge = GitPullRequestAction::Merge {
        number: 7,
        method: muxy_protocol::GitMergeMethod::Squash,
        delete_branch: false,
        expected_head: Some("abc".into()),
    };
    let request = |action| GitRequest {
        project: worktree,
        action,
    };
    view.update(cx, |model, cx| {
        model.merge_and_remove_worktree(worktree, merge.clone(), cx);
        model.receive_git(&request(GitAction::PullRequest(merge)), Ok(GitReply::Done), cx);
        assert!(sent.take().iter().any(|work| matches!(
            work,
            Work::Git(GitRequest { project, action: GitAction::InspectRemoval }) if *project == worktree
        )));
        model.dismiss_overlay(cx);
        let expected = muxy_protocol::WorktreeRemoval {
            directory: ServerPath(b"/unused".to_vec()),
            device: 1,
            inode: 1,
            dirty: false,
            status: vec![],
            head: None,
            branch: None,
        };
        model.receive_git(
            &request(GitAction::InspectRemoval),
            Ok(GitReply::Removal(expected)),
            cx,
        );
        model.dismiss_overlay(cx);
        model.receive_git(
            &request(GitAction::WorktreeHooks { teardown: true }),
            Ok(GitReply::WorktreeHooks(vec![])),
            cx,
        );
        assert!(model.close_prompt.is_some(), "removal is confirmed first");
    });
}

fn prepare_merge_removal(
    model: &mut AppModel,
    worktree: ProjectId,
    cx: &mut Context<AppModel>,
) -> muxy_protocol::WorktreeRemoval {
    use muxy_protocol::{GitAction, GitPullRequestAction, GitReply, GitRequest};
    let merge = GitPullRequestAction::Merge {
        number: 7,
        method: muxy_protocol::GitMergeMethod::Squash,
        delete_branch: false,
        expected_head: Some("abc".into()),
    };
    model.merge_and_remove_worktree(worktree, merge.clone(), cx);
    model.receive_git(
        &GitRequest {
            project: worktree,
            action: GitAction::PullRequest(merge),
        },
        Ok(GitReply::Done),
        cx,
    );
    let expected = muxy_protocol::WorktreeRemoval {
        directory: ServerPath(b"/unused".to_vec()),
        device: 1,
        inode: 1,
        dirty: true,
        status: vec![],
        head: Some("abc".into()),
        branch: Some("feature".into()),
    };
    model.receive_git(
        &GitRequest {
            project: worktree,
            action: GitAction::InspectRemoval,
        },
        Ok(GitReply::Removal(expected.clone())),
        cx,
    );
    expected
}

#[gpui::test]
fn merge_removal_waits_for_another_confirmation_and_still_requires_approval(
    cx: &mut TestAppContext,
) {
    use muxy_protocol::{GitAction, GitReply, GitRequest, WorktreeAction, WorktreeHook};
    for hooks in [
        Ok(vec![WorktreeHook {
            command: "echo teardown".into(),
            name: None,
            project: true,
        }]),
        Err(muxy_client::ClientError::Disconnected),
    ] {
        let approved_hooks = hooks.as_ref().ok().cloned();
        let (mut state, root, [worktree, _, _], _directory) = pruning_fixture();
        state.select_project(worktree).expect("worktree");
        let (view, cx, _sent) = connected(state, cx);
        let expected = view.update(cx, |model, cx| {
            let expected = prepare_merge_removal(model, worktree, cx);
            model.select_project(root, cx);
            model.confirm_remove_project(root, cx);
            model.receive_git(
                &GitRequest {
                    project: worktree,
                    action: GitAction::WorktreeHooks { teardown: true },
                },
                hooks.map(GitReply::WorktreeHooks),
                cx,
            );
            expected
        });
        cx.run_until_parked();
        assert!(cx.has_pending_prompt());
        assert!(!view.read_with(cx, |model, _| model.worktree_removing(worktree)));
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert!(
            cx.has_pending_prompt(),
            "removal must wait for its own confirmation"
        );
        assert!(!view.read_with(cx, |model, _| model.worktree_removing(worktree)));
        cx.simulate_prompt_answer(if approved_hooks.is_some() {
            "Confirm with teardown commands"
        } else {
            "Confirm"
        });
        cx.run_until_parked();
        view.read_with(cx, |model, _| {
            let action = model
                .git
                .projects
                .get(&worktree)
                .and_then(super::super::git::Repository::mutation);
            let Some(GitAction::Worktree(intent)) = action else {
                panic!("removal was not requested");
            };
            assert_eq!(intent.action, WorktreeAction::Remove { expected });
            assert_eq!(
                intent
                    .options
                    .as_ref()
                    .and_then(|options| options.hooks.as_ref()),
                approved_hooks.as_ref()
            );
            assert!(
                model.state.project(root).is_some(),
                "the unrelated removal was cancelled"
            );
        });
    }
}

#[gpui::test]
fn disconnect_discards_a_merge_removal_waiting_for_confirmation(cx: &mut TestAppContext) {
    use muxy_protocol::{GitAction, GitReply, GitRequest};
    let (mut state, root, [worktree, _, _], _directory) = pruning_fixture();
    state.select_project(worktree).expect("worktree");
    let (view, cx, _sent) = connected(state, cx);
    view.update(cx, |model, cx| {
        prepare_merge_removal(model, worktree, cx);
        model.confirm_remove_project(root, cx);
        model.receive_git(
            &GitRequest {
                project: worktree,
                action: GitAction::WorktreeHooks { teardown: true },
            },
            Ok(GitReply::WorktreeHooks(vec![])),
            cx,
        );
        model.disconnect(ServerId::local(), cx);
    });
    cx.run_until_parked();
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(!cx.has_pending_prompt());
    assert!(!view.read_with(cx, |model, _| model.worktree_removing(worktree)));
}
