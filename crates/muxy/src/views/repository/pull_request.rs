use crate::project_operations::ProjectOperationKind;
use crate::repository::{RepositoryKey, RepositoryRefreshSet};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, actions, div,
};
use muxy_api::repository::{
    GitHubRepositoryIdentity, PullRequestChecks, PullRequestChecksStatus, PullRequestInfo,
    PullRequestMergeMethod, PullRequestMergeState, PullRequestMergeable, PullRequestState,
    RepositorySummary, ResolvedPullRequest,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::popover::PopoverSurface;
use muxy_ui::theme::{Metrics, Theme};
use std::time::{Duration, Instant};

pub(crate) const CONFIRMATION_DURATION: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestTone {
    Positive,
    Negative,
    Warning,
    Muted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestValue {
    pub(crate) text: String,
    pub(crate) tone: PullRequestTone,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestAvailability {
    pub(crate) enabled: bool,
    pub(crate) help: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestPresentation {
    pub(crate) title: String,
    pub(crate) state: String,
    pub(crate) base: String,
    pub(crate) mergeability: Option<PullRequestValue>,
    pub(crate) checks: Option<PullRequestValue>,
    pub(crate) local_changes: Option<PullRequestValue>,
    pub(crate) merge: PullRequestAvailability,
    pub(crate) update: Option<PullRequestAvailability>,
    pub(crate) close: PullRequestAvailability,
}

pub(crate) fn present_pull_request(
    info: &PullRequestInfo,
    summary: &RepositorySummary,
) -> PullRequestPresentation {
    PullRequestPresentation {
        title: format!("Pull Request #{}", info.number),
        state: state_label(info),
        base: info.base_branch.clone(),
        mergeability: mergeability(info),
        checks: checks_value(info.checks),
        local_changes: summary.is_dirty().then(|| PullRequestValue {
            text: "Uncommitted changes".to_owned(),
            tone: PullRequestTone::Warning,
        }),
        merge: merge_availability(info, tracked_dirty(summary)),
        update: update_availability(info, tracked_dirty(summary)),
        close: PullRequestAvailability {
            enabled: info.state == PullRequestState::Open,
            help: if info.state == PullRequestState::Open {
                "Close this pull request without merging it.".to_owned()
            } else {
                "Only open pull requests can be closed.".to_owned()
            },
        },
    }
}

fn tracked_dirty(summary: &RepositorySummary) -> bool {
    summary.staged_count > 0 || summary.unstaged_count > 0 || summary.conflicted_count > 0
}

fn state_label(info: &PullRequestInfo) -> String {
    match &info.state {
        PullRequestState::Open if info.is_draft => "Draft · Open".to_owned(),
        PullRequestState::Open => "Open".to_owned(),
        PullRequestState::Closed => "Closed".to_owned(),
        PullRequestState::Merged => "Merged".to_owned(),
        PullRequestState::Unknown(value) => value.clone(),
    }
}

fn checks_value(checks: PullRequestChecks) -> Option<PullRequestValue> {
    match checks.status {
        PullRequestChecksStatus::None => None,
        PullRequestChecksStatus::Success => Some(PullRequestValue {
            text: format!("{}/{} passing", checks.passing, checks.total),
            tone: PullRequestTone::Positive,
        }),
        PullRequestChecksStatus::Pending => Some(PullRequestValue {
            text: format!("{} running", checks.pending),
            tone: PullRequestTone::Warning,
        }),
        PullRequestChecksStatus::Failure => Some(PullRequestValue {
            text: format!("{} failing", checks.failing),
            tone: PullRequestTone::Negative,
        }),
    }
}

fn mergeability(info: &PullRequestInfo) -> Option<PullRequestValue> {
    if info.is_draft {
        return Some(PullRequestValue {
            text: "Draft".to_owned(),
            tone: PullRequestTone::Muted,
        });
    }
    let value = match &info.merge_state {
        PullRequestMergeState::Dirty => ("Conflicts", PullRequestTone::Negative),
        PullRequestMergeState::Behind => ("Behind base", PullRequestTone::Negative),
        PullRequestMergeState::Blocked => ("Blocked", PullRequestTone::Negative),
        PullRequestMergeState::Draft => ("Draft", PullRequestTone::Muted),
        PullRequestMergeState::Clean | PullRequestMergeState::HasHooks => {
            ("Ready", PullRequestTone::Positive)
        }
        PullRequestMergeState::Unstable => match info.checks.status {
            PullRequestChecksStatus::Failure => ("Checks failing", PullRequestTone::Warning),
            PullRequestChecksStatus::Pending => ("Checks running", PullRequestTone::Warning),
            PullRequestChecksStatus::None | PullRequestChecksStatus::Success => {
                ("Ready", PullRequestTone::Positive)
            }
        },
        PullRequestMergeState::Unknown(_) => match info.mergeable {
            PullRequestMergeable::Mergeable => ("Ready", PullRequestTone::Positive),
            PullRequestMergeable::Conflicting => ("Conflicts", PullRequestTone::Negative),
            PullRequestMergeable::Unknown(_) => return None,
        },
    };
    Some(PullRequestValue {
        text: value.0.to_owned(),
        tone: value.1,
    })
}

fn merge_availability(info: &PullRequestInfo, tracked_dirty: bool) -> PullRequestAvailability {
    let disabled = if info.state != PullRequestState::Open {
        Some("Only open pull requests can be merged.")
    } else if info.is_draft {
        Some("Mark this pull request ready for review before merging.")
    } else if info.mergeable == PullRequestMergeable::Conflicting {
        Some("This PR has conflicts and cannot be merged.")
    } else if tracked_dirty {
        Some("Commit or stash tracked local changes before merging.")
    } else {
        match info.merge_state {
            PullRequestMergeState::Dirty => Some("This PR has conflicts and cannot be merged."),
            PullRequestMergeState::Behind => {
                Some("This branch is out of date with the base branch. Update it before merging.")
            }
            PullRequestMergeState::Blocked => {
                Some("Merging is blocked by branch protection, required reviews, or checks.")
            }
            PullRequestMergeState::Draft => {
                Some("Mark this pull request ready for review before merging.")
            }
            PullRequestMergeState::Clean
            | PullRequestMergeState::HasHooks
            | PullRequestMergeState::Unstable
            | PullRequestMergeState::Unknown(_) => None,
        }
    };
    match disabled {
        Some(help) => PullRequestAvailability {
            enabled: false,
            help: help.to_owned(),
        },
        None => PullRequestAvailability {
            enabled: true,
            help: merge_confirmation_help(info),
        },
    }
}

fn merge_confirmation_help(info: &PullRequestInfo) -> String {
    match info.checks.status {
        PullRequestChecksStatus::Failure => {
            "Checks are failing. Click to start the five-second merge confirmation.".to_owned()
        }
        PullRequestChecksStatus::Pending => {
            "Checks are still running. Click to start the five-second merge confirmation."
                .to_owned()
        }
        PullRequestChecksStatus::None | PullRequestChecksStatus::Success => format!(
            "Start the five-second confirmation to merge PR #{}.",
            info.number
        ),
    }
}

fn update_availability(
    info: &PullRequestInfo,
    tracked_dirty: bool,
) -> Option<PullRequestAvailability> {
    if info.merge_state != PullRequestMergeState::Behind || info.is_cross_repository {
        return None;
    }
    Some(PullRequestAvailability {
        enabled: info.state == PullRequestState::Open && !tracked_dirty,
        help: if tracked_dirty {
            "Commit or stash tracked local changes before updating the branch.".to_owned()
        } else if info.state != PullRequestState::Open {
            "Only open pull requests can be updated.".to_owned()
        } else {
            format!("Merge {} into this branch and push it.", info.base_branch)
        },
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestConfirmationIdentity {
    pub(crate) key: RepositoryKey,
    pub(crate) repository: GitHubRepositoryIdentity,
    pub(crate) number: u64,
    pub(crate) branch: String,
    pub(crate) head_oid: String,
}

impl PullRequestConfirmationIdentity {
    pub(crate) fn new(
        key: RepositoryKey,
        repository: GitHubRepositoryIdentity,
        info: &PullRequestInfo,
    ) -> Self {
        Self {
            key,
            repository,
            number: info.number,
            branch: info.head_branch.clone(),
            head_oid: info.head_oid.clone(),
        }
    }

    pub(crate) fn from_resolved(key: RepositoryKey, resolved: &ResolvedPullRequest) -> Self {
        Self::new(key, resolved.repository_identity().clone(), &resolved.info)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestConfirmedAction {
    Merge(PullRequestMergeMethod),
    Close,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingPullRequestConfirmation {
    pub(crate) generation: u64,
    pub(crate) action: PullRequestConfirmedAction,
    pub(crate) identity: PullRequestConfirmationIdentity,
    pub(crate) deadline: Instant,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PullRequestConfirmationState {
    generation: u64,
    pending: Option<PendingPullRequestConfirmation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestConfirmationActivation {
    Armed(u64),
    Execute {
        action: PullRequestConfirmedAction,
        identity: PullRequestConfirmationIdentity,
    },
}

impl PullRequestConfirmationState {
    pub(crate) fn activate(
        &mut self,
        action: PullRequestConfirmedAction,
        identity: PullRequestConfirmationIdentity,
        now: Instant,
    ) -> PullRequestConfirmationActivation {
        if self.pending.as_ref().is_some_and(|pending| {
            pending.action == action && pending.identity == identity && now < pending.deadline
        }) {
            let pending = self
                .pending
                .take()
                .expect("confirmed action requires pending state");
            return PullRequestConfirmationActivation::Execute {
                action: pending.action,
                identity: pending.identity,
            };
        }
        self.generation = self.generation.wrapping_add(1).max(1);
        self.pending = Some(PendingPullRequestConfirmation {
            generation: self.generation,
            action,
            identity,
            deadline: now + CONFIRMATION_DURATION,
        });
        PullRequestConfirmationActivation::Armed(self.generation)
    }

    pub(crate) fn expire(
        &mut self,
        generation: u64,
        identity: &PullRequestConfirmationIdentity,
        now: Instant,
    ) -> Option<(PullRequestConfirmedAction, PullRequestConfirmationIdentity)> {
        let pending = self.pending.as_ref()?;
        if pending.generation != generation
            || &pending.identity != identity
            || now < pending.deadline
        {
            return None;
        }
        self.pending
            .take()
            .map(|pending| (pending.action, pending.identity))
    }

    pub(crate) fn cancel(&mut self) -> bool {
        self.pending.take().is_some()
    }

    pub(crate) fn retain_identity(&mut self, identity: &PullRequestConfirmationIdentity) {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| &pending.identity != identity)
        {
            self.pending = None;
        }
    }

    pub(crate) fn pending(&self) -> Option<&PendingPullRequestConfirmation> {
        self.pending.as_ref()
    }

    pub(crate) fn remaining_seconds(&self, now: Instant) -> Option<u64> {
        self.pending.as_ref().map(|pending| {
            let remaining = pending.deadline.saturating_duration_since(now);
            remaining.as_millis().div_ceil(1_000).max(1) as u64
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PullRequestMutationKind {
    Update,
    Merge(PullRequestMergeMethod),
    Close,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PullRequestMutationPlan {
    pub(crate) key: RepositoryKey,
    pub(crate) resolved: Box<ResolvedPullRequest>,
    pub(crate) kind: PullRequestMutationKind,
    pub(crate) operation_kind: ProjectOperationKind,
    pub(crate) refresh: RepositoryRefreshSet,
    pub(crate) background: bool,
    pub(crate) revalidate_key: bool,
}

pub(crate) fn pull_request_mutation_plan(
    key: RepositoryKey,
    resolved: Box<ResolvedPullRequest>,
    kind: PullRequestMutationKind,
) -> PullRequestMutationPlan {
    let refresh = pull_request_mutation_refresh(&kind);
    PullRequestMutationPlan {
        key,
        resolved,
        kind,
        operation_kind: ProjectOperationKind::RepositoryMutation,
        refresh,
        background: true,
        revalidate_key: true,
    }
}

fn pull_request_mutation_refresh(kind: &PullRequestMutationKind) -> RepositoryRefreshSet {
    match kind {
        PullRequestMutationKind::Update | PullRequestMutationKind::Merge(_) => {
            RepositoryRefreshSet::repository_truth()
        }
        PullRequestMutationKind::Close => RepositoryRefreshSet::pull_request(),
    }
}

actions!(pull_request_popover, [Dismiss]);

const KEY_CONTEXT: &str = "PullRequestPopover";

pub(crate) fn key_bindings() -> Vec<gpui::KeyBinding> {
    vec![gpui::KeyBinding::new("escape", Dismiss, Some(KEY_CONTEXT))]
}

#[derive(Clone, Debug)]
pub(crate) enum PullRequestPopoverEvent {
    Refresh,
    Open,
    Update(PullRequestConfirmationIdentity),
    Execute {
        action: PullRequestConfirmedAction,
        identity: PullRequestConfirmationIdentity,
    },
    Dismiss,
}

pub(crate) struct PullRequestOperationState {
    pub(crate) busy: bool,
    pub(crate) error: Option<String>,
    pub(crate) message: Option<String>,
}

pub(crate) struct PullRequestPanel {
    presentation: PullRequestPresentation,
    identity: PullRequestConfirmationIdentity,
    merge_method: PullRequestMergeMethod,
    confirmation: PullRequestConfirmationState,
    busy: bool,
    operation_error: Option<String>,
    operation_message: Option<String>,
    focus: FocusHandle,
    theme: Theme,
    metrics: Metrics,
}

impl EventEmitter<PullRequestPopoverEvent> for PullRequestPanel {}

impl Focusable for PullRequestPanel {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PullRequestPanel {
    pub(crate) fn new(
        key: RepositoryKey,
        repository: GitHubRepositoryIdentity,
        info: &PullRequestInfo,
        summary: &RepositorySummary,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            presentation: present_pull_request(info, summary),
            identity: PullRequestConfirmationIdentity::new(key, repository, info),
            merge_method: PullRequestMergeMethod::Squash,
            confirmation: PullRequestConfirmationState::default(),
            busy: false,
            operation_error: None,
            operation_message: None,
            focus: cx.focus_handle(),
            theme,
            metrics,
        }
    }

    pub(crate) fn presentation(&self) -> PullRequestPresentation {
        self.presentation.clone()
    }

    pub(crate) fn sync(
        &mut self,
        key: RepositoryKey,
        repository: GitHubRepositoryIdentity,
        info: &PullRequestInfo,
        summary: &RepositorySummary,
        operation: PullRequestOperationState,
        cx: &mut Context<Self>,
    ) {
        let identity = PullRequestConfirmationIdentity::new(key, repository, info);
        self.confirmation.retain_identity(&identity);
        self.identity = identity;
        self.presentation = present_pull_request(info, summary);
        self.busy = operation.busy;
        self.operation_error = operation.error;
        self.operation_message = operation.message;
        if operation.busy {
            self.confirmation.cancel();
        }
        cx.notify();
    }

    pub(crate) fn set_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        if self.busy != busy {
            self.busy = busy;
            if busy {
                self.confirmation.cancel();
            }
            cx.notify();
        }
    }

    fn set_merge_method(&mut self, method: PullRequestMergeMethod, cx: &mut Context<Self>) {
        if self.merge_method != method {
            self.merge_method = method;
            self.confirmation.cancel();
            cx.notify();
        }
    }

    fn request_confirmed_action(
        &mut self,
        action: PullRequestConfirmedAction,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        match self
            .confirmation
            .activate(action, self.identity.clone(), Instant::now())
        {
            PullRequestConfirmationActivation::Execute { action, identity } => {
                cx.emit(PullRequestPopoverEvent::Execute { action, identity });
            }
            PullRequestConfirmationActivation::Armed(generation) => {
                let identity = self.identity.clone();
                cx.spawn(async move |panel, cx| {
                    loop {
                        cx.background_executor().timer(Duration::from_secs(1)).await;
                        let complete = panel
                            .update(cx, |panel, cx| {
                                if let Some((action, identity)) =
                                    panel
                                        .confirmation
                                        .expire(generation, &identity, Instant::now())
                                {
                                    cx.emit(PullRequestPopoverEvent::Execute { action, identity });
                                    cx.notify();
                                    true
                                } else if panel
                                    .confirmation
                                    .pending()
                                    .is_some_and(|pending| pending.generation == generation)
                                {
                                    cx.notify();
                                    false
                                } else {
                                    true
                                }
                            })
                            .unwrap_or(true);
                        if complete {
                            break;
                        }
                    }
                })
                .detach();
                cx.notify();
            }
        }
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.confirmation.cancel() {
            cx.notify();
        } else if !self.busy {
            cx.emit(PullRequestPopoverEvent::Dismiss);
        }
    }

    fn tone_color(&self, tone: PullRequestTone) -> gpui::Hsla {
        match tone {
            PullRequestTone::Positive => self.theme.accent,
            PullRequestTone::Negative => self.theme.danger,
            PullRequestTone::Warning => self.theme.warning,
            PullRequestTone::Muted => self.theme.fg_muted,
        }
    }

    fn info_row(
        &self,
        label: &'static str,
        value: impl Into<SharedString>,
        tone: PullRequestTone,
    ) -> AnyElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(self.metrics.spacing3())
            .h(self.metrics.control_small())
            .child(
                div()
                    .w(self.metrics.scaled(50.0))
                    .flex_none()
                    .text_size(self.metrics.font_footnote())
                    .text_color(self.theme.fg_muted)
                    .child(label),
            )
            .child(
                div()
                    .min_w(gpui::px(0.0))
                    .flex_grow()
                    .truncate()
                    .text_size(self.metrics.font_footnote())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(self.tone_color(tone))
                    .child(value.into()),
            )
            .into_any_element()
    }

    fn action_button(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        enabled: bool,
        destructive: bool,
        on_click: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let color = if !enabled {
            self.theme.fg_dim
        } else if destructive {
            self.theme.danger
        } else {
            self.theme.fg
        };
        div()
            .id(id)
            .flex()
            .items_center()
            .justify_center()
            .h(self.metrics.control_medium())
            .px(self.metrics.spacing4())
            .rounded(self.metrics.radius_sm())
            .border_1()
            .border_color(self.theme.border)
            .bg(self.theme.surface)
            .text_size(self.metrics.font_footnote())
            .font_weight(FontWeight::MEDIUM)
            .text_color(color)
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|style| style.bg(self.theme.hover))
                    .on_click(cx.listener(move |panel, _, _, cx| on_click(panel, cx)))
            })
            .child(label.into())
            .into_any_element()
    }

    fn merge_method_selector(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut selector = div()
            .flex()
            .flex_row()
            .gap(gpui::px(1.0))
            .p(gpui::px(1.0))
            .rounded(self.metrics.radius_sm())
            .border_1()
            .border_color(self.theme.border)
            .bg(self.theme.surface);
        for (method, label) in [
            (PullRequestMergeMethod::Squash, "Squash"),
            (PullRequestMergeMethod::Merge, "Merge"),
            (PullRequestMergeMethod::Rebase, "Rebase"),
        ] {
            let selected = self.merge_method == method;
            let enabled = !self.busy && self.confirmation.pending().is_none();
            selector = selector.child(
                div()
                    .id(SharedString::from(format!("pr-method-{label}")))
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .h(self.metrics.control_small())
                    .rounded(self.metrics.radius_sm())
                    .text_size(self.metrics.font_footnote())
                    .text_color(if selected {
                        self.theme.accent_foreground
                    } else if enabled {
                        self.theme.fg
                    } else {
                        self.theme.fg_dim
                    })
                    .when(selected, |item| item.bg(self.theme.accent))
                    .when(enabled && !selected, |item| {
                        item.cursor_pointer()
                            .hover(|style| style.bg(self.theme.hover))
                            .on_click(cx.listener(move |panel, _, _, cx| {
                                panel.set_merge_method(method, cx);
                            }))
                    })
                    .child(label),
            );
        }
        selector.into_any_element()
    }

    fn confirmation_progress(&self, now: Instant) -> f32 {
        let Some(pending) = self.confirmation.pending() else {
            return 0.0;
        };
        let remaining = pending.deadline.saturating_duration_since(now);
        1.0 - remaining.as_secs_f32() / CONFIRMATION_DURATION.as_secs_f32()
    }
}

impl Render for PullRequestPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let pending = self.confirmation.pending().map(|pending| pending.action);
        let remaining = self.confirmation.remaining_seconds(now).unwrap_or(5);
        let progress = self.confirmation_progress(now).clamp(0.0, 1.0);
        let open_enabled = !self.busy && pending.is_none();
        let refresh_enabled = !self.busy && pending.is_none();
        let update_enabled = self
            .presentation
            .update
            .as_ref()
            .is_some_and(|update| update.enabled)
            && !self.busy
            && pending.is_none();
        let merge_enabled = self.presentation.merge.enabled && !self.busy;
        let close_enabled = self.presentation.close.enabled && !self.busy;
        let merge_label = match pending {
            Some(PullRequestConfirmedAction::Merge(method)) if method == self.merge_method => {
                format!("{} in {remaining}s · click again", method_label(method))
            }
            _ if self.busy => "Working…".to_owned(),
            _ => method_label(self.merge_method).to_owned(),
        };
        let close_label = match pending {
            Some(PullRequestConfirmedAction::Close) => {
                format!("Close in {remaining}s · click again")
            }
            _ if self.busy => "Working…".to_owned(),
            _ => "Close PR".to_owned(),
        };

        let mut details = div().flex().flex_col().gap(self.metrics.spacing1());
        details = details.child(self.info_row(
            "Base",
            self.presentation.base.clone(),
            PullRequestTone::Muted,
        ));
        if let Some(value) = &self.presentation.mergeability {
            details = details.child(self.info_row("Merge", value.text.clone(), value.tone));
        }
        if let Some(value) = &self.presentation.checks {
            details = details.child(self.info_row("Checks", value.text.clone(), value.tone));
        }
        if let Some(value) = &self.presentation.local_changes {
            details = details.child(self.info_row("Local", value.text.clone(), value.tone));
        }

        let mut actions = div().flex().flex_col().gap(self.metrics.spacing2());
        actions = actions.child(self.action_button(
            "pr-open",
            "Open on GitHub",
            open_enabled,
            false,
            |_, cx| cx.emit(PullRequestPopoverEvent::Open),
            cx,
        ));
        if self.presentation.update.is_some() {
            let base = self.presentation.base.clone();
            actions = actions.child(self.action_button(
                "pr-update",
                format!("Update from {base}"),
                update_enabled,
                false,
                |panel, cx| cx.emit(PullRequestPopoverEvent::Update(panel.identity.clone())),
                cx,
            ));
        }
        if self.presentation.close.enabled {
            actions = actions.child(self.merge_method_selector(cx));
            let method = self.merge_method;
            let merge_progress = matches!(
                pending,
                Some(PullRequestConfirmedAction::Merge(pending_method)) if pending_method == method
            );
            actions = actions.child(
                div()
                    .relative()
                    .child(
                        div()
                            .absolute()
                            .left(gpui::px(0.0))
                            .top(gpui::px(0.0))
                            .bottom(gpui::px(0.0))
                            .w(gpui::relative(if merge_progress { progress } else { 0.0 }))
                            .rounded(self.metrics.radius_sm())
                            .bg(self.theme.accent_soft),
                    )
                    .child(self.action_button(
                        "pr-merge",
                        merge_label,
                        merge_enabled,
                        false,
                        move |panel, cx| {
                            panel.request_confirmed_action(
                                PullRequestConfirmedAction::Merge(method),
                                cx,
                            );
                        },
                        cx,
                    )),
            );
            let close_progress = matches!(pending, Some(PullRequestConfirmedAction::Close));
            actions = actions.child(
                div()
                    .relative()
                    .child(
                        div()
                            .absolute()
                            .left(gpui::px(0.0))
                            .top(gpui::px(0.0))
                            .bottom(gpui::px(0.0))
                            .w(gpui::relative(if close_progress { progress } else { 0.0 }))
                            .rounded(self.metrics.radius_sm())
                            .bg(self.theme.accent_soft),
                    )
                    .child(self.action_button(
                        "pr-close",
                        close_label,
                        close_enabled,
                        true,
                        |panel, cx| {
                            panel.request_confirmed_action(PullRequestConfirmedAction::Close, cx);
                        },
                        cx,
                    )),
            );
        }
        if pending.is_some() {
            actions = actions.child(self.action_button(
                "pr-cancel-confirmation",
                "Cancel",
                true,
                false,
                |panel, cx| {
                    panel.confirmation.cancel();
                    cx.notify();
                },
                cx,
            ));
        }

        let help = if let Some(error) = &self.operation_error {
            (error.clone(), self.theme.danger)
        } else if let Some(message) = &self.operation_message {
            (message.clone(), self.theme.warning)
        } else if let Some(update) = &self.presentation.update
            && !update.enabled
        {
            (update.help.clone(), self.theme.fg_muted)
        } else if !self.presentation.merge.enabled {
            (self.presentation.merge.help.clone(), self.theme.fg_muted)
        } else {
            (self.presentation.merge.help.clone(), self.theme.fg_dim)
        };

        let content = div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::dismiss))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .flex()
            .flex_col()
            .p(self.metrics.spacing5())
            .gap(self.metrics.spacing4())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(self.metrics.spacing3())
                    .child(IconGlyph::new(
                        Icon::GitBranch,
                        self.metrics.icon_md(),
                        self.theme.accent,
                    ))
                    .child(
                        div()
                            .min_w(gpui::px(0.0))
                            .flex_grow()
                            .flex()
                            .flex_col()
                            .gap(self.metrics.spacing1())
                            .child(
                                div()
                                    .truncate()
                                    .text_size(self.metrics.font_body())
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(self.theme.fg)
                                    .child(self.presentation.title.clone()),
                            )
                            .child(
                                div()
                                    .text_size(self.metrics.font_caption())
                                    .text_color(self.theme.fg_muted)
                                    .child(self.presentation.state.clone()),
                            ),
                    )
                    .child(self.action_button(
                        "pr-refresh",
                        "Refresh",
                        refresh_enabled,
                        false,
                        |_, cx| cx.emit(PullRequestPopoverEvent::Refresh),
                        cx,
                    )),
            )
            .child(details)
            .child(div().h(gpui::px(1.0)).bg(self.theme.border))
            .child(actions)
            .child(
                div()
                    .min_h(self.metrics.control_small())
                    .text_size(self.metrics.font_caption())
                    .text_color(help.1)
                    .child(help.0),
            );
        PopoverSurface::new(
            self.theme.clone(),
            self.metrics,
            pull_request_overlay_policy(&self.presentation).target_width,
            pull_request_overlay_policy(&self.presentation).target_height,
            content,
        )
    }
}

fn method_label(method: PullRequestMergeMethod) -> &'static str {
    match method {
        PullRequestMergeMethod::Squash => "Squash and merge",
        PullRequestMergeMethod::Merge => "Merge pull request",
        PullRequestMergeMethod::Rebase => "Rebase and merge",
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PullRequestOverlayPolicy {
    pub(crate) target_width: f32,
    pub(crate) target_height: f32,
}

pub(crate) fn pull_request_overlay_policy(
    presentation: &PullRequestPresentation,
) -> PullRequestOverlayPolicy {
    PullRequestOverlayPolicy {
        target_width: 300.0,
        target_height: if presentation.close.enabled {
            352.0
        } else {
            236.0
        },
    }
}

pub(crate) struct PullRequestPopover {
    pub(crate) key: RepositoryKey,
    pub(crate) resolved: Box<ResolvedPullRequest>,
    pub(crate) panel: Entity<PullRequestPanel>,
    pub(crate) operation_error: Option<String>,
    pub(crate) operation_message: Option<String>,
}

pub(crate) fn render(
    popover: &PullRequestPopover,
    bounds: gpui::Bounds<gpui::Pixels>,
) -> AnyElement {
    div()
        .absolute()
        .left(bounds.origin.x)
        .top(bounds.origin.y)
        .child(popover.panel.clone())
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::RepositoryReadKind;
    use muxy_api::repository::{PullRequestChecks, RepositoryHead, ValidatedExternalUrl};
    use std::path::PathBuf;

    fn info() -> PullRequestInfo {
        PullRequestInfo {
            url: ValidatedExternalUrl::try_from("https://github.com/muxy/app/pull/42".to_owned())
                .unwrap(),
            number: 42,
            state: PullRequestState::Open,
            is_draft: false,
            base_branch: "main".to_owned(),
            mergeable: PullRequestMergeable::Mergeable,
            merge_state: PullRequestMergeState::Clean,
            checks: PullRequestChecks {
                status: PullRequestChecksStatus::Success,
                passing: 3,
                failing: 0,
                pending: 0,
                total: 3,
            },
            is_cross_repository: false,
            head_oid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            head_branch: "topic".to_owned(),
        }
    }

    fn summary() -> RepositorySummary {
        RepositorySummary {
            branch: "topic".to_owned(),
            head: RepositoryHead::Commit("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned()),
            is_detached: false,
            upstream: Some("origin/topic".to_owned()),
            ahead: 0,
            behind: 0,
            changed_count: 0,
            staged_count: 0,
            unstaged_count: 0,
            untracked_count: 0,
            conflicted_count: 0,
        }
    }

    fn key() -> RepositoryKey {
        RepositoryKey {
            project_id: "project".to_owned(),
            worktree_id: "worktree".to_owned(),
            normalized_path: PathBuf::from("/repo"),
        }
    }

    fn repository_identity() -> GitHubRepositoryIdentity {
        GitHubRepositoryIdentity {
            host: "github.com".to_owned(),
            owner: "muxy".to_owned(),
            name: "app".to_owned(),
        }
    }

    #[test]
    fn pr_presentation_covers_states_checks_and_local_warning() {
        let mut info = info();
        let mut summary = summary();
        let open = present_pull_request(&info, &summary);
        assert_eq!(open.title, "Pull Request #42");
        assert_eq!(open.state, "Open");
        assert_eq!(open.checks.unwrap().text, "3/3 passing");
        assert!(open.merge.enabled);

        info.is_draft = true;
        assert_eq!(present_pull_request(&info, &summary).state, "Draft · Open");
        info.is_draft = false;
        info.state = PullRequestState::Closed;
        assert_eq!(present_pull_request(&info, &summary).state, "Closed");
        info.state = PullRequestState::Merged;
        assert_eq!(present_pull_request(&info, &summary).state, "Merged");
        info.state = PullRequestState::Open;
        info.checks.status = PullRequestChecksStatus::None;
        assert!(present_pull_request(&info, &summary).checks.is_none());
        info.checks.status = PullRequestChecksStatus::Pending;
        info.checks.pending = 2;
        assert_eq!(
            present_pull_request(&info, &summary).checks.unwrap().text,
            "2 running"
        );
        info.checks.status = PullRequestChecksStatus::Failure;
        info.checks.failing = 1;
        assert_eq!(
            present_pull_request(&info, &summary).checks.unwrap().text,
            "1 failing"
        );
        summary.untracked_count = 1;
        summary.changed_count = 1;
        assert_eq!(
            present_pull_request(&info, &summary)
                .local_changes
                .unwrap()
                .text,
            "Uncommitted changes"
        );
    }

    #[test]
    fn merge_matrix_matches_every_retained_blocker_and_allowed_state() {
        let cases = [
            (
                PullRequestState::Closed,
                false,
                PullRequestMergeable::Mergeable,
                PullRequestMergeState::Clean,
                "Only open",
            ),
            (
                PullRequestState::Open,
                true,
                PullRequestMergeable::Mergeable,
                PullRequestMergeState::Clean,
                "ready for review",
            ),
            (
                PullRequestState::Open,
                false,
                PullRequestMergeable::Conflicting,
                PullRequestMergeState::Clean,
                "conflicts",
            ),
            (
                PullRequestState::Open,
                false,
                PullRequestMergeable::Mergeable,
                PullRequestMergeState::Behind,
                "out of date",
            ),
            (
                PullRequestState::Open,
                false,
                PullRequestMergeable::Mergeable,
                PullRequestMergeState::Blocked,
                "blocked",
            ),
            (
                PullRequestState::Open,
                false,
                PullRequestMergeable::Mergeable,
                PullRequestMergeState::Dirty,
                "conflicts",
            ),
            (
                PullRequestState::Open,
                false,
                PullRequestMergeable::Mergeable,
                PullRequestMergeState::Draft,
                "ready for review",
            ),
        ];
        for (state, draft, mergeable, merge_state, reason) in cases {
            let mut info = info();
            info.state = state;
            info.is_draft = draft;
            info.mergeable = mergeable;
            info.merge_state = merge_state;
            let availability = present_pull_request(&info, &summary()).merge;
            assert!(!availability.enabled);
            assert!(availability.help.contains(reason));
        }
        let mut dirty = summary();
        dirty.unstaged_count = 1;
        dirty.changed_count = 1;
        let availability = present_pull_request(&info(), &dirty).merge;
        assert!(!availability.enabled);
        assert!(availability.help.contains("tracked local changes"));

        for merge_state in [
            PullRequestMergeState::Clean,
            PullRequestMergeState::HasHooks,
            PullRequestMergeState::Unstable,
            PullRequestMergeState::Unknown("FUTURE".to_owned()),
        ] {
            let mut info = info();
            info.merge_state = merge_state;
            assert!(present_pull_request(&info, &summary()).merge.enabled);
        }
        let mut warning = info();
        warning.checks.status = PullRequestChecksStatus::Failure;
        let availability = present_pull_request(&warning, &summary()).merge;
        assert!(availability.enabled);
        assert!(availability.help.contains("Checks are failing"));
        warning.checks.status = PullRequestChecksStatus::Pending;
        assert!(present_pull_request(&warning, &summary()).merge.enabled);
    }

    #[test]
    fn update_excludes_cross_repository_and_requires_clean_tracked_state() {
        let mut info = info();
        info.merge_state = PullRequestMergeState::Behind;
        assert!(
            present_pull_request(&info, &summary())
                .update
                .unwrap()
                .enabled
        );
        info.is_cross_repository = true;
        assert!(present_pull_request(&info, &summary()).update.is_none());
        info.is_cross_repository = false;
        let mut dirty = summary();
        dirty.staged_count = 1;
        dirty.changed_count = 1;
        let update = present_pull_request(&info, &dirty).update.unwrap();
        assert!(!update.enabled);
        assert!(update.help.contains("tracked local changes"));
    }

    #[test]
    fn confirmation_arms_confirms_cancels_expires_once_and_rejects_stale_timers() {
        let now = Instant::now();
        let identity = PullRequestConfirmationIdentity::new(key(), repository_identity(), &info());
        let mut state = PullRequestConfirmationState::default();
        let PullRequestConfirmationActivation::Armed(first) = state.activate(
            PullRequestConfirmedAction::Merge(PullRequestMergeMethod::Squash),
            identity.clone(),
            now,
        ) else {
            panic!("arm")
        };
        assert_eq!(state.remaining_seconds(now), Some(5));
        assert_eq!(
            state.activate(
                PullRequestConfirmedAction::Merge(PullRequestMergeMethod::Squash),
                identity.clone(),
                now + Duration::from_secs(1),
            ),
            PullRequestConfirmationActivation::Execute {
                action: PullRequestConfirmedAction::Merge(PullRequestMergeMethod::Squash),
                identity: identity.clone(),
            }
        );
        assert!(
            state
                .expire(first, &identity, now + CONFIRMATION_DURATION)
                .is_none()
        );

        let PullRequestConfirmationActivation::Armed(stale) =
            state.activate(PullRequestConfirmedAction::Close, identity.clone(), now)
        else {
            panic!("arm")
        };
        assert!(state.cancel());
        assert!(
            state
                .expire(stale, &identity, now + CONFIRMATION_DURATION)
                .is_none()
        );
        let PullRequestConfirmationActivation::Armed(current) =
            state.activate(PullRequestConfirmedAction::Close, identity.clone(), now)
        else {
            panic!("arm")
        };
        assert_eq!(
            state.expire(current, &identity, now + CONFIRMATION_DURATION),
            Some((PullRequestConfirmedAction::Close, identity.clone()))
        );
        assert!(
            state
                .expire(current, &identity, now + CONFIRMATION_DURATION)
                .is_none()
        );
    }

    #[test]
    fn confirmation_disarms_on_method_or_identity_change() {
        let now = Instant::now();
        let identity = PullRequestConfirmationIdentity::new(key(), repository_identity(), &info());
        let mut state = PullRequestConfirmationState::default();
        state.activate(
            PullRequestConfirmedAction::Merge(PullRequestMergeMethod::Squash),
            identity.clone(),
            now,
        );
        state.cancel();
        assert!(state.pending().is_none());
        state.activate(
            PullRequestConfirmedAction::Merge(PullRequestMergeMethod::Merge),
            identity.clone(),
            now,
        );
        let mut changed = identity.clone();
        changed.head_oid = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned();
        state.retain_identity(&changed);
        assert!(state.pending().is_none());

        state.activate(PullRequestConfirmedAction::Close, identity.clone(), now);
        let mut changed_repository = identity;
        changed_repository.repository.name = "another-app".to_owned();
        state.retain_identity(&changed_repository);
        assert!(state.pending().is_none());
    }

    #[test]
    fn mutation_plans_refresh_only_the_truth_they_change() {
        for kind in [
            PullRequestMutationKind::Update,
            PullRequestMutationKind::Merge(PullRequestMergeMethod::Squash),
        ] {
            let refresh = pull_request_mutation_refresh(&kind);
            assert!(refresh.contains(RepositoryReadKind::Summary));
            assert!(refresh.contains(RepositoryReadKind::Branches));
            assert!(refresh.contains(RepositoryReadKind::Changes));
            assert!(refresh.contains(RepositoryReadKind::PullRequest));
            assert!(!refresh.contains(RepositoryReadKind::Providers));
        }

        let close = pull_request_mutation_refresh(&PullRequestMutationKind::Close);
        assert!(close.contains(RepositoryReadKind::PullRequest));
        assert!(!close.contains(RepositoryReadKind::Summary));
        assert!(!close.contains(RepositoryReadKind::Branches));
        assert!(!close.contains(RepositoryReadKind::Changes));
        assert!(!close.contains(RepositoryReadKind::Providers));
    }

    #[test]
    fn overlay_policy_stays_compact_for_open_and_closed_pull_requests() {
        let open = present_pull_request(&info(), &summary());
        assert_eq!(pull_request_overlay_policy(&open).target_width, 300.0);
        assert_eq!(pull_request_overlay_policy(&open).target_height, 352.0);

        let mut closed_info = info();
        closed_info.state = PullRequestState::Closed;
        let closed = present_pull_request(&closed_info, &summary());
        assert_eq!(pull_request_overlay_policy(&closed).target_width, 300.0);
        assert_eq!(pull_request_overlay_policy(&closed).target_height, 236.0);
    }
}
