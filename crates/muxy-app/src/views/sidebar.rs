mod resize;
mod vibrancy;
pub(super) mod worktrees;

pub(crate) use resize::SidebarResize;
pub(super) use resize::handle as resize_handle;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Bounds, Context, DragMoveEvent, Empty, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, SharedString, StatefulInteractiveElement, Styled,
    StyledImage, Window, div, px,
};
use muxy_app_core::{
    Project, ProjectId, ProjectStatus,
    settings::{AppLayout, ProjectOrder, SidebarCollapsedStyle},
};

use super::{
    menu::{Command, Item},
    tab_sidebar,
};
use muxy_ui::components::{IconGlyph, SymbolGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::theme::{contrasting_foreground, parse_hex};

use crate::model::AppModel;

pub(crate) fn register_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
    cx: &Context<AppModel>,
) {
    use super::command_palette::{Handler, action};
    use muxy_core::shortcuts::ShortcutId;
    use muxy_ui::command_palette::{Command, Registry};

    registry.register(action(
        model,
        ShortcutId::AddProject,
        "Open Project…",
        super::workspace::AddProject,
    ));
    let has_workspaces = !model.state.workspaces().is_empty();
    let model = cx.weak_entity();
    registry.register(Command::list(
        "switch_project",
        "Switch Project…",
        move |cx| {
            let mut projects = Registry::default();
            let Some(model) = model.upgrade() else {
                return projects;
            };
            let model = model.read(cx);
            for project in model.state.projects() {
                let id = project.id;
                let handler: Handler = Rc::new(move |model, _, cx| model.select_project(id, cx));
                let title = project
                    .parent_id
                    .and_then(|parent| {
                        model
                            .state
                            .projects()
                            .iter()
                            .find(|project| project.id == parent)
                    })
                    .map_or_else(
                        || project.name.clone(),
                        |parent| format!("{} / {}", parent.name, project.name),
                    );
                projects.register(
                    Command::new(id.to_string(), title, handler)
                        .keywords(project.directory.to_string_lossy())
                        .disabled(project.status() == ProjectStatus::Missing),
                );
            }
            projects
        },
    ));
    if !has_workspaces {
        return;
    }
    let model = cx.weak_entity();
    registry.register(Command::list(
        "switch_workspace",
        "Switch Workspace…",
        move |cx| {
            let mut workspaces = Registry::default();
            let Some(model) = model.upgrade() else {
                return workspaces;
            };
            let model = model.read(cx);
            let all = std::iter::once((None, "All Projects".to_owned()));
            let named = model
                .state
                .workspaces()
                .iter()
                .map(|workspace| (Some(workspace.id), workspace.name.clone()));
            for (id, title) in all.chain(named) {
                let handler: Handler = Rc::new(move |model, _, cx| model.select_workspace(id, cx));
                let key = id.map_or_else(|| "all".to_owned(), |id| id.to_string());
                workspaces.register(Command::new(key, title, handler));
            }
            workspaces
        },
    ));
}

pub(crate) fn sidebar(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    if model.extension_sidebar_active() {
        return div()
            .debug_selector(|| "workspace-sidebar".into())
            .flex()
            .flex_col()
            .flex_none()
            .w(px(model.sidebar_width()))
            .h_full()
            .min_h(px(0.0))
            .bg(model.sidebar_background())
            .child(div().h(m.title_bar_height()).flex_none())
            .children(model.webviews.sidebar.as_ref().map(|sidebar| {
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(sidebar.surface.view.clone())
            }))
            .into_any_element();
    }
    let header = header(model, cx);
    let contents = match model.appearance.layout {
        AppLayout::ProjectFocused => project_list(model, cx),
        AppLayout::TabFocused => tab_sidebar::contents(model, window, cx),
    };
    div()
        .debug_selector(|| "workspace-sidebar".into())
        .flex()
        .flex_col()
        .flex_none()
        .w(px(model.sidebar_width()))
        .h_full()
        .min_h(px(0.0))
        .bg(model.sidebar_background())
        .child(div().h(m.title_bar_height()).flex_none())
        .child(header)
        .child(contents)
        .into_any_element()
}

