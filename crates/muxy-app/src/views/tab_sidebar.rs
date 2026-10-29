use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_app_core::{Project, ProjectId, ProjectStatus, Tab, TabId, settings::AppLayout};
use muxy_ui::components::{IconButton, IconGlyph, SymbolGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use super::{tab_activity, tab_strip, titlebar};
use crate::model::AppModel;

impl AppModel {
    pub(crate) fn set_layout(&mut self, layout: AppLayout, cx: &mut Context<Self>) {
        if self.appearance.layout == layout {
            return;
        }
        self.cancel_titlebar_drag(cx);
        self.appearance.layout = layout;
        self.tab_sidebar_selection = None;
        self.save_appearance(cx);
        self.dismiss_overlay(cx);
        cx.notify();
    }

    pub(super) fn project_expanded(&self, id: ProjectId) -> bool {
        self.appearance
            .tab_focused_expanded
            .get(&id)
            .copied()
            .unwrap_or_else(|| {
                let active = self.state.current_project();
                active.id == id
            })
    }

    fn toggle_project_tabs(&mut self, id: ProjectId, cx: &mut Context<Self>) {
        let expanded = !self.project_expanded(id);
        self.appearance.tab_focused_expanded.insert(id, expanded);
        self.cancel_titlebar_drag(cx);
        self.save_appearance(cx);
        cx.notify();
    }

    pub(crate) fn sync_tab_sidebar(&mut self, cx: &mut Context<Self>) {
        let selected = (self.state.current_project().id, self.active_tab());
        let previous = self.tab_sidebar_selection.replace(selected);
        if previous == Some(selected) {
            return;
        }
        let project = self.state.current_project();
        let id = project.id;
        let parent = project.parent_id.unwrap_or(id);
        let switched = previous.is_none_or(|previous| previous.0 != id);
        let mut changed = false;
        if switched {
            if self.appearance.project_recent.is_empty() {
                let parents: std::collections::HashMap<_, _> = self
                    .state
                    .projects()
                    .iter()
                    .map(|project| (project.id, project.parent_id.unwrap_or(project.id)))
                    .collect();
                let mut seen = std::collections::HashSet::new();
                self.appearance.project_recent = self
                    .appearance
                    .worktree_recent
                    .iter()
                    .filter_map(|id| parents.get(id).copied())
                    .filter(|id| seen.insert(*id))
                    .collect();
            }
            self.appearance.project_recent.retain(|candidate| {
                *candidate != parent && self.state.project(*candidate).is_some()
            });
            self.appearance.project_recent.insert(0, parent);
            self.appearance
                .worktree_recent
                .retain(|candidate| *candidate != id && self.state.project(*candidate).is_some());
            self.appearance.worktree_recent.insert(0, id);
            self.expanded_worktrees
                .retain(|id| self.state.project(*id).is_some());
            if self.appearance.auto_expand_worktrees && self.worktrees_visible(parent) {
                self.expanded_worktrees.insert(parent);
            }
            changed = true;
        }
        if self.appearance.layout != AppLayout::ProjectFocused
            && (!self.appearance.tab_focused_expanded.contains_key(&id)
                || (previous.is_some() && !self.project_expanded(id)))
        {
            self.appearance.tab_focused_expanded.insert(id, true);
            changed = true;
        }
        if changed {
            self.save_appearance(cx);
        }
    }

    pub(crate) fn sidebar_tabs<'a>(
        &'a self,
        project: &'a Project,
    ) -> impl Iterator<Item = &'a Tab> {
        project.tabs.iter().filter(|tab| {
            self.appearance.layout != AppLayout::AgentsFocused || self.agent_tab_pane(tab).is_some()
        })
    }

    /// The projects the sidebar lists, and how many of them have no tabs; those
    /// are left out until shown. The current project is always listed.
    pub(crate) fn tab_sidebar_projects(&self) -> (Vec<&Project>, usize) {
        let current = self.state.current_project().id;
        let listed = |project: &&Project| {
            self.appearance.sidebar_focus
                || project.id == current
                || self.sidebar_tabs(project).next().is_some()
        };
        let mut projects = self.sidebar_projects();
        let without_tabs = projects.iter().filter(|project| !listed(project)).count();
        if !self.projects_without_tabs_shown {
            projects.retain(listed);
        }
        (projects, without_tabs)
    }

    pub(crate) fn toggle_projects_without_tabs(&mut self, cx: &mut Context<Self>) {
        self.projects_without_tabs_shown = !self.projects_without_tabs_shown;
        if self.appearance.remember_projects_without_tabs {
            self.appearance.projects_without_tabs_shown = self.projects_without_tabs_shown;
            self.save_appearance(cx);
        }
        cx.notify();
    }

    pub(crate) fn navigation_tabs(&self) -> Vec<TabId> {
        let project = self.state.current_project();
        if project.groups().is_some()
            && let Some(tab) = self.active_tab()
        {
            return project.group_tabs(tab);
        }
        if self.appearance.layout != AppLayout::TabFocused {
            return self
                .state
                .current_project()
                .tabs
                .iter()
                .map(|tab| tab.id)
                .collect();
        }
        self.sidebar_projects()
            .into_iter()
            .filter(|project| {
                project.status() == ProjectStatus::Available && self.project_expanded(project.id)
            })
            .flat_map(|project| project.tabs.iter().map(|tab| tab.id))
            .collect()
    }
}

