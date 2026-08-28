use crate::repository::RepositoryKey;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, actions, div,
};
use muxy_core::repository_ai::{
    ADDITIONAL_PROMPT_CHARACTER_LIMIT, CONFIGURED_PROMPT_BYTE_LIMIT, PROVIDERS, RepositoryAiAction,
};
use muxy_ui::command_popover::{
    CommandPopover, CommandPopoverItem, CommandPopoverLeading, CommandPopoverRow,
    CommandPopoverStatus,
};
use muxy_ui::popover::PopoverSurface;
use muxy_ui::text_input::{self, InputEvent, InputStyle, TextInput};
use muxy_ui::theme::{Metrics, Theme};

const KEY_CONTEXT: &str = "RepositoryAiPopover";
const PROVIDER_POPOVER_WIDTH: f32 = 300.0;
const PROVIDER_POPOVER_HEIGHT: f32 = 260.0;

actions!(repository_ai_popover, [Dismiss, Submit]);

pub(crate) fn key_bindings() -> Vec<gpui::KeyBinding> {
    vec![
        gpui::KeyBinding::new("escape", Dismiss, Some(KEY_CONTEXT)),
        gpui::KeyBinding::new("cmd-enter", Submit, Some(KEY_CONTEXT)),
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RepositoryAiPanelMode {
    Confirmation,
    ProjectPrompt,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RepositoryAiPanelEvent {
    Dismiss,
    Confirm {
        action: RepositoryAiAction,
        identity: RepositoryAiConfirmationIdentity,
        additional_prompt: Option<String>,
    },
    SaveProjectPrompt(String),
    UseGlobalPrompt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryAiConfirmationIdentity {
    pub(crate) key: RepositoryKey,
    pub(crate) branch: String,
    pub(crate) head: muxy_api::repository::RepositoryHead,
}

pub(crate) struct RepositoryAiPanel {
    key: RepositoryKey,
    action: RepositoryAiAction,
    mode: RepositoryAiPanelMode,
    provider: String,
    branch: String,
    head: muxy_api::repository::RepositoryHead,
    input: Entity<TextInput>,
    input_text: String,
    fallback_prompt: String,
    project_override: bool,
    focus: FocusHandle,
    theme: Theme,
    metrics: Metrics,
    focused: bool,
    confirmation_submitted: bool,
    prompt_visible: bool,
    _subscription: Subscription,
}

struct RepositoryAiPanelInit {
    key: RepositoryKey,
    action: RepositoryAiAction,
    mode: RepositoryAiPanelMode,
    provider: String,
    branch: String,
    head: muxy_api::repository::RepositoryHead,
    text: String,
    fallback_prompt: String,
    project_override: bool,
}

#[derive(Clone)]
enum RepositoryAiButton {
    Emit(RepositoryAiPanelEvent),
    ShowPrompt,
    Submit,
}

fn bounded_additional_prompt(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(
        value
            .chars()
            .take(ADDITIONAL_PROMPT_CHARACTER_LIMIT)
            .collect(),
    )
}

fn project_prompt_save_enabled(project_override: bool, value: &str) -> bool {
    project_override && !value.trim().is_empty() && value.len() <= CONFIGURED_PROMPT_BYTE_LIMIT
}

fn project_prompt_initial_state(
    project_prompt: Option<String>,
    fallback_prompt: String,
) -> (String, String, bool) {
    let project_override = project_prompt.is_some();
    let text = project_prompt.unwrap_or_else(|| fallback_prompt.clone());
    (text, fallback_prompt, project_override)
}

fn take_confirmation_event(
    submitted: &mut bool,
    action: RepositoryAiAction,
    identity: RepositoryAiConfirmationIdentity,
    value: &str,
) -> Option<RepositoryAiPanelEvent> {
    if *submitted {
        return None;
    }
    *submitted = true;
    Some(RepositoryAiPanelEvent::Confirm {
        action,
        identity,
        additional_prompt: bounded_additional_prompt(value),
    })
}

fn confirmation_copy(
    action: RepositoryAiAction,
    provider: &str,
    branch: &str,
) -> (&'static str, String, String) {
    match action {
        RepositoryAiAction::Commit => (
            "Commit Changes",
            format!("{provider} · {branch}"),
            "Stages all changes, generates a commit message, commits, and pushes.".to_owned(),
        ),
        RepositoryAiAction::CreatePullRequest => (
            "Create Pull Request",
            format!("{provider} · {branch}"),
            "Stages all changes, creates a branch and commit, pushes, and opens a pull request."
                .to_owned(),
        ),
    }
}

impl EventEmitter<RepositoryAiPanelEvent> for RepositoryAiPanel {}

impl Focusable for RepositoryAiPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl RepositoryAiPanel {
    pub(crate) fn confirmation(
        identity: RepositoryAiConfirmationIdentity,
        action: RepositoryAiAction,
        provider: String,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new(
            RepositoryAiPanelInit {
                key: identity.key,
                action,
                mode: RepositoryAiPanelMode::Confirmation,
                provider,
                branch: identity.branch,
                head: identity.head,
                text: String::new(),
                fallback_prompt: String::new(),
                project_override: false,
            },
            theme,
            metrics,
            cx,
        )
    }

    pub(crate) fn project_prompt(
        key: RepositoryKey,
        project_prompt: Option<String>,
        fallback_prompt: String,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let (text, fallback_prompt, project_override) =
            project_prompt_initial_state(project_prompt, fallback_prompt);
        Self::new(
            RepositoryAiPanelInit {
                key,
                action: RepositoryAiAction::CreatePullRequest,
                mode: RepositoryAiPanelMode::ProjectPrompt,
                provider: String::new(),
                branch: String::new(),
                head: muxy_api::repository::RepositoryHead::Unborn,
                text,
                fallback_prompt,
                project_override,
            },
            theme,
            metrics,
            cx,
        )
    }

    fn new(
        init: RepositoryAiPanelInit,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let RepositoryAiPanelInit {
            key,
            action,
            mode,
            provider,
            branch,
            head,
            text,
            fallback_prompt,
            project_override,
        } = init;
        let input = cx.new(|cx| {
            TextInput::new(InputStyle::field(&theme, &metrics), cx)
                .with_key_context(text_input::BARE_CONTEXT)
                .with_placeholder(if mode == RepositoryAiPanelMode::Confirmation {
                    "Add instructions for this run…"
                } else {
                    "Describe how pull requests should be written…"
                })
                .with_text(text.clone())
                .multiline()
        });
        let subscription = cx.subscribe(&input, |panel: &mut Self, input, event, cx| {
            if !matches!(event, InputEvent::Changed) {
                return;
            }
            let mut value = input.read(cx).text().to_owned();
            if panel.mode == RepositoryAiPanelMode::Confirmation {
                value = value
                    .chars()
                    .take(ADDITIONAL_PROMPT_CHARACTER_LIMIT)
                    .collect();
            }
            if panel.input_text != value {
                panel.input_text = value;
                if panel.mode == RepositoryAiPanelMode::ProjectPrompt {
                    panel.project_override = true;
                }
                cx.notify();
            }
        });
        Self {
            key,
            action,
            mode,
            provider,
            branch,
            head,
            input,
            input_text: text,
            fallback_prompt,
            project_override,
            focus: cx.focus_handle(),
            theme,
            metrics,
            focused: false,
            confirmation_submitted: false,
            prompt_visible: false,
            _subscription: subscription,
        }
    }

    pub(crate) fn key(&self) -> &RepositoryKey {
        &self.key
    }

    pub(crate) fn size(&self) -> (f32, f32) {
        match self.mode {
            RepositoryAiPanelMode::Confirmation if self.prompt_visible => (340.0, 224.0),
            RepositoryAiPanelMode::Confirmation => (340.0, 164.0),
            RepositoryAiPanelMode::ProjectPrompt => (380.0, 284.0),
        }
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        self.emit_submit(cx);
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(RepositoryAiPanelEvent::Dismiss);
    }

    fn emit_submit(&mut self, cx: &mut Context<Self>) {
        match self.mode {
            RepositoryAiPanelMode::Confirmation => {
                if let Some(event) = take_confirmation_event(
                    &mut self.confirmation_submitted,
                    self.action,
                    RepositoryAiConfirmationIdentity {
                        key: self.key.clone(),
                        branch: self.branch.clone(),
                        head: self.head.clone(),
                    },
                    &self.input_text,
                ) {
                    cx.emit(event);
                }
            }
            RepositoryAiPanelMode::ProjectPrompt => {
                if self.project_prompt_valid() {
                    cx.emit(RepositoryAiPanelEvent::SaveProjectPrompt(
                        self.input_text.trim().to_owned(),
                    ));
                }
            }
        }
    }

    fn project_prompt_valid(&self) -> bool {
        project_prompt_save_enabled(self.project_override, &self.input_text)
    }

    fn button(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        primary: bool,
        enabled: bool,
        action: RepositoryAiButton,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(id)
            .h(self.metrics.control_medium())
            .px(self.metrics.spacing4())
            .flex()
            .items_center()
            .justify_center()
            .rounded(self.metrics.radius_sm())
            .text_size(self.metrics.font_footnote())
            .font_weight(FontWeight::MEDIUM)
            .when(primary && enabled, |button| {
                button
                    .bg(self.theme.accent)
                    .text_color(self.theme.accent_foreground)
            })
            .when(!primary && enabled, |button| {
                button
                    .border_1()
                    .border_color(self.theme.border)
                    .bg(self.theme.surface)
                    .text_color(self.theme.fg)
            })
            .when(!enabled, |button| {
                button.bg(self.theme.surface).text_color(self.theme.fg_dim)
            })
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|style| style.bg(self.theme.hover))
                    .on_click(cx.listener(move |panel, _, window, cx| match &action {
                        RepositoryAiButton::Emit(event) => cx.emit(event.clone()),
                        RepositoryAiButton::ShowPrompt => {
                            panel.prompt_visible = true;
                            window.focus(&panel.input.focus_handle(cx));
                            cx.notify();
                        }
                        RepositoryAiButton::Submit => panel.emit_submit(cx),
                    }))
            })
            .child(label.into())
            .into_any_element()
    }
}

impl Render for RepositoryAiPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused {
            self.focused = true;
            if self.mode == RepositoryAiPanelMode::ProjectPrompt || self.prompt_visible {
                window.focus(&self.input.focus_handle(cx));
            } else {
                window.focus(&self.focus);
            }
        }
        let (width, height) = self.size();
        let (title, subtitle, disclosure) = match self.mode {
            RepositoryAiPanelMode::Confirmation => {
                let (title, subtitle, disclosure) =
                    confirmation_copy(self.action, &self.provider, &self.branch);
                (title, subtitle, Some(disclosure))
            }
            RepositoryAiPanelMode::ProjectPrompt if self.project_override => (
                "Project Pull Request Prompt",
                "This project overrides the global pull request prompt.".to_owned(),
                None,
            ),
            RepositoryAiPanelMode::ProjectPrompt => (
                "Project Pull Request Prompt",
                "Using the global pull request prompt until you save an override.".to_owned(),
                None,
            ),
        };
        let input_height = match self.mode {
            RepositoryAiPanelMode::Confirmation => 72.0,
            RepositoryAiPanelMode::ProjectPrompt => 148.0,
        };
        let mut buttons = div()
            .flex()
            .items_center()
            .justify_end()
            .gap(self.metrics.spacing3());
        if self.mode == RepositoryAiPanelMode::ProjectPrompt {
            buttons = buttons.child(self.button(
                "ai-use-global",
                "Use Global",
                false,
                self.project_override || self.input_text != self.fallback_prompt,
                RepositoryAiButton::Emit(RepositoryAiPanelEvent::UseGlobalPrompt),
                cx,
            ));
        } else if !self.prompt_visible {
            buttons = buttons.child(self.button(
                "ai-add-prompt",
                "Add Prompt",
                false,
                true,
                RepositoryAiButton::ShowPrompt,
                cx,
            ));
        }
        buttons = buttons
            .child(self.button(
                "ai-cancel",
                "Cancel",
                false,
                true,
                RepositoryAiButton::Emit(RepositoryAiPanelEvent::Dismiss),
                cx,
            ))
            .child(self.button(
                "ai-submit",
                if self.mode == RepositoryAiPanelMode::Confirmation {
                    "Continue"
                } else {
                    "Save"
                },
                true,
                self.mode == RepositoryAiPanelMode::Confirmation || self.project_prompt_valid(),
                RepositoryAiButton::Submit,
                cx,
            ));
        let mut content = div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::dismiss))
            .on_action(cx.listener(Self::submit))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .p(self.metrics.spacing5())
            .flex()
            .flex_col()
            .gap(self.metrics.spacing4())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(self.metrics.spacing1())
                    .child(
                        div()
                            .text_size(self.metrics.font_body())
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(self.theme.fg)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(self.metrics.font_caption())
                            .text_color(self.theme.fg_muted)
                            .child(subtitle),
                    )
                    .when_some(disclosure, |header, disclosure| {
                        header.child(
                            div()
                                .pt(self.metrics.spacing1())
                                .text_size(self.metrics.font_caption())
                                .text_color(self.theme.fg_muted)
                                .child(disclosure),
                        )
                    }),
            );
        if self.mode == RepositoryAiPanelMode::ProjectPrompt || self.prompt_visible {
            content = content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(self.metrics.spacing1())
                    .child(
                        div()
                            .h(self.metrics.scaled(input_height))
                            .rounded(self.metrics.radius_md())
                            .border_1()
                            .border_color(
                                if self.input.read(cx).focus_handle(cx).is_focused(window) {
                                    self.theme.accent
                                } else {
                                    self.theme.border
                                },
                            )
                            .bg(self.theme.surface)
                            .p(self.metrics.spacing3())
                            .child(self.input.clone()),
                    )
                    .when(self.mode == RepositoryAiPanelMode::Confirmation, |input| {
                        input.child(
                            div()
                                .flex()
                                .justify_end()
                                .text_size(self.metrics.font_caption())
                                .text_color(self.theme.fg_dim)
                                .child(format!(
                                    "{}/{}",
                                    self.input_text.chars().count(),
                                    ADDITIONAL_PROMPT_CHARACTER_LIMIT
                                )),
                        )
                    }),
            );
        }
        content = content
            .when(
                self.mode == RepositoryAiPanelMode::ProjectPrompt
                    && self.input_text.len() > CONFIGURED_PROMPT_BYTE_LIMIT,
                |content| {
                    content.child(
                        div()
                            .text_size(self.metrics.font_caption())
                            .text_color(self.theme.danger)
                            .child("Prompt exceeds the 16 KB limit"),
                    )
                },
            )
            .child(buttons);
        PopoverSurface::new(self.theme.clone(), self.metrics, width, height, content)
    }
}