impl AppModel {
    pub(crate) fn cached_sidebar(&self) -> gpui::AnyView {
        gpui::AnyView::from(self.sidebar_view.clone()).cached(
            div()
                .w(px(self.sidebar_width()))
                .h_full()
                .min_h(px(0.0))
                .flex_none()
                .style()
                .clone(),
        )
    }

    pub(crate) fn sidebar_width(&self) -> f32 {
        if self.appearance.sidebar_expanded {
            let width = self
                .appearance
                .sidebar_expanded_width
                .filter(|width| width.is_finite())
                .map_or_else(
                    || self.metrics.sidebar_expanded_width(),
                    |width| self.metrics.scaled(width),
                );
            f32::from(self.clamp_expanded_sidebar_width(width))
        } else if (self.appearance.layout == AppLayout::ProjectFocused
            || self.extension_sidebar_active())
            && self.appearance.sidebar_collapsed_style == SidebarCollapsedStyle::Icons
        {
            f32::from(self.metrics.sidebar_collapsed_width())
        } else {
            0.0
        }
    }

    fn clamp_expanded_sidebar_width(&self, width: Pixels) -> Pixels {
        width.clamp(
            self.metrics.sidebar_expanded_min_width(),
            self.metrics.sidebar_expanded_max_width(),
        )
    }

    pub(crate) fn sidebar_filter_label(&self) -> SharedString {
        if self.appearance.sidebar_focus {
            return "Focused Project".into();
        }
        self.state.active_workspace().map_or_else(
            || "All Projects".into(),
            |workspace| workspace.name.clone().into(),
        )
    }

    /// Top-level projects the workspace filter lists, in sidebar order.
    pub(crate) fn listed_parents(&self) -> Vec<&Project> {
        let mut parents: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| project.parent_id.is_none() && self.state.is_listed(project))
            .collect();
        if self.appearance.sidebar_project_order == ProjectOrder::Name {
            parents.sort_by_cached_key(|project| (!project.home, project.name.to_lowercase()));
        }
        parents
    }

    pub(crate) fn sidebar_projects(&self) -> Vec<&Project> {
        let active = self.state.current_project();
        let focused = active.parent_id.unwrap_or(active.id);
        self.listed_parents()
            .into_iter()
            .filter(|project| !self.appearance.sidebar_focus || project.id == focused)
            .flat_map(|parent| {
                let children = if self.appearance.layout == AppLayout::TabFocused
                    && self.worktrees_visible(parent.id)
                {
                    self.worktree_children(parent.id)
                } else {
                    Vec::new()
                };
                let children = children.into_iter().filter(move |child| {
                    !child.tabs.is_empty()
                        || child.id == active.id
                        || child.status() == ProjectStatus::Missing
                });
                std::iter::once(parent).chain(children)
            })
            .collect()
    }
}

