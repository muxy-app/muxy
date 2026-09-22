use super::{AppModel, Work};
use crate::views::overlays::Overlay;
use gpui::Context;
use muxy_protocol::{GitAction, GitBranch, GitFile, GitReply, GitRequest, GitSummary, ProjectId};
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
pub(crate) struct Repository {
    pub(crate) summary: Option<GitSummary>,
    pub(crate) branches: Vec<GitBranch>,
    pub(crate) files: Vec<GitFile>,
    pub(crate) error: Option<String>,
    pub(crate) pending: bool,
    mutating: bool,
    context: u64,
    pub(super) queued: Option<(GitAction, u64)>,
    pub(crate) loaded: bool,
    reading: Option<GitAction>,
    refresh: VecDeque<GitAction>,
    read_loaded: [bool; 5],
    read_errors: [Option<String>; 5],
}
fn read_slot(action: &GitAction) -> Option<usize> {
    match action {
        GitAction::Summary => Some(0),
        GitAction::Branches => Some(1),
        GitAction::Changes => Some(2),
        GitAction::Worktrees => Some(3),
        GitAction::Watch => Some(4),
        _ => None,
    }
}
impl Repository {
    pub(crate) fn has_loaded(&self, action: &GitAction) -> bool {
        read_slot(action).is_some_and(|slot| self.read_loaded[slot])
    }
    pub(crate) fn load_error(&self, action: &GitAction) -> Option<&String> {
        self.error
            .as_ref()
            .or_else(|| read_slot(action).and_then(|slot| self.read_errors[slot].as_ref()))
    }
    pub(super) fn disconnect(&mut self) {
        self.pending = false;
        self.mutating = false;
        self.queued = None;
        self.reading = None;
        self.refresh.clear();
    }

    pub(crate) fn busy(&self) -> bool {
        self.mutating || self.queued.is_some()
    }
}
#[derive(Default)]
pub(crate) struct GitState {
    pub(crate) branch_anchor: muxy_ui::popover::PopoverAnchor,
    pub(crate) changes_anchor: muxy_ui::popover::PopoverAnchor,
    pub(crate) projects: HashMap<ProjectId, Repository>,
    current: Option<ProjectId>,
    pub(crate) select_after_catalog: Option<(ProjectId, u64)>,
    pub(crate) interaction: u64,
}
impl GitState {
    pub(super) fn reset_context(&mut self) {
        self.current = None;
    }
}
impl AppModel {
    pub(super) fn git_invalidated(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        if self.git.current != Some(project) || !self.session_listing_ready() {
            return;
        }
        let mut actions = vec![GitAction::Summary];
        if let Some(Overlay::Git(picker)) = &self.overlay
            && picker.project == project
        {
            actions.push(picker.kind.action());
        }
        self.queue_git_refresh(project, actions, cx);
    }

    pub(crate) fn git_request(
        &mut self,
        project: ProjectId,
        action: GitAction,
        cx: &mut Context<Self>,
    ) {
        self.request_git_context(project, action, self.git.interaction, cx);
    }

    fn request_git_context(
        &mut self,
        project: ProjectId,
        action: GitAction,
        context: u64,
        cx: &mut Context<Self>,
    ) {
        if !self.session_listing_ready() {
            return;
        }
        if action == GitAction::Watch && self.git.current != Some(project) {
            self.dispatch_git_refresh(project, cx);
            return;
        }
        let read = read_slot(&action).is_some();
        if let Some(repository) = self.git.projects.get_mut(&project).filter(|r| r.pending) {
            if read {
                if repository.reading.as_ref() != Some(&action)
                    && !repository.refresh.contains(&action)
                {
                    repository.refresh.push_back(action);
                }
            } else if !repository.mutating && repository.queued.is_none() {
                repository.queued = Some((action, context));
                self.update_git_picker(cx);
                cx.notify();
            } else {
                self.fail("A Git action is already pending".into(), cx);
            }
            return;
        }
        let request = GitRequest {
            project,
            action: action.clone(),
        };
        if self.send(Work::Git(request), cx) {
            let repository = self.git.projects.entry(project).or_default();
            repository.pending = true;
            repository.mutating = !read;
            repository.context = context;
            repository.reading = read.then_some(action.clone());
            if let Some(slot) = read_slot(&action) {
                repository.read_errors[slot] = None;
            } else {
                repository.error = None;
            }
        }
        self.update_git_picker(cx);
        cx.notify();
    }
    pub(super) fn sync_git(&mut self, cx: &mut Context<Self>) {
        let current = self.state.current_project().id;
        if self.git.current != Some(current) {
            self.git.current = Some(current);
            self.git.interaction = self.git.interaction.wrapping_add(1);
            if matches!(self.overlay, Some(Overlay::Git(_) | Overlay::GitForm(_))) {
                self.dismiss_overlay(cx);
            }
            self.queue_git_refresh(current, vec![GitAction::Watch, GitAction::Summary], cx);
        }
    }
    pub(crate) fn refresh_git(&mut self, cx: &mut Context<Self>) {
        let current = self.state.current_project().id;
        let mut actions = vec![GitAction::Watch, GitAction::Summary];
        if self
            .git
            .projects
            .get(&current)
            .is_some_and(|r| r.summary.is_some())
        {
            actions.push(GitAction::Branches);
        }
        if let Some(Overlay::Git(picker)) = &self.overlay {
            if picker.project == current {
                actions.push(picker.kind.action());
            } else {
                self.queue_git_refresh(
                    picker.project,
                    vec![GitAction::Summary, picker.kind.action()],
                    cx,
                );
            }
        }
        self.queue_git_refresh(current, actions, cx);
    }

