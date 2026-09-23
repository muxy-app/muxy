use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, Context, Corner, Entity, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Pixels, Point, StatefulInteractiveElement, Styled, Subscription, Window,
    anchored, deferred, div, point, px,
};
use muxy_protocol::ProjectId;
use muxy_ui::components::ButtonInteraction;
use muxy_ui::controls::{self, Style};
use muxy_ui::picker::Picker;
use muxy_ui::text_input::TextInput;

use crate::ai::{Cancellation, PROVIDERS, Provider};
use crate::model::AppModel;
use crate::repository_actions::{Action, Draft, Mode, Plan};
use crate::views::menu::{Command, Item};

pub(crate) const ADDITIONAL_PROMPT_LIMIT: usize = 2_000;
const LISTED_FILES: usize = 200;

/// One AI repository action, from reading the changes to applying the reviewed draft.
pub(crate) struct AiSheet {
    pub(crate) id: u64,
    pub(crate) project: ProjectId,
    pub(crate) action: Action,
    pub(crate) provider: Provider,
    pub(crate) branch: String,
    pub(crate) step: AiStep,
    pub(crate) error: Option<String>,
    pub(crate) plan: Option<Arc<Plan>>,
    pub(crate) prompt: Entity<TextInput>,
    pub(crate) adding_prompt: bool,
    pub(crate) review: Option<AiReview>,
    pub(crate) subscriptions: Vec<Subscription>,
    pub(crate) drafting: Option<Cancellation>,
}