fn header(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    if !model.appearance.sidebar_expanded {
        return div().into_any_element();
    }
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .flex()
        .items_center()
        .gap(m.spacing2())
        .px(m.spacing3())
        .pt(m.spacing2())
        .child(
            div()
                .id("sidebar-project-filter")
                .debug_selector(|| "sidebar-project-filter".into())
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .items_center()
                .justify_between()
                .px(m.spacing4())
                .h(m.control_medium())
                .rounded(m.radius_md())
                .bg(theme.surface)
                .cursor_pointer()
                .hover(|style| style.bg(theme.hover))
                .text_size(m.font_caption())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg_muted)
                .on_click(cx.listener(|model, event: &gpui::ClickEvent, window, cx| {
                    model.open_menu(filter_items(model), event.position(), window, cx);
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .child(model.sidebar_filter_label()),
                )
                .child(IconGlyph::new(
                    Icon::ChevronDown,
                    m.font_caption(),
                    theme.fg_muted,
                )),
        )
        .child(
            div()
                .id("sidebar-project-sort")
                .debug_selector(|| "sidebar-project-sort".into())
                .flex()
                .items_center()
                .justify_center()
                .size(m.control_medium())
                .rounded(m.radius_md())
                .bg(theme.surface)
                .cursor_pointer()
                .hover(|style| style.bg(theme.hover))
                .on_click(cx.listener(|model, event: &gpui::ClickEvent, window, cx| {
                    model.open_menu(
                        vec![
                            Item::action(
                                "Manual Order",
                                Command::SortProjects(ProjectOrder::Manual),
                            )
                            .checked_if(
                                model.appearance.sidebar_project_order == ProjectOrder::Manual,
                            ),
                            Item::action("Name", Command::SortProjects(ProjectOrder::Name))
                                .checked_if(
                                    model.appearance.sidebar_project_order == ProjectOrder::Name,
                                ),
                        ],
                        event.position(),
                        window,
                        cx,
                    );
                }))
                .child(IconGlyph::new(
                    Icon::ArrowUpDown,
                    m.font_body(),
                    theme.fg_muted,
                )),
        )
        .child(layout_selector(model, cx))
        .into_any_element()
}

fn filter_items(model: &AppModel) -> Vec<Item> {
    let focused = model.appearance.sidebar_focus;
    let active = model.state.active_workspace().map(|workspace| workspace.id);
    let mut items = vec![
        Item::action("All Projects", Command::SelectWorkspace(None))
            .checked_if(!focused && active.is_none()),
    ];
    items.extend(model.state.workspaces().iter().map(|workspace| {
        Item::action(
            workspace.name.clone(),
            Command::SelectWorkspace(Some(workspace.id)),
        )
        .checked_if(!focused && active == Some(workspace.id))
    }));
    items.push(
        Item::action("Focus Current Project", Command::FocusProject(true))
            .checked_if(focused)
            .separated(),
    );
    items.push(Item::action("New Workspace…", Command::NewWorkspace(None)).separated());
    if let Some(id) = active {
        items.push(Item::action(
            "Rename Workspace…",
            Command::RenameWorkspace(id),
        ));
        items.push(Item::action(
            "Delete Workspace…",
            Command::DeleteWorkspace(id),
        ));
    }
    items
}

fn layout_selector(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .id("layout-menu")
        .debug_selector(|| "layout-menu".into())
        .group("layout-menu")
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(m.control_medium())
        .rounded(m.radius_md())
        .bg(theme.surface)
        .hover(|style| style.bg(theme.hover))
        .cursor_pointer()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(|model, event: &gpui::ClickEvent, window, cx| {
            cx.stop_propagation();
            model.open_menu(
                vec![
                    Item::action(
                        "Project Focused",
                        Command::Layout(AppLayout::ProjectFocused),
                    )
                    .checked_if(model.appearance.layout == AppLayout::ProjectFocused),
                    Item::action("Tab Focused", Command::Layout(AppLayout::TabFocused))
                        .checked_if(model.appearance.layout == AppLayout::TabFocused),
                    Item::action("Agents Focused", Command::Dismiss).disabled(),
                ],
                event.position(),
                window,
                cx,
            );
        }))
        .child(
            IconGlyph::new(Icon::Grid, m.font_body(), theme.fg_muted)
                .hover_in_group("layout-menu", theme.fg),
        )
        .into_any_element()
}

struct DraggedProject {
    id: ProjectId,
    last_target: Cell<Option<ProjectId>>,
}

impl DraggedProject {
    fn move_to(&self, target: Option<ProjectId>, model: &mut AppModel, cx: &mut Context<AppModel>) {
        if model.overlay.is_some()
            || model.close_prompt.is_some()
            || model.appearance.sidebar_project_order != ProjectOrder::Manual
        {
            return;
        }
        let target = target.filter(|target| *target != self.id);
        if self.last_target.replace(target) != target
            && let Some(target) = target
        {
            model.move_project(self.id, target, cx);
        }
    }
}

#[derive(Default)]
struct ProjectRows(Vec<(ProjectId, Bounds<Pixels>)>);

impl ProjectRows {
    fn at(&self, position: Point<Pixels>) -> Option<ProjectId> {
        self.0
            .iter()
            .find_map(|(id, bounds)| bounds.contains(&position).then_some(*id))
    }
}

