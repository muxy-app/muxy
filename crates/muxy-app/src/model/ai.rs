use std::collections::HashMap;
use std::sync::Arc;

use gpui::{AppContext, Context, Focusable, Window};
use muxy_protocol::{GitAction, GitPullRequestAction, ProjectId};
use muxy_ui::picker::{Picker, PickerConfig, PickerEvent, PickerItem, PickerRow};
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};

use super::{AppModel, Work, git::Presence};
use crate::ai::{Cancellation, Provider};
use crate::repository_actions::{self, Action, Draft, Mode, Outcome, Plan};
use crate::views::git::{ADDITIONAL_PROMPT_LIMIT, AiReview, AiSheet, AiStep};
use crate::views::overlays::Overlay;

/// Installed providers and the AI work in flight for each project.
pub(crate) struct Runtime {
    pub(crate) installed: Vec<Provider>,
    runs: HashMap<ProjectId, Run>,
    next: u64,
}

struct Run {
    id: u64,
    action: Action,
    cancellation: Cancellation,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            installed: if cfg!(test) {
                Vec::new()
            } else {
                Provider::installed()
            },
            runs: HashMap::new(),
            next: 0,
        }
    }
}

pub(crate) enum Availability {
    Available(Provider),
    Disabled(String),
    Hidden,
}

impl Runtime {
    pub(crate) fn refresh_installed(&mut self) {
        self.installed = Provider::installed();
    }

    pub(crate) fn running(&self, project: ProjectId) -> bool {
        self.runs.contains_key(&project)
    }

    pub(crate) fn running_action(&self, project: ProjectId) -> Option<Action> {
        self.runs.get(&project).map(|run| run.action)
    }

    pub(crate) fn provider(
        &self,
        settings: &muxy_app_core::settings::Settings,
        action: Action,
    ) -> Result<Provider, String> {
        match settings
            .ai
            .providers
            .get(action.key())
            .filter(|id| !id.is_empty())
        {
            Some(id) => match crate::ai::provider(id) {
                None => Err(format!(
                    "The selected AI provider \"{id}\" is no longer supported. Choose another one in Settings → AI."
                )),
                Some(provider) if self.installed.contains(&provider) => Ok(provider),
                Some(provider) => Err(format!(
                    "{} CLI is not installed. Choose another provider or install its CLI.",
                    provider.name
                )),
            },
            None => self.installed.first().copied().ok_or_else(|| {
                "Install a supported AI provider CLI or choose one in Settings → AI.".into()
            }),
        }
    }

    fn start(&mut self, project: ProjectId, id: u64, action: Action) -> Cancellation {
        let cancellation = Cancellation::default();
        self.runs.insert(
            project,
            Run {
                id,
                action,
                cancellation: cancellation.clone(),
            },
        );
        cancellation
    }

    fn finish(&mut self, project: ProjectId, id: u64) {
        if self.runs.get(&project).is_some_and(|run| run.id == id) {
            self.runs.remove(&project);
        }
    }

    pub(crate) fn cancel(&mut self, project: ProjectId, id: u64) {
        if let Some(run) = self.runs.get(&project).filter(|run| run.id == id) {
            run.cancellation.cancel();
            self.runs.remove(&project);
        }
    }
}

