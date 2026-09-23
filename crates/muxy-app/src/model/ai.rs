use std::collections::HashMap;

use gpui::{Context, Task, Window};
use muxy_protocol::{GitAction, GitPullRequestAction, ProjectId};
use muxy_ui::dialog::ADDITIONAL_PROMPT_LIMIT;

use super::{AppModel, Work, git::Presence};
use crate::ai::{Cancellation, Provider};
use crate::repository_actions::{self, Action, Outcome};
use crate::views::git::AiConfirmation;
use crate::views::overlays::Overlay;

/// Installed providers and the AI work in flight for each project.
pub(crate) struct Runtime {
    pub(crate) installed: Vec<Provider>,
    pub(crate) confirmation: Option<PendingConfirmation>,
    runs: HashMap<ProjectId, Run>,
    next: u64,
    pub(crate) anchors: [muxy_ui::popover::PopoverAnchor; 2],
}

pub(crate) struct PendingConfirmation {
    pub(crate) project: ProjectId,
    _task: Task<()>,
}

struct Run {
    id: u64,
    action: Action,
    cancellation: Cancellation,
}

impl Drop for Run {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            installed: if cfg!(test) {
                Vec::new()
            } else {
                Provider::installed()
            },
            confirmation: None,
            runs: HashMap::new(),
            next: 0,
            anchors: Default::default(),
        }
    }
}

pub(crate) enum Availability {
    Available(Provider),
    Disabled(String),
    Hidden,
}

impl Runtime {
    #[cfg(not(test))]
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
            None => crate::ai::selected(&self.installed, None).ok_or_else(|| {
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
        let Some(_) = &summary.branch else {
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
        if summary.changed == 0 {
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
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.ai.confirmation.is_some() {
            return;
        }
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
        let confirmation = AiConfirmation {
            project,
            action,
            provider,
            branch,
            head: summary.head,
        };
        self.dismiss_overlay(cx);
        let window = self.window;
        let theme = self.theme.clone();
        let metrics = self.metrics;
        self.ai.confirmation = Some(PendingConfirmation {
            project,
            _task: cx.spawn(async move |model, cx| {
                let style = muxy_ui::controls::Style {
                    theme: &theme,
                    metrics: &metrics,
                };
                let response = confirmation.prompt(window, style, cx).await;
                let _ = model.update(cx, |model, cx| {
                    model.ai.confirmation = None;
                    match response {
                        Ok(Some(prompt)) => model.advance_ai_action(confirmation, &prompt, cx),
                        Ok(None) => {}
                        Err(error) => model.fail_detail(
                            format!("Couldn't start {}", action.settings_title()),
                            &error,
                            cx,
                        ),
                    }
                });
            }),
        });
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

    pub(crate) fn save_project_pr_prompt(&mut self, reset: bool, cx: &mut Context<Self>) {
        let Some(Overlay::AiProvider(menu)) = &self.overlay else {
            return;
        };
        let Some(input) = &menu.prompt else {
            return;
        };
        let text = input.read(cx).text();
        if !reset && text.trim().is_empty() {
            return;
        }
        let key = menu.project.to_string();
        match muxy_app_core::settings::Settings::save_ai_entry(
            &self.path.with_file_name("settings.toml"),
            "project_pr_prompts",
            &key,
            (!reset).then_some(text),
        ) {
            Ok(saved) => {
                self.settings.ai = saved;
                self.dismiss_overlay(cx);
            }
            Err(error) => self.fail(format!("Could not save project prompt: {error}"), cx),
        }
    }

    pub(crate) fn ai_prompt_project(&self, project: ProjectId) -> ProjectId {
        self.state
            .project(project)
            .and_then(|project| project.parent_id)
            .unwrap_or(project)
    }

    pub(crate) fn configured_ai_prompt(
        &self,
        action: Action,
        project: Option<ProjectId>,
    ) -> String {
        project
            .filter(|_| action == Action::CreatePullRequest)
            .and_then(|project| {
                self.settings
                    .ai
                    .project_pr_prompts
                    .get(&self.ai_prompt_project(project).to_string())
            })
            .filter(|prompt| !prompt.trim().is_empty())
            .or_else(|| {
                self.settings
                    .ai
                    .prompts
                    .get(action.key())
                    .filter(|prompt| !prompt.trim().is_empty())
            })
            .cloned()
            .unwrap_or_else(|| action.default_prompt().into())
    }

    fn advance_ai_action(
        &mut self,
        confirmation: AiConfirmation,
        additional: &str,
        cx: &mut Context<Self>,
    ) {
        let AiConfirmation {
            project,
            action,
            provider,
            branch,
            head,
        } = confirmation;
        let valid = project == self.state.current_project().id
            && matches!(self.ai_availability(action), Availability::Available(_))
            && self
                .git
                .projects
                .get(&project)
                .and_then(|repository| repository.summary.as_ref())
                .is_some_and(|summary| {
                    summary.branch.as_deref() == Some(&branch) && summary.head == head
                });
        if !valid {
            self.fail(
                format!(
                    "{} is no longer available. Try again.",
                    action.settings_title()
                ),
                cx,
            );
            return;
        }
        let configured = self.configured_ai_prompt(action, Some(project));
        let instructions = ai_instructions(&configured, additional);
        let directory = self.state.current_project().directory.clone();
        self.ai.next += 1;
        let id = self.ai.next;
        let cancellation = self.ai.start(project, id, action);
        self.with_client(
            cx,
            move |client| {
                let plan = repository_actions::prepare(
                    &client,
                    project,
                    action,
                    &branch,
                    head.as_deref(),
                )?;
                let prompt = repository_actions::prompt(&plan, &instructions);
                let output = provider.generate(&prompt, &directory, &cancellation)?;
                let draft = repository_actions::parse_draft(&plan, &output)?;
                Ok(repository_actions::apply(&client, &plan, &draft))
            },
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
}

fn ai_instructions(configured: &str, additional: &str) -> String {
    let additional: String = additional.chars().take(ADDITIONAL_PROMPT_LIMIT).collect();
    let additional = additional.trim();
    if additional.is_empty() {
        configured.into()
    } else {
        format!("{configured}\n\n{additional}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additional_prompt_is_trimmed_bounded_and_appended_only_for_this_run() {
        assert_eq!(ai_instructions("Configured", "  \n "), "Configured");
        assert_eq!(
            ai_instructions("Configured", "  Extra\nInstructions  "),
            "Configured\n\nExtra\nInstructions"
        );
        let prompt = "🦀".repeat(ADDITIONAL_PROMPT_LIMIT + 10);
        assert_eq!(
            ai_instructions("Configured", &prompt),
            format!("Configured\n\n{}", "🦀".repeat(ADDITIONAL_PROMPT_LIMIT))
        );
    }
}