    pub(crate) fn queue_git_refresh(
        &mut self,
        project: ProjectId,
        actions: Vec<GitAction>,
        cx: &mut Context<Self>,
    ) {
        if !self.session_listing_ready() {
            return;
        }
        let repository = self.git.projects.entry(project).or_default();
        for action in actions {
            if !repository.refresh.contains(&action) {
                repository.refresh.push_back(action);
            }
        }
        self.dispatch_git_refresh(project, cx);
    }

    fn dispatch_git_refresh(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        let Some(repository) = self.git.projects.get_mut(&project).filter(|r| !r.pending) else {
            return;
        };
        if let Some((action, context)) = repository.queued.take() {
            self.request_git_context(project, action, context, cx);
        } else if let Some(action) = repository.refresh.pop_front() {
            self.git_request(project, action, cx);
        }
    }
    pub(super) fn receive_git(
        &mut self,
        request: &GitRequest,
        result: Result<GitReply, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        let repository = self.git.projects.entry(request.project).or_default();
        repository.pending = false;
        repository.mutating = false;
        repository.reading = None;
        if let Some(slot) = read_slot(&request.action) {
            repository.read_loaded[slot] = true;
            if result.is_ok() && request.action != GitAction::Watch {
                repository.error = None;
            }
        }
        let context_matches = repository.context == self.git.interaction;
        match result {
            Ok(GitReply::Summary(summary)) => {
                let head_changed = repository.summary.as_ref().map(|s| (&s.branch, &s.head))
                    != summary.as_ref().map(|s| (&s.branch, &s.head));
                repository.summary = summary;
                repository.loaded = true;
                if repository.summary.is_none() {
                    repository.refresh.retain(|action| {
                        !matches!(action, GitAction::Branches | GitAction::Changes)
                    });
                } else if head_changed
                    && request.project == self.state.current_project().id
                    && !repository.refresh.contains(&GitAction::Branches)
                {
                    repository.refresh.push_back(GitAction::Branches);
                }
            }
            Ok(GitReply::Branches(branches)) => {
                repository.branches = branches;
                if context_matches {
                    self.update_worktree_default(request.project, cx);
                }
            }
            Ok(GitReply::Changes(files)) => repository.files = files,
            Ok(GitReply::Removal(expected)) if context_matches => {
                self.confirm_git_action(request.project, GitAction::Worktree(muxy_protocol::WorktreeIntent {
                    operation: muxy_protocol::OperationId::new(), action: muxy_protocol::WorktreeAction::Remove { expected: expected.clone() },
                }), if expected.dirty { "Remove worktree and permanently discard its uncommitted changes? Local processes running from this worktree will stop and its files will be deleted." } else { "Remove worktree and delete its files? Local processes running from this worktree will stop." }.into(), cx);
            }
            Ok(GitReply::Project(project)) => {
                if context_matches {
                    self.dismiss_overlay(cx);
                    self.git.select_after_catalog = Some((project.id, self.git.interaction));
                }
                self.refresh_catalog(cx);
            }
            Ok(GitReply::Done) if request.action == GitAction::Watch => (),
            Ok(GitReply::Done) => {
                if context_matches && matches!(self.overlay, Some(Overlay::GitForm(_))) {
                    self.dismiss_overlay(cx);
                }
                if matches!(request.action, GitAction::Worktree(_)) {
                    if context_matches {
                        self.dismiss_overlay(cx);
                    }
                    self.refresh_catalog(cx);
                }
                let mut actions = vec![GitAction::Summary];
                if let Some(Overlay::Git(picker)) = &self.overlay
                    && picker.project == request.project
                {
                    actions.push(picker.kind.action());
                }
                self.queue_git_refresh(request.project, actions, cx);
            }
            Ok(_) => (),
            Err(error) => {
                if let Some(slot) = read_slot(&request.action) {
                    repository.read_errors[slot] = Some(error.to_string());
                    if matches!(request.action, GitAction::Summary) {
                        repository.summary = None;
                        repository.loaded = true;
                    }
                } else {
                    repository.error = Some(error.to_string());
                    if context_matches {
                        self.fail(error.to_string(), cx);
                    }
                    let mut actions = vec![GitAction::Summary];
                    if let Some(Overlay::Git(picker)) = &self.overlay
                        && picker.project == request.project
                    {
                        actions.push(picker.kind.action());
                    }
                    self.queue_git_refresh(request.project, actions, cx);
                }
            }
        }
        self.dispatch_git_refresh(request.project, cx);
        self.update_git_picker(cx);
        cx.notify();
    }
}
