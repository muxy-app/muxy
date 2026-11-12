use gpui::prelude::FluentBuilder;
use gpui::{
    AccessibleAction, AnyElement, Context, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window,
    div, px,
};
use muxy_app_core::{Project, ProjectId, ProjectStatus, Tab, TabId, settings::AppLayout};
use muxy_core::shortcuts::ShortcutId;
use muxy_ui::accessibility::OnPress;
use muxy_ui::components::{ButtonInteraction, IconButton, IconGlyph, SymbolGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::l10n::translate;
use muxy_ui::tr;

use super::sidebar::{client_button, client_press, describe, project_description};
use super::{tab_activity, tab_strip, titlebar};
use crate::model::AppModel;
use crate::views::move_drag;

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
        project.listed_tabs().filter(|tab| {
            self.appearance.layout != AppLayout::AgentsFocused || self.agent_tab_pane(tab).is_some()
        })
    }

    /// The sidebar's servers with the projects it shows under each, leaving
    /// out projects without tabs unless they are shown, and how many each
    /// has without tabs.
    pub(crate) fn tab_sidebar_groups(&self) -> Vec<(super::sidebar::ServerGroup<'_>, usize)> {
        let listed = |project: &&Project| {
            self.appearance.sidebar_focus || self.sidebar_tabs(project).next().is_some()
        };
        self.sidebar_groups()
            .into_iter()
            .map(|mut group| {
                let without_tabs = group
                    .projects
                    .iter()
                    .filter(|project| !listed(project))
                    .count();
                if !self.projects_without_tabs_shown {
                    group.projects.retain(listed);
                }
                (group, without_tabs)
            })
            .collect()
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
                .listed_tabs()
                .map(|tab| tab.id)
                .collect();
        }
        self.sidebar_projects()
            .into_iter()
            .filter(|project| {
                project.status() == ProjectStatus::Available && self.project_expanded(project.id)
            })
            .flat_map(|project| project.listed_tabs().map(|tab| tab.id))
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
    for (position, (group, without_tabs)) in model.tab_sidebar_groups().into_iter().enumerate() {
        if group.heading {
            rows = rows.child(
                div()
                    .px(model.metrics.spacing3())
                    .when(position > 0, |heading| heading.pt(model.metrics.spacing3()))
                    .child(super::sidebar::server_heading(
                        group.server,
                        super::sidebar::HeadingRows::Tabs,
                        model,
                        cx,
                    )),
            );
        }
        for project in group.projects {
            rows = rows.child(project_group(project, &numbers, targets.clone(), model, cx));
        }
        if without_tabs > 0 {
            rows = rows.child(projects_without_tabs_toggle(
                group.server,
                without_tabs,
                model,
                cx,
            ));
        }
        if group.heading && model.server_expanded(group.server) && !model.appearance.sidebar_focus {
            rows = rows.child(add_project_button(group.server, model, cx));
        }
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
                .role(Role::Tree)
                .accessibility_id("tab-sidebar-scroll")
                .aria_label(tr!("Projects"))
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

fn add_project_button(
    server: muxy_app_core::ServerId,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let id = SharedString::from(if server.is_local() {
        "tab-add-project".to_owned()
    } else {
        format!("tab-add-project-{server}")
    });
    div()
        .id(id.clone())
        .role(Role::Button)
        .accessibility_id(id.clone())
        .aria_label(tr!("Add Project"))
        .debug_selector(move || id.to_string())
        .mx(m.spacing3())
        .my(m.spacing1())
        .pl(m.spacing4())
        .h(m.control_medium())
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .rounded(m.radius_md())
        .cursor_pointer()
        .text_size(m.font_body())
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.fg_muted)
        .hover(|style| style.bg(theme.hover).text_color(theme.fg))
        .focus(|style| style.bg(theme.hover).text_color(theme.fg))
        .button_interaction(
            cx.listener(move |model, event: &gpui::ClickEvent, window, cx| {
                model.choose_project_source(server, event.position(), window, cx);
            }),
        )
        .child(IconGlyph::new(Icon::Plus, m.font_body(), theme.fg_muted))
        .child(tr!("Add Project"))
        .into_any_element()
}

/// The projects of `server`'s group that have no tabs, shown or hidden
/// together with every other group's.
fn projects_without_tabs_toggle(
    server: muxy_app_core::ServerId,
    count: usize,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let shown = model.projects_without_tabs_shown;
    let label = if shown {
        tr!("Hide projects without tabs")
    } else if count == 1 {
        tr!("1 more project")
    } else {
        tr!("%lld more projects", count)
    };
    let id = SharedString::from(if server.is_local() {
        "tab-sidebar-projects-without-tabs".to_owned()
    } else {
        format!("tab-sidebar-projects-without-tabs-{server}")
    });
    div()
        .my(m.spacing1())
        .h(row_height(model))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .child(
            div()
                .id(id.clone())
                .role(Role::Button)
                .accessibility_id(id.clone())
                .aria_label(label.clone())
                .aria_expanded(shown)
                .debug_selector(move || id.to_string())
                .cursor_pointer()
                .text_size(m.font_footnote())
                .text_color(theme.fg_dim)
                .hover(|style| style.text_color(theme.fg_muted))
                .on_press(cx.listener(|model, _, window, cx| {
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
                })
                .id(SharedString::from(format!("tab-project-tabs-{id}")))
                .role(Role::TabList)
                .aria_label(tr!("Tabs in %@", model.project_title(project).as_str())),
        );
    }
    group.into_any_element()
}

