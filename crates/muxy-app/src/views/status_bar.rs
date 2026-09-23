use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, FontWeight, Hsla, InteractiveElement, IntoElement,
    MouseButton, ParentElement, StatefulInteractiveElement, Styled, canvas, div, px,
};
use muxy_ui::components::{ButtonInteraction, IconGlyph, Tooltip};
use muxy_ui::icon::Icon;
use muxy_ui::popover::PopoverAnchor;

use super::git::Kind;
use super::menu::{Command, Item};
use crate::model::{AppModel, ai::Availability, git::Presence};
use crate::repository_actions::Action as AiAction;

pub(crate) fn status_bar(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .debug_selector(|| "project-status-bar".into())
        .flex()
        .flex_none()
        .items_center()
        .h(m.status_bar_height())
        .bg(theme.bg)
        .border_t_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .flex_grow()
                .min_w(px(0.0))
                .items_center()
                .gap(px(8.0))
                .h_full()
                .px(px(10.0))
                .child(path_chip(model, cx))
                .children(git_controls(model, cx)),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.0))
                .h_full()
                .px(px(10.0))
                .children(update_control(model, cx))
                .children(super::disconnected::status(model, cx))
                .child(super::server_status::control(model, cx)),
        )
}

fn update_control(model: &AppModel, cx: &mut Context<AppModel>) -> Option<AnyElement> {
    let details = model.update_details();
    if details.is_none() && !matches!(model.overlay, Some(super::overlays::Overlay::Updates)) {
        model.update_anchor().set(None);
        return None;
    }
    let m = model.metrics;
    let theme = &model.theme;
    let color = if details.as_ref().is_some_and(|details| details.failed) {
        theme.warning
    } else {
        theme.accent
    };
    let label = details.map_or_else(|| "Updates".into(), |details| details.label);
    let anchor = model.update_anchor();
    Some(
        div()
            .id("beta-update-status")
            .debug_selector(|| "beta-update-status".into())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h_full()
            .gap(px(6.0))
            .pl(px(8.0))
            .border_l_1()
            .border_color(theme.border)
            .text_color(theme.fg_muted)
            .cursor_pointer()
            .hover(|style| style.text_color(theme.fg))
            .focus(|style| style.bg(theme.hover))
            .button_interaction(
                cx.listener(|model, _, window, cx| model.toggle_update_popover(window, cx)),
            )
            .child(
                canvas(
                    move |bounds, _, _| anchor.set(Some(bounds)),
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .size(m.scaled(5.0))
                    .flex_none()
                    .rounded_full()
                    .bg(color),
            )
            .child(
                div()
                    .text_size(m.font_footnote())
                    .font_weight(FontWeight::MEDIUM)
                    .child(label),
            )
            .into_any_element(),
    )
}

fn path_chip(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let project = model.state.current_project();
    let id = project.id;
    let available = project.status() == muxy_app_core::ProjectStatus::Available;
    let display = super::project_picker::display_path(&project.directory);
    let count = display.chars().count();
    let display = if count > 40 {
        format!("…{}", display.chars().skip(count - 39).collect::<String>())
    } else {
        display
    };
    div()
        .id("status-path")
        .debug_selector(|| "status-path".into())
        .flex()
        .flex_initial()
        .min_w(px(0.0))
        .items_center()
        .gap(px(4.0))
        .h_full()
        .text_color(model.theme.fg_muted)
        .when(available, |path| {
            path.cursor_pointer()
                .on_click(cx.listener(move |model, _, _, cx| {
                    if let Some(project) = model.state.project(id) {
                        cx.reveal_path(&project.directory);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                        model.open_menu(
                            vec![
                                Item::action("Copy Path", Command::CopyPath(id)),
                                Item::action("Reveal in Finder", Command::RevealPath(id)),
                            ],
                            event.position,
                            window,
                            cx,
                        );
                    }),
                )
        })
        .child(IconGlyph::new(
            Icon::Folder,
            model.metrics.font_caption(),
            model.theme.fg_muted,
        ))
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_size(model.metrics.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .child(display),
        )
}

fn separator(model: &AppModel) -> AnyElement {
    div()
        .debug_selector(|| "git-status-separator".into())
        .w(px(1.0))
        .h_full()
        .flex_none()
        .bg(model.theme.border)
        .into_any_element()
}

fn git_controls(model: &AppModel, cx: &mut Context<AppModel>) -> Vec<AnyElement> {
    let project = model.state.current_project().id;
    let repository = model.git.projects.get(&project);
    let Some(summary) = repository.and_then(|r| r.summary.as_ref()) else {
        return vec![];
    };
    let enabled = model.session_listing_ready()
        && repository.is_some_and(|r| !r.busy())
        && !model.ai.running(project);
    let branch = summary.branch.clone().unwrap_or_else(|| {
        summary.head.as_ref().map_or_else(
            || "Unborn HEAD".into(),
            |h| format!("Detached {}", &h[..h.len().min(8)]),
        )
    });
    let changes = match summary.changed {
        0 => "Clean".into(),
        1 => "1 Change".into(),
        count => format!("{count} Changes"),
    };
    let color = if summary.conflicted > 0 {
        model.theme.danger
    } else if summary.changed > 0 {
        model.theme.warning
    } else {
        model.theme.accent
    };
    let mut controls = vec![
        separator(model),
        repository_chip(
            Kind::Branches,
            branch,
            model.theme.fg_muted,
            enabled,
            model,
            cx,
        ),
        separator(model),
        repository_chip(Kind::Changes, changes, color, enabled, model, cx),
    ];
    if let Some(chip) = ai_action_chip(AiAction::Commit, model, cx) {
        controls.push(separator(model));
        controls.push(chip);
    }
    match repository.map_or(
        Presence::Loading,
        crate::model::git::Repository::pull_request_presence,
    ) {
        Presence::Loading => (),
        Presence::Found => {
            if let Some(pr) = repository.and_then(|r| r.pull_request.as_ref()) {
                controls.push(separator(model));
                controls.push(pull_request_chip(
                    pr.number, &pr.state, &pr.checks, enabled, model, cx,
                ));
            }
        }
        Presence::Unavailable => {
            controls.push(separator(model));
            controls.push(pull_request_unavailable(model, cx));
        }
        Presence::None => {
            if let Some(chip) = ai_action_chip(AiAction::CreatePullRequest, model, cx) {
                controls.push(separator(model));
                controls.push(chip);
            }
        }
    }
    controls
}

fn tooltip(
    text: String,
    model: &AppModel,
) -> impl Fn(&mut gpui::Window, &mut gpui::App) -> gpui::AnyView + 'static {
    let theme = model.theme.clone();
    move |_, cx| {
        cx.new(|_| Tooltip::new(text.clone(), theme.raised(), theme.fg, theme.border))
            .into()
    }
}

fn ai_action_chip(
    action: AiAction,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> Option<AnyElement> {
    let project = model.state.current_project().id;
    let running = model.ai.running_action(project) == Some(action);
    let (can_run, help) = match model.ai_availability(action) {
        Availability::Hidden => return None,
        Availability::Available(provider) => (
            true,
            format!("{} with {}", action.settings_title(), provider.name),
        ),
        Availability::Disabled(reason) => (false, reason),
    };
    let label = if running {
        action.running_title()
    } else {
        action.title()
    };
    let id = match action {
        AiAction::Commit => "ai-commit-status",
        AiAction::CreatePullRequest => "ai-create-pr-status",
    };
    let color = if can_run || running {
        model.theme.fg_muted
    } else {
        model.theme.fg_dim
    };
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .h_full()
            .child(
                div()
                    .id(id)
                    .debug_selector(move || id.into())
                    .flex()
                    .items_center()
                    .h_full()
                    .gap(px(4.0))
                    .px(px(4.0))
                    .text_color(color)
                    .tooltip(tooltip(help, model))
                    .when(can_run, |button| {
                        button
                            .cursor_pointer()
                            .hover(|style| style.text_color(model.theme.fg))
                            .button_interaction(cx.listener(move |model, _, window, cx| {
                                model.open_ai_action(action, window, cx);
                            }))
                    })
                    .child(IconGlyph::new(
                        Icon::ArrowUp,
                        model.metrics.font_caption(),
                        color,
                    ))
                    .child(
                        div()
                            .text_size(model.metrics.font_footnote())
                            .font_weight(FontWeight::MEDIUM)
                            .child(label),
                    ),
            )
            .child(provider_menu(action, color, running, model, cx))
            .into_any_element(),
    )
}

fn provider_menu(
    action: AiAction,
    color: Hsla,
    running: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = match action {
        AiAction::Commit => "ai-commit-provider",
        AiAction::CreatePullRequest => "ai-create-pr-provider",
    };
    let project = model.state.current_project().id;
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .items_center()
        .h_full()
        .px(px(2.0))
        .text_color(if running {
            model.theme.fg_dim
        } else {
            model.theme.fg_muted
        })
        .tooltip(tooltip(
            format!("Choose the AI provider for {}", action.title()),
            model,
        ))
        .when(!model.ai.running(project), |button| {
            button
                .cursor_pointer()
                .hover(|style| style.text_color(model.theme.fg))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                        model.open_ai_provider_menu(action, event.position, window, cx);
                    }),
                )
        })
        .child(IconGlyph::new(
            Icon::ChevronDown,
            model.metrics.font_caption(),
            color,
        ))
        .into_any_element()
}