impl Drop for AiSheet {
    /// However the sheet goes away, its provider stops drafting.
    fn drop(&mut self) {
        if let Some(drafting) = &self.drafting {
            drafting.cancel();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AiStep {
    Loading,
    Confirm,
    Drafting,
    Review,
    Applying,
}

pub(crate) struct AiReview {
    /// The commit message, or the pull request title.
    pub(crate) message: Entity<TextInput>,
    pub(crate) summary: Option<Entity<TextInput>>,
    pub(crate) branch: Option<Entity<TextInput>>,
    pub(crate) target: String,
    pub(crate) chooser: Option<Entity<Picker>>,
}

impl AiReview {
    pub(crate) fn draft(&self, plan: &Plan, cx: &App) -> Draft {
        let text = |input: &Entity<TextInput>| input.read(cx).text().trim().to_owned();
        match plan.action {
            Action::Commit => Draft::Commit {
                message: text(&self.message),
            },
            Action::CreatePullRequest => Draft::PullRequest {
                title: text(&self.message),
                summary: self.summary.as_ref().map(text).unwrap_or_default(),
                branch: self
                    .branch
                    .as_ref()
                    .map_or_else(|| plan.branch.clone(), text),
                target: self.target.clone(),
            },
        }
    }

    pub(crate) fn head(&self, plan: &Plan, cx: &App) -> String {
        self.branch.as_ref().map_or_else(
            || plan.branch.clone(),
            |branch| branch.read(cx).text().trim().to_owned(),
        )
    }
}

impl AppModel {
    pub(crate) fn open_ai_provider_menu(
        &mut self,
        action: Action,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let project = self.state.current_project().id;
        if self.ai.running(project) {
            return;
        }
        self.ai.refresh_installed();
        let configured = self
            .settings
            .ai
            .providers
            .get(action.key())
            .map_or("", String::as_str);
        let automatic = crate::ai::selected(&self.ai.installed, None).map_or_else(
            || "Auto".to_owned(),
            |provider| format!("Auto · {}", provider.name),
        );
        let mut items = vec![
            Item::action(automatic, Command::AiProvider(action, ""))
                .checked_if(configured.is_empty()),
        ];
        for provider in PROVIDERS {
            let installed = self.ai.installed.contains(provider);
            let label = if installed {
                provider.name.to_owned()
            } else {
                format!("{} · Not installed", provider.name)
            };
            let mut item = Item::action(label, Command::AiProvider(action, provider.id))
                .checked_if(configured == provider.id);
            if !installed {
                item = item.disabled();
            }
            items.push(item);
        }
        self.open_menu(items, position, window, cx);
    }
}

fn description(sheet: &AiSheet, plan: Option<&Plan>) -> String {
    let provider = sheet.provider.name;
    let Some(plan) = plan else {
        return "Reading the changes…".into();
    };
    if matches!(sheet.step, AiStep::Review | AiStep::Applying) {
        return match plan.action {
            Action::Commit => "Edit the message before Muxy commits these changes.".into(),
            Action::CreatePullRequest => "Edit anything before Muxy opens the pull request.".into(),
        };
    }
    let destination = plan
        .preview
        .destination
        .as_ref()
        .map(|destination| format!("{}/{}", destination.remote, destination.branch));
    match (plan.mode, plan.has_changes()) {
        (Mode::Commit, _) => match destination {
            Some(destination) => format!(
                "{provider} drafts a commit message. You review it before Muxy commits the changes below and pushes them to {destination}."
            ),
            None => format!(
                "{provider} drafts a commit message. You review it before Muxy commits the changes below. No remote is configured, so the commit stays local."
            ),
        },
        (Mode::NewBranch, _) => format!(
            "{provider} drafts a branch name, target, title, and summary. You review them before Muxy creates the branch, commits the changes below, pushes it, and opens the pull request."
        ),
        (Mode::CurrentBranch, true) => format!(
            "{provider} drafts a title and summary. You review them before Muxy commits the changes below to {}, pushes it, and opens the pull request.",
            plan.branch
        ),
        (Mode::CurrentBranch, false) => format!(
            "{provider} drafts a title and summary from {}'s commits. You review them before Muxy pushes the branch and opens the pull request.",
            plan.branch
        ),
    }
}

fn title(sheet: &AiSheet, plan: Option<&Plan>) -> String {
    match (sheet.action, sheet.step) {
        (Action::Commit, AiStep::Review | AiStep::Applying) => "Review commit".into(),
        (Action::CreatePullRequest, AiStep::Review | AiStep::Applying) => {
            "Review pull request".into()
        }
        (Action::Commit, _) if plan.is_some_and(|plan| plan.preview.destination.is_none()) => {
            format!("Commit to “{}”?", sheet.branch)
        }
        (Action::Commit, _) => format!("Commit and push to “{}”?", sheet.branch),
        (Action::CreatePullRequest, _) => {
            format!("Create a pull request from “{}”?", sheet.branch)
        }
    }
}

fn primary_label(sheet: &AiSheet, plan: Option<&Plan>) -> String {
    match sheet.step {
        AiStep::Loading | AiStep::Confirm => format!("Draft with {}", sheet.provider.name),
        AiStep::Drafting => "Drafting…".into(),
        AiStep::Applying => sheet.action.running_title().into(),
        AiStep::Review => match sheet.action {
            Action::Commit if plan.is_some_and(|plan| plan.preview.destination.is_none()) => {
                "Commit".into()
            }
            Action::Commit => "Commit and Push".into(),
            Action::CreatePullRequest => "Create Pull Request".into(),
        },
    }
}

fn label(text: &str, model: &AppModel) -> AnyElement {
    div()
        .text_size(model.metrics.font_footnote())
        .text_color(model.theme.fg_muted)
        .child(text.to_owned())
        .into_any_element()
}

fn files(plan: &Plan, model: &AppModel) -> AnyElement {
    let theme = &model.theme;
    let m = model.metrics;
    let untracked = plan
        .preview
        .files
        .iter()
        .filter(|file| file.untracked)
        .count();
    let heading = match (plan.preview.files.len(), untracked) {
        (0, _) => format!(
            "No uncommitted changes · {}'s commits are used",
            plan.branch
        ),
        (1, 0) => "1 file".into(),
        (1, _) => "1 file · untracked".into(),
        (count, 0) => format!("{count} files"),
        (count, untracked) => format!("{count} files · {untracked} untracked"),
    };
    let hidden = plan.preview.files.len().saturating_sub(LISTED_FILES);
    let rows = plan.preview.files.iter().take(LISTED_FILES).map(|file| {
        let path = String::from_utf8_lossy(&file.path.0).into_owned();
        let counts = match (file.added, file.removed) {
            (Some(added), Some(removed)) => format!("+{added} −{removed}"),
            _ => "binary".into(),
        };
        div()
            .flex()
            .items_center()
            .gap(m.spacing3())
            .text_size(m.font_footnote())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .text_color(theme.fg)
                    .child(path),
            )
            .when(file.untracked, |row| {
                row.child(div().text_color(theme.warning).child("untracked"))
            })
            .child(div().text_color(theme.fg_muted).child(counts))
    });
    div()
        .flex()
        .flex_col()
        .gap(m.spacing2())
        .child(label(&heading, model))
        .when(!plan.preview.files.is_empty(), |list| {
            list.child(
                div()
                    .id("ai-sheet-files")
                    .debug_selector(|| "ai-sheet-files".into())
                    .flex()
                    .flex_col()
                    .gap(m.spacing1())
                    .max_h(m.scaled(132.0))
                    .overflow_y_scroll()
                    .p(m.spacing3())
                    .rounded(m.radius_sm())
                    .bg(theme.surface)
                    .border_1()
                    .border_color(theme.border)
                    .children(rows)
                    .when(hidden > 0, |list| {
                        list.child(
                            div()
                                .text_size(m.font_footnote())
                                .text_color(theme.fg_muted)
                                .child(format!("and {hidden} more")),
                        )
                    }),
            )
        })
        .into_any_element()
}

fn field(title: &str, control: AnyElement, model: &AppModel) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(model.metrics.spacing2())
        .child(label(title, model))
        .child(control)
        .into_any_element()
}

fn review(
    review: &AiReview,
    plan: &Plan,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let style = Style {
        theme: &model.theme,
        metrics: &m,
    };
    let mut fields = div().flex().flex_col().gap(m.spacing5());
    match plan.action {
        Action::Commit => {
            fields = fields.child(field(
                "Message",
                controls::text_area(style, "ai-message", &review.message, Some(96.0)),
                model,
            ));
        }
        Action::CreatePullRequest => {
            fields = fields.child(field(
                "Title",
                controls::text_field(style, "ai-title", &review.message, None),
                model,
            ));
            if let Some(summary) = &review.summary {
                fields = fields.child(field(
                    "Summary",
                    controls::text_area(style, "ai-summary", summary, Some(120.0)),
                    model,
                ));
            }
            fields = fields.child(field(
                "Branch",
                match &review.branch {
                    Some(branch) => controls::text_field(style, "ai-branch", branch, None),
                    None => div()
                        .text_size(m.font_footnote())
                        .text_color(model.theme.fg)
                        .child(format!("{} (current branch)", plan.branch))
                        .into_any_element(),
                },
                model,
            ));
            let trigger = controls::picker_trigger(
                style,
                "ai-target",
                &review.target,
                None,
                review.chooser.is_some(),
                cx.listener(|model, _, window, cx| model.choose_ai_target(window, cx)),
            );
            let mut target = div().relative().child(field("Target", trigger, model));
            if let Some(chooser) = &review.chooser {
                target = target.child(
                    deferred(
                        anchored()
                            .anchor(Corner::TopLeft)
                            .offset(point(px(0.0), m.control_medium() + m.spacing8()))
                            .snap_to_window_with_margin(px(8.0))
                            .child(chooser.clone()),
                    )
                    .with_priority(2),
                );
            }
            fields = fields.child(target);
        }
    }
    let head = review.head(plan, cx);
    let destination = plan.destination(&head).map_or_else(
        || "Nowhere — no remote is configured, so the commit stays local".to_owned(),
        |destination| format!("{}/{}", destination.remote, destination.branch),
    );
    fields
        .child(field(
            "Push to",
            div()
                .text_size(m.font_footnote())
                .text_color(model.theme.fg)
                .child(destination)
                .into_any_element(),
            model,
        ))
        .into_any_element()
}

fn primary(
    label: String,
    enabled: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let m = model.metrics;
    div()
        .id("ai-sheet-primary")
        .debug_selector(|| "ai-sheet-primary".into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(m.control_medium())
        .px(m.spacing5())
        .rounded(m.radius_sm())
        .bg(theme.accent)
        .text_color(theme.accent_foreground)
        .text_size(m.font_footnote())
        .font_weight(FontWeight::MEDIUM)
        .when(!enabled, |button| button.opacity(0.4))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.opacity(0.85))
                .button_interaction(cx.listener(|model, _, _, cx| model.advance_ai_action(cx)))
        })
        .child(label)
        .into_any_element()
}