pub(super) fn contents(
    model: &AppModel,
    _window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let targets = tab_strip::drag::TabBounds::default();
    let numbers = if model.appearance.layout == AppLayout::TabFocused {
        model.navigation_tabs()
    } else {
        Vec::new()
    };
    let mut rows = div()
        .flex()
        .flex_col()
        .pt(model.metrics.spacing5())
        .pb(model.metrics.spacing3());
    let (projects, without_tabs) = model.tab_sidebar_projects();
    for project in projects {
        rows = rows.child(project_group(project, &numbers, targets.clone(), model, cx));
    }
    if without_tabs > 0 {
        rows = rows.child(projects_without_tabs_toggle(without_tabs, model, cx));
    }
    div()
        .debug_selector(|| "tab-sidebar".into())
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.0))
        .child(
            div()
                .id("tab-sidebar-scroll")
                .debug_selector(|| "tab-sidebar-scroll".into())
                .flex_1()
                .min_h(px(0.0))
                .overflow_y_scroll()
                .child(rows),
        )
        .child(tab_strip::drag::track_pointer(
            targets,
            tab_strip::drag::Source::Sidebar,
            cx,
        ))
        .into_any_element()
}

fn projects_without_tabs_toggle(
    count: usize,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let label = if model.projects_without_tabs_shown {
        tr!("Hide projects without tabs")
    } else if count == 1 {
        tr!("1 more project")
    } else {
        tr!("%lld more projects", count)
    };
    div()
        .my(m.spacing1())
        .h(row_height(model))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .child(
            div()
                .id("tab-sidebar-projects-without-tabs")
                .debug_selector(|| "tab-sidebar-projects-without-tabs".into())
                .cursor_pointer()
                .text_size(m.font_footnote())
                .text_color(theme.fg_dim)
                .hover(|style| style.text_color(theme.fg_muted))
                .on_click(cx.listener(|model, _, window, cx| {
                    model.toggle_projects_without_tabs(cx);
                    model.focus_active(window, cx);
                }))
                .child(label),
        )
        .into_any_element()
}

fn project_group(
    project: &Project,
    numbers: &[TabId],
    targets: tab_strip::drag::TabBounds,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = project.id;
    let missing = project.status() == ProjectStatus::Missing;
    let expanded = model.project_expanded(id);
    let mut group = div()
        .flex()
        .flex_col()
        .child(project_header(project, model, cx));
    if expanded && !missing {
        let tabs: Vec<_> = model.sidebar_tabs(project).collect();
        let measured: Vec<_> = tabs.iter().map(|tab| tab.id).collect();
        group = group.child(
            div()
                .flex()
                .flex_col()
                .children(tabs.into_iter().map(|tab| {
                    let number = numbers
                        .iter()
                        .position(|id| *id == tab.id)
                        .filter(|index| *index < 9);
                    tab_row(project.id, tab, number, model, cx)
                }))
                .on_children_prepainted(move |bounds, window, _| {
                    let clip = window.content_mask().bounds;
                    let mut targets = targets.borrow_mut();
                    targets.retain(|(id, _)| !measured.contains(id));
                    targets.extend(
                        measured
                            .iter()
                            .zip(bounds)
                            .map(|(id, bounds)| (*id, bounds.intersect(&clip))),
                    );
                }),
        );
    }
    group.into_any_element()
}