fn project_header(project: &Project, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let id = project.id;
    let m = model.metrics;
    let missing = project.status() == ProjectStatus::Missing;
    let title = model.project_title(project);
    let activity = tab_activity::project_status(id, model);
    let description = project_description(
        missing,
        super::sidebar::remote_description(project, model),
        activity,
    );
    div()
        .id(SharedString::from(format!("tab-project-{id}")))
        .role(Role::TreeItem)
        .accessibility_id(format!("tab-project-{}", project.name))
        .aria_label(title.clone())
        .aria_selected(model.state.current_project().id == id)
        .aria_level(super::sidebar::project_level(project))
        .when(!missing, |row| {
            row.aria_expanded(model.project_expanded(id))
        })
        .when_some(description, StatefulInteractiveElement::aria_description)
        .on_a11y_action(
            AccessibleAction::Click,
            client_press(cx, move |model, window, cx| {
                model.toggle_project_tabs(id, cx);
                model.focus_active(window, cx);
            }),
        )
        .debug_selector(move || format!("tab-project-{id}"))
        .relative()
        .when(!missing, |row| {
            row.child(move_drag::zone(
                &model.move_drag.zones,
                move_drag::Owner::Sidebar,
                move_drag::ZoneKind::Project(id),
            ))
        })
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
        .child(super::sidebar::project_actions(project, &title, cx))
        .child(project_title(project, model, cx))
        .child(project_accessory(project, activity, model))
        .when(!missing, |row| {
            row.child(new_tab_button(project, model, cx))
                .child(project_disclosure(project, model, cx))
        })
        .into_any_element()
}

fn project_title(project: &Project, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let id = project.id;
    let title = SharedString::from(model.project_title(project));
    let group = SharedString::from(format!("tab-project-title-{id}"));
    div()
        .id(group.clone())
        .debug_selector(move || format!("tab-project-title-{id}"))
        .group(group.clone())
        .relative()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .on_click(cx.listener(move |model, _, window, cx| {
            cx.stop_propagation();
            model.toggle_project_tabs(id, cx);
            model.focus_active(window, cx);
        }))
        .child(
            group_label(title.clone(), model)
                .group_hover(group.clone(), |style| style.opacity(0.0)),
        )
        .child(
            group_label(title, model)
                .absolute()
                .inset_0()
                .text_color(model.theme.fg_muted)
                .opacity(0.0)
                .group_hover(group, |style| style.opacity(1.0)),
        )
        .into_any_element()
}

/// What a status glyph's tooltip says, for clients that can't hover it.
pub(super) fn activity_description(status: tab_activity::Status) -> Option<SharedString> {
    use muxy_protocol::ProgressState;
    use tab_activity::Status;
    Some(match status {
        Status::None => return None,
        Status::Busy(key) => translate(key),
        Status::Progress(progress) => match progress.state {
            ProgressState::Error => tr!("Work reported an error."),
            ProgressState::Paused => tr!("Work is paused."),
            ProgressState::Running
            | ProgressState::Indeterminate
            | ProgressState::Unrecognized(_) => tr!("Work is in progress."),
        },
        Status::Blocked => tr!("An agent is waiting for your attention."),
        Status::Completed => tr!("Work finished and is ready to review."),
        Status::Unread(1) => tr!("1 unread notification"),
        Status::Unread(count) => tr!("%lld unread notifications", count),
    })
}

/// What a client reads after a tab's title, beyond its selected state.
pub(super) fn tab_description(
    pinned: bool,
    status: tab_activity::Status,
    bell: bool,
) -> Option<SharedString> {
    describe([
        pinned.then(|| tr!("Pinned")),
        activity_description(status),
        bell.then(|| tr!("Bell")),
    ])
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

fn header_button(id: String, name: SharedString, icon: Icon, model: &AppModel) -> IconButton {
    let theme = &model.theme;
    IconButton::new(
        SharedString::from(id),
        name,
        icon,
        model.metrics.icon_xs(),
        model.metrics.control_small(),
        theme.fg_dim,
        theme.fg_muted,
    )
}

fn new_tab_button(project: &Project, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let id = project.id;
    let theme = &model.theme;
    div()
        .debug_selector(move || format!("project-new-tab-{id}"))
        .flex_none()
        .child(
            header_button(
                format!("project-new-tab-{id}"),
                tr!("New Terminal Tab"),
                Icon::Plus,
                model,
            )
            .accessibility_id(format!("project-new-tab-{}", project.name))
            .tooltip(theme)
            .shortcut(
                (id == model.state.current_project().id)
                    .then(|| model.shortcut_label(ShortcutId::NewTab))
                    .flatten(),
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
                    tr!("Collapse Project")
                } else {
                    tr!("Expand Project")
                },
                if expanded {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                model,
            )
            .accessibility_id(format!("tab-project-toggle-{}", project.name))
            .expanded(expanded)
            .tooltip(theme)
            .on_click(cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                model.toggle_project_tabs(id, cx);
                model.focus_active(window, cx);
            })),
        )
        .into_any_element()
}