pub(crate) enum RepositoryAiPopover {
    Provider {
        key: RepositoryKey,
        action: RepositoryAiAction,
        picker: Entity<CommandPopover>,
    },
    Panel(Entity<RepositoryAiPanel>),
}

impl RepositoryAiPopover {
    pub(crate) fn key(&self, cx: &Context<crate::views::window::MainWindow>) -> RepositoryKey {
        match self {
            Self::Provider { key, .. } => key.clone(),
            Self::Panel(panel) => panel.read(cx).key().clone(),
        }
    }

    pub(crate) fn size(&self, cx: &Context<crate::views::window::MainWindow>) -> (f32, f32) {
        match self {
            Self::Provider { .. } => provider_popover_size(),
            Self::Panel(panel) => panel.read(cx).size(),
        }
    }

    pub(crate) fn render(&self, origin: gpui::Point<gpui::Pixels>) -> AnyElement {
        let child: AnyElement = match self {
            Self::Provider { picker, .. } => picker.clone().into_any_element(),
            Self::Panel(panel) => panel.clone().into_any_element(),
        };
        div()
            .absolute()
            .left(origin.x)
            .top(origin.y)
            .child(child)
            .into_any_element()
    }
}

pub(crate) fn provider_popover_size() -> (f32, f32) {
    (PROVIDER_POPOVER_WIDTH, PROVIDER_POPOVER_HEIGHT)
}