fn project_list(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let wide = model.appearance.sidebar_expanded;
    let targets = Rc::new(RefCell::new(ProjectRows::default()));
    let measured = targets.clone();
    let moving = targets.clone();
    let projects = model.sidebar_projects();
    let ids: Vec<_> = projects
        .iter()
        .map(|project| {
            (!project.home
                && project.parent_id.is_none()
                && project.status() == ProjectStatus::Available)
                .then_some(project.id)
        })
        .collect();
    div()
        .id("projects-scroll")
        .debug_selector(|| "projects-scroll".into())
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .on_drag_move(cx.listener(
            move |model, event: &DragMoveEvent<DraggedProject>, window, cx| {
                if event.event.pressed_button != Some(gpui::MouseButton::Left) {
                    cx.stop_active_drag(window);
                    return;
                }
                let target = event
                    .bounds
                    .contains(&event.event.position)
                    .then(|| moving.borrow().at(event.event.position))
                    .flatten();
                let drag = event.dragged_item().downcast_ref::<DraggedProject>();
                if let Some(drag) = drag {
                    drag.move_to(target, model, cx);
                }
            },
        ))
        .on_drop(
            cx.listener(move |model, drag: &DraggedProject, window, cx| {
                drag.move_to(targets.borrow().at(window.mouse_position()), model, cx);
            }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .px(if wide { m.spacing3() } else { m.spacing4() })
                .gap(m.spacing3())
                .pt(if wide { m.spacing5() } else { m.spacing2() })
                .pb(m.spacing3())
                .when(!wide, Styled::items_center)
                .children(
                    projects
                        .iter()
                        .enumerate()
                        .map(|(index, project)| worktrees::group(project, index, model, cx)),
                )
                .when(!model.appearance.sidebar_focus, |list| {
                    list.child(add_project_button(model, cx))
                })
                .on_children_prepainted(move |bounds, _, _| {
                    measured.borrow_mut().0 = ids
                        .iter()
                        .zip(bounds)
                        .filter_map(|(id, bounds)| id.map(|id| (id, bounds)))
                        .collect();
                }),
        )
        .into_any_element()
}

#[allow(
    clippy::too_many_lines,
    reason = "Declarative project row and interaction layout"
)]
fn project_row(
    project: &Project,
    index: usize,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let wide = model.appearance.sidebar_expanded;
    let id = project.id;
    let missing = project.status() == ProjectStatus::Missing;
    let active = model.state.current_project().id == id
        || model.state.current_project().parent_id == Some(id);
    let has_worktrees = model.has_worktrees(project) && !missing;
    let group = SharedString::from(format!("project-{id}"));
    let tile = project_tile(project, model, group.clone());
    let activity = super::tab_activity::project_status(id, model);
    let drag = DraggedProject {
        id,
        last_target: Cell::new(None),
    };
    div()
        .id(group.clone())
        .debug_selector(|| format!("project-row-{index}"))
        .group(group)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .when(wide && project.parent_id.is_some(), |row| {
            row.ml(m.spacing5())
        })
        .when(wide, |row| {
            row.p(m.spacing2())
                .h(m.icon_xxl() + m.spacing2() * 2.0)
                .gap(m.spacing4())
                .rounded(m.radius_lg())
                .when(active, |row| row.bg(theme.surface))
                .hover(|style| style.bg(if active { theme.surface } else { theme.hover }))
        })
        .when(!wide, |row| row.justify_center().size(m.scaled(34.0)))
        .when(missing, |row| row.opacity(0.5))
        .when(!missing, |row| {
            row.cursor_pointer()
                .on_click(cx.listener(move |model, _, window, cx| {
                    if active && has_worktrees && wide {
                        model.toggle_worktree_list(id, cx);
                    } else if active && has_worktrees {
                        model.expanded_worktrees.insert(id);
                        model.toggle_sidebar(window, cx);
                    } else {
                        model.select_project(model.preferred_worktree(id), cx);
                        model.focus_active(window, cx);
                    }
                }))
        })
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                if let Some(project) = model.state.project(id) {
                    model.open_menu(
                        super::project_menu::items(project, model.worktrees_visible(id)),
                        event.position,
                        window,
                        cx,
                    );
                }
            }),
        )
        .when(
            !project.home
                && project.parent_id.is_none()
                && !missing
                && model.appearance.sidebar_project_order == ProjectOrder::Manual,
            |row| row.on_drag(drag, |_, _, _, cx| cx.new(|_| Empty)),
        )
        .child(tile)
        .when(wide, |row| {
            row.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .gap(m.scaled(1.0))
                    .text_color(theme.fg)
                    .child(
                        div()
                            .truncate()
                            .text_size(m.font_emphasis())
                            .font_weight(FontWeight::MEDIUM)
                            .child(project.name.clone()),
                    )
                    .when(has_worktrees, |label| {
                        let selected = model.state.project(model.preferred_worktree(id));
                        label.child(
                            div()
                                .debug_selector(move || format!("project-worktree-label-{id}"))
                                .truncate()
                                .text_size(m.font_footnote())
                                .font_family(".AppleSystemUIFontMonospaced")
                                .font_weight(FontWeight::NORMAL)
                                .child(
                                    selected
                                        .filter(|p| p.parent_id.is_some())
                                        .map_or("primary", |p| p.name.as_str())
                                        .to_owned(),
                                ),
                        )
                    }),
            )
        })
        .when(activity != super::tab_activity::Status::None, |row| {
            row.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .min_w(m.scaled(18.0))
                    .h(m.scaled(18.0))
                    .when(!wide, |badge| {
                        badge.absolute().right(m.scaled(-3.0)).top(m.scaled(-3.0))
                    })
                    .child(super::tab_activity::status_glyph(
                        format!("project-activity-{id}"),
                        activity,
                        m.icon_sm(),
                        model,
                    )),
            )
        })
        .when(wide && has_worktrees, |row| {
            row.child(worktrees::disclosure(project, model, cx))
        })
        .when(!wide && active, |row| {
            row.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(m.scaled(9.0))
                    .border(m.scaled(1.5))
                    .border_color(theme.accent),
            )
        })
        .when(missing, |row| {
            row.child(div().absolute().top_0().right_0().child(SymbolGlyph::new(
                "exclamationmark.triangle.fill",
                m.font_caption(),
                theme.warning,
            )))
        })
        .into_any_element()
}