#[allow(
    clippy::too_many_lines,
    reason = "Declarative AI review sheet across its steps"
)]
pub(crate) fn render_sheet(
    sheet: &AiSheet,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let m = model.metrics;
    let style = Style { theme, metrics: &m };
    let plan = sheet.plan.as_deref();
    let mut view = muxy_ui::popover::surface(theme, m)
        .id("ai-sheet")
        .debug_selector(|| "ai-sheet".into())
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .on_action(
            cx.listener(|model, _: &crate::views::menu::DismissMenu, _, cx| {
                model.dismiss_overlay(cx);
            }),
        )
        .w(m.scaled(480.0)
            .min((window.viewport_size().width - px(16.0)).max(px(0.0))))
        .max_h((window.viewport_size().height - m.scaled(80.0)).max(px(0.0)))
        .overflow_y_scroll()
        .p(m.spacing8())
        .gap(m.scaled(14.0))
        .text_color(theme.fg)
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .text_size(m.font_headline())
                .font_weight(FontWeight::SEMIBOLD)
                .child(title(sheet, plan)),
        )
        .child(
            div()
                .text_size(m.font_body())
                .text_color(theme.fg_muted)
                .child(description(sheet, plan)),
        );
    if let Some(plan) = plan {
        view = view.child(files(plan, model));
    }
    if matches!(sheet.step, AiStep::Confirm | AiStep::Drafting) && sheet.adding_prompt {
        let count = sheet.prompt.read(cx).text().chars().count();
        view = view.child(
            div()
                .flex()
                .flex_col()
                .gap(m.spacing2())
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .child(label("Additional prompt", model))
                        .child(label(&format!("{count}/{ADDITIONAL_PROMPT_LIMIT}"), model)),
                )
                .child(controls::text_area(
                    style,
                    "ai-prompt",
                    &sheet.prompt,
                    Some(96.0),
                ))
                .child(label(
                    "Added after the configured prompt for this run only.",
                    model,
                )),
        );
    }
    if let (Some(review_state), Some(plan)) = (&sheet.review, plan)
        && matches!(sheet.step, AiStep::Review | AiStep::Applying)
    {
        view = view.child(review(review_state, plan, model, cx));
    }
    if let Some(error) = &sheet.error {
        view = view.child(
            div()
                .debug_selector(|| "ai-sheet-error".into())
                .text_size(m.font_footnote())
                .text_color(theme.danger)
                .child(error.clone()),
        );
    }
    let enabled = match sheet.step {
        AiStep::Confirm => plan.is_some(),
        AiStep::Review => true,
        AiStep::Loading | AiStep::Drafting | AiStep::Applying => false,
    };
    view.child(
        div()
            .flex()
            .items_center()
            .gap(m.spacing3())
            .pt(m.spacing3())
            .when(
                sheet.step == AiStep::Confirm && !sheet.adding_prompt,
                |row| {
                    row.child(controls::button(
                        style,
                        "ai-add-prompt",
                        "Add Prompt",
                        true,
                        cx.listener(|model, _, window, cx| model.toggle_ai_prompt(window, cx)),
                    ))
                },
            )
            .child(div().flex_1())
            .child(
                controls::button(
                    style,
                    "ai-sheet-cancel",
                    "Cancel",
                    sheet.step != AiStep::Applying,
                    cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
                )
                .debug_selector(|| "ai-sheet-cancel".into()),
            )
            .child(primary(primary_label(sheet, plan), enabled, model, cx)),
    )
    .into_any_element()
}
