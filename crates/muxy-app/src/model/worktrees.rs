use std::collections::{HashSet, VecDeque};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use gpui::Context;
use muxy_app_core::{ProjectStatus, ServerId};
use muxy_client::{Client, ClientError};
use muxy_protocol::{
    GitAction, GitReply, GitRequest, GitWorktree, OperationId, ProjectId, ReplyBody, ServerPath,
    WorktreeAction, WorktreeIntent,
};

use super::{AppModel, Work};

/// Brings Git worktrees made outside Muxy into each project, one project at a
/// time so a long sidebar never floods the server.
#[derive(Default)]
pub(crate) struct WorktreeSync {
    queue: VecDeque<ProjectId>,
    /// The project syncing now, with the connection generation it runs on.
    running: Option<(ProjectId, u64)>,
    candidates: HashSet<ProjectId>,
    pending_pruning: HashSet<ProjectId>,
    /// Git changed while the running project synced, so it syncs once more.
    again: bool,
    /// New projects keep worktrees hidden unless their repository already has some.
    unchecked: HashSet<ProjectId>,
}

/// Git's worktrees for a top-level project.
pub(super) async fn list(
    client: &Client,
    root: ProjectId,
) -> Result<Vec<GitWorktree>, ClientError> {
    let request = GitRequest {
        project: root,
        action: GitAction::Worktrees,
    };
    match client.git_async(request).await? {
        GitReply::Worktrees(worktrees) => Ok(worktrees),
        reply => Err(ClientError::UnexpectedReply(Box::new(ReplyBody::Git(
            reply,
        )))),
    }
}

/// Registers every worktree `root` doesn't show yet. A failure only counts if
/// the worktree is still unregistered afterwards, since another client may
/// have registered it meanwhile. `local` says the folders are on this
/// computer.
pub(super) async fn register(
    client: &Client,
    root: ProjectId,
    directory: &ServerPath,
    local: bool,
    worktrees: &[GitWorktree],
) -> Result<(), ClientError> {
    let own = resolved(directory, local);
    let mut failures = Vec::new();
    for worktree in worktrees
        .iter()
        .filter(|worktree| listed(worktree, &own, local) && worktree.registered.is_none())
    {
        let request = GitRequest {
            project: root,
            action: GitAction::Worktree(WorktreeIntent {
                options: None,
                operation: OperationId::new(),
                action: WorktreeAction::Register {
                    project: ProjectId::new(),
                    directory: worktree.directory.clone(),
                },
            }),
        };
        if let Err(error) = client.git_async(request).await {
            failures.push((&worktree.directory, error));
        }
    }
    if failures.is_empty() {
        return Ok(());
    }
    let current = list(client, root).await?;
    failures
        .into_iter()
        .find(|(directory, _)| {
            current
                .iter()
                .any(|worktree| worktree.directory == **directory && worktree.registered.is_none())
        })
        .map_or(Ok(()), |(_, error)| Err(error))
}

/// A worktree the project at `own` lists under itself: neither the main
/// checkout nor the project's own folder.
pub(super) fn listed(worktree: &GitWorktree, own: &Path, local: bool) -> bool {
    !worktree.primary
        && !worktree.bare
        && !worktree.prunable
        && resolved(&worktree.directory, local) != own
}

/// A folder the way Git reports it, with symlinks and letter case resolved
/// on this computer. Another computer's folders can't be resolved here.
pub(super) fn resolved(directory: &ServerPath, local: bool) -> PathBuf {
    let path = Path::new(OsStr::from_bytes(&directory.0));
    if !local {
        return path.to_owned();
    }
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}

/// Registers the worktrees `root` doesn't show yet and returns the ones it lists.
async fn import(
    client: &Client,
    root: ProjectId,
    directory: &ServerPath,
    local: bool,
) -> Result<Vec<GitWorktree>, ClientError> {
    let worktrees = list(client, root).await?;
    if let Err(error) = register(client, root, directory, local, &worktrees).await {
        crate::diagnostics::event(
            "worktrees.register",
            format_args!("project={root} error={error}"),
        );
    }
    let own = resolved(directory, local);
    Ok(worktrees
        .into_iter()
        .filter(|worktree| listed(worktree, &own, local) || worktree.locked)
        .collect())
}

pub(super) async fn prune_candidates<
    F: Future<Output = Result<bool, ClientError>>,
    G: Future<Output = bool>,
>(
    worktrees: &[GitWorktree],
    candidates: Vec<(ProjectId, PathBuf)>,
    mut has_sessions: impl FnMut(ProjectId) -> F,
    mut folder_gone: impl FnMut(ProjectId, &Path) -> G,
) -> HashSet<ProjectId> {
    let mut eligible = HashSet::new();
    for (project, directory) in candidates {
        if worktrees.iter().any(|worktree| {
            worktree.registered == Some(project)
                || worktree.directory.0 == directory.as_os_str().as_bytes()
        }) || !folder_gone(project, &directory).await
        {
            continue;
        }
        if has_sessions(project)
            .await
            .is_ok_and(|has_sessions| !has_sessions)
        {
            eligible.insert(project);
        }
    }
    eligible
}

