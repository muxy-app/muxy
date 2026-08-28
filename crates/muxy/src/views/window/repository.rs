use super::*;
use crate::repository::{
    LoadState, PullRequestLoadState, PullRequestReadIdentity, RepositoryCoordinator,
    RepositoryReadKind, RepositoryRefreshSet,
};
use crate::views::repository::branch::{
    BranchMutationKind, BranchMutationPlan, branch_mutation_plan, present_branches,
};
use muxy_api::repository::{
    ActiveRepositoryWatcher, GitHubControl, MutationControl, MutationEffect, MutationOutcome,
    PullRequestLookup, RepositoryHead, RepositoryIdentity, RepositoryOptions, RepositoryService,
    StashAction, StashEntry,
};
use std::time::{Duration, Instant};

const REPOSITORY_WATCHER_DEBOUNCE: Duration = Duration::from_millis(800);

pub(super) struct RepositoryViewState {
    pub(super) coordinator: RepositoryCoordinator,
    watcher: Option<ActiveRepositoryWatcher>,
    watcher_task: Option<Task<()>>,
}

impl RepositoryViewState {
    pub(super) fn new() -> Self {
        let mut coordinator = RepositoryCoordinator::default();
        coordinator.set_changes_monitoring(false);
        Self {
            coordinator,
            watcher: None,
            watcher_task: None,
        }
    }

    fn reset_watcher(&mut self) {
        self.watcher_task = None;
        self.watcher = None;
    }
}

impl Drop for RepositoryViewState {
    fn drop(&mut self) {
        self.coordinator.close();
    }
}

impl MainWindow {
    pub(super) fn sync_repository_context(&mut self, cx: &mut Context<Self>) {
        let key = self.state.active_repository_key();
        if !self.view.repository.coordinator.activate(key) {
            return;
        }
        self.view.repository.reset_watcher();
        if matches!(self.view.overlay, Overlay::Repository { .. }) {
            self.view.overlay = Overlay::None;
            self.view
                .repository
                .coordinator
                .set_changes_monitoring(false);
        }
        self.view
            .repository
            .coordinator
            .request_refresh(RepositoryRefreshSet::all());
        self.dispatch_repository_refresh(cx);
    }

    pub(super) fn refresh_repository_on_activation(&mut self, cx: &mut Context<Self>) {
        self.sync_repository_context(cx);
        self.view.repository.coordinator.app_activated();
        self.dispatch_repository_refresh(cx);
    }

    pub(super) fn repository_environment_upgraded(&mut self, cx: &mut Context<Self>) {
        let revision = self.project_runtime.execution_environment.revision();
        if self
            .view
            .repository
            .coordinator
            .environment_upgraded(revision)
        {
            self.view.repository.reset_watcher();
            self.dispatch_repository_refresh(cx);
        }
    }

    pub(super) fn repository_service(&self) -> RepositoryService {
        RepositoryService::new(RepositoryOptions {
            git: self.project_runtime.git_options.clone(),
            environment: self.project_runtime.execution_environment.snapshot(),
        })
    }

    pub(super) fn dispatch_repository_refresh(&mut self, cx: &mut Context<Self>) {
        let refresh = self.view.repository.coordinator.take_refresh();
        if refresh.contains(RepositoryReadKind::Summary) {
            self.request_repository_summary(cx);
        }
        if refresh.contains(RepositoryReadKind::Branches) {
            self.request_repository_branches(cx);
        }
        if refresh.contains(RepositoryReadKind::Changes) {
            self.request_repository_changes(cx);
        }
        if refresh.contains(RepositoryReadKind::PullRequest) {
            self.request_current_pull_request(cx);
        }
        if refresh.contains(RepositoryReadKind::Providers) {
            self.resolve_repository_providers();
        }
    }