pub(super) fn add_project_button(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let wide = model.appearance.sidebar_expanded;
    div()
        .id("add-project")
        .flex()
        .flex_none()
        .items_center()
        .cursor_pointer()
        .when(wide, |row| {
            row.p(m.spacing2())
                .gap(m.spacing4())
                .rounded(m.radius_lg())
                .hover(|style| style.bg(theme.hover))
        })
        .when(!wide, |row| row.justify_center().size(m.scaled(34.0)))
        .on_click(cx.listener(|model, _, window, cx| model.open_project_picker(window, cx)))
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(m.icon_xxl())
                .rounded(m.radius_md())
                .bg(theme.surface)
                .child(IconGlyph::new(
                    Icon::Plus,
                    m.font_emphasis(),
                    theme.fg_muted,
                )),
        )
        .when(wide, |row| {
            row.child(
                div()
                    .text_size(m.font_body())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.fg_muted)
                    .child("Add Project"),
            )
        })
        .into_any_element()
}

fn project_tile(project: &Project, model: &AppModel, group: SharedString) -> AnyElement {
    let m = model.metrics;
    let color = if project.home {
        model.theme.accent.to_rgb()
    } else {
        parse_hex(project.color.as_str()).unwrap_or(gpui::rgb(0x80_80_80))
    };
    let foreground = contrasting_foreground(color);
    let glyph = if let Some((_, logo)) = model.project_logos.get(&project.id) {
        gpui::img(logo.clone())
            .size(m.icon_xxl())
            .rounded(m.radius_md())
            .object_fit(gpui::ObjectFit::Cover)
            .into_any_element()
    } else if let Some(icon) = &project.icon {
        if let Some(symbol) = icon.strip_prefix("sf:") {
            SymbolGlyph::new(symbol.to_owned(), m.font_title_large(), foreground.into())
                .into_any_element()
        } else {
            div()
                .text_size(m.font_title_large())
                .child(icon.clone())
                .into_any_element()
        }
    } else if project.home {
        SymbolGlyph::new("house.fill", m.font_title_large(), foreground.into()).into_any_element()
    } else {
        div()
            .text_size(m.font_emphasis())
            .font_weight(FontWeight::BOLD)
            .text_color(foreground)
            .child(project.initial().to_uppercase())
            .into_any_element()
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(m.icon_xxl())
        .rounded(m.radius_md())
        .when(!model.project_logos.contains_key(&project.id), |tile| {
            tile.bg(color)
        })
        .overflow_hidden()
        .group_hover(group, |style| style.opacity(0.85))
        .child(glyph)
        .into_any_element()
}