fn project_accessory(
    project: &Project,
    activity: tab_activity::Status,
    model: &AppModel,
) -> AnyElement {
    let id = project.id;
    let m = model.metrics;
    let has_activity = activity != tab_activity::Status::None;
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
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
    let zone = tab_activity::HoverZone::default();
    let status = tab_activity::tab_status(tab, model);
    let hover = color.map_or(if active { theme.surface } else { theme.hover }, |color| {
        color.opacity(if active { 0.18 } else { 0.08 })
    });
    div()
        .id(group.clone())
        .role(Role::Tab)
        .aria_label(title.to_owned())
        .aria_selected(active)
        .when_some(
            tab_description(tab.pinned, status, bell),
            StatefulInteractiveElement::aria_description,
        )
        .on_a11y_action(
            AccessibleAction::Click,
            client_press(cx, move |model, window, cx| {
                model.select_tab(id, cx);
                if let Some(pane) = agent_pane {
                    model.focus_pane(pane, cx);
                }
                model.focus_active(window, cx);
            }),
        )
        .debug_selector(move || format!("sidebar-tab-{id}"))
        .relative()
        .child(move_drag::zone(
            &model.move_drag.zones,
            move_drag::Owner::Sidebar,
            move_drag::ZoneKind::Tab {
                project,
                tab: id,
                axis: muxy_app_core::Axis::Vertical,
            },
        ))
        .when(!model.tab_drag.is_active(), |row| {
            row.group(group.clone()).child(zone.track())
        })
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
        .child(client_button(
            format!("sidebar-tab-actions-{id}"),
            tr!("Actions for %@", title),
            cx,
            move |model, position, window, cx| model.open_tab_menu(id, position, window, cx),
        ))
        .when(!tab.pinned, |row| {
            row.child(client_button(
                format!("sidebar-tab-close-{id}"),
                tr!("Close %@", title),
                cx,
                move |model, _, window, cx| {
                    model.close_tab(id, cx);
                    model.focus_active(window, cx);
                },
            ))
        })
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
            status,
            number.and_then(|index| {
                model
                    .shortcut_hints
                    .label(ShortcutId::TABS[index], &model.settings.keymap)
            }),
            bell,
            group,
            &zone,
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
        .h(model.metrics.scaled(32.0))
        .pl(px((titlebar::navigation_width(model, window)
            - sidebar_width)
            .max(0.0)
            + f32::from(model.metrics.scaled(12.0))))
        .child(
            div()
                .debug_selector(|| "project-title".into())
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_size(model.metrics.scaled(12.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg_muted)
                .child(model.project_title(project)),
        )
        .when(
            project.status() == ProjectStatus::Available
                && project.groups().is_none()
                && !tab_strip::sidebar_opens_tabs(model),
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

#[allow(
    clippy::too_many_arguments,
    reason = "One tab row's accessory and its hover"
)]
fn tab_accessory(
    tab: &Tab,
    status: tab_activity::Status,
    shortcut: Option<String>,
    bell: bool,
    group: SharedString,
    zone: &tab_activity::HoverZone,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = tab.id;
    let m = model.metrics;
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
        tab_activity::status_glyph_in(
            format!("tab-{id}"),
            status,
            m.icon_sm(),
            None,
            Some(zone),
            model,
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use tab_activity::Status;

    #[test]
    fn a_tab_reports_pinned_activity_and_bell() {
        assert_eq!(tab_description(false, Status::None, false), None);
        assert_eq!(
            tab_description(true, Status::Blocked, true),
            Some("Pinned, An agent is waiting for your attention., Bell".into()),
        );
    }

    #[test]
    fn activity_reads_as_its_tooltip_says() {
        assert_eq!(activity_description(Status::None), None);
        assert_eq!(
            activity_description(Status::Unread(1)),
            Some("1 unread notification".into()),
        );
        assert_eq!(
            activity_description(Status::Unread(3)),
            Some("3 unread notifications".into()),
        );
        assert_eq!(
            activity_description(Status::Completed),
            Some("Work finished and is ready to review.".into()),
        );
    }
}