    fn request_repository_summary(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self
            .view
            .repository
            .coordinator
            .begin_read(RepositoryReadKind::Summary, None)
        else {
            return;
        };
        self.view
            .repository
            .coordinator
            .state_mut()
            .summary
            .begin_refresh();
        let path = self
            .view
            .repository
            .coordinator
            .key()
            .expect("repository read requires an active key")
            .normalized_path
            .clone();
        let service = self
            .repository_service()
            .with_cancellation(token.cancellation());
        cx.spawn(async move |window, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let summary = service.summary(&path)?;
                    let identity = service.repository_identity(&path)?;
                    Ok::<_, muxy_api::repository::RepositoryError>((summary, identity))
                })
                .await;
            let _ = window.update(cx, |window, cx| {
                if !window.view.repository.coordinator.finish_read(&token, None) {
                    return;
                }
                match result {
                    Ok((summary, identity)) => {
                        let previous = window.current_pull_request_identity();
                        window.view.repository.coordinator.state_mut().summary =
                            LoadState::Ready(summary);
                        let current = window.current_pull_request_identity();
                        if previous != current {
                            match &current {
                                Some(identity) => {
                                    window.view.repository.coordinator.invalidate_pull_request(
                                        identity.branch.clone(),
                                        identity.head_oid.clone(),
                                    )
                                }
                                None => {
                                    window.view.repository.coordinator.state_mut().pull_request =
                                        PullRequestLoadState::NoPullRequest;
                                }
                            }
                            if previous.is_some() {
                                window
                                    .view
                                    .repository
                                    .coordinator
                                    .request_refresh(RepositoryRefreshSet::branches());
                            }
                        }
                        window.install_repository_watcher(identity, cx);
                        window.request_current_pull_request(cx);
                        window.dispatch_repository_refresh(cx);
                        window.sync_pull_request_popover(cx);
                    }
                    Err(error) => {
                        window.view.repository.coordinator.state_mut().summary =
                            LoadState::Error(error.to_string());
                        window.view.repository.coordinator.state_mut().pull_request =
                            PullRequestLoadState::Idle;
                        window.view.repository.reset_watcher();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_repository_branches(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self
            .view
            .repository
            .coordinator
            .begin_read(RepositoryReadKind::Branches, None)
        else {
            return;
        };
        self.view.repository.coordinator.state_mut().branches = LoadState::Loading;
        let path = self
            .view
            .repository
            .coordinator
            .key()
            .expect("repository read requires an active key")
            .normalized_path
            .clone();
        let service = self
            .repository_service()
            .with_cancellation(token.cancellation());
        cx.spawn(async move |window, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { service.local_branches(&path) })
                .await;
            let _ = window.update(cx, |window, cx| {
                if !window.view.repository.coordinator.finish_read(&token, None) {
                    return;
                }
                window.view.repository.coordinator.state_mut().branches = match result {
                    Ok(branches) => LoadState::Ready(branches),
                    Err(error) => LoadState::Error(error.to_string()),
                };
                if let LoadState::Ready(branches) =
                    &window.view.repository.coordinator.state().branches
                    && let Overlay::Repository {
                        kind: crate::views::overlay::RepositoryPopoverKind::Branch(popover),
                        ..
                    } = &mut window.view.overlay
                    && window.view.repository.coordinator.key() == Some(&popover.key)
                {
                    popover.deletion.repository_changed(&popover.key);
                    popover.deletion.retain_branches(branches);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn request_repository_changes(&mut self, cx: &mut Context<Self>) {
        let Some(token) = self
            .view
            .repository
            .coordinator
            .begin_read(RepositoryReadKind::Changes, None)
        else {
            return;
        };
        self.view
            .repository
            .coordinator
            .state_mut()
            .changes
            .begin_refresh();
        let path = self
            .view
            .repository
            .coordinator
            .key()
            .expect("repository read requires an active key")
            .normalized_path
            .clone();
        let service = self
            .repository_service()
            .with_cancellation(token.cancellation());
        cx.spawn(async move |window, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { service.changed_files(&path) })
                .await;
            let _ = window.update(cx, |window, cx| {
                if !window.view.repository.coordinator.finish_read(&token, None) {
                    return;
                }
                window.view.repository.coordinator.state_mut().changes = match result {
                    Ok(changes) => LoadState::Ready(changes),
                    Err(error) => LoadState::Error(error.to_string()),
                };
                window.sync_changes_picker(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn request_current_pull_request(&mut self, cx: &mut Context<Self>) {
        let Some(identity) = self.current_pull_request_identity() else {
            if matches!(
                self.view.repository.coordinator.state().summary,
                LoadState::Ready(_)
            ) {
                self.view.repository.coordinator.state_mut().pull_request =
                    PullRequestLoadState::NoPullRequest;
            }
            return;
        };
        let Some(token) = self
            .view
            .repository
            .coordinator
            .begin_read(RepositoryReadKind::PullRequest, Some(identity.clone()))
        else {
            return;
        };
        self.view.repository.coordinator.state_mut().pull_request = PullRequestLoadState::Loading;
        let path = self
            .view
            .repository
            .coordinator
            .key()
            .expect("repository read requires an active key")
            .normalized_path
            .clone();
        let branch = identity.branch.clone();
        let head_oid = identity.head_oid.clone();
        let control = GitHubControl::with_cancellation(token.cancellation());
        let service = self.repository_service();
        cx.spawn(async move |window, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    service.pull_request(&path, branch.as_bytes(), head_oid.as_bytes(), &control)
                })
                .await;
            let _ = window.update(cx, |window, cx| {
                let current = window.current_pull_request_identity();
                if !window
                    .view
                    .repository
                    .coordinator
                    .finish_read(&token, current.as_ref())
                {
                    return;
                }
                window.view.repository.coordinator.state_mut().pull_request = match result {
                    Ok(PullRequestLookup::Found(pull_request)) => PullRequestLoadState::Found {
                        info: Box::new(pull_request.info.clone()),
                        resolved: Some(pull_request),
                    },
                    Ok(PullRequestLookup::NoPullRequest(_)) => PullRequestLoadState::NoPullRequest,
                    Err(error) => PullRequestLoadState::Unavailable(error.to_string()),
                };
                window.sync_pull_request_popover(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn current_pull_request_identity(&self) -> Option<PullRequestReadIdentity> {
        let LoadState::Ready(summary) = &self.view.repository.coordinator.state().summary else {
            return None;
        };
        if summary.is_detached || summary.branch.is_empty() {
            return None;
        }
        let RepositoryHead::Commit(head_oid) = &summary.head else {
            return None;
        };
        Some(PullRequestReadIdentity::new(
            summary.branch.clone(),
            head_oid.clone(),
        ))
    }

    fn resolve_repository_providers(&mut self) {
        let Some(token) = self
            .view
            .repository
            .coordinator
            .begin_read(RepositoryReadKind::Providers, None)
        else {
            return;
        };
        if self.view.repository.coordinator.finish_read(&token, None) {
            self.view.repository.coordinator.state_mut().providers = LoadState::Ready(());
        }
    }

    pub(crate) fn switch_repository_branch(&mut self, branch: Vec<u8>, cx: &mut Context<Self>) {
        self.start_branch_mutation(BranchMutationKind::Switch(branch), cx);
    }

    pub(crate) fn switch_repository_remote_branch(
        &mut self,
        branch: Vec<u8>,
        cx: &mut Context<Self>,
    ) {
        self.start_branch_mutation(BranchMutationKind::SwitchRemote(branch), cx);
    }

    pub(crate) fn create_repository_branch(&mut self, branch: String, cx: &mut Context<Self>) {
        let Some((current, busy)) = self.branch_presentation_context() else {
            return;
        };
        let presented = present_branches(
            &self.view.repository.coordinator.state().branches,
            current.as_deref(),
            "",
            &branch,
            busy,
        );
        if !presented.create_enabled {
            cx.notify();
            return;
        }
        self.start_branch_mutation(BranchMutationKind::Create(branch.trim().to_owned()), cx);
    }

    pub(crate) fn create_repository_stash(&mut self, cx: &mut Context<Self>) {
        self.start_stash_mutation(StashMutationKind::Create, cx);
    }

    pub(crate) fn apply_repository_stash(&mut self, entry: StashEntry, cx: &mut Context<Self>) {
        self.start_stash_mutation(StashMutationKind::Apply(entry), cx);
    }

    pub(crate) fn pop_repository_stash(&mut self, entry: StashEntry, cx: &mut Context<Self>) {
        self.start_stash_mutation(StashMutationKind::Pop(entry), cx);
    }

    pub(crate) fn drop_repository_stash(&mut self, entry: StashEntry, cx: &mut Context<Self>) {
        self.start_stash_mutation(StashMutationKind::Drop(entry), cx);
    }

    pub(crate) fn preview_repository_stash(&mut self, entry: StashEntry, cx: &mut Context<Self>) {
        let Some(key) = self.view.repository.coordinator.key().cloned() else {
            return;
        };
        let service = self.repository_service();
        let path = key.normalized_path.clone();
        let title = format!("{} — {}", entry.reference, entry.message);
        cx.spawn(async move |window, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { service.stash_preview(&path, &entry) })
                .await;
            let _ = window.update(cx, |window, cx| {
                let Overlay::Repository {
                    kind: crate::views::overlay::RepositoryPopoverKind::Branch(popover),
                    ..
                } = &window.view.overlay
                else {
                    return;
                };
                if popover.key != key {
                    return;
                }
                popover.picker.update(cx, |picker, cx| match result {
                    Ok(preview) => picker.show_detail(title, &preview.text, cx),
                    Err(error) => picker.set_status(
                        muxy_ui::command_popover::CommandPopoverStatus::Error(
                            error.to_string().into(),
                        ),
                        cx,
                    ),
                });
            });
        })
        .detach();
    }

    fn start_stash_mutation(&mut self, kind: StashMutationKind, cx: &mut Context<Self>) {
        let Some(key) = self.view.repository.coordinator.key().cloned() else {
            return;
        };
        let token = match self.state.project_operations.begin_operation(
            &key.project_id,
            crate::project_operations::ProjectOperationKind::RepositoryMutation,
        ) {
            Ok(token) => token,
            Err(_) => {
                self.set_branch_operation_error("Another project mutation is running".to_owned());
                return;
            }
        };
        let Some((cancellation, boundary)) = self
            .view
            .repository
            .coordinator
            .begin_mutation(token.request_id())
        else {
            let _ = self.state.project_operations.finish_operation(&token);
            return;
        };
        let service = self
            .repository_service()
            .with_cancellation(cancellation.clone());
        let path = key.normalized_path.clone();
        let control = MutationControl::with_cancellation_and_boundary(cancellation, boundary);
        cx.spawn(async move |window, cx| {
            let execution = cx
                .background_executor()
                .spawn(async move { execute_stash_mutation(service, path, kind, control) })
                .await;
            let _ = window.update(cx, |window, cx| {
                let _ = window.state.project_operations.finish_operation(&token);
                let Some(completion) = window
                    .view
                    .repository
                    .coordinator
                    .finish_mutation(token.request_id(), execution.effect)
                else {
                    return;
                };
                if !completion.current_identity
                    || window.view.repository.coordinator.key() != Some(&key)
                {
                    return;
                }
                match execution.result {
                    Ok(_) => {
                        window.set_branch_operation_error(String::new());
                        window.load_repository_picker_data(cx);
                    }
                    Err(error) => window.set_branch_operation_error(error),
                }
                window
                    .view
                    .repository
                    .coordinator
                    .request_refresh(RepositoryRefreshSet::all());
                window.dispatch_repository_refresh(cx);
                window.sync_repository_picker(cx);
            });
        })
        .detach();
        self.sync_repository_picker(cx);
    }

    pub(crate) fn request_branch_deletion(
        &mut self,
        key: crate::repository::RepositoryKey,
        branch: Vec<u8>,
        cx: &mut Context<Self>,
    ) {
        let current = self.current_repository_branch();
        let Overlay::Repository {
            kind: crate::views::overlay::RepositoryPopoverKind::Branch(popover),
            ..
        } = &mut self.view.overlay
        else {
            return;
        };
        if popover.key != key {
            return;
        }
        popover.operation_error = None;
        let _ = popover.deletion.request(key, branch, current.as_deref());
        cx.notify();
    }

    pub(crate) fn confirm_branch_deletion(&mut self, branch: Vec<u8>, cx: &mut Context<Self>) {
        let valid = matches!(
            &self.view.overlay,
            Overlay::Repository {
                kind: crate::views::overlay::RepositoryPopoverKind::Branch(popover),
                ..
            } if popover
                .deletion
                .pending()
                .is_some_and(|pending| pending.key == popover.key && pending.branch == branch)
        );
        if valid {
            self.start_branch_mutation(BranchMutationKind::Delete(branch), cx);
        }
    }

    fn branch_presentation_context(&self) -> Option<(Option<String>, bool)> {
        let key = self.view.repository.coordinator.key()?;
        Some((
            self.current_repository_branch(),
            self.state.project_operations.is_mutating(&key.project_id),
        ))
    }

    fn current_repository_branch(&self) -> Option<String> {
        match &self.view.repository.coordinator.state().summary {
            LoadState::Ready(summary) if !summary.is_detached && !summary.branch.is_empty() => {
                Some(summary.branch.clone())
            }
            _ => None,
        }
    }

    fn start_branch_mutation(&mut self, kind: BranchMutationKind, cx: &mut Context<Self>) {
        let Some(key) = self.view.repository.coordinator.key().cloned() else {
            return;
        };
        let expected_current_branch = self.current_repository_branch();
        let plan = branch_mutation_plan(key.clone(), expected_current_branch, kind);
        if !plan.background
            || !plan.revalidate_key
            || !plan.revalidate_current_branch
            || self.state.active_repository_key().as_ref() != Some(&plan.key)
        {
            return;
        }
        let token = match self
            .state
            .project_operations
            .begin_operation(&key.project_id, plan.operation_kind)
        {
            Ok(token) => token,
            Err(_) => {
                self.set_branch_operation_error("Another project mutation is running".to_owned());
                cx.notify();
                return;
            }
        };
        let Some((cancellation, boundary)) = self
            .view
            .repository
            .coordinator
            .begin_mutation(token.request_id())
        else {
            let _ = self.state.project_operations.finish_operation(&token);
            return;
        };
        self.set_branch_operation_error(String::new());
        let service = self
            .repository_service()
            .with_cancellation(cancellation.clone());
        let operation_plan = plan.clone();
        let completion_kind = plan.kind.clone();
        let control = MutationControl::with_cancellation_and_boundary(cancellation, boundary);
        cx.spawn(async move |window, cx| {
            let execution = cx
                .background_executor()
                .spawn(async move { execute_branch_mutation(service, operation_plan, control) })
                .await;
            let _ = window.update(cx, |window, cx| {
                let _ = window.state.project_operations.finish_operation(&token);
                let Some(completion) = window
                    .view
                    .repository
                    .coordinator
                    .finish_mutation(token.request_id(), execution.effect)
                else {
                    return;
                };
                let current = completion.current_identity
                    && window.state.active_repository_key().as_ref() == Some(&plan.key)
                    && window.view.repository.coordinator.key() == Some(&plan.key);
                if !current {
                    return;
                }
                window
                    .view
                    .repository
                    .coordinator
                    .request_refresh(plan.refresh);
                match execution.result {
                    Ok(_) => match &completion_kind {
                        BranchMutationKind::Delete(branch) => {
                            if let Overlay::Repository {
                                kind: crate::views::overlay::RepositoryPopoverKind::Branch(popover),
                                ..
                            } = &mut window.view.overlay
                                && popover.key == plan.key
                            {
                                popover.deletion.finish(&plan.key, branch, Ok(()));
                                popover.operation_error = None;
                                window.view.pending_focus = Some(popover.picker.focus_handle(cx));
                            }
                        }
                        BranchMutationKind::Switch(_)
                        | BranchMutationKind::SwitchRemote(_)
                        | BranchMutationKind::Create(_) => {
                            window.close_repository_overlay(cx);
                        }
                    },
                    Err(error) => {
                        if let BranchMutationKind::Delete(branch) = &completion_kind
                            && let Overlay::Repository {
                                kind: crate::views::overlay::RepositoryPopoverKind::Branch(popover),
                                ..
                            } = &mut window.view.overlay
                            && popover.key == plan.key
                        {
                            popover
                                .deletion
                                .finish(&plan.key, branch, Err(error.clone()));
                        }
                        window.set_branch_operation_error(error);
                    }
                }
                window.dispatch_repository_refresh(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn set_branch_operation_error(&mut self, error: String) {
        if let Overlay::Repository {
            kind: crate::views::overlay::RepositoryPopoverKind::Branch(popover),
            ..
        } = &mut self.view.overlay
        {
            popover.operation_error = (!error.is_empty()).then_some(error);
        }
    }

    pub(crate) fn start_changes_mutation(
        &mut self,
        kind: crate::views::repository::changes::ChangesMutationKind,
        cx: &mut Context<Self>,
    ) {
        let discard = matches!(
            kind,
            crate::views::repository::changes::ChangesMutationKind::Discard(_)
        );
        let Some(key) = self.view.repository.coordinator.key().cloned() else {
            if discard {
                self.set_changes_discard_in_flight(false);
            }
            return;
        };
        let plan = crate::views::repository::changes::changes_mutation_plan(key.clone(), kind);
        if !plan.background
            || !plan.revalidate_key
            || self.state.active_repository_key().as_ref() != Some(&plan.key)
        {
            if discard {
                self.set_changes_discard_in_flight(false);
            }
            return;
        }
        let token = match self
            .state
            .project_operations
            .begin_operation(&key.project_id, plan.operation_kind)
        {
            Ok(token) => token,
            Err(_) => {
                if discard {
                    self.set_changes_discard_in_flight(false);
                }
                self.set_changes_operation_error("Another project mutation is running".to_owned());
                self.sync_changes_picker(cx);
                return;
            }
        };
        let Some((cancellation, boundary)) = self
            .view
            .repository
            .coordinator
            .begin_mutation(token.request_id())
        else {
            let _ = self.state.project_operations.finish_operation(&token);
            if discard {
                self.set_changes_discard_in_flight(false);
            }
            return;
        };
        self.set_changes_operation_error(String::new());
        let service = self
            .repository_service()
            .with_cancellation(cancellation.clone());
        let operation_plan = plan.clone();
        let control = MutationControl::with_cancellation_and_boundary(cancellation, boundary);
        cx.spawn(async move |window, cx| {
            let execution = cx
                .background_executor()
                .spawn(async move { execute_changes_mutation(service, operation_plan, control) })
                .await;
            let _ = window.update(cx, |window, cx| {
                let _ = window.state.project_operations.finish_operation(&token);
                if discard
                    && matches!(
                        &window.view.overlay,
                        Overlay::Repository {
                            kind: crate::views::overlay::RepositoryPopoverKind::Changes(popover),
                            ..
                        } if popover.key == plan.key
                    )
                {
                    window.set_changes_discard_in_flight(false);
                }
                let Some(completion) = window
                    .view
                    .repository
                    .coordinator
                    .finish_mutation(token.request_id(), execution.effect)
                else {
                    return;
                };
                let current = completion.current_identity
                    && window.state.active_repository_key().as_ref() == Some(&plan.key)
                    && window.view.repository.coordinator.key() == Some(&plan.key);
                if !current {
                    return;
                }
                window
                    .view
                    .repository
                    .coordinator
                    .request_refresh(completion.refresh);
                window
                    .view
                    .repository
                    .coordinator
                    .request_refresh(plan.refresh);
                match execution.result {
                    Ok(_) => {
                        if let Overlay::Repository {
                            kind: crate::views::overlay::RepositoryPopoverKind::Changes(popover),
                            ..
                        } = &mut window.view.overlay
                            && popover.key == plan.key
                        {
                            popover.selection.clear();
                            popover.discard.cancel();
                            popover.operation_error = None;
                        }
                    }
                    Err(error) => window.set_changes_operation_error(error),
                }
                window.dispatch_repository_refresh(cx);
                window.sync_changes_picker(cx);
                cx.notify();
            });
        })
        .detach();
        self.sync_changes_picker(cx);
    }

    fn set_changes_operation_error(&mut self, error: String) {
        if let Overlay::Repository {
            kind: crate::views::overlay::RepositoryPopoverKind::Changes(popover),
            ..
        } = &mut self.view.overlay
        {
            popover.operation_error = (!error.is_empty()).then_some(error);
        }
    }

    fn set_changes_discard_in_flight(&mut self, in_flight: bool) {
        if let Overlay::Repository {
            kind: crate::views::overlay::RepositoryPopoverKind::Changes(popover),
            ..
        } = &mut self.view.overlay
        {
            popover.discard_in_flight = in_flight;
        }
    }

    pub(crate) fn start_pull_request_mutation(
        &mut self,
        kind: crate::views::repository::pull_request::PullRequestMutationKind,
        expected_identity: &crate::views::repository::pull_request::PullRequestConfirmationIdentity,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self.view.repository.coordinator.key().cloned() else {
            return;
        };
        let (resolved, current_identity, allowed) = match (
            &self.view.overlay,
            &self.view.repository.coordinator.state().pull_request,
            &self.view.repository.coordinator.state().summary,
        ) {
            (
                Overlay::Repository {
                    kind: crate::views::overlay::RepositoryPopoverKind::PullRequest(popover),
                    ..
                },
                PullRequestLoadState::Found { info, .. },
                LoadState::Ready(summary),
            ) if popover.key == key => {
                let current_identity = crate::views::repository::pull_request::PullRequestConfirmationIdentity::from_resolved(
                    key.clone(),
                    &popover.resolved,
                );
                let presentation =
                    crate::views::repository::pull_request::present_pull_request(info, summary);
                let allowed = match kind {
                    crate::views::repository::pull_request::PullRequestMutationKind::Update => {
                        presentation.update.is_some_and(|update| update.enabled)
                    }
                    crate::views::repository::pull_request::PullRequestMutationKind::Merge(_) => {
                        presentation.merge.enabled
                    }
                    crate::views::repository::pull_request::PullRequestMutationKind::Close => {
                        presentation.close.enabled
                    }
                };
                (popover.resolved.clone(), current_identity, allowed)
            }
            _ => return,
        };
        if let Err(error) =
            validate_pull_request_action(expected_identity, &current_identity, allowed)
        {
            self.set_pull_request_operation_error(error.to_owned());
            self.sync_pull_request_popover(cx);
            return;
        }
        let plan = crate::views::repository::pull_request::pull_request_mutation_plan(
            key.clone(),
            resolved,
            kind,
        );
        if !plan.background
            || !plan.revalidate_key
            || self.state.active_repository_key().as_ref() != Some(&plan.key)
        {
            return;
        }
        let token = match self
            .state
            .project_operations
            .begin_operation(&key.project_id, plan.operation_kind)
        {
            Ok(token) => token,
            Err(_) => {
                self.set_pull_request_operation_error(
                    "Another project mutation is running".to_owned(),
                );
                self.sync_pull_request_popover(cx);
                return;
            }
        };
        let Some((cancellation, boundary)) = self
            .view
            .repository
            .coordinator
            .begin_mutation(token.request_id())
        else {
            let _ = self.state.project_operations.finish_operation(&token);
            return;
        };
        self.set_pull_request_operation_error(String::new());
        self.set_pull_request_operation_message(None);
        self.sync_pull_request_popover(cx);
        let service = self.repository_service();
        let operation_plan = plan.clone();
        let control = GitHubControl::with_cancellation_and_boundary(cancellation, boundary);
        cx.spawn(async move |window, cx| {
            let execution = cx
                .background_executor()
                .spawn(
                    async move { execute_pull_request_mutation(service, operation_plan, control) },
                )
                .await;
            let _ = window.update(cx, |window, cx| {
                let _ = window.state.project_operations.finish_operation(&token);
                let Some(completion) = window
                    .view
                    .repository
                    .coordinator
                    .finish_mutation(token.request_id(), execution.effect)
                else {
                    return;
                };
                let current = completion.current_identity
                    && window.state.active_repository_key().as_ref() == Some(&plan.key)
                    && window.view.repository.coordinator.key() == Some(&plan.key);
                let Some((completion_refresh, plan_refresh)) =
                    pull_request_completion_refreshes(current, completion.refresh, plan.refresh)
                else {
                    return;
                };
                window
                    .view
                    .repository
                    .coordinator
                    .request_refresh(completion_refresh);
                window
                    .view
                    .repository
                    .coordinator
                    .request_refresh(plan_refresh);
                match execution.result {
                    Ok(message) => {
                        window.set_pull_request_operation_error(String::new());
                        window.set_pull_request_operation_message(message);
                    }
                    Err(error) => {
                        window.set_pull_request_operation_message(None);
                        window.set_pull_request_operation_error(error);
                    }
                }
                window.dispatch_repository_refresh(cx);
                window.sync_pull_request_popover(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn set_pull_request_operation_error(&mut self, error: String) {
        if let Overlay::Repository {
            kind: crate::views::overlay::RepositoryPopoverKind::PullRequest(popover),
            ..
        } = &mut self.view.overlay
        {
            popover.operation_error = (!error.is_empty()).then_some(error);
        }
    }

    fn set_pull_request_operation_message(&mut self, message: Option<String>) {
        if let Overlay::Repository {
            kind: crate::views::overlay::RepositoryPopoverKind::PullRequest(popover),
            ..
        } = &mut self.view.overlay
        {
            popover.operation_message = message;
        }
    }

    pub(crate) fn count_untracked_lines(
        &mut self,
        key: crate::repository::RepositoryKey,
        file: muxy_api::repository::ChangedFile,
        cx: &mut Context<Self>,
    ) {
        let service = self.repository_service();
        let path = key.normalized_path.clone();
        let id = file.stable_id();
        cx.spawn(async move |window, cx| {
            let count = cx
                .background_executor()
                .spawn(async move { service.untracked_line_count(&path, &file) })
                .await;
            let _ = window.update(cx, |window, cx| {
                let current = matches!(
                    &window.view.repository.coordinator.state().changes,
                    LoadState::Ready(changes) if changes.files.iter().any(|file| {
                        file.stable_id() == id && file.is_untracked
                    })
                );
                let Overlay::Repository {
                    kind: crate::views::overlay::RepositoryPopoverKind::Changes(popover),
                    ..
                } = &mut window.view.overlay
                else {
                    return;
                };
                if !current || popover.key != key {
                    return;
                }
                popover.line_counts.insert(id, count);
                window.sync_changes_picker(cx);
            });
        })
        .detach();
    }

    fn install_repository_watcher(&mut self, identity: RepositoryIdentity, cx: &mut Context<Self>) {
        self.view.repository.reset_watcher();
        let Ok((watcher, events)) =
            ActiveRepositoryWatcher::new(&identity.worktree_root, &identity.git_dir)
        else {
            return;
        };
        let task = cx.spawn(async move |window, cx| {
            while events.recv().await.is_ok() {
                if window
                    .update(cx, |window, _| {
                        window
                            .view
                            .repository
                            .coordinator
                            .watcher_invalidated(Instant::now());
                    })
                    .is_err()
                {
                    return;
                }
                loop {
                    cx.background_executor()
                        .timer(REPOSITORY_WATCHER_DEBOUNCE)
                        .await;
                    let mut received = false;
                    while events.try_recv().is_ok() {
                        received = true;
                    }
                    if !received {
                        break;
                    }
                    if window
                        .update(cx, |window, _| {
                            window
                                .view
                                .repository
                                .coordinator
                                .watcher_invalidated(Instant::now());
                        })
                        .is_err()
                    {
                        return;
                    }
                }
                if window
                    .update(cx, |window, cx| {
                        let refresh = window
                            .view
                            .repository
                            .coordinator
                            .take_debounced_refresh(Instant::now());
                        window.view.repository.coordinator.request_refresh(refresh);
                        window.dispatch_repository_refresh(cx);
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
        self.view.repository.watcher = Some(watcher);
        self.view.repository.watcher_task = Some(task);
    }
}

enum StashMutationKind {
    Create,
    Apply(StashEntry),
    Pop(StashEntry),
    Drop(StashEntry),
}

struct StashMutationExecution {
    result: Result<MutationOutcome, String>,
    effect: MutationEffect,
}

fn execute_stash_mutation(
    service: RepositoryService,
    path: std::path::PathBuf,
    kind: StashMutationKind,
    control: MutationControl,
) -> StashMutationExecution {
    let result = match kind {
        StashMutationKind::Create => service.create_stash(&path, None, &control),
        StashMutationKind::Apply(entry) => service
            .prepare_stash_action(&path, &entry, StashAction::Apply, &control)
            .and_then(|intent| service.apply_stash(&path, &intent, &control)),
        StashMutationKind::Pop(entry) => service
            .prepare_stash_action(&path, &entry, StashAction::Pop, &control)
            .and_then(|intent| service.pop_stash(&path, &intent, &control)),
        StashMutationKind::Drop(entry) => service
            .prepare_stash_action(&path, &entry, StashAction::Drop, &control)
            .and_then(|intent| service.drop_stash(&path, &intent, &control)),
    };
    match result {
        Ok(outcome) => StashMutationExecution {
            result: Ok(outcome),
            effect: match outcome {
                MutationOutcome::NoMutation => MutationEffect::NoMutation,
                MutationOutcome::Success => MutationEffect::Uncertain,
            },
        },
        Err(error) => StashMutationExecution {
            effect: error.effect(),
            result: Err(error.to_string()),
        },
    }
}

struct BranchMutationExecution {
    result: Result<MutationOutcome, String>,
    effect: MutationEffect,
}

struct ChangesMutationExecution {
    result: Result<MutationOutcome, String>,
    effect: MutationEffect,
}

struct PullRequestMutationExecution {
    result: Result<Option<String>, String>,
    effect: MutationEffect,
}

fn validate_pull_request_action(
    expected: &crate::views::repository::pull_request::PullRequestConfirmationIdentity,
    current: &crate::views::repository::pull_request::PullRequestConfirmationIdentity,
    allowed: bool,
) -> Result<(), &'static str> {
    if expected != current {
        Err("Pull request identity changed; review it before continuing")
    } else if !allowed {
        Err("Pull request action is no longer available")
    } else {
        Ok(())
    }
}

fn pull_request_completion_refreshes(
    current: bool,
    completion: RepositoryRefreshSet,
    planned: RepositoryRefreshSet,
) -> Option<(RepositoryRefreshSet, RepositoryRefreshSet)> {
    current.then_some((completion, planned))
}

fn execute_pull_request_mutation(
    service: RepositoryService,
    plan: crate::views::repository::pull_request::PullRequestMutationPlan,
    control: GitHubControl,
) -> PullRequestMutationExecution {
    use crate::views::repository::pull_request::PullRequestMutationKind;
    match plan.kind {
        PullRequestMutationKind::Update => pull_request_update_execution(
            service.update_pull_request(&plan.key.normalized_path, &plan.resolved, &control),
        ),
        PullRequestMutationKind::Merge(method) => pull_request_merge_execution(
            service.merge_pull_request(&plan.key.normalized_path, &plan.resolved, method, &control),
        ),
        PullRequestMutationKind::Close => pull_request_close_execution(service.close_pull_request(
            &plan.key.normalized_path,
            &plan.resolved,
            &control,
        )),
    }
}

fn pull_request_update_execution(
    result: Result<MutationOutcome, muxy_api::repository::GitHubError>,
) -> PullRequestMutationExecution {
    map_pull_request_execution(result.map(|outcome| {
        (
            None,
            if outcome == MutationOutcome::Success {
                MutationEffect::Uncertain
            } else {
                MutationEffect::NoMutation
            },
        )
    }))
}

fn pull_request_merge_execution(
    result: Result<
        muxy_api::repository::PullRequestMergeOutcome,
        muxy_api::repository::GitHubError,
    >,
) -> PullRequestMutationExecution {
    use muxy_api::repository::PullRequestMergeOutcome;
    map_pull_request_execution(result.map(|outcome| {
        (
            match outcome {
                PullRequestMergeOutcome::Success => None,
                PullRequestMergeOutcome::SuccessWithWarning(message) => Some(message),
            },
            MutationEffect::Uncertain,
        )
    }))
}

fn pull_request_close_execution(
    result: Result<(), muxy_api::repository::GitHubError>,
) -> PullRequestMutationExecution {
    map_pull_request_execution(result.map(|_| (None, MutationEffect::Uncertain)))
}

fn map_pull_request_execution(
    result: Result<(Option<String>, MutationEffect), muxy_api::repository::GitHubError>,
) -> PullRequestMutationExecution {
    use muxy_api::repository::GitHubMutationEffect;
    match result {
        Ok((message, effect)) => PullRequestMutationExecution {
            result: Ok(message),
            effect,
        },
        Err(error) => PullRequestMutationExecution {
            effect: match error.mutation_effect() {
                Some(GitHubMutationEffect::Uncertain) => MutationEffect::Uncertain,
                Some(GitHubMutationEffect::PartialSuccess) => MutationEffect::PartialSuccess {
                    completed: "pull request action",
                },
                Some(GitHubMutationEffect::NoMutation) | None => MutationEffect::NoMutation,
            },
            result: Err(error.to_string()),
        },
    }
}

fn execute_changes_mutation(
    service: RepositoryService,
    plan: crate::views::repository::changes::ChangesMutationPlan,
    control: MutationControl,
) -> ChangesMutationExecution {
    let identity = match service.repository_identity(&plan.key.normalized_path) {
        Ok(identity) if identity.worktree_root == plan.key.normalized_path => identity,
        Ok(_) => {
            return ChangesMutationExecution {
                result: Err("Repository identity changed".to_owned()),
                effect: MutationEffect::NoMutation,
            };
        }
        Err(error) => {
            return ChangesMutationExecution {
                result: Err(error.to_string()),
                effect: MutationEffect::NoMutation,
            };
        }
    };
    let mut changed = false;
    let result = match plan.kind {
        crate::views::repository::changes::ChangesMutationKind::Stage(files) => {
            execute_file_mutations(files, &mut changed, |file| {
                service.stage(&identity.worktree_root, file, &control)
            })
        }
        crate::views::repository::changes::ChangesMutationKind::StageAll => service
            .stage_all(&identity.worktree_root, &control)
            .inspect(|outcome| {
                changed = *outcome == MutationOutcome::Success;
            }),
        crate::views::repository::changes::ChangesMutationKind::Unstage(files) => {
            execute_file_mutations(files, &mut changed, |file| {
                service.unstage(&identity.worktree_root, file, &control)
            })
        }
        crate::views::repository::changes::ChangesMutationKind::UnstageAll => service
            .unstage_all(&identity.worktree_root, &control)
            .inspect(|outcome| {
                changed = *outcome == MutationOutcome::Success;
            }),
        crate::views::repository::changes::ChangesMutationKind::Discard(files) => {
            execute_file_mutations(files, &mut changed, |file| {
                service.discard(&identity.worktree_root, file, &control)
            })
        }
    };
    match result {
        Ok(outcome) => ChangesMutationExecution {
            result: Ok(outcome),
            effect: if changed {
                MutationEffect::Uncertain
            } else {
                MutationEffect::NoMutation
            },
        },
        Err(error) => ChangesMutationExecution {
            effect: if changed {
                MutationEffect::PartialSuccess {
                    completed: "earlier file actions",
                }
            } else {
                error.effect()
            },
            result: Err(if changed {
                format!("Some files changed before the operation failed: {error}")
            } else {
                error.to_string()
            }),
        },
    }
}

fn execute_file_mutations(
    files: Vec<muxy_api::repository::ChangedFile>,
    changed: &mut bool,
    mut execute: impl FnMut(
        &muxy_api::repository::ChangedFile,
    )
        -> Result<MutationOutcome, muxy_api::repository::RepositoryMutationError>,
) -> Result<MutationOutcome, muxy_api::repository::RepositoryMutationError> {
    let mut outcome = MutationOutcome::NoMutation;
    for file in &files {
        let current = execute(file)?;
        if current == MutationOutcome::Success {
            *changed = true;
            outcome = MutationOutcome::Success;
        }
    }
    Ok(outcome)
}

fn execute_branch_mutation(
    service: RepositoryService,
    plan: BranchMutationPlan,
    control: MutationControl,
) -> BranchMutationExecution {
    let identity = match service.repository_identity(&plan.key.normalized_path) {
        Ok(identity) if identity.worktree_root == plan.key.normalized_path => identity,
        Ok(_) => {
            return branch_execution_error(
                "Repository identity changed",
                MutationEffect::NoMutation,
            );
        }
        Err(error) => {
            return branch_execution_error(&error.to_string(), MutationEffect::NoMutation);
        }
    };
    let summary = match service.summary(&identity.worktree_root) {
        Ok(summary) => summary,
        Err(error) => {
            return branch_execution_error(&error.to_string(), MutationEffect::NoMutation);
        }
    };
    let current_branch =
        (!summary.is_detached && !summary.branch.is_empty()).then_some(summary.branch.as_str());
    if current_branch != plan.expected_current_branch.as_deref() {
        return branch_execution_error("Current branch changed", MutationEffect::NoMutation);
    }
    let result = match &plan.kind {
        BranchMutationKind::Switch(branch) => {
            service.switch_branch(&identity.worktree_root, branch, &control)
        }
        BranchMutationKind::SwitchRemote(branch) => {
            service.switch_remote_branch(&identity.worktree_root, branch, &control)
        }
        BranchMutationKind::Create(branch) => {
            service.create_branch(&identity.worktree_root, branch.as_bytes(), &control)
        }
        BranchMutationKind::Delete(branch) => service
            .prepare_branch_deletion(&identity.worktree_root, branch, &control)
            .and_then(|intent| service.delete_branch(&identity.worktree_root, &intent, &control)),
    };
    match result {
        Ok(outcome) => BranchMutationExecution {
            result: Ok(outcome),
            effect: match outcome {
                MutationOutcome::NoMutation => MutationEffect::NoMutation,
                MutationOutcome::Success => MutationEffect::Uncertain,
            },
        },
        Err(error) => BranchMutationExecution {
            effect: error.effect(),
            result: Err(error.to_string()),
        },
    }
}

fn branch_execution_error(message: &str, effect: MutationEffect) -> BranchMutationExecution {
    BranchMutationExecution {
        result: Err(message.to_owned()),
        effect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_operations::{BeginOperationError, ProjectOperationKind, ProjectOperations};
    use crate::repository::RepositoryKey;
    use crate::views::repository::pull_request::PullRequestConfirmationIdentity;
    use muxy_api::repository::{
        GitHubMutationEffect, GitHubRepositoryIdentity, PullRequestMergeOutcome,
    };

    #[test]
    fn phase_eight_wires_pull_request_actions_without_rendering_ai_controls() {
        let status_bar = include_str!("../status_bar.rs");
        let overlays = include_str!("overlays.rs");
        let changes = include_str!("../repository/changes.rs");
        let repository = include_str!("repository.rs");
        assert!(status_bar.contains("\"status-branch\""));
        assert!(status_bar.contains("\"status-changes\""));
        assert!(status_bar.contains("\"status-pull-request\""));
        assert!(!status_bar.contains(".id(\"status-commit-ai\")"));
        assert!(!status_bar.contains(".id(\"status-create-pr-ai\")"));
        assert!(!status_bar.contains(".id(\"status-repository-unavailable\")"));
        assert!(overlays.contains("request_worktree_removal_inspection"));
        assert!(!overlays.contains("remove_worktree(&"));
        assert!(overlays.contains("open_external_url"));
        assert!(changes.contains("action.disabled = mutation_busy"));
        assert!(repository.contains("with_cancellation_and_boundary"));
        assert!(repository.contains("update_pull_request"));
        assert!(repository.contains("merge_pull_request"));
        assert!(repository.contains("close_pull_request"));
    }

    fn confirmation_identity(repository_name: &str) -> PullRequestConfirmationIdentity {
        PullRequestConfirmationIdentity {
            key: RepositoryKey {
                project_id: "project".to_owned(),
                worktree_id: "worktree".to_owned(),
                normalized_path: std::path::PathBuf::from("/repo"),
            },
            repository: GitHubRepositoryIdentity {
                host: "github.com".to_owned(),
                owner: "muxy".to_owned(),
                name: repository_name.to_owned(),
            },
            number: 42,
            branch: "topic".to_owned(),
            head_oid: "a".repeat(40),
        }
    }

    #[test]
    fn pull_request_action_revalidation_rejects_repository_and_availability_changes() {
        let expected = confirmation_identity("app");
        assert!(validate_pull_request_action(&expected, &expected, true).is_ok());
        assert_eq!(
            validate_pull_request_action(&expected, &confirmation_identity("other"), true),
            Err("Pull request identity changed; review it before continuing")
        );
        assert_eq!(
            validate_pull_request_action(&expected, &expected, false),
            Err("Pull request action is no longer available")
        );
    }

    #[test]
    fn pull_request_actions_share_the_project_mutation_lane() {
        let mut operations = ProjectOperations::default();
        let active = operations
            .begin_operation("project", ProjectOperationKind::RepositoryMutation)
            .unwrap();
        assert!(matches!(
            operations.begin_operation("project", ProjectOperationKind::RepositoryMutation),
            Err(BeginOperationError::Busy(
                ProjectOperationKind::RepositoryMutation
            ))
        ));
        operations.finish_operation(&active).unwrap();
    }

    #[test]
    fn pull_request_completion_refreshes_only_the_current_identity() {
        let completion = RepositoryRefreshSet::summary_and_changes();
        let planned = RepositoryRefreshSet::pull_request();
        assert!(pull_request_completion_refreshes(false, completion, planned).is_none());
        let (actual_completion, actual_planned) =
            pull_request_completion_refreshes(true, completion, planned).unwrap();
        assert!(actual_completion.contains(RepositoryReadKind::Summary));
        assert!(actual_completion.contains(RepositoryReadKind::Changes));
        assert!(!actual_completion.contains(RepositoryReadKind::PullRequest));
        assert!(actual_planned.contains(RepositoryReadKind::PullRequest));
        assert!(!actual_planned.contains(RepositoryReadKind::Summary));
    }

    #[test]
    fn pull_request_execution_preserves_no_mutation_warnings_and_partial_success() {
        let no_update = pull_request_update_execution(Ok(MutationOutcome::NoMutation));
        assert!(matches!(no_update.effect, MutationEffect::NoMutation));
        assert_eq!(no_update.result, Ok(None));

        let warning = pull_request_merge_execution(Ok(
            PullRequestMergeOutcome::SuccessWithWarning("local follow-up failed".to_owned()),
        ));
        assert!(matches!(warning.effect, MutationEffect::Uncertain));
        assert_eq!(
            warning.result,
            Ok(Some("local follow-up failed".to_owned()))
        );

        let partial =
            pull_request_merge_execution(Err(muxy_api::repository::GitHubError::Command {
                operation: "merge pull request",
                effect: GitHubMutationEffect::PartialSuccess,
                message: "follow-up failed".to_owned(),
            }));
        assert!(matches!(
            partial.effect,
            MutationEffect::PartialSuccess {
                completed: "pull request action"
            }
        ));
        assert!(partial.result.is_err());
    }
}
