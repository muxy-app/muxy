use super::{AppModel, Work};
use crate::views::overlays::Overlay;
use gpui::Context;
use muxy_protocol::{
    GitAction, GitBaseSwitch, GitBranch, GitFile, GitPullRequest, GitPullRequestAction, GitReply,
    GitRequest, GitSummary, ProjectId,
};
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
pub(crate) struct Repository {
    pub(crate) summary: Option<GitSummary>,
    pub(crate) branches: Vec<GitBranch>,
    pub(crate) files: Vec<GitFile>,
    pub(crate) pull_request: Option<GitPullRequest>,
    pub(crate) error: Option<String>,
    pub(crate) pending: bool,
    mutating: bool,
    context: u64,
    pub(super) queued: Option<(GitAction, u64)>,
    pub(crate) loaded: bool,
    reading: Option<GitAction>,
    refresh: VecDeque<GitAction>,
    read_loaded: [bool; 6],
    read_errors: [Option<String>; 6],
    /// The merged pull request number and base while the local base branch is updated.
    post_merge: Option<(u64, String)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Presence {
    Loading,
    None,
    Unavailable,
    Found,
}
const PULL_REQUEST_SLOT: usize = 5;
fn read_slot(action: &GitAction) -> Option<usize> {
    match action {
        GitAction::Summary => Some(0),
        GitAction::Branches => Some(1),
        GitAction::Changes => Some(2),
        GitAction::Worktrees => Some(3),
        GitAction::Watch => Some(4),
        GitAction::PullRequest(GitPullRequestAction::Info) => Some(PULL_REQUEST_SLOT),
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
        self.post_merge = None;
    }

    pub(crate) fn busy(&self) -> bool {
        self.mutating || self.queued.is_some()
    }

    pub(crate) fn pull_request_presence(&self) -> Presence {
        if !self.read_loaded[PULL_REQUEST_SLOT] {
            Presence::Loading
        } else if self.pull_request.is_some() {
            Presence::Found
        } else if self.read_errors[PULL_REQUEST_SLOT].is_some() {
            Presence::Unavailable
        } else {
            Presence::None
        }
    }
}
#[derive(Default)]
pub(crate) struct GitState {
    pub(crate) branch_anchor: muxy_ui::popover::PopoverAnchor,
    pub(crate) changes_anchor: muxy_ui::popover::PopoverAnchor,
    pub(crate) pull_request_anchor: muxy_ui::popover::PopoverAnchor,
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
        if read_slot(&action).is_none() && self.ai.running(project) {
            self.fail("Wait for the AI repository action to finish".into(), cx);
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
        if self
            .ai
            .confirmation
            .as_ref()
            .is_some_and(|pending| pending.project != current)
        {
            self.ai.confirmation = None;
        }
        if self.project_creation_pending(current) {
            return;
        }
        if self.git.current != Some(current) {
            self.git.current = Some(current);
            self.git.interaction = self.git.interaction.wrapping_add(1);
            if matches!(
                self.overlay,
                Some(
                    Overlay::Git(_)
                        | Overlay::GitForm(_)
                        | Overlay::PullRequest(_)
                        | Overlay::AiProvider(_)
                )
            ) {
                self.dismiss_overlay(cx);
            }
            self.queue_git_refresh(
                current,
                vec![
                    GitAction::Watch,
                    GitAction::Summary,
                    GitAction::Branches,
                    GitAction::PullRequest(GitPullRequestAction::Info),
                ],
                cx,
            );
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
        actions.push(GitAction::PullRequest(GitPullRequestAction::Info));
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
        if !self.session_listing_ready() || self.project_creation_pending(project) {
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
    #[allow(
        clippy::too_many_lines,
        reason = "Handle all Git reply variants in one place"
    )]
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
                    repository.pull_request = None;
                    repository.read_loaded[PULL_REQUEST_SLOT] = false;
                    repository.refresh.retain(|action| {
                        !matches!(
                            action,
                            GitAction::Branches
                                | GitAction::Changes
                                | GitAction::PullRequest(GitPullRequestAction::Info)
                        )
                    });
                } else if head_changed
                    && request.project == self.state.current_project().id
                    && !repository.refresh.contains(&GitAction::Branches)
                {
                    repository.refresh.push_back(GitAction::Branches);
                }
                if head_changed && repository.summary.is_some() {
                    repository.pull_request = None;
                    repository.read_loaded[PULL_REQUEST_SLOT] = false;
                    if !repository
                        .refresh
                        .contains(&GitAction::PullRequest(GitPullRequestAction::Info))
                    {
                        repository
                            .refresh
                            .push_back(GitAction::PullRequest(GitPullRequestAction::Info));
                    }
                }
            }
            Ok(GitReply::Branches(branches)) => {
                repository.branches = branches;
                if context_matches {
                    self.update_worktree_default(request.project, cx);
                }
            }
            Ok(GitReply::Changes(files)) => repository.files = files,
            Ok(GitReply::PullRequest(pr)) => repository.pull_request = pr.map(|pr| *pr),
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
                let active = request.project == self.state.current_project().id;
                let merged = match &request.action {
                    GitAction::PullRequest(GitPullRequestAction::Merge { number, .. }) => {
                        Some(*number)
                    }
                    _ => None,
                };
                let follow_up = merged.filter(|_| active).and_then(|number| {
                    repository
                        .pull_request
                        .as_ref()
                        .filter(|pr| pr.number == number)
                        .map(|pr| (number, pr.base_branch.clone()))
                });
                repository.post_merge.clone_from(&follow_up);
                let notice =
                    if active {
                        match &request.action {
                            GitAction::PullRequest(GitPullRequestAction::Merge {
                                number, ..
                            }) if follow_up.is_none() => Some(format!("Merged PR #{number}")),
                            GitAction::PullRequest(GitPullRequestAction::Close { number }) => {
                                Some(format!("Closed PR #{number}"))
                            }
                            GitAction::PullRequest(GitPullRequestAction::UpdateBranch {
                                number,
                                ..
                            }) => Some(format!("Updated branch for PR #{number}")),
                            _ => None,
                        }
                    } else {
                        None
                    };
                if let Some(notice) = notice {
                    self.show_notice(notice, cx);
                }
                if context_matches && matches!(self.overlay, Some(Overlay::GitForm(_))) {
                    self.dismiss_overlay(cx);
                }
                if matches!(request.action, GitAction::Worktree(_)) {
                    if context_matches {
                        self.dismiss_overlay(cx);
                    }
                    self.refresh_catalog(cx);
                }
                if let Some((_, base)) = follow_up {
                    self.git_request(request.project, GitAction::SwitchToBase(base), cx);
                } else {
                    let mut actions = vec![GitAction::Summary];
                    if matches!(request.action, GitAction::PullRequest(_)) {
                        actions.push(GitAction::PullRequest(GitPullRequestAction::Info));
                    }
                    if let Some(Overlay::Git(picker)) = &self.overlay
                        && picker.project == request.project
                    {
                        actions.push(picker.kind.action());
                    }
                    self.queue_git_refresh(request.project, actions, cx);
                }
            }
            Ok(GitReply::BaseSwitch(result)) => {
                let merged = repository.post_merge.take();
                if let Some((number, base)) = merged
                    && request.project == self.state.current_project().id
                {
                    let detail = match result {
                        GitBaseSwitch::Updated => {
                            format!("Switched to {base} and brought it up to date.")
                        }
                        GitBaseSwitch::CheckedOutElsewhere(_) => format!(
                            "{base} is checked out in another worktree, so this one stays on its branch."
                        ),
                    };
                    self.show_toast(format!("Merged PR #{number} into {base}"), Some(detail), cx);
                }
                self.queue_git_refresh(
                    request.project,
                    vec![
                        GitAction::Summary,
                        GitAction::Branches,
                        GitAction::PullRequest(GitPullRequestAction::Info),
                    ],
                    cx,
                );
            }
            Ok(_) => (),
            Err(error) => {
                if let Some(slot) = read_slot(&request.action) {
                    repository.read_errors[slot] = Some(error.to_string());
                    if matches!(request.action, GitAction::Summary) {
                        repository.summary = None;
                        repository.loaded = true;
                    } else if matches!(
                        request.action,
                        GitAction::PullRequest(GitPullRequestAction::Info)
                    ) {
                        repository.pull_request = None;
                    }
                } else {
                    let failed = match &request.action {
                        GitAction::SwitchToBase(base) => {
                            repository.post_merge.take().map(|(number, _)| {
                                format!("Merged PR #{number}, but couldn't update {base}")
                            })
                        }
                        GitAction::PullRequest(GitPullRequestAction::Merge { number, .. }) => {
                            Some(format!("Couldn't merge PR #{number}"))
                        }
                        GitAction::PullRequest(GitPullRequestAction::Close { number }) => {
                            Some(format!("Couldn't close PR #{number}"))
                        }
                        GitAction::PullRequest(GitPullRequestAction::UpdateBranch {
                            number,
                            ..
                        }) => Some(format!("Couldn't update PR #{number}")),
                        _ => None,
                    };
                    repository.error = Some(failed.clone().unwrap_or_else(|| error.to_string()));
                    let active = request.project == self.state.current_project().id;
                    match failed {
                        Some(title) if active => self.fail_detail(title, &error.to_string(), cx),
                        None if context_matches => self.fail(error.to_string(), cx),
                        _ => (),
                    }
                    let mut actions = vec![GitAction::Summary];
                    if matches!(
                        request.action,
                        GitAction::SwitchToBase(_) | GitAction::PullRequest(_)
                    ) {
                        actions.push(GitAction::Branches);
                        actions.push(GitAction::PullRequest(GitPullRequestAction::Info));
                    }
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
        if matches!(request.action, GitAction::Summary) {
            self.sync_extension_events(cx);
        }
        cx.notify();
    }
}