fn pull_request_chip(
    number: u64,
    state: &str,
    checks: &muxy_protocol::GitChecks,
    enabled: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let anchor = model.git.pull_request_anchor.clone();
    let project = model.state.current_project().id;
    let color = if state == "OPEN" {
        if checks.failing > 0 {
            model.theme.danger
        } else if checks.pending > 0 {
            model.theme.warning
        } else {
            model.theme.accent
        }
    } else {
        model.theme.fg_muted
    };
    let checks_label = if checks.failing > 0 {
        Some(format!("{} failing", checks.failing))
    } else if checks.pending > 0 {
        Some(format!("{} running", checks.pending))
    } else if checks.passing > 0 {
        Some(format!("{} passed", checks.passing))
    } else {
        None
    };
    div()
        .id("git-pr-status")
        .debug_selector(|| "git-pr-status".into())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .h_full()
        .text_color(color)
        .when(enabled, |chip| {
            chip.cursor_pointer()
                .hover(|style| style.text_color(model.theme.fg))
                .button_interaction(
                    cx.listener(move |model, _, _, cx| model.open_pull_request(project, cx)),
                )
        })
        .child(
            canvas(
                move |bounds, _, _| anchor.set(Some(bounds)),
                |_, (), _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(IconGlyph::new(
            Icon::GitBranch,
            model.metrics.font_caption(),
            color,
        ))
        .child(
            div()
                .text_size(model.metrics.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .child(format!("PR #{number}")),
        )
        .children(checks_label.map(|label| {
            div()
                .text_size(model.metrics.font_caption())
                .child(format!("· {label}"))
        }))
        .into_any_element()
}

fn pull_request_unavailable(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let project = model.state.current_project().id;
    div()
        .id("git-pr-unavailable")
        .debug_selector(|| "git-pr-unavailable".into())
        .flex()
        .flex_none()
        .items_center()
        .h_full()
        .px(px(2.0))
        .cursor_pointer()
        .hover(|style| style.text_color(model.theme.fg))
        .tooltip(tooltip(
            "Pull request status is unavailable. Click to retry. GitHub pull requests require an installed and authenticated gh CLI.".into(),
            model,
        ))
        .button_interaction(
            cx.listener(move |model, _, _, cx| model.refresh_pull_request(project, cx)),
        )
        .child(IconGlyph::new(
            Icon::GitBranch,
            model.metrics.font_caption(),
            model.theme.fg_dim,
        ))
        .into_any_element()
}

fn repository_chip(
    kind: Kind,
    label: String,
    color: Hsla,
    enabled: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let (id, icon, anchor): (_, _, PopoverAnchor) = match kind {
        Kind::Branches => (
            "git-branch-status",
            Icon::GitBranch,
            model.git.branch_anchor.clone(),
        ),
        Kind::Changes => (
            "git-changes-status",
            Icon::ArrowUpDown,
            model.git.changes_anchor.clone(),
        ),
    };
    let project = model.state.current_project().id;
    let color = if enabled { color } else { model.theme.fg_dim };
    div()
        .id(id)
        .debug_selector(move || id.into())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .h_full()
        .text_color(color)
        .when(enabled, |chip| {
            chip.cursor_pointer()
                .hover(|style| style.text_color(model.theme.fg))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |model, _, window, cx| {
                        model.open_git_picker(project, kind, window, cx);
                    }),
                )
        })
        .child(
            canvas(
                move |bounds, _, _| anchor.set(Some(bounds)),
                |_, (), _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(IconGlyph::new(icon, model.metrics.font_caption(), color))
        .child(
            div()
                .text_size(model.metrics.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .child(label),
        )
        .into_any_element()
}