fn project_header(project: &Project, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let id = project.id;
    let m = model.metrics;
    let missing = project.status() == ProjectStatus::Missing;
    div()
        .id(SharedString::from(format!("tab-project-{id}")))
        .debug_selector(move || format!("tab-project-{id}"))
        .mx(m.spacing3())
        .my(m.spacing1())
        .pl(m.spacing4())
        .h(m.control_medium())
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .when(missing, |row| row.opacity(0.5))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                if let Some(project) = model.state.project(id) {
                    model.open_menu(model.project_menu(project), event.position, window, cx);
                }
            }),
        )
        .child(
            group_label(model.project_title(project), model)
                .debug_selector(move || format!("tab-project-title-{id}")),
        )
        .child(project_accessory(project, model))
        .when(!missing, |row| {
            row.child(new_tab_button(id, model, cx))
                .child(project_disclosure(project, model, cx))
        })
        .into_any_element()
}

/// The height of a tab row, shared with the agents panel's rows.
pub(super) fn row_height(model: &AppModel) -> gpui::Pixels {
    model.metrics.control_small() + model.metrics.spacing2() * 2.0
}

/// The dim name above a project's rows, shared with the agents panel.
pub(super) fn group_label(text: impl Into<SharedString>, model: &AppModel) -> gpui::Div {
    div()
        .flex_1()
        .min_w(px(0.0))
        .truncate()
        .text_size(model.metrics.font_footnote())
        .text_color(model.theme.fg_dim)
        .child(text.into())
}

fn header_button(id: String, icon: Icon, model: &AppModel) -> IconButton {
    let theme = &model.theme;
    IconButton::new(
        SharedString::from(id),
        icon,
        model.metrics.icon_xs(),
        model.metrics.control_small(),
        theme.fg_dim,
        theme.fg_muted,
    )
}

fn new_tab_button(id: ProjectId, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let theme = &model.theme;
    div()
        .debug_selector(move || format!("project-new-tab-{id}"))
        .flex_none()
        .child(
            header_button(format!("project-new-tab-{id}"), Icon::Plus, model)
                .tooltip(
                    tr!("New Terminal Tab"),
                    theme.raised(),
                    theme.fg,
                    theme.border,
                    theme.bg,
                )
                .on_click(cx.listener(move |model, _, window, cx| {
                    cx.stop_propagation();
                    model.select_project(id, cx);
                    model.new_tab(cx);
                    model.focus_active(window, cx);
                })),
        )
        .into_any_element()
}

fn project_disclosure(
    project: &Project,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = project.id;
    let expanded = model.project_expanded(id);
    let theme = &model.theme;
    div()
        .debug_selector(move || format!("tab-project-toggle-{id}"))
        .flex_none()
        .child(
            header_button(
                format!("tab-project-toggle-{id}"),
                if expanded {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                model,
            )
            .tooltip(
                if expanded {
                    tr!("Collapse Project")
                } else {
                    tr!("Expand Project")
                },
                theme.raised(),
                theme.fg,
                theme.border,
                theme.bg,
            )
            .on_click(cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                model.toggle_project_tabs(id, cx);
                model.focus_active(window, cx);
            })),
        )
        .into_any_element()
}

fn project_accessory(project: &Project, model: &AppModel) -> AnyElement {
    let id = project.id;
    let m = model.metrics;
    let activity = tab_activity::project_status(id, model);
    let has_activity = activity != tab_activity::Status::None;
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .children(super::sidebar::remote_marker(project, model))
        .when(project.parent_id.is_some() && !has_activity, |row| {
            row.child(IconGlyph::new(
                Icon::GitBranch,
                m.icon_xs(),
                model.theme.fg_dim,
            ))
        })
        .when(has_activity, |row| {
            row.child(tab_activity::status_glyph(
                format!("project-activity-{id}"),
                activity,
                m.icon_sm(),
                model,
            ))
        })
        .into_any_element()
}

