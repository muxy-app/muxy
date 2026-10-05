use super::git::registered_current_project;
use super::*;
use muxy_app_core::settings::Settings;
use muxy_protocol::{ErrorCode, ErrorReply, GitWorktree, ProjectMutation, ServerPath};
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

fn register(model: &mut AppModel, project: ProjectId, cx: &mut Context<AppModel>) {
    let create = model
        .state
        .project_intents(ServerId::local())
        .iter()
        .find(|intent| matches!(&intent.mutation, ProjectMutation::Create(record) if record.id == project))
        .expect("pending create")
        .operation;
    model.receive_project_mutation(ServerId::local(), create, Ok(1), cx);
}

fn add_and_register(model: &mut AppModel, cx: &mut Context<AppModel>) -> ProjectId {
    let project = model
        .add_project(std::env::temp_dir(), cx)
        .expect("project");
    register(model, project, cx);
    project
}

fn connected(
    state: AppState,
    cx: &mut TestAppContext,
) -> (Entity<AppModel>, &mut VisualTestContext, Sent, PathBuf) {
    let (boot, requests) = stub_boot(state);
    let settings = boot.state_path.with_file_name("settings.toml");
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let mut sent = Sent(requests, Vec::new());
    view.update(cx, |model, _| {
        model.servers.local.connection = ConnectionState::Ready;
    });
    sent.take();
    (view, cx, sent, settings)
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

#[gpui::test]
fn new_projects_show_worktrees_once_git_lists_linked_ones(cx: &mut TestAppContext) {
    let (view, cx, mut sent, settings) = connected(AppState::bootstrap().expect("state"), cx);
    view.update(cx, |model, cx| {
        let project = model
            .add_project(std::env::temp_dir(), cx)
            .expect("project");
        assert!(!model.worktrees_visible(project));
        assert_eq!(sent.syncs(), 0, "the server doesn't know the project yet");
        register(model, project, cx);
        assert_eq!(sent.syncs(), 1);
        model.servers.local.catalog.pending = false;
        let linked = vec![worktree(b"/code/app-feature", false)];
        model.worktrees_synced(
            project,
            model.servers.local.generation + 1,
            Ok(linked.clone()),
            cx,
        );
        assert!(
            !model.worktrees_visible(project),
            "results from another connection are ignored"
        );
        model.worktrees_synced(project, model.servers.local.generation, Ok(linked), cx);
        assert!(model.worktrees_visible(project));
        let saved = Settings::load(&settings).expect("settings").appearance;
        assert!(!saved.hidden_worktrees.contains(&project));
        assert!(
            sent.take()
                .iter()
                .any(|work| matches!(work, Work::ReadCatalog)),
            "imported worktrees are read back into the sidebar"
        );
    });
}

#[gpui::test]
fn new_projects_keep_worktrees_hidden_without_linked_ones(cx: &mut TestAppContext) {
    let (view, cx, mut sent, settings) = connected(AppState::bootstrap().expect("state"), cx);
    view.update(cx, |model, cx| {
        let project = add_and_register(model, cx);
        assert_eq!(sent.syncs(), 1, "one listing per new project");
        model.servers.local.catalog.pending = false;
        model.worktrees_synced(project, model.servers.local.generation, Ok(Vec::new()), cx);
        assert!(!model.worktrees_visible(project));
        let saved = Settings::load(&settings).expect("settings").appearance;
        assert!(saved.hidden_worktrees.contains(&project));
        assert!(
            !sent
                .take()
                .iter()
                .any(|work| matches!(work, Work::ReadCatalog))
        );
        model.sync_all_worktrees(cx);
        assert_eq!(sent.syncs(), 0, "hidden worktrees are not synced");
    });
}

#[gpui::test]
fn new_project_check_retries_after_disconnects_but_not_after_server_errors(
    cx: &mut TestAppContext,
) {
    let (view, cx, mut sent, _) = connected(AppState::bootstrap().expect("state"), cx);
    view.update(cx, |model, cx| {
        let project = add_and_register(model, cx);
        assert_eq!(sent.syncs(), 1);
        model.worktrees_synced(
            project,
            model.servers.local.generation,
            Err(muxy_client::ClientError::Disconnected),
            cx,
        );
        assert!(!model.worktrees_visible(project));
        model.sync_all_worktrees(cx);
        assert_eq!(sent.syncs(), 1, "the check runs again on the next sync");
        model.worktrees_synced(
            project,
            model.servers.local.generation,
            Err(muxy_client::ClientError::Server(ErrorReply {
                code: ErrorCode::BadRequest,
                message: "not a git repository".into(),
            })),
            cx,
        );
        model.sync_all_worktrees(cx);
        assert!(!model.worktrees_visible(project));
        assert_eq!(sent.syncs(), 0);
    });
}

#[gpui::test]
fn a_worktrees_menu_choice_replaces_the_new_project_default(cx: &mut TestAppContext) {
    let (view, cx, mut sent, _) = connected(AppState::bootstrap().expect("state"), cx);
    view.update(cx, |model, cx| {
        let project = add_and_register(model, cx);
        assert_eq!(sent.syncs(), 1);
        model.toggle_worktree_visibility(project, cx);
        model.toggle_worktree_visibility(project, cx);
        model.worktrees_synced(
            project,
            model.servers.local.generation,
            Ok(vec![worktree(b"/code/app-feature", false)]),
            cx,
        );
        assert!(!model.worktrees_visible(project));
    });
}

#[gpui::test]
fn only_git_changes_rerun_a_sync_that_is_already_running(cx: &mut TestAppContext) {
    let (state, project) = registered_current_project();
    let (view, cx, mut sent, _) = connected(state, cx);
    view.update(cx, |model, cx| {
        model.sync_git(cx);
        model.sync_worktrees(project, cx);
        assert_eq!(sent.syncs(), 1);
        model.sync_worktrees(project, cx);
        model.worktrees_synced(project, model.servers.local.generation, Ok(Vec::new()), cx);
        assert_eq!(
            sent.syncs(),
            0,
            "asking again while it runs changes nothing"
        );
        model.sync_worktrees(project, cx);
        assert_eq!(sent.syncs(), 1);
        model.git_invalidated(project, cx);
        model.worktrees_synced(project, model.servers.local.generation, Ok(Vec::new()), cx);
        assert_eq!(sent.syncs(), 1, "a Git change during a sync runs it again");
    });
}

#[gpui::test]
fn worktrees_sync_one_project_at_a_time_and_skip_hidden_ones(cx: &mut TestAppContext) {
    let mut state = AppState::bootstrap().expect("state");
    let [first, second, hidden] = [(); 3].map(|()| {
        state
            .add_project(ServerId::local(), std::env::temp_dir())
            .expect("project")
    });
    while let Some(intent) = state.project_intents(ServerId::local()).first().cloned() {
        state
            .complete_project_intent(ServerId::local(), intent.operation)
            .expect("registered");
    }
    state.select_project(state.home().id).expect("home");
    let (mut boot, requests) = stub_boot(state);
    boot.settings.appearance.hidden_worktrees.insert(hidden);
    let (view, cx) = cx.add_window_view(|window, cx| AppModel::new(boot, window, cx));
    let mut sent = Sent(requests, Vec::new());
    view.update(cx, |model, cx| {
        model.receive((ServerId::local(), 1, Update::Connected(Vec::new())), cx);
        sent.take();
        acknowledge_catalog(model, cx);
        assert_eq!(sent.syncs(), 1, "one project syncs at a time");
        for (project, next) in [(first, 1), (second, 0)] {
            model.worktrees_synced(project, model.servers.local.generation, Ok(Vec::new()), cx);
            assert_eq!(sent.syncs(), next);
        }
    });
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
    let (view, cx, mut sent, _) = connected(state, cx);
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
    let (view, cx, mut sent, _) = connected(state, cx);
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
    let (view, cx, mut sent, _) = connected(state, cx);
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
    let (view, cx, mut sent, _) = connected(state, cx);
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
    let (view, cx, mut sent, _) = connected(state, cx);
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