pub(crate) fn provider_items(
    configured: &str,
    inventory: &muxy_api::repository::ProviderInventory,
    query: &str,
) -> Vec<CommandPopoverItem> {
    let query = query.trim().to_lowercase();
    let automatic = inventory
        .automatic()
        .map(|provider| provider.descriptor.display_name)
        .unwrap_or("No provider installed");
    let mut items = Vec::new();
    if query.is_empty() || "automatic".contains(&query) || automatic.to_lowercase().contains(&query)
    {
        let mut row = CommandPopoverRow::new("provider:auto", "Automatic");
        row.trailing = Some(automatic.to_owned().into());
        row.leading = Some(CommandPopoverLeading::Icon(muxy_ui::icon::Icon::Lightbulb));
        row.current = configured.trim().is_empty();
        row.disabled = inventory.automatic().is_none();
        items.push(CommandPopoverItem::Row(row));
    }
    for provider in PROVIDERS {
        if !query.is_empty()
            && !provider.display_name.to_lowercase().contains(&query)
            && !provider.id.contains(&query)
        {
            continue;
        }
        let installed = inventory.installation(provider.id).is_some();
        let mut row =
            CommandPopoverRow::new(format!("provider:{}", provider.id), provider.display_name);
        row.trailing = Some(if installed {
            "Installed".into()
        } else {
            "Not installed".into()
        });
        row.leading = Some(CommandPopoverLeading::Asset(
            format!("icons/providers/{}.svg", provider.icon_key).into(),
        ));
        row.current = configured.trim() == provider.id;
        row.disabled = !installed;
        items.push(CommandPopoverItem::Row(row));
    }
    items
}