/// Whether a worktree's folder is gone: checked on this disk, or for
/// another computer's project, by its server.
fn folder_gone(
    client: &Client,
    local: bool,
    project: ProjectId,
    directory: &Path,
) -> impl Future<Output = bool> + use<> {
    let here = local.then(|| directory.try_exists().is_ok_and(|exists| !exists));
    let there = (!local).then(|| {
        client.files_async(muxy_protocol::FilesRequest {
            project,
            action: muxy_protocol::FilesAction::Stat(ServerPath(Vec::new())),
        })
    });
    async move {
        match (here, there) {
            (Some(gone), _) => gone,
            (None, Some(stat)) => matches!(
                stat.await,
                Err(ClientError::Server(error)) if error.code == muxy_protocol::ErrorCode::BadPath
            ),
            (None, None) => false,
        }
    }
}

impl AppModel {
    pub(crate) fn save_worktree_preference(
        &mut self,
        project: ProjectId,
        location: muxy_app_core::settings::WorktreeLocation,
        cx: &mut Context<Self>,
    ) {
        let mut settings = self.settings.clone();
        if location.is_default() {
            settings.worktrees.projects.remove(&project);
        } else {
            settings.worktrees.projects.insert(project, location);
        }
        match settings.save_worktrees(&self.path.with_file_name("settings.toml")) {
            Ok(()) => self.settings.worktrees = settings.worktrees,
            Err(error) => self.fail(
                format!(
                    "Worktree created, but its location preference could not be saved: {error}"
                ),
                cx,
            ),
        }
    }

    pub(super) fn hide_new_project_worktrees(
        &mut self,
        project: ProjectId,
        cx: &mut Context<Self>,
    ) {
        self.git.worktrees.unchecked.insert(project);
        self.appearance.hidden_worktrees.insert(project);
        self.save_appearance(cx);
    }

    /// A choice made in the project menu replaces the new-project default.
    pub(crate) fn worktrees_toggled(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.git.worktrees.unchecked.remove(&project);
        self.sync_worktrees(project, cx);
    }

    /// Syncs the worktrees of `project`'s top-level project next.
    pub(super) fn sync_worktrees(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.request_worktree_sync(project, false, cx);
    }

    /// Like `sync_worktrees`, and syncs again if a sync is already running,
    /// because Git changed since it started.
    pub(super) fn resync_worktrees(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.request_worktree_sync(project, true, cx);
    }

    fn request_worktree_sync(&mut self, project: ProjectId, changed: bool, cx: &mut Context<Self>) {
        let Some(root) = self
            .state
            .project(project)
            .map(|project| project.parent_id.unwrap_or(project.id))
        else {
            return;
        };
        let generation = self.project_generation(root);
        let sync = &mut self.git.worktrees;
        if sync.running == Some((root, generation)) {
            sync.again |= changed;
        } else {
            sync.queue.retain(|queued| *queued != root);
            sync.queue.push_front(root);
        }
        self.next_worktree_sync(cx);
    }