impl AppModel {
    #[cfg(not(test))]
    pub(crate) fn discover_ai_providers(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let installed = crate::ai::run(|| Ok(Provider::discover())).await;
            if let Ok(installed) = installed {
                let _ = this.update(cx, |model, cx| {
                    if model.ai.installed != installed {
                        model.ai.installed = installed;
                        model.sync_preferences(cx);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn set_ai_provider(&mut self, action: Action, id: &str, cx: &mut Context<Self>) {
        if !id.is_empty() && crate::ai::provider(id).is_none() {
            return;
        }
        self.save_ai_entry(
            "providers",
            action,
            Some(id).filter(|id| !id.is_empty()),
            cx,
        );
    }

    pub(crate) fn set_ai_prompt(&mut self, action: Action, prompt: &str, cx: &mut Context<Self>) {
        let prompt = prompt.trim();
        let value =
            Some(prompt).filter(|prompt| !prompt.is_empty() && *prompt != action.default_prompt());
        self.save_ai_entry("prompts", action, value, cx);
    }

    fn save_ai_entry(
        &mut self,
        table: &str,
        action: Action,
        value: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        match muxy_app_core::settings::Settings::save_ai_entry(
            &self.path.with_file_name("settings.toml"),
            table,
            action.key(),
            value,
        ) {
            Ok(saved) => self.settings.ai = saved,
            Err(error) => self.fail(format!("Could not save AI settings: {error}"), cx),
        }
        self.sync_preferences(cx);
        cx.notify();
    }

    pub(crate) fn ai_availability(&self, action: Action) -> Availability {
        let project = self.state.current_project().id;
        let Some(repository) = self.git.projects.get(&project) else {
            return Availability::Hidden;
        };
        let Some(summary) = &repository.summary else {
            return Availability::Hidden;
        };
        if action == Action::CreatePullRequest
            && repository.pull_request_presence() != Presence::None
        {
            return Availability::Hidden;
        }
        if let Some(running) = self.ai.running_action(project) {
            return Availability::Disabled(if running == action {
                format!("{} is running", action.settings_title())
            } else {
                "Wait for the current AI action to finish".into()
            });
        }
        if !self.session_listing_ready() {
            return Availability::Disabled("Connect to the server first".into());
        }
        if repository.busy() {
            return Availability::Disabled("Wait for the current Git action to finish".into());
        }
        let Some(branch) = &summary.branch else {
            return Availability::Disabled(match action {
                Action::Commit => "Switch to a branch before committing and pushing".into(),
                Action::CreatePullRequest => {
                    "Switch to a branch before creating a pull request".into()
                }
            });
        };
        if summary.conflicted > 0 {
            return Availability::Disabled("Resolve merge conflicts first".into());
        }
        let on_default = repository
            .branches
            .iter()
            .any(|candidate| candidate.default && candidate.name == *branch);
        if summary.changed == 0 && (action == Action::Commit || on_default) {
            return Availability::Disabled("The working tree is clean.".into());
        }
        match self.ai.provider(&self.settings, action) {
            Ok(provider) => Availability::Available(provider),
            Err(reason) => Availability::Disabled(reason),
        }
    }

    pub(crate) fn open_ai_action(
        &mut self,
        action: Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(not(test))]
        self.ai.refresh_installed();
        let provider = match self.ai_availability(action) {
            Availability::Available(provider) => provider,
            Availability::Disabled(reason) => {
                self.fail_detail(
                    format!("Couldn't start {}", action.settings_title()),
                    &reason,
                    cx,
                );
                return;
            }
            Availability::Hidden => return,
        };
        let project = self.state.current_project().id;
        let Some(summary) = self
            .git
            .projects
            .get(&project)
            .and_then(|repository| repository.summary.clone())
        else {
            return;
        };
        let Some(branch) = summary.branch else {
            return;
        };
        self.ai.next += 1;
        let id = self.ai.next;
        let prompt = cx.new(|cx| {
            TextInput::new(InputStyle::field(&self.theme, &self.metrics), cx)
                .multiline()
                .with_placeholder("Additional instructions for this run")
        });
        let subscription = cx.subscribe(&prompt, |model, input, event, cx| match event {
            InputEvent::Cancelled => model.dismiss_overlay(cx),
            InputEvent::Changed => {
                let text = input.read(cx).text();
                if text.chars().count() > ADDITIONAL_PROMPT_LIMIT {
                    let clipped: String = text.chars().take(ADDITIONAL_PROMPT_LIMIT).collect();
                    input.update(cx, |input, cx| input.set_text(clipped, cx));
                }
                cx.notify();
            }
            InputEvent::Submitted => (),
        });
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::AiAction(AiSheet {
            id,
            project,
            action,
            provider,
            branch: branch.clone(),
            step: AiStep::Loading,
            error: None,
            plan: None,
            prompt,
            adding_prompt: false,
            review: None,
            subscriptions: vec![subscription],
            drafting: None,
        }));
        self.overlay_focus.focus(window);
        self.ai.start(project, id, action);
        let head = summary.head;
        self.with_client(
            cx,
            move |client| {
                repository_actions::prepare(&client, project, action, &branch, head.as_deref())
            },
            move |model, result, cx| model.ai_prepared(id, project, result, cx),
        );
        cx.notify();
    }

    fn with_client<T: Send + 'static>(
        &mut self,
        cx: &mut Context<Self>,
        operation: impl FnOnce(muxy_client::Client) -> Result<T, String> + Send + 'static,
        finish: impl FnOnce(&mut Self, Result<T, String>, &mut Context<Self>) + 'static,
    ) {
        let (sender, receiver) = async_channel::bounded(1);
        self.send(Work::ExtensionClient(sender), cx);
        cx.spawn(async move |this, cx| {
            let result = match receiver.recv().await {
                Ok(Some(client)) => crate::ai::run(move || operation(client)).await,
                _ => Err("Connect to the server first".into()),
            };
            let _ = this.update(cx, |model, cx| finish(model, result, cx));
        })
        .detach();
    }

    fn ai_sheet(&mut self, id: u64) -> Option<&mut AiSheet> {
        match &mut self.overlay {
            Some(Overlay::AiAction(sheet)) if sheet.id == id => Some(sheet),
            _ => None,
        }
    }

    fn ai_prepared(
        &mut self,
        id: u64,
        project: ProjectId,
        result: Result<Plan, String>,
        cx: &mut Context<Self>,
    ) {
        self.ai.finish(project, id);
        cx.notify();
        let Some(sheet) = self.ai_sheet(id) else {
            return;
        };
        match result {
            Ok(plan) => {
                sheet.plan = Some(Arc::new(plan));
                sheet.step = AiStep::Confirm;
            }
            Err(error) => {
                let title = format!("Couldn't start {}", sheet.action.settings_title());
                self.close_ai_sheet(cx);
                self.fail_detail(title, &error, cx);
            }
        }
    }

    fn close_ai_sheet(&mut self, cx: &mut Context<Self>) {
        self.overlay = None;
        self.overlay_subscription = None;
        self.focus_requested = true;
        cx.notify();
    }

    /// Cancels work that has not changed the repository; applying always finishes.
    pub(crate) fn ai_sheet_dismissed(&mut self, sheet: &AiSheet) {
        if matches!(sheet.step, AiStep::Loading | AiStep::Drafting) {
            self.ai.cancel(sheet.project, sheet.id);
        }
    }

    pub(crate) fn toggle_ai_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::AiAction(sheet)) = &mut self.overlay else {
            return;
        };
        sheet.adding_prompt = true;
        sheet.prompt.focus_handle(cx).focus(window);
        cx.notify();
    }

