use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, StatefulInteractiveElement, Styled, canvas, div, px,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::popover::PopoverAnchor;

use super::git::Kind;
use super::menu::{Command, Item};
use crate::model::AppModel;

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
                .children(model.update_status().map(|status| {
                    div()
                        .id("beta-update-status")
                        .flex_none()
                        .text_size(m.font_footnote())
                        .text_color(theme.fg_muted)
                        .cursor_pointer()
                        .child(status)
                        .on_click(cx.listener(|model, _, _, cx| model.show_update_status(cx)))
                }))
                .children(super::disconnected::status(model, cx).map(|status| {
                    div()
                        .debug_selector(|| "project-connection-status".into())
                        .flex_none()
                        .child(status)
                })),
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
    let enabled = model.session_listing_ready() && repository.is_some_and(|r| !r.busy());
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
    vec![
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
    ]
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
        Kind::Worktrees => unreachable!(),
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