    pub(super) fn sync_all_worktrees(&mut self, cx: &mut Context<Self>) {
        let roots: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| project.parent_id.is_none())
            .map(|project| (project.id, self.project_generation(project.id)))
            .collect();
        let sync = &mut self.git.worktrees;
        for (root, generation) in roots {
            if sync.running != Some((root, generation)) && !sync.queue.contains(&root) {
                sync.queue.push_back(root);
            }
        }
        self.next_worktree_sync(cx);
    }

    /// Syncs one project at a time, on its own server. Projects whose server
    /// is not connected wait in the queue.
    fn next_worktree_sync(&mut self, cx: &mut Context<Self>) {
        if self
            .git
            .worktrees
            .running
            .is_some_and(|(root, running)| running == self.project_generation(root))
        {
            return;
        }
        self.git.worktrees.running = None;
        self.git.worktrees.again = false;
        let mut waiting = Vec::new();
        while let Some(root) = self.git.worktrees.queue.pop_front() {
            let Some(directory) = self.worktree_sync_directory(root) else {
                continue;
            };
            let server = self.project_server_or_local(root);
            if !self.ready(server) {
                waiting.push(root);
                continue;
            }
            let generation = self.generation(server);
            let local = server.is_local();
            let (sender, receiver) = async_channel::bounded(1);
            if !self.send(server, Work::ExtensionClient(sender), cx) {
                waiting.push(root);
                break;
            }
            self.git.worktrees.running = Some((root, generation));
            let candidates: Vec<_> = self
                .state
                .projects()
                .iter()
                .filter(|project| project.parent_id == Some(root) && project.tabs.is_empty())
                .map(|project| (project.id, project.directory.clone()))
                .collect();
            self.git.worktrees.candidates = candidates.iter().map(|(id, _)| *id).collect();
            let task = cx.background_executor().spawn(async move {
                match receiver.recv().await {
                    Ok(Some(client)) => {
                        let worktrees = import(&client, root, &directory, local).await?;
                        let eligible = prune_candidates(
                            &worktrees,
                            candidates,
                            |project| client.project_has_sessions_async(project),
                            |project, directory| folder_gone(&client, local, project, directory),
                        )
                        .await;
                        Ok((worktrees, eligible))
                    }
                    _ => Err(ClientError::Disconnected),
                }
            });
            cx.spawn(async move |model, cx| {
                let result = task.await;
                let _ = model.update(cx, |model, cx| {
                    let result = result.map(|(worktrees, eligible)| {
                        if model.git.worktrees.running == Some((root, generation)) {
                            model
                                .git
                                .worktrees
                                .candidates
                                .retain(|id| eligible.contains(id));
                        }
                        worktrees
                    });
                    model.worktrees_synced(root, generation, result, cx);
                });
            })
            .detach();
            break;
        }
        for root in waiting.into_iter().rev() {
            self.git.worktrees.queue.push_front(root);
        }
    }

    /// Syncs again the roots of `server` that had more worktrees to prune
    /// than its pending edits could hold, once those are confirmed.
    pub(super) fn resume_worktree_pruning(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if !self.state.project_intents(server).is_empty() {
            return;
        }
        let roots: Vec<_> = self
            .git
            .worktrees
            .pending_pruning
            .iter()
            .copied()
            .filter(|root| self.project_server_or_local(*root) == server)
            .collect();
        for root in roots {
            self.git.worktrees.pending_pruning.remove(&root);
            self.sync_worktrees(root, cx);
        }
    }

    fn worktree_sync_directory(&self, root: ProjectId) -> Option<ServerPath> {
        let project = self.state.project(root)?;
        (!project.home
            && project.parent_id.is_none()
            && project.status() == ProjectStatus::Available
            && (self.worktrees_visible(root) || self.git.worktrees.unchecked.contains(&root))
            && !self.state.project_creation_pending(root))
        .then(|| ServerPath(project.directory.as_os_str().as_bytes().to_vec()))
    }

    /// Takes the worktrees `root` lists, as `import` returns them.
    pub(super) fn worktrees_synced(
        &mut self,
        root: ProjectId,
        generation: u64,
        result: Result<Vec<GitWorktree>, ClientError>,
        cx: &mut Context<Self>,
    ) {
        let server = self.project_server_or_local(root);
        if generation != self.generation(server)
            || !self.ready(server)
            || self.git.worktrees.running != Some((root, generation))
        {
            return;
        }
        let candidates = std::mem::take(&mut self.git.worktrees.candidates);
        match result {
            Ok(worktrees) => {
                let registered: HashSet<_> = worktrees
                    .iter()
                    .filter_map(|worktree| worktree.registered)
                    .collect();
                let directories: HashSet<_> = worktrees
                    .iter()
                    .map(|worktree| worktree.directory.0.as_slice())
                    .collect();
                let candidates: Vec<_> = self
                    .state
                    .projects()
                    .iter()
                    .filter(|project| {
                        candidates.contains(&project.id)
                            && !registered.contains(&project.id)
                            && project.parent_id == Some(root)
                            && project.tabs.is_empty()
                            && !directories.contains(project.directory.as_os_str().as_bytes())
                    })
                    .map(|project| project.id)
                    .collect();
                // Another computer's projects are pruned only once its
                // catalog shows it is still the server the app knew.
                let capacity = if self.confirmed(server) {
                    self.state.project_intent_capacity(server)
                } else {
                    0
                };
                if candidates.len() > capacity {
                    self.git.worktrees.pending_pruning.insert(root);
                }
                if !candidates.is_empty() && capacity > 0 {
                    self.edit_project(
                        |state| {
                            for project in candidates.into_iter().take(capacity) {
                                state.prune_worktree(project)?;
                            }
                            Ok(())
                        },
                        cx,
                    );
                }
                if self.git.worktrees.unchecked.remove(&root) && !worktrees.is_empty() {
                    self.appearance.hidden_worktrees.remove(&root);
                    self.save_appearance(cx);
                }
                if worktrees
                    .iter()
                    .any(|worktree| worktree.registered.is_none())
                {
                    self.refresh_catalog(self.project_server_or_local(root), cx);
                }
            }
            Err(error) => {
                if matches!(error, ClientError::Server(_)) {
                    self.git.worktrees.unchecked.remove(&root);
                }
                crate::diagnostics::event(
                    "worktrees.sync",
                    format_args!("project={root} error={error}"),
                );
            }
        }
        self.git.worktrees.running = None;
        if std::mem::take(&mut self.git.worktrees.again) {
            self.git.worktrees.queue.push_front(root);
        }
        self.next_worktree_sync(cx);
        cx.notify();
    }
}
