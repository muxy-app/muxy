use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, div, px,
};
use muxy_app_core::{Project, ProjectId, ProjectStatus};
use muxy_ui::{
    components::{IconButton, IconGlyph},
    icon::Icon,
};

use crate::model::AppModel;

impl AppModel {
    pub(crate) fn worktrees_visible(&self, id: ProjectId) -> bool {
        !self.appearance.hidden_worktrees.contains(&id)
    }

    pub(crate) fn toggle_worktree_visibility(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        if !self.appearance.hidden_worktrees.remove(&id) {
            self.appearance.hidden_worktrees.insert(id);
        }
        self.save_appearance(cx);
        if !self.worktrees_visible(id) {
            self.expanded_worktrees.remove(&id);
            if self.state.current_project().parent_id == Some(id) {
                self.select_project(id, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn has_worktrees(&self, project: &Project) -> bool {
        !project.home
            && project.parent_id.is_none()
            && self.worktrees_visible(project.id)
            && (self
                .state
                .projects()
                .iter()
                .any(|child| child.parent_id == Some(project.id))
                || self
                    .git
                    .projects
                    .get(&project.id)
                    .is_some_and(|repo| repo.summary.is_some()))
    }

    pub(crate) fn worktree_children(&self, parent: ProjectId) -> Vec<&Project> {
        let mut children: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| project.parent_id == Some(parent))
            .collect();
        if self.appearance.worktree_order_by_mru {
            children.sort_by_cached_key(|project| {
                self.appearance
                    .worktree_recent
                    .iter()
                    .position(|id| *id == project.id)
                    .unwrap_or(usize::MAX)
            });
        }
        children
    }

    pub(crate) fn preferred_worktree(&self, parent: ProjectId) -> ProjectId {
        if !self.worktrees_visible(parent) {
            return parent;
        }
        let active = self.state.current_project();
        if (active.id == parent || active.parent_id == Some(parent))
            && active.status() == ProjectStatus::Available
        {
            return active.id;
        }
        self.appearance
            .worktree_recent
            .iter()
            .copied()
            .find(|id| {
                self.state.project(*id).is_some_and(|project| {
                    (project.id == parent || project.parent_id == Some(parent))
                        && project.status() == ProjectStatus::Available
                })
            })
            .unwrap_or(parent)
    }

    pub(crate) fn toggle_worktree_list(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        if !self.expanded_worktrees.remove(&id) {
            self.expanded_worktrees.insert(id);
        }
        cx.notify();
    }
}

pub(super) fn disclosure(
    project: &Project,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = project.id;
    let expanded = model.expanded_worktrees.contains(&id);
    let theme = &model.theme;
    div()
        .debug_selector(move || format!("project-worktrees-toggle-{id}"))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            IconButton::new(
                SharedString::from(format!("project-worktrees-toggle-{id}")),
                if expanded {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                model.metrics.icon_xs(),
                model.metrics.control_small(),
                theme.fg_muted,
                theme.fg,
            )
            .tooltip(
                if expanded {
                    "Collapse Worktrees"
                } else {
                    "Expand Worktrees"
                },
                theme.raised(),
                theme.fg,
                theme.border,
            )
            .on_click(cx.listener(move |model, _, _, cx| {
                cx.stop_propagation();
                model.toggle_worktree_list(id, cx);
            })),
        )
        .into_any_element()
}

pub(super) fn group(
    project: &Project,
    index: usize,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let mut group = div()
        .flex()
        .flex_col()
        .child(super::project_row(project, index, model, cx));
    if model.appearance.sidebar_expanded
        && model.expanded_worktrees.contains(&project.id)
        && model.has_worktrees(project)
        && project.status() == ProjectStatus::Available
    {
        let mut worktrees = div()
            .flex()
            .flex_col()
            .gap(model.metrics.scaled(1.0))
            .pt(model.metrics.spacing1())
            .pb(model.metrics.spacing2())
            .child(row(project, true, model, cx));
        for child in model.worktree_children(project.id) {
            worktrees = worktrees.child(row(child, false, model, cx));
        }
        group = group.child(worktrees.child(new_worktree(project.id, model, cx)));
    }
    group.into_any_element()
}

fn row(
    project: &Project,
    primary: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = project.id;
    let m = model.metrics;
    let theme = &model.theme;
    let active = model.state.current_project().id == id;
    let missing = project.status() == ProjectStatus::Missing;
    let activity = super::super::tab_activity::worktree_status(id, model);
    div()
        .id(SharedString::from(format!("worktree-{id}")))
        .debug_selector(move || format!("worktree-{id}"))
        .flex()
        .items_center()
        .gap(m.spacing4())
        .px(m.spacing2())
        .h(m.control_large())
        .rounded(m.radius_md())
        .text_size(m.font_body())
        .text_color(theme.fg)
        .when(active, |row| row.bg(theme.hover))
        .when(missing, |row| row.opacity(0.5))
        .hover(|style| style.bg(theme.hover))
        .when(!missing, |row| {
            row.cursor_pointer()
                .on_click(cx.listener(move |model, _, window, cx| {
                    model.select_project(id, cx);
                    model.focus_active(window, cx);
                }))
        })
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                if let Some(project) = model.state.project(id) {
                    model.open_menu(
                        super::super::project_menu::worktree_items(project, primary),
                        event.position,
                        window,
                        cx,
                    );
                }
            }),
        )
        .child(
            div()
                .flex_none()
                .w(m.icon_xxl())
                .flex()
                .items_center()
                .justify_center()
                .child(super::super::tab_activity::status_glyph(
                    format!("worktree-activity-{id}"),
                    activity,
                    m.icon_sm(),
                    model,
                )),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .items_center()
                .gap(m.spacing2())
                .child(div().min_w(px(0.0)).truncate().child(project.name.clone()))
                .when(primary, |label| {
                    label.child(
                        div()
                            .flex_none()
                            .px(m.spacing2())
                            .py(m.scaled(1.0))
                            .rounded_full()
                            .bg(theme.surface)
                            .text_size(m.font_micro())
                            .font_weight(FontWeight::BOLD)
                            .child("PRIMARY"),
                    )
                }),
        )
        .into_any_element()
}

pub(crate) fn new_worktree(
    id: ProjectId,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let group = SharedString::from(format!("new-worktree-{id}"));
    div()
        .id(SharedString::from(format!("new-worktree-{id}")))
        .debug_selector(move || format!("new-worktree-{id}"))
        .group(group.clone())
        .flex()
        .items_center()
        .gap(m.spacing4())
        .px(m.spacing2())
        .h(m.control_large())
        .rounded(m.radius_md())
        .text_size(m.font_footnote())
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.fg)
        .hover(|style| style.text_color(theme.accent))
        .cursor_pointer()
        .on_click(cx.listener(move |model, _, _, cx| model.open_git_form(id, true, cx)))
        .child(
            div()
                .w(m.icon_xxl())
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    IconGlyph::new(Icon::Plus, m.font_caption(), theme.fg)
                        .hover_in_group(group, theme.accent),
                ),
        )
        .child("New Worktree")
        .into_any_element()
}