#[allow(
    clippy::too_many_lines,
    reason = "Declarative sidebar tab row and interaction layout"
)]
fn tab_row(
    project: ProjectId,
    tab: &Tab,
    number: Option<usize>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = tab.id;
    let m = model.metrics;
    let theme = &model.theme;
    let active = model.active_tab() == Some(id) && model.state.current_project().id == project;
    let bell = tab.panes.iter().any(|pane| {
        model
            .terminal(&pane.id)
            .is_some_and(|pane| pane.view.read(cx).bell_flashing)
    });
    let agent = (model.appearance.layout == AppLayout::AgentsFocused)
        .then(|| model.agent_tab_pane(tab))
        .flatten();
    let agent_pane = agent.map(|(pane, _)| pane.id);
    let title = agent_pane.map_or_else(
        || model.webview_title(tab, cx),
        |pane| tab.title(Some(pane)),
    );
    let color = tab
        .color
        .as_ref()
        .and_then(|color| muxy_ui::theme::parse_hex(color.as_str()))
        .map(gpui::Hsla::from);
    let group = SharedString::from(format!("sidebar-tab-{id}"));
    let hover = color.map_or(if active { theme.surface } else { theme.hover }, |color| {
        color.opacity(if active { 0.18 } else { 0.08 })
    });
    div()
        .id(group.clone())
        .debug_selector(move || format!("sidebar-tab-{id}"))
        .when(!model.tab_drag.is_active(), |row| row.group(group.clone()))
        .mx(m.spacing3())
        .my(m.spacing1())
        .pl(m.spacing4())
        .h(row_height(model))
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .rounded(m.radius_md())
        .cursor_pointer()
        .text_size(m.font_body())
        .text_color(theme.fg)
        .when(active, |row| row.bg(theme.surface))
        .when_some(color, |row, color| {
            row.bg(color.opacity(if active { 0.18 } else { 0.04 }))
        })
        .when(!model.tab_drag.is_active(), |row| {
            row.hover(|style| style.bg(hover))
        })
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                cx.stop_propagation();
                model.open_tab_menu(id, event.position, window, cx);
            }),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                cx.stop_propagation();
                model.begin_tab_drag(id, event.position, tab_strip::drag::Source::Sidebar, cx);
                model.select_tab(id, cx);
                if let Some(pane) = agent_pane {
                    model.focus_pane(pane, cx);
                }
                model.focus_active(window, cx);
            }),
        )
        .on_click(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(
            MouseButton::Middle,
            cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                model.close_tab(id, cx);
                model.focus_active(window, cx);
            }),
        )
        .child(
            div()
                .debug_selector(move || format!("sidebar-tab-icon-{id}"))
                .size(m.icon_sm())
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(if let Some((_, agent)) = agent {
                    tab_activity::provider_icon(agent.provider, m.icon_sm(), model)
                } else {
                    tab_activity::icon(
                        tab,
                        model,
                        m.icon_sm(),
                        webview_tab_icon(tab, model, active, cx),
                    )
                }),
        )
        .child(
            div()
                .debug_selector(move || format!("sidebar-tab-title-{id}"))
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .child(title.to_owned()),
        )
        .child(tab_accessory(
            tab,
            number.and_then(|index| {
                model.shortcut_hints.label(
                    muxy_core::shortcuts::ShortcutId::TABS[index],
                    &model.settings.keymap,
                )
            }),
            bell,
            group,
            model,
            cx,
        ))
        .into_any_element()
}