pub(crate) fn sync_provider_picker(
    picker: &Entity<CommandPopover>,
    configured: &str,
    inventory: &muxy_api::repository::ProviderInventory,
    query: &str,
    cx: &mut Context<crate::views::window::MainWindow>,
) {
    let items = provider_items(configured, inventory, query);
    picker.update(cx, |picker, cx| {
        if items.is_empty() {
            picker.set_status(
                CommandPopoverStatus::Empty("No matching providers".into()),
                cx,
            );
        } else {
            picker.set_items(items, cx);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_api::execution_environment::ExecutionEnvironment;
    use std::ffi::OsString;

    fn rows(items: Vec<CommandPopoverItem>) -> Vec<CommandPopoverRow> {
        items
            .into_iter()
            .filter_map(CommandPopoverItem::row_data)
            .collect()
    }

    fn confirmation_identity() -> RepositoryAiConfirmationIdentity {
        RepositoryAiConfirmationIdentity {
            key: RepositoryKey {
                project_id: "project".to_owned(),
                worktree_id: "primary".to_owned(),
                normalized_path: std::path::PathBuf::from("/repo"),
            },
            branch: "topic".to_owned(),
            head: muxy_api::repository::RepositoryHead::Commit("a".repeat(40)),
        }
    }

    #[test]
    fn project_prompt_validation_requires_nonblank_bounded_override() {
        assert!(!project_prompt_save_enabled(false, "prompt"));
        assert!(!project_prompt_save_enabled(true, "  \n"));
        assert!(project_prompt_save_enabled(true, "prompt"));
        assert!(!project_prompt_save_enabled(
            true,
            &"x".repeat(CONFIGURED_PROMPT_BYTE_LIMIT + 1)
        ));
    }

    #[test]
    fn project_prompt_uses_global_fallback_until_an_override_exists() {
        assert_eq!(
            project_prompt_initial_state(None, "global".to_owned()),
            ("global".to_owned(), "global".to_owned(), false)
        );
        assert_eq!(
            project_prompt_initial_state(Some("project".to_owned()), "global".to_owned()),
            ("project".to_owned(), "global".to_owned(), true)
        );
        assert_eq!(
            muxy_core::repository_ai::normalized_project_prompt(Some(" \n")),
            None
        );
        assert_eq!(muxy_core::repository_ai::use_global_prompt(), None);
    }

    #[test]
    fn confirmation_prompt_is_trimmed_and_bounded_to_two_thousand_characters() {
        assert_eq!(bounded_additional_prompt("  "), None);
        assert_eq!(
            bounded_additional_prompt("  focus tests  ").as_deref(),
            Some("focus tests")
        );
        assert_eq!(
            bounded_additional_prompt(&"é".repeat(ADDITIONAL_PROMPT_CHARACTER_LIMIT + 4))
                .unwrap()
                .chars()
                .count(),
            ADDITIONAL_PROMPT_CHARACTER_LIMIT
        );
    }

    #[test]
    fn confirmation_can_submit_exactly_once() {
        let mut submitted = false;
        let identity = confirmation_identity();
        assert_eq!(
            take_confirmation_event(
                &mut submitted,
                RepositoryAiAction::Commit,
                identity.clone(),
                "focus tests"
            ),
            Some(RepositoryAiPanelEvent::Confirm {
                action: RepositoryAiAction::Commit,
                identity,
                additional_prompt: Some("focus tests".to_owned()),
            })
        );
        assert_eq!(
            take_confirmation_event(
                &mut submitted,
                RepositoryAiAction::Commit,
                confirmation_identity(),
                "again"
            ),
            None
        );
    }

    #[test]
    fn confirmation_copy_names_provider_branch_and_complete_mutation_sequence() {
        let (title, subtitle, disclosure) =
            confirmation_copy(RepositoryAiAction::Commit, "Codex", "topic");
        assert_eq!(title, "Commit Changes");
        assert_eq!(subtitle, "Codex · topic");
        for step in ["Stages", "generates", "commits", "pushes"] {
            assert!(disclosure.contains(step));
        }

        let (_, subtitle, disclosure) =
            confirmation_copy(RepositoryAiAction::CreatePullRequest, "Claude Code", "main");
        assert_eq!(subtitle, "Claude Code · main");
        for step in [
            "Stages",
            "creates a branch",
            "commit",
            "pushes",
            "pull request",
        ] {
            assert!(disclosure.contains(step));
        }
    }

    #[test]
    fn provider_menu_contains_automatic_and_all_catalog_entries() {
        let inventory = muxy_api::repository::ProviderInventory::default();
        let rows = rows(provider_items("codex", &inventory, ""));
        assert_eq!(provider_popover_size(), (300.0, 260.0));
        assert_eq!(rows.len(), PROVIDERS.len() + 1);
        assert_eq!(rows[0].id.as_ref(), "provider:auto");
        assert_eq!(
            rows[0].leading,
            Some(CommandPopoverLeading::Icon(muxy_ui::icon::Icon::Lightbulb))
        );
        assert!(rows[0].disabled);
        assert!(rows.iter().all(|row| row.subtitle.is_none()));
        assert!(rows.iter().all(|row| row.trailing.is_some()));
        assert!(
            rows.iter()
                .any(|row| { row.id.as_ref() == "provider:codex" && row.current && row.disabled })
        );
        for provider in PROVIDERS {
            assert!(
                rows.iter()
                    .any(|row| row.id.as_ref() == format!("provider:{}", provider.id))
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn provider_menu_refreshes_installed_and_automatic_state_from_environment() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join(".local/bin");
        fs::create_dir_all(&bin).unwrap();
        let environment =
            ExecutionEnvironment::fallback([(OsString::from("PATH"), OsString::new())]);
        let before =
            muxy_api::repository::ProviderInventory::discover(&environment, temp.path(), false);
        assert!(before.installation("codex").is_none());

        let executable = bin.join("codex");
        fs::write(&executable, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let after =
            muxy_api::repository::ProviderInventory::discover(&environment, temp.path(), false);
        let rows = rows(provider_items("codex", &after, "code"));
        assert!(rows.iter().any(|row| {
            row.id.as_ref() == "provider:auto"
                && row
                    .trailing
                    .as_ref()
                    .is_some_and(|subtitle| subtitle.as_ref() == "Codex")
                && !row.disabled
        }));
        assert!(
            rows.iter()
                .any(|row| { row.id.as_ref() == "provider:codex" && row.current && !row.disabled })
        );
    }
}
