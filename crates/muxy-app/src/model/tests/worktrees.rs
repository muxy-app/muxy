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
    // Another computer's folders are compared as its server reports them,
    // never resolved against this disk.
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