pub(super) fn titlebar(
    model: &AppModel,
    sidebar_width: f32,
    window: &Window,
    actions: gpui::Div,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let project = model.state.current_project();
    titlebar::background("project-titlebar")
        .flex()
        .items_center()
        .flex_none()
        .h(px(32.0))
        .pl(px((titlebar::navigation_width(model, window)
            - sidebar_width)
            .max(0.0)
            + 12.0))
        .child(
            div()
                .debug_selector(|| "project-title".into())
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(px(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg_muted)
                .child(model.project_title(project)),
        )
        .when(
            project.status() == ProjectStatus::Available && project.groups().is_none(),
            |bar| bar.child(tab_strip::new_tab_button(None, model, cx)),
        )
        .child(actions)
        .into_any_element()
}

fn close_tab_button(
    id: TabId,
    group: SharedString,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    div()
        .id(SharedString::from(format!("sidebar-close-{id}")))
        .debug_selector(move || format!("sidebar-close-{id}"))
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .rounded(model.metrics.radius_sm())
        .absolute()
        .inset_0()
        .opacity(0.0)
        .group_hover(group, |style| style.opacity(1.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |model, _, window, cx| {
            cx.stop_propagation();
            model.close_tab(id, cx);
            model.focus_active(window, cx);
        }))
        .child(
            div()
                .id(SharedString::from(format!("sidebar-close-glyph-{id}")))
                .size(model.metrics.icon_md())
                .flex()
                .items_center()
                .justify_center()
                .rounded(model.metrics.radius_sm())
                .hover(|style| style.bg(theme.hover))
                .child(IconGlyph::new(
                    Icon::X,
                    model.metrics.icon_xs(),
                    theme.fg_muted,
                )),
        )
        .into_any_element()
}

impl AppModel {
    pub(crate) fn set_project_focus(&mut self, project: Option<ProjectId>, cx: &mut Context<Self>) {
        self.appearance.sidebar_focus = project.is_some();
        if let Some(project) = project {
            let selected = self.preferred_worktree(project);
            self.select_project(selected, cx);
            self.appearance.tab_focused_expanded.insert(selected, true);
        }
        self.save_appearance(cx);
        cx.notify();
    }
}

fn tab_accessory(
    tab: &Tab,
    shortcut: Option<String>,
    bell: bool,
    group: SharedString,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = tab.id;
    let m = model.metrics;
    let status = tab_activity::tab_status(tab, model);
    let content = if tab.pinned {
        div()
            .debug_selector(move || format!("sidebar-tab-pin-{id}"))
            .child(SymbolGlyph::new(
                "pin.fill",
                m.font_xs(),
                model.theme.fg_muted,
            ))
            .into_any_element()
    } else if let Some(shortcut) = shortcut {
        div()
            .debug_selector(move || format!("sidebar-tab-shortcut-{id}"))
            .child(super::shortcut_hints::badge(shortcut, m.icon_md(), model))
            .into_any_element()
    } else if status != tab_activity::Status::None {
        tab_activity::status_glyph(format!("tab-{id}"), status, m.icon_sm(), model)
    } else if bell {
        div()
            .size(m.scaled(7.0))
            .rounded_full()
            .bg(model.theme.warning)
            .into_any_element()
    } else {
        div().into_any_element()
    };
    div()
        .relative()
        .flex_none()
        .size(m.control_small())
        .child(
            div()
                .debug_selector(move || format!("sidebar-tab-accessory-{id}"))
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .when(!tab.pinned, |slot| {
                    slot.group_hover(group.clone(), |style| style.opacity(0.0))
                })
                .child(content),
        )
        .when(!tab.pinned, |slot| {
            slot.child(close_tab_button(id, group, model, cx))
        })
        .into_any_element()
}

fn webview_tab_icon(tab: &Tab, model: &AppModel, active: bool, cx: &gpui::App) -> AnyElement {
    let color = if active {
        model.theme.fg
    } else {
        model.theme.fg_muted
    };
    let id = tab.id;
    model
        .webview_glyph(tab, model.metrics.font_footnote(), color, cx)
        .unwrap_or_else(|| {
            if tab.uses_proxy() {
                div()
                    .debug_selector(move || format!("sidebar-tab-proxy-{id}"))
                    .child(SymbolGlyph::new(
                        "network",
                        model.metrics.font_footnote(),
                        model.theme.accent,
                    ))
                    .into_any_element()
            } else {
                SymbolGlyph::new("terminal", model.metrics.font_footnote(), color)
                    .into_any_element()
            }
        })
}
