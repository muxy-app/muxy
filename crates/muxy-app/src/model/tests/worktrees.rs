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
fn empty_worktrees_stay_visible_in_tab_and_agents_layouts(cx: &mut TestAppContext) {
    use muxy_app_core::settings::AppLayout;

    let (state, root, [child, ..], _directory) = pruning_fixture();
    let expected: Vec<_> = state
        .projects()
        .iter()
        .filter(|project| project.parent_id == Some(root))
        .map(|project| {
            assert!(project.tabs.is_empty());
            project.id
        })
        .collect();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let listed = |model: &AppModel| {
            model
                .sidebar_projects()
                .into_iter()
                .filter(|project| project.parent_id == Some(root))
                .map(|project| project.id)
                .collect::<Vec<_>>()
        };
        for layout in [AppLayout::TabFocused, AppLayout::AgentsFocused] {
            model.set_layout(layout, cx);
            assert_eq!(listed(model), expected);
            for selected in [child, root, model.state.home().id] {
                model.select_project(selected, cx);
                model.sync_tab_sidebar(cx);
                assert_eq!(listed(model), expected);
            }
            model.select_project(root, cx);
            model.appearance.sidebar_focus = true;
            assert_eq!(listed(model), expected);
            model.toggle_worktree_visibility(root, cx);
            assert!(listed(model).is_empty());
            model.toggle_worktree_visibility(root, cx);
            assert_eq!(listed(model), expected);
            model.appearance.sidebar_focus = false;
        }
    });
}

#[gpui::test]
fn switching_worktrees_preserves_their_order_in_every_layout(cx: &mut TestAppContext) {
    use muxy_app_core::settings::AppLayout;

    let (mut state, root, children, _directory) = pruning_fixture();
    for child in children {
        state.open_terminal_tab(child).expect("worktree tab");
    }
    state.select_project(root).expect("parent");
    let expected: Vec<_> = state
        .projects()
        .iter()
        .filter(|project| project.parent_id == Some(root))
        .map(|project| project.id)
        .collect();
    let (mut boot, _requests) = stub_boot(state);
    boot.settings.appearance.worktree_recent = expected.iter().rev().copied().collect();
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    view.update(cx, |model, cx| {
        let order = |model: &AppModel| {
            model
                .worktree_children(root)
                .into_iter()
                .map(|project| project.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(order(model), expected);
        for layout in [
            AppLayout::ProjectFocused,
            AppLayout::TabFocused,
            AppLayout::AgentsFocused,
        ] {
            model.set_layout(layout, cx);
            for selected in [children[2], children[0], children[1], root, children[2]] {
                model.select_project(selected, cx);
                model.sync_tab_sidebar(cx);
                assert_eq!(model.state.current_project().id, selected);
                assert_eq!(order(model), expected);
                assert_eq!(model.preferred_worktree(root), selected);
                if layout != AppLayout::ProjectFocused {
                    let listed: Vec<_> = model
                        .sidebar_projects()
                        .into_iter()
                        .filter(|project| project.parent_id == Some(root))
                        .map(|project| project.id)
                        .collect();
                    assert_eq!(listed, expected);
                }
            }
            model.select_project(model.state.home().id, cx);
            model.sync_tab_sidebar(cx);
            assert_eq!(model.preferred_worktree(root), children[2]);
        }
    });
}

#[gpui::test]
fn worktree_menus_only_offer_worktree_removal(cx: &mut TestAppContext) {
    let (state, root, [child, ..], _directory) = pruning_fixture();
    let (boot, _requests) = stub_boot(state);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    cx.update(|window, cx| {
        view.update(cx, |model, cx| {
            let child_project = model.state.project(child).expect("worktree");
            let root_project = model.state.project(root).expect("parent");
            for (items, remove_project, remove_worktree) in [
                (model.project_menu(child_project), false, true),
                (
                    crate::views::project_menu::worktree_items(root_project, true),
                    false,
                    false,
                ),
                (model.project_menu(root_project), true, false),
            ] {
                model.open_menu(items, gpui::point(px(10.0), px(60.0)), window, cx);
                let rows = model.menu_outline();
                assert_eq!(
                    rows.iter().flatten().any(|row| row == "Remove Project…"),
                    remove_project
                );
                assert_eq!(
                    rows.iter()
                        .flatten()
                        .any(|row| row == "Remove Worktree and Files…"),
                    remove_worktree
                );
            }
            std::fs::remove_dir_all(&model.state.project(child).expect("worktree").directory)
                .expect("remove worktree folder");
            model.state.refresh_project_statuses();
            let missing = model.state.project(child).expect("missing worktree");
            assert_eq!(missing.status(), ProjectStatus::Missing);
            assert!(model.project_menu(missing).is_empty());
        });
    });
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
