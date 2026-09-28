use std::collections::{HashSet, VecDeque};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use gpui::Context;
use muxy_app_core::ProjectStatus;
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
/// have registered it meanwhile.
pub(super) async fn register(
    client: &Client,
    root: ProjectId,
    directory: &ServerPath,
    worktrees: &[GitWorktree],
) -> Result<(), ClientError> {
    let own = resolved(directory);
    let mut failures = Vec::new();
    for worktree in worktrees
        .iter()
        .filter(|worktree| listed(worktree, &own) && worktree.registered.is_none())
    {
        let request = GitRequest {
            project: root,
            action: GitAction::Worktree(WorktreeIntent {
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
pub(super) fn listed(worktree: &GitWorktree, own: &Path) -> bool {
    !worktree.primary
        && !worktree.bare
        && !worktree.prunable
        && resolved(&worktree.directory) != own
}

/// A folder the way Git reports it, with symlinks and letter case resolved.
pub(super) fn resolved(directory: &ServerPath) -> PathBuf {
    let path = Path::new(OsStr::from_bytes(&directory.0));
    path.canonicalize().unwrap_or_else(|_| path.to_owned())
}

/// Registers the worktrees `root` doesn't show yet and returns the ones it lists.
async fn import(
    client: &Client,
    root: ProjectId,
    directory: &ServerPath,
) -> Result<Vec<GitWorktree>, ClientError> {
    let worktrees = list(client, root).await?;
    if let Err(error) = register(client, root, directory, &worktrees).await {
        crate::diagnostics::event(
            "worktrees.register",
            format_args!("project={root} error={error}"),
        );
    }
    let own = resolved(directory);
    Ok(worktrees
        .into_iter()
        .filter(|worktree| listed(worktree, &own))
        .collect())
}

impl AppModel {
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
        let sync = &mut self.git.worktrees;
        if sync.running == Some((root, self.generation)) {
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
            .map(|project| project.id)
            .collect();
        let sync = &mut self.git.worktrees;
        for root in roots {
            if sync.running != Some((root, self.generation)) && !sync.queue.contains(&root) {
                sync.queue.push_back(root);
            }
        }
        self.next_worktree_sync(cx);
    }

    fn next_worktree_sync(&mut self, cx: &mut Context<Self>) {
        let generation = self.generation;
        if !self.session_listing_ready()
            || self
                .git
                .worktrees
                .running
                .is_some_and(|(_, running)| running == generation)
        {
            return;
        }
        self.git.worktrees.running = None;
        self.git.worktrees.again = false;
        while let Some(root) = self.git.worktrees.queue.pop_front() {
            let Some(directory) = self.worktree_sync_directory(root) else {
                continue;
            };
            let (sender, receiver) = async_channel::bounded(1);
            if !self.send(Work::ExtensionClient(sender), cx) {
                self.git.worktrees.queue.push_front(root);
                return;
            }
            self.git.worktrees.running = Some((root, generation));
            let task = cx.background_executor().spawn(async move {
                match receiver.recv().await {
                    Ok(Some(client)) => import(&client, root, &directory).await,
                    _ => Err(ClientError::Disconnected),
                }
            });
            cx.spawn(async move |model, cx| {
                let result = task.await;
                let _ = model.update(cx, |model, cx| {
                    model.worktrees_synced(root, generation, result, cx);
                });
            })
            .detach();
            return;
        }
    }

    fn worktree_sync_directory(&self, root: ProjectId) -> Option<ServerPath> {
        let project = self.state.project(root)?;
        (!project.home
            && project.parent_id.is_none()
            && project.status() == ProjectStatus::Available
            && (self.worktrees_visible(root) || self.git.worktrees.unchecked.contains(&root))
            && !self.project_creation_pending(root))
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
        if self.git.worktrees.running != Some((root, generation)) {
            return;
        }
        self.git.worktrees.running = None;
        match result {
            Ok(worktrees) => {
                if self.git.worktrees.unchecked.remove(&root) && !worktrees.is_empty() {
                    self.appearance.hidden_worktrees.remove(&root);
                    self.save_appearance(cx);
                }
                if worktrees
                    .iter()
                    .any(|worktree| worktree.registered.is_none())
                {
                    self.refresh_catalog(cx);
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
        if std::mem::take(&mut self.git.worktrees.again) {
            self.git.worktrees.queue.push_front(root);
        }
        self.next_worktree_sync(cx);
        cx.notify();
    }
}