    pub(crate) fn advance_ai_action(&mut self, cx: &mut Context<Self>) {
        match &self.overlay {
            Some(Overlay::AiAction(sheet)) if sheet.step == AiStep::Confirm => {
                self.draft_ai_action(cx);
            }
            Some(Overlay::AiAction(sheet)) if sheet.step == AiStep::Review => {
                self.approve_ai_action(cx);
            }
            _ => (),
        }
    }

    fn draft_ai_action(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::AiAction(sheet)) = &mut self.overlay else {
            return;
        };
        let Some(plan) = sheet.plan.clone() else {
            return;
        };
        if self.ai.running(sheet.project) {
            return;
        }
        let Some(directory) = self
            .state
            .project(sheet.project)
            .map(|project| project.directory.clone())
        else {
            return;
        };
        let configured = self
            .settings
            .ai
            .prompts
            .get(sheet.action.key())
            .filter(|prompt| !prompt.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| sheet.action.default_prompt().into());
        let additional = sheet.prompt.read(cx).text().trim().to_owned();
        let instructions = if additional.is_empty() {
            configured
        } else {
            format!("{configured}\n\n{additional}")
        };
        let prompt = repository_actions::prompt(&plan, &instructions);
        let (id, project, action, provider) =
            (sheet.id, sheet.project, sheet.action, sheet.provider);
        sheet.step = AiStep::Drafting;
        sheet.error = None;
        let cancellation = self.ai.start(project, id, action);
        sheet.drafting = Some(cancellation.clone());
        cx.spawn(async move |this, cx| {
            let result =
                crate::ai::run(move || provider.generate(&prompt, &directory, &cancellation)).await;
            let _ = this.update(cx, |model, cx| model.ai_drafted(id, project, result, cx));
        })
        .detach();
        cx.notify();
    }

    fn ai_drafted(
        &mut self,
        id: u64,
        project: ProjectId,
        result: Result<String, String>,
        cx: &mut Context<Self>,
    ) {
        self.ai.finish(project, id);
        cx.notify();
        let Some(plan) = self
            .ai_sheet(id)
            .filter(|sheet| sheet.step == AiStep::Drafting)
            .and_then(|sheet| sheet.plan.clone())
        else {
            return;
        };
        match result.and_then(|output| repository_actions::parse_draft(&plan, &output)) {
            Ok(draft) => self.review_ai_draft(id, &plan, draft, cx),
            Err(error) => {
                if let Some(sheet) = self.ai_sheet(id) {
                    sheet.step = AiStep::Confirm;
                    sheet.error = Some(error);
                }
            }
        }
    }

    fn review_ai_draft(&mut self, id: u64, plan: &Plan, draft: Draft, cx: &mut Context<Self>) {
        let style = InputStyle::field(&self.theme, &self.metrics);
        let input = |text: String, multiline: bool, cx: &mut Context<Self>| {
            cx.new(|cx| {
                let input = TextInput::new(style, cx).with_text(text);
                if multiline { input.multiline() } else { input }
            })
        };
        let review = match draft {
            Draft::Commit { message } => AiReview {
                message: input(message, true, cx),
                summary: None,
                branch: None,
                target: String::new(),
                chooser: None,
            },
            Draft::PullRequest {
                title,
                summary,
                branch,
                target,
            } => AiReview {
                message: input(title, false, cx),
                summary: Some(input(summary, true, cx)),
                branch: (plan.mode == Mode::NewBranch).then(|| input(branch, false, cx)),
                target,
                chooser: None,
            },
        };
        let mut subscriptions = Vec::new();
        for field in std::iter::once(&review.message)
            .chain(review.summary.as_ref())
            .chain(review.branch.as_ref())
        {
            subscriptions.push(cx.subscribe(field, |model, _, event, cx| match event {
                InputEvent::Submitted => model.approve_ai_action(cx),
                InputEvent::Cancelled => model.dismiss_overlay(cx),
                InputEvent::Changed => {
                    if let Some(Overlay::AiAction(sheet)) = &mut model.overlay {
                        sheet.error = None;
                    }
                    cx.notify();
                }
            }));
        }
        let focus = review.message.focus_handle(cx);
        let Some(sheet) = self.ai_sheet(id) else {
            return;
        };
        sheet.review = Some(review);
        sheet.step = AiStep::Review;
        sheet.subscriptions.extend(subscriptions);
        let _ = self.window.update(cx, |_, window, _| focus.focus(window));
    }

    fn approve_ai_action(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::AiAction(sheet)) = &mut self.overlay else {
            return;
        };
        if sheet.step != AiStep::Review {
            return;
        }
        let (Some(plan), Some(review)) = (sheet.plan.clone(), &sheet.review) else {
            return;
        };
        let draft = review.draft(&plan, cx);
        if let Err(error) = repository_actions::validate(&plan, &draft) {
            sheet.error = Some(error);
            cx.notify();
            return;
        }
        let (id, project, action) = (sheet.id, sheet.project, sheet.action);
        sheet.step = AiStep::Applying;
        sheet.error = None;
        self.ai.start(project, id, action);
        self.with_client(
            cx,
            move |client| Ok(repository_actions::apply(&client, &plan, &draft)),
            move |model, result, cx| model.ai_applied(id, project, action, result, cx),
        );
        cx.notify();
    }

    fn ai_applied(
        &mut self,
        id: u64,
        project: ProjectId,
        action: Action,
        result: Result<Result<Outcome, repository_actions::Failure>, String>,
        cx: &mut Context<Self>,
    ) {
        self.ai.finish(project, id);
        if self.ai_sheet(id).is_some() {
            self.close_ai_sheet(cx);
        }
        let elsewhere = (project != self.state.current_project().id)
            .then(|| {
                self.state
                    .project(project)
                    .map(|record| record.name.clone())
            })
            .flatten();
        let located = |detail: String| match &elsewhere {
            Some(name) => format!("{name}: {detail}"),
            None => detail,
        };
        match result {
            Ok(Ok(Outcome::Committed {
                hash,
                branch,
                pushed,
            })) => {
                let short = &hash[..hash.len().min(7)];
                let (title, detail) = match pushed {
                    Some(destination) => (
                        "Committed and pushed",
                        format!("{short} → {}/{}", destination.remote, destination.branch),
                    ),
                    None => ("Committed locally", format!("{short} on {branch}")),
                };
                self.show_toast(title.into(), Some(located(detail)), cx);
            }
            Ok(Ok(Outcome::PullRequest(url))) => {
                #[cfg(target_os = "macos")]
                if let Some(notifications) = &self.notifications {
                    notifications.deliver(
                        &format!("pull-request:{url}"),
                        "Pull request created",
                        &url,
                    );
                }
                self.show_toast("Pull request created".into(), Some(located(url)), cx);
            }
            Ok(Err(failure)) => {
                self.fail_detail(failure.title, &located(failure.detail), cx);
            }
            Err(error) => self.fail_detail(
                format!("Couldn't {}", action.settings_title().to_lowercase()),
                &located(error),
                cx,
            ),
        }
        self.queue_git_refresh(
            project,
            vec![
                GitAction::Summary,
                GitAction::Branches,
                GitAction::PullRequest(GitPullRequestAction::Info),
            ],
            cx,
        );
        cx.notify();
    }

    pub(crate) fn choose_ai_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (theme, metrics) = (self.theme.clone(), self.metrics);
        let Some(Overlay::AiAction(sheet)) = &mut self.overlay else {
            return;
        };
        let Some(review) = &mut sheet.review else {
            return;
        };
        if review.chooser.take().is_some() {
            cx.notify();
            return;
        }
        let chooser = cx.new(|cx| {
            Picker::new(
                PickerConfig {
                    width: Some(420.0),
                    ..PickerConfig::popover("ai-target", "Search branches…")
                },
                theme,
                metrics,
                cx,
            )
        });
        let subscription = cx.subscribe(&chooser, |model, chooser, event, cx| {
            match event {
                PickerEvent::Confirmed(selection) => {
                    if let Some(Overlay::AiAction(sheet)) = &mut model.overlay
                        && let Some(review) = &mut sheet.review
                    {
                        review.target = selection.id.to_string();
                        review.chooser = None;
                        sheet.error = None;
                    }
                }
                PickerEvent::Dismissed => {
                    if let Some(Overlay::AiAction(sheet)) = &mut model.overlay
                        && let Some(review) = &mut sheet.review
                    {
                        review.chooser = None;
                    }
                }
                PickerEvent::QueryChanged { query, .. } => {
                    model.ai_target_choices(&chooser, query.as_ref(), cx);
                }
                _ => (),
            }
            cx.notify();
        });
        review.chooser = Some(chooser.clone());
        sheet.subscriptions.push(subscription);
        self.ai_target_choices(&chooser, "", cx);
        chooser.focus_handle(cx).focus(window);
        cx.notify();
    }

    fn ai_target_choices(
        &self,
        chooser: &gpui::Entity<Picker>,
        query: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(Overlay::AiAction(sheet)) = &self.overlay else {
            return;
        };
        let (Some(plan), Some(review)) = (&sheet.plan, &sheet.review) else {
            return;
        };
        let query = query.to_lowercase();
        let items = plan
            .targets(&review.head(plan, cx))
            .into_iter()
            .filter(|branch| branch.to_lowercase().contains(&query))
            .map(|branch| PickerItem::Row(PickerRow::new(branch.clone(), branch)))
            .collect();
        chooser.update(cx, |picker, cx| picker.set_items(items, cx));
    }
}
