mod resize;
mod vibrancy;
pub(super) mod worktrees;

pub(crate) use resize::SidebarResize;
pub(super) use resize::handle as resize_handle;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AccessibleAction, AnyElement, App, AppContext, Bounds, Context, Div, DragMoveEvent, Empty,
    FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels, Point, Role,
    SharedString, Stateful, StatefulInteractiveElement, Styled, StyledImage, Window, canvas, div,
    px,
};
use muxy_app_core::{
    Project, ProjectId, ProjectStatus, ServerId,
    settings::{AppLayout, ProjectOrder, SidebarCollapsedStyle},
};

use super::{
    menu::{Command, Item},
    overlays::Overlay,
    tab_activity, tab_sidebar,
};
use muxy_ui::accessibility::hidden_button;
use muxy_ui::components::{ButtonInteraction, IconGlyph, SymbolGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::theme::{contrasting_foreground, parse_hex};
use muxy_ui::tr;

use crate::model::AppModel;

pub(crate) fn register_commands(
    registry: &mut muxy_ui::command_palette::Registry<super::command_palette::Handler>,
    model: &AppModel,
    cx: &Context<AppModel>,
) {
    use super::command_palette::Handler;
    use super::workspace::commands::register_placed;
    use muxy_core::shortcuts::ShortcutId;
    use muxy_ui::command_palette::{Command, Registry};

    register_placed(registry, model, ShortcutId::AddProject);
    register_placed(registry, model, ShortcutId::NewTabInProject);
    let create: Handler = Rc::new(|model, _, cx| {
        model.open_extension_agent(crate::extensions::authoring::AgentTask::Create, cx);
    });
    registry.register(
        Command::new("create_extension", tr!("Create Extension…"), create)
            .keywords("new make build extension agent"),
    );
    let manage: Handler = Rc::new(AppModel::show_extensions_settings);
    registry.register(
        Command::new("manage_extensions", tr!("Manage Extensions"), manage)
            .keywords("settings installed marketplace extension"),
    );
    let has_workspaces = !model.state.workspaces().is_empty();
    let model = cx.weak_entity();
    registry.register(
        Command::list("switch_project", tr!("Switch Project…"), move |cx| {
            let mut projects = Registry::default();
            let Some(model) = model.upgrade() else {
                return projects;
            };
            let model = model.read(cx);
            for project in model
                .state
                .projects()
                .iter()
                .filter(|project| model.project_listed(project))
            {
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
                        || model.project_title(project),
                        |parent| format!("{} / {}", parent.name, project.name),
                    );
                let command = Command::new(id.to_string(), title, handler)
                    .keywords(project.directory.to_string_lossy())
                    .disabled(project.status() == ProjectStatus::Missing);
                projects.register(if project.server_id.is_local() {
                    command
                } else {
                    command.badge(model.server_label(project.server_id))
                });
            }
            projects
        })
        .keywords("Switch Project"),
    );
    if !has_workspaces {
        return;
    }
    let model = cx.weak_entity();
    registry.register(
        Command::list("switch_workspace", tr!("Switch Workspace…"), move |cx| {
            let mut workspaces = Registry::default();
            let Some(model) = model.upgrade() else {
                return workspaces;
            };
            let model = model.read(cx);
            let all = std::iter::once((None, tr!("All Projects").to_string()));
            let named = model
                .state
                .workspaces()
                .iter()
                .map(|workspace| (Some(workspace.id), workspace.name.clone()));
            for (id, title) in all.chain(named) {
                let handler: Handler = Rc::new(move |model, _, cx| model.select_workspace(id, cx));
                let key = id.map_or_else(|| "all".to_owned(), |id| id.to_string());
                let command = Command::new(key, title, handler);
                workspaces.register(if id.is_none() {
                    command.keywords("All Projects")
                } else {
                    command
                });
            }
            workspaces
        })
        .keywords("Switch Workspace"),
    );
}

pub(crate) fn sidebar(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    model.move_drag.zones.clear(
        window.window_handle().window_id(),
        super::move_drag::Owner::Sidebar,
    );
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
            .map(|sidebar| super::titlebar::top_corners(sidebar, window, true, false))
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
        AppLayout::TabFocused | AppLayout::AgentsFocused => {
            tab_sidebar::contents(model, window, cx)
        }
    };
    div()
        .debug_selector(|| "workspace-sidebar".into())
        .flex()
        .flex_col()
        .flex_none()
        .w(px(model.sidebar_width()))
        .h_full()
        .min_h(px(0.0))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.0))
                .bg(model.sidebar_background())
                .map(|sidebar| super::titlebar::top_corners(sidebar, window, true, false))
                .child(div().h(m.title_bar_height()).flex_none())
                .child(header)
                .child(contents)
                .children(super::agents_panel::collapsed_button(model, cx)),
        )
        .children(super::agents_panel::docked(model, window, cx))
        .child(footer(model, cx))
        .into_any_element()
}

fn footer(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    div()
        .flex()
        .flex_none()
        .items_center()
        .pb(m.spacing3())
        .when(model.appearance.sidebar_expanded, |footer| {
            footer.gap(m.spacing1()).px(m.spacing3())
        })
        .when(!model.appearance.sidebar_expanded, |footer| {
            footer.flex_col().gap(m.spacing3())
        })
        .bg(model.sidebar_background())
        .child(extensions_button(model, cx))
        .child(super::remote_servers::section(model, cx))
        .child(settings_button(model, cx))
        .into_any_element()
}

/// The side of the square icon buttons at the bottom of the sidebar.
pub(super) fn footer_button_size(model: &AppModel) -> Pixels {
    if model.appearance.sidebar_expanded {
        model.metrics.control_large()
    } else {
        collapsed_item_size(model)
    }
}

pub(super) fn collapsed_item_size(model: &AppModel) -> Pixels {
    model.metrics.scaled(34.0)
}

/// A footer control: square until a caller gives it a label to fill the row.
pub(super) fn footer_button(
    id: &'static str,
    label: impl Into<SharedString>,
    model: &AppModel,
) -> Stateful<Div> {
    let label = label.into();
    let tooltip = muxy_ui::components::tooltip(label.to_string(), &model.theme);
    footer_control(id, &label, model).tooltip(tooltip)
}

/// A footer control without a tooltip, for one whose menu is open.
fn footer_control(id: &'static str, label: &SharedString, model: &AppModel) -> Stateful<Div> {
    let theme = &model.theme;
    div()
        .id(id)
        .role(Role::Button)
        .accessibility_id(id)
        .aria_label(label.clone())
        .debug_selector(move || id.into())
        .group(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(footer_button_size(model))
        .rounded(model.metrics.radius_md())
        .cursor_pointer()
        .hover(|style| style.bg(theme.hover))
        .focus(|style| style.bg(theme.hover))
}

/// A footer icon, with a dot on its corner for something worth a look.
pub(super) fn footer_icon(
    group: &'static str,
    icon: Icon,
    color: Hsla,
    dot: Option<(&'static str, Hsla)>,
    model: &AppModel,
) -> Div {
    let m = model.metrics;
    div()
        .relative()
        .flex_none()
        .child(IconGlyph::new(icon, m.icon_md(), color).hover_in_group(group, model.theme.fg))
        .children(dot.map(|(selector, color)| {
            super::remote_servers::dot(color, model)
                .debug_selector(move || selector.into())
                .absolute()
                .top(-m.spacing1())
                .right(-m.spacing1())
        }))
}

fn extensions_button(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    const ID: &str = "sidebar-extensions";
    let m = model.metrics;
    let theme = &model.theme;
    let name = tr!("Extensions");
    let label = match model.extension_update_count() {
        0 => name.to_string(),
        1 => format!("{name} · {}", tr!("1 update available")),
        count => format!("{name} · {}", tr!("%lld updates available", count)),
    };
    let dot =
        (model.extension_update_count() > 0).then_some(("sidebar-extensions-dot", theme.accent));
    let wide = model.appearance.sidebar_expanded;
    let anchor = model.extensions_anchor.clone();
    let open = matches!(
        &model.overlay,
        Some(Overlay::Menu(menu)) if menu.rises_from.is_some() && menu.rises_from == anchor.get()
    );
    let tooltip = muxy_ui::components::tooltip(label.clone(), theme);
    footer_control(ID, &label.into(), model)
        .when(!open, |button| button.tooltip(tooltip))
        .when(wide, |button| {
            button
                .flex_1()
                .min_w(px(0.0))
                .w_auto()
                .justify_start()
                .gap(m.spacing3())
                .px(m.spacing4())
        })
        .aria_expanded(open)
        .when(open, |button| button.bg(theme.hover))
        .button_interaction(cx.listener(|model, _, window, cx| {
            model.open_extensions_menu(window, cx);
        }))
        .child(
            canvas(
                move |bounds, _, _| anchor.set(Some(bounds)),
                |_, (), _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .child(footer_icon(ID, Icon::Puzzle, theme.fg_muted, dot, model))
        .when(wide, |button| {
            button.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .text_size(m.font_body())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.fg_muted)
                    .group_hover(ID, |style| style.text_color(theme.fg))
                    .child(name),
            )
        })
        .into_any_element()
}

fn settings_button(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    const ID: &str = "settings-button";
    footer_button(ID, super::tab_strip::settings_tooltip(model), model)
        .aria_label(tr!("Settings"))
        .button_interaction(cx.listener(|model, _, window, cx| model.open_settings(window, cx)))
        .child(footer_icon(
            ID,
            Icon::Settings,
            model.theme.fg_muted,
            None,
            model,
        ))
        .into_any_element()
}

/// The row after a server's last project, in the list's own layout.
fn add_project_row(server: ServerId, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let wide = model.appearance.sidebar_expanded;
    let id = add_project_id(server);
    div()
        .id(id.clone())
        .role(Role::Button)
        .accessibility_id(id.clone())
        .aria_label(tr!("Add Project"))
        .debug_selector(move || id.to_string())
        .flex()
        .flex_none()
        .items_center()
        .rounded(m.radius_md())
        .cursor_pointer()
        .hover(|style| style.bg(theme.hover))
        .focus(|style| style.bg(theme.hover))
        .when(wide, |row| {
            row.p(m.spacing2())
                .h(tab_sidebar::row_height(model))
                .gap(m.spacing3())
        })
        .when(!wide, |row| {
            row.justify_center()
                .size(collapsed_item_size(model))
                .tooltip(muxy_ui::components::tooltip(
                    tr!("Add Project").to_string(),
                    &model.theme,
                ))
        })
        .button_interaction(
            cx.listener(move |model, event: &gpui::ClickEvent, window, cx| {
                model.choose_project_source(server, event.position(), window, cx);
            }),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(tile_size(model))
                .rounded(m.radius_md())
                .bg(theme.surface)
                .child(IconGlyph::new(Icon::Plus, m.font_body(), theme.fg_muted)),
        )
        .when(wide, |row| {
            row.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .text_size(m.font_body())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.fg_muted)
                    .child(tr!("Add Project")),
            )
        })
        .into_any_element()
}

/// What a client's press on a row runs. It goes straight to the action, since
/// the mouse events a press otherwise becomes land on whatever is at the row's
/// center, and on nothing once the row has scrolled away.
pub(super) fn client_press(
    cx: &Context<AppModel>,
    action: impl Fn(&mut AppModel, &mut Window, &mut Context<AppModel>) + 'static,
) -> impl FnMut(Option<&gpui::accesskit::ActionData>, &mut Window, &mut App) + 'static {
    let press = cx.listener(move |model, (): &(), window, cx| action(model, window, cx));
    move |_, window, cx| press(&(), window, cx)
}

/// A button for what the pointer reaches by hovering or right-clicking a row.
pub(super) fn client_button(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    cx: &Context<AppModel>,
    action: impl Fn(&mut AppModel, Point<Pixels>, &mut Window, &mut Context<AppModel>) + 'static,
) -> Stateful<Div> {
    let press = cx.listener(move |model, position: &Point<Pixels>, window, cx| {
        action(model, *position, window, cx);
    });
    hidden_button(id, name, move |position, window, cx| {
        press(&position, window, cx);
    })
}

/// What applies to a row, read after its name.
pub(super) fn describe(
    parts: impl IntoIterator<Item = Option<SharedString>>,
) -> Option<SharedString> {
    let parts: Vec<_> = parts.into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join(", ").into())
}

impl AppModel {
    /// Offers the two ways into extensions: building one, or managing them.
    pub(crate) fn open_extensions_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let manage = tr!("Manage Extensions");
        let manage = match self.extension_update_count() {
            0 => manage.to_string(),
            1 => format!("{manage} · {}", tr!("1 update available")),
            count => format!("{manage} · {}", tr!("%lld updates available", count)),
        };
        let items = vec![
            Item::action(tr!("Create Extension…"), Command::CreateExtension),
            Item::action(manage, Command::ManageExtensions),
        ];
        let anchor = self.extensions_anchor.get();
        let position = anchor.map_or_else(
            || gpui::point(self.metrics.scaled(8.0), self.metrics.scaled(40.0)),
            |anchor| anchor.bottom_left(),
        );
        self.open_menu(items, position, window, cx);
        if let Some(Overlay::Menu(menu)) = &mut self.overlay {
            menu.rises_from = anchor;
        }
    }

    /// Whether the sidebar's bottom row is on screen. A hidden sidebar and an
    /// extension's sidebar don't draw it.
    pub(crate) fn sidebar_footer_shown(&self) -> bool {
        self.sidebar_width() > 0.0 && !self.extension_sidebar_active()
    }

    pub(crate) fn choose_project_source(
        &mut self,
        server: ServerId,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = self.project_source_items(server);
        self.open_menu(items, position, window, cx);
    }

    /// Add Project's choices: a folder on `server`, then projects removed
    /// from it that can come back.
    fn project_source_items(&self, server: ServerId) -> Vec<Item> {
        let mut items = vec![Item::action(tr!("Folder…"), Command::AddProject(server))];
        let removed = self.restorable_projects(server);
        if !removed.is_empty() {
            items.push(Item::submenu(tr!("Recently Removed"), removed).separated());
        }
        items
    }

    fn restorable_projects(&self, server: ServerId) -> Vec<Item> {
        self.state
            .removed_projects()
            .iter()
            .enumerate()
            .filter(|(_, removed)| {
                removed.server_id == server
                    && self.state.server_home(removed.server_id).is_some()
                    && !self.state.projects().iter().any(|project| {
                        project.server_id == removed.server_id
                            && project.directory == removed.directory
                    })
            })
            .map(|(index, removed)| {
                Item::action(removed.name.clone(), Command::RestoreRemovedProject(index))
            })
            .collect()
    }

    /// The name shown for a project: another computer's Home goes by its
    /// server's name.
    pub(crate) fn project_title(&self, project: &Project) -> String {
        match self.server_name(project.server_id) {
            Some(name) if project.home => name.to_owned(),
            _ => project.name.clone(),
        }
    }

    /// The project's folder, with `~` for its own computer's Home.
    pub(crate) fn project_path_label(&self, project: &Project) -> String {
        if project.server_id.is_local() {
            return super::project_picker::display_path(&project.directory);
        }
        let directory = project.directory.to_string_lossy();
        match self.remote_home(project.server_id) {
            Some(home) if project.directory == home => "~".to_owned(),
            Some(home) => project.directory.strip_prefix(home).map_or_else(
                |_| directory.into_owned(),
                |relative| format!("~/{}", relative.display()),
            ),
            None => directory.into_owned(),
        }
    }

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
        let width = width.clamp(
            self.metrics.sidebar_expanded_min_width(),
            self.metrics.sidebar_expanded_max_width(),
        );
        #[cfg(target_os = "linux")]
        let width = width.min(self.sidebar_available_width);
        width
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn fit_sidebar_to_window(
        &mut self,
        window: &Window,
        actions_width: f32,
        cx: &mut Context<Self>,
    ) {
        let available = (super::titlebar::content_size(window).width
            - px(actions_width)
            - self.metrics.control_medium()
            - self.metrics.spacing2()
            - self.metrics.scaled(56.0))
        .max(self.metrics.sidebar_expanded_min_width());
        if self.sidebar_available_width != available {
            self.sidebar_available_width = available;
            self.sidebar_view.update(cx, |_, cx| cx.notify());
        }
    }

    pub(crate) fn sidebar_filter_label(&self) -> SharedString {
        if self.appearance.sidebar_focus {
            return tr!("Focused Project");
        }
        self.state.active_workspace().map_or_else(
            || tr!("All Projects"),
            |workspace| workspace.name.clone().into(),
        )
    }

    /// Top-level projects the workspace filter lists under the expanded
    /// servers, in sidebar order.
    pub(crate) fn listed_parents(&self) -> Vec<&Project> {
        self.sidebar_servers()
            .into_iter()
            .filter(|server| self.server_expanded(*server))
            .flat_map(|server| self.listed_parents_on(server))
            .collect()
    }

    /// Top-level projects the workspace filter lists on `server`, in sidebar
    /// order.
    fn listed_parents_on(&self, server: ServerId) -> Vec<&Project> {
        let mut parents: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| {
                project.server_id == server
                    && project.parent_id.is_none()
                    && self.state.is_listed(project)
                    && self.project_listed(project)
            })
            .collect();
        self.sort_parents(&mut parents);
        parents
    }

    /// Sorts top-level projects as the sidebar lists them.
    pub(crate) fn sort_parents(&self, parents: &mut [&Project]) {
        self.sort_projects(parents);
    }

    pub(crate) fn sort_projects(&self, projects: &mut [&Project]) {
        match self.appearance.sidebar_project_order {
            ProjectOrder::Manual => {}
            ProjectOrder::Name | ProjectOrder::NameDescending => {
                projects.sort_by(|left, right| {
                    right.home.cmp(&left.home).then_with(|| {
                        let order =
                            crate::picker::path_service::natural_compare(&left.name, &right.name);
                        if self.appearance.sidebar_project_order == ProjectOrder::NameDescending {
                            order.reverse()
                        } else {
                            order
                        }
                    })
                });
            }
            ProjectOrder::RecentlyActive => {
                let recent: std::collections::HashMap<_, _> = self
                    .appearance
                    .project_recent
                    .iter()
                    .enumerate()
                    .map(|(index, id)| (*id, index))
                    .collect();
                projects.sort_by_key(|project| {
                    (
                        !project.home,
                        recent.get(&project.id).copied().unwrap_or(usize::MAX),
                    )
                });
            }
            ProjectOrder::DateAdded => {
                projects.sort_by_key(|project| (!project.home, project.created_at));
            }
        }
    }

    /// The projects the sidebar shows, with their worktrees, in its order.
    pub(crate) fn sidebar_projects(&self) -> Vec<&Project> {
        self.sidebar_groups()
            .into_iter()
            .flat_map(|group| group.projects)
            .collect()
    }

    /// The sidebar's servers, each with the projects it shows under its
    /// heading: none while collapsed. Focus Current Project shows only that
    /// project, under no heading.
    pub(crate) fn sidebar_groups(&self) -> Vec<ServerGroup<'_>> {
        let active = self.state.current_project();
        if self.appearance.sidebar_focus {
            let focused = active.parent_id.unwrap_or(active.id);
            let projects = self
                .listed_parents_on(active.server_id)
                .into_iter()
                .filter(|project| project.id == focused)
                .flat_map(|parent| self.with_worktrees(parent))
                .collect();
            return vec![ServerGroup {
                server: active.server_id,
                heading: false,
                projects,
            }];
        }
        self.sidebar_servers()
            .into_iter()
            .map(|server| ServerGroup {
                server,
                heading: true,
                projects: if self.server_expanded(server) {
                    self.listed_parents_on(server)
                        .into_iter()
                        .flat_map(|parent| self.with_worktrees(parent))
                        .collect()
                } else {
                    Vec::new()
                },
            })
            .collect()
    }

    fn with_worktrees<'a>(&'a self, parent: &'a Project) -> Vec<&'a Project> {
        let children = if self.appearance.layout != AppLayout::ProjectFocused
            && self.worktrees_visible(parent.id)
        {
            self.worktree_children(parent.id)
        } else {
            Vec::new()
        };
        std::iter::once(parent).chain(children).collect()
    }
}

/// One server in the sidebar and the projects it shows.
pub(crate) struct ServerGroup<'a> {
    pub(crate) server: ServerId,
    /// Whether the server's heading shows above its projects.
    pub(crate) heading: bool,
    pub(crate) projects: Vec<&'a Project>,
}

/// The Add Project row of `server`'s group. This computer's keeps the
/// plain name.
fn add_project_id(server: ServerId) -> SharedString {
    if server.is_local() {
        "add-project".into()
    } else {
        format!("add-project-{server}").into()
    }
}

/// Which sidebar a server heading lines up with.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum HeadingRows {
    /// Project rows, whose tiles start the heading's name.
    Projects,
    /// The tab layouts' project headers: the name starts where theirs do,
    /// and the menu button sits where their disclosure does.
    Tabs,
}

/// A server's heading in the sidebar: its name, which expands or collapses
/// its projects when clicked, and a button for the server's menu, which a
/// right click opens too. Collapsed to icons, it is a short rule.
pub(super) fn server_heading(
    server: ServerId,
    rows: HeadingRows,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let wide = model.appearance.sidebar_expanded;
    let expanded = model.server_expanded(server);
    let name = SharedString::from(model.server_label(server));
    let status = server_status_label(server, model);
    let id = SharedString::from(format!("sidebar-server-{server}"));
    let selector = if server.is_local() {
        SharedString::from("sidebar-server-local")
    } else {
        id.clone()
    };
    let group = id.clone();
    div()
        .id(id.clone())
        .role(Role::Button)
        .accessibility_id(id)
        .aria_label(name.clone())
        .aria_description(status.clone())
        .aria_expanded(expanded)
        .debug_selector(move || selector.to_string())
        .group(group.clone())
        .flex()
        .flex_none()
        .items_center()
        .cursor_pointer()
        .when(!wide, |row| {
            row.tooltip(muxy_ui::components::tooltip(
                format!("{name} · {status}"),
                theme,
            ))
        })
        .button_interaction(cx.listener(move |model, _, _, cx| {
            model.toggle_server_group(server, cx);
        }))
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                cx.stop_propagation();
                model.open_sidebar_server_menu(server, event.position, window, cx);
            }),
        )
        .when(wide, |row| {
            row.h(m.control_small())
                .gap(m.spacing2())
                .map(|row| match rows {
                    HeadingRows::Projects => row.pl(m.spacing2()),
                    HeadingRows::Tabs => row.pl(m.spacing4()),
                })
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .text_size(m.font_caption())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.fg_muted)
                        .group_hover(group, |style| style.text_color(theme.fg))
                        .child(name.clone()),
                )
                .child(server_menu_button(server, &name, model, cx))
        })
        .when(!wide, |row| {
            row.justify_center()
                .w(collapsed_item_size(model))
                .h(m.control_small())
                .child(
                    div()
                        .w(m.scaled(12.0))
                        .h(px(1.0))
                        .flex_none()
                        .bg(theme.border),
                )
        })
        .into_any_element()
}

/// The ellipsis at the end of a server's heading, which opens its menu. It
/// takes keyboard focus of its own, apart from the heading's, and opens the
/// menu under itself when activated from the keyboard.
fn server_menu_button(
    server: ServerId,
    name: &SharedString,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let m = model.metrics;
    let id = SharedString::from(format!("sidebar-server-menu-{server}"));
    let group = id.clone();
    let bounds = Rc::new(Cell::new(None::<Bounds<Pixels>>));
    let anchor = bounds.clone();
    div()
        .id(id.clone())
        .role(Role::Button)
        .accessibility_id(id.clone())
        .aria_label(tr!("Server Actions for %@", name.as_ref()))
        .debug_selector(move || id.to_string())
        .group(group.clone())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(m.control_small())
        .rounded(m.radius_sm())
        .cursor_pointer()
        .focus(|style| style.bg(theme.hover))
        .tooltip(muxy_ui::components::tooltip(tr!("Server Actions"), theme))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .button_interaction(
            cx.listener(move |model, event: &gpui::ClickEvent, window, cx| {
                cx.stop_propagation();
                let position = match event {
                    gpui::ClickEvent::Mouse(_) => None,
                    gpui::ClickEvent::Keyboard(_) => anchor.get().map(|b| b.bottom_left()),
                }
                .unwrap_or_else(|| window.mouse_position());
                model.open_sidebar_server_menu(server, position, window, cx);
            }),
        )
        .child(
            canvas(move |b, _, _| bounds.set(Some(b)), |_, (), _, _| {})
                .absolute()
                .size_full(),
        )
        .child(
            IconGlyph::new(Icon::Ellipsis, m.icon_xs(), theme.fg_dim)
                .hover_in_group(group, theme.fg),
        )
        .into_any_element()
}

/// What a server's heading says about how it is doing.
fn server_status_label(server: ServerId, model: &AppModel) -> SharedString {
    if let Some(remote) = model
        .remote_servers()
        .into_iter()
        .find(|remote| remote.entry.id == server)
    {
        return super::remote_servers::status_label(&remote);
    }
    muxy_ui::l10n::translate(model.status_of(server).label())
}

/// What an expanded server with nothing to list yet is doing.
fn server_note(server: ServerId, model: &AppModel) -> Option<AnyElement> {
    if server.is_local()
        || !model.appearance.sidebar_expanded
        || model.state.server_home(server).is_some()
    {
        return None;
    }
    let m = model.metrics;
    let status = server_status_label(server, model);
    let id = format!("sidebar-server-note-{server}");
    Some(
        div()
            .debug_selector(move || id.clone())
            .px(m.spacing2())
            .text_size(m.font_footnote())
            .text_color(model.theme.fg_dim)
            .truncate()
            .child(status)
            .into_any_element(),
    )
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
                .role(Role::Button)
                .accessibility_id("sidebar-project-filter")
                .aria_label(tr!(
                    "Workspace Filter: %@",
                    model.sidebar_filter_label().as_ref()
                ))
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
                .role(Role::Button)
                .accessibility_id("sidebar-project-sort")
                .aria_label(tr!("Sort Projects"))
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
                        [
                            (tr!("Manual Order"), ProjectOrder::Manual),
                            (tr!("Name (A–Z)"), ProjectOrder::Name),
                            (tr!("Name (Z–A)"), ProjectOrder::NameDescending),
                            (tr!("Recently Active"), ProjectOrder::RecentlyActive),
                            (tr!("Date Added"), ProjectOrder::DateAdded),
                        ]
                        .into_iter()
                        .map(|(title, order)| {
                            Item::action(title, Command::SortProjects(order))
                                .checked_if(model.appearance.sidebar_project_order == order)
                        })
                        .collect(),
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

fn current_parent(model: &AppModel) -> ProjectId {
    let project = model.state.current_project();
    project.parent_id.unwrap_or(project.id)
}

fn filter_items(model: &AppModel) -> Vec<Item> {
    let focused = model.appearance.sidebar_focus;
    let active = model.state.active_workspace().map(|workspace| workspace.id);
    let mut items = vec![
        Item::action(tr!("All Projects"), Command::SelectWorkspace(None))
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
        Item::action(
            tr!("Focus Current Project"),
            Command::FocusProject(Some(current_parent(model))),
        )
        .checked_if(focused)
        .separated(),
    );
    if focused {
        items.push(
            Item::submenu(
                tr!("Add Project"),
                model.project_source_items(model.state.current_project().server_id),
            )
            .separated(),
        );
    }
    items.push(Item::action(tr!("New Workspace…"), Command::NewWorkspace(None)).separated());
    if let Some(id) = active {
        items.push(Item::action(
            tr!("Rename Workspace…"),
            Command::RenameWorkspace(id),
        ));
        items.push(Item::action(
            tr!("Delete Workspace…"),
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
        .role(Role::Button)
        .accessibility_id("layout-menu")
        .aria_label(tr!("Layout"))
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
                        tr!("Project Focused"),
                        Command::Layout(AppLayout::ProjectFocused),
                    )
                    .checked_if(model.appearance.layout == AppLayout::ProjectFocused),
                    Item::action(tr!("Tab Focused"), Command::Layout(AppLayout::TabFocused))
                        .checked_if(model.appearance.layout == AppLayout::TabFocused),
                    Item::action(
                        tr!("Agents Focused"),
                        Command::Layout(AppLayout::AgentsFocused),
                    )
                    .checked_if(model.appearance.layout == AppLayout::AgentsFocused),
                    Item::action(
                        if model.appearance.agents_panel_visible {
                            tr!("Hide agents panel")
                        } else {
                            tr!("Show agents panel")
                        },
                        Command::AgentsPanel(!model.appearance.agents_panel_visible),
                    )
                    .separated(),
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
        if model.coverage().modal()
            || model.appearance.sidebar_project_order != ProjectOrder::Manual
        {
            return;
        }
        let server = model.state.project_server(self.id);
        let target = target
            .filter(|target| *target != self.id && model.state.project_server(*target) == server);
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
    let (rows, ids) = grouped_rows(model, cx);
    div()
        .id("projects-scroll")
        .role(Role::Tree)
        .accessibility_id("projects-scroll")
        .aria_label(tr!("Projects"))
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
                .gap(if wide { m.spacing2() } else { m.spacing3() })
                .pt(if wide { m.spacing5() } else { m.spacing2() })
                .pb(m.spacing3())
                .when(!wide, Styled::items_center)
                .children(rows)
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

/// Each server's heading, its projects, and its Add Project row, with the
/// project a row can be dragged onto, if any.
fn grouped_rows(
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> (Vec<AnyElement>, Vec<Option<ProjectId>>) {
    let m = model.metrics;
    let wide = model.appearance.sidebar_expanded;
    let shortcuts = model.navigation_projects();
    let mut rows = Vec::new();
    let mut ids = Vec::new();
    let mut index = 0;
    for (position, group) in model.sidebar_groups().into_iter().enumerate() {
        let expanded = !group.heading || model.server_expanded(group.server);
        if group.heading {
            rows.push(
                div()
                    .when(position > 0, |heading| heading.pt(m.spacing3()))
                    .child(server_heading(
                        group.server,
                        HeadingRows::Projects,
                        model,
                        cx,
                    ))
                    .into_any_element(),
            );
            ids.push(None);
        }
        for project in group.projects {
            let shortcut = shortcuts.iter().take(9).position(|id| *id == project.id);
            rows.push(worktrees::group(project, index, shortcut, model, cx));
            ids.push(
                (!project.home
                    && project.parent_id.is_none()
                    && project.status() == ProjectStatus::Available)
                    .then_some(project.id),
            );
            index += 1;
        }
        if !expanded {
            continue;
        }
        if group.heading
            && let Some(note) = server_note(group.server, model)
        {
            rows.push(note);
            ids.push(None);
        }
        if !model.appearance.sidebar_focus || !wide {
            rows.push(add_project_row(group.server, model, cx));
            ids.push(None);
        }
    }
    (rows, ids)
}

#[allow(
    clippy::too_many_lines,
    reason = "Declarative project row and interaction layout"
)]
fn project_row(
    project: &Project,
    index: usize,
    shortcut: Option<usize>,
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
    let hint = shortcut.and_then(|index| {
        model.shortcut_hints.label(
            muxy_core::shortcuts::ShortcutId::PROJECTS[index],
            &model.settings.keymap,
        )
    });
    let hinted = hint.is_some();
    let tile = hint.map_or_else(
        || project_tile(project, model, tile_size(model), Some(group.clone())),
        |label| {
            div()
                .debug_selector(move || format!("project-shortcut-{id}"))
                .child(super::shortcut_hints::badge(label, tile_size(model), model))
                .into_any_element()
        },
    );
    let activity = tab_activity::project_status(id, model);
    // A collapsed tile dims under its spinner rather than wearing it as a badge.
    let dimmed = !wide
        && !hinted
        && matches!(
            activity,
            tab_activity::Status::Busy(_) | tab_activity::Status::Progress(_)
        );
    let name = SharedString::from(model.project_title(project));
    let description = project_description(missing, remote_description(project, model), activity);
    let expanded = has_worktrees.then(|| wide && model.expanded_worktrees.contains(&id));
    let drag = DraggedProject {
        id,
        last_target: Cell::new(None),
    };
    div()
        .id(group.clone())
        .role(Role::TreeItem)
        .accessibility_id(format!("project-row-{}", project.name))
        .aria_label(name.clone())
        .aria_selected(active)
        .aria_level(project_level(project))
        .aria_disabled(missing)
        .when_some(expanded, StatefulInteractiveElement::aria_expanded)
        .when_some(description, StatefulInteractiveElement::aria_description)
        .debug_selector(|| format!("project-row-{index}"))
        .group(group)
        .relative()
        .when(!missing, |row| {
            row.child(super::move_drag::zone(
                &model.move_drag.zones,
                super::move_drag::Owner::Sidebar,
                super::move_drag::ZoneKind::Project(id),
            ))
        })
        .flex()
        .flex_none()
        .items_center()
        .when(wide && project.parent_id.is_some(), |row| {
            row.ml(m.spacing5())
        })
        .when(wide, |row| {
            row.p(m.spacing2())
                .h(tab_sidebar::row_height(model))
                .gap(m.spacing3())
                .rounded(m.radius_md())
                .when(active, |row| row.bg(theme.surface))
                .hover(|style| style.bg(if active { theme.surface } else { theme.hover }))
        })
        .when(!wide, |row| {
            row.justify_center().size(collapsed_item_size(model))
        })
        .when(missing, |row| row.opacity(0.5))
        .when(!missing, |row| {
            let opens_worktrees = active && has_worktrees;
            row.cursor_pointer()
                .on_click(cx.listener(move |model, _, window, cx| {
                    press_project(model, id, opens_worktrees, wide, window, cx);
                }))
                .on_a11y_action(
                    AccessibleAction::Click,
                    client_press(cx, move |model, window, cx| {
                        press_project(model, id, opens_worktrees, wide, window, cx);
                    }),
                )
        })
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                if let Some(project) = model.state.project(id) {
                    model.open_menu(model.project_menu(project), event.position, window, cx);
                }
            }),
        )
        .child(project_actions(project, &name, cx))
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
                    .truncate()
                    .text_size(m.font_body())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.fg)
                    .child(name),
            )
        })
        .when(dimmed, |row| {
            row.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(tile_size(model))
                            .rounded(m.radius_md())
                            .bg(theme.bg.opacity(0.6))
                            .child(tab_activity::status_glyph(
                                format!("project-activity-{id}"),
                                activity,
                                m.icon_lg(),
                                model,
                            )),
                    ),
            )
        })
        .when(activity != tab_activity::Status::None && !dimmed, |row| {
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
                    .child(tab_activity::status_glyph(
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

fn press_project(
    model: &mut AppModel,
    id: ProjectId,
    opens_worktrees: bool,
    wide: bool,
    window: &mut Window,
    cx: &mut Context<AppModel>,
) {
    if opens_worktrees && wide {
        model.toggle_worktree_list(id, cx);
    } else if opens_worktrees {
        model.expanded_worktrees.insert(id);
        model.toggle_sidebar(window, cx);
    } else {
        model.select_project(model.preferred_worktree(id), cx);
        model.focus_active(window, cx);
    }
}

/// Opens the menu a project's right click opens.
pub(super) fn project_actions(
    project: &Project,
    title: &str,
    cx: &Context<AppModel>,
) -> Stateful<Div> {
    let id = project.id;
    client_button(
        format!("project-actions-{}", project.name),
        tr!("Actions for %@", title),
        cx,
        move |model, position, window, cx| {
            if let Some(project) = model.state.project(id) {
                model.open_menu(model.project_menu(project), position, window, cx);
            }
        },
    )
}

/// A worktree sits under its project.
pub(super) fn project_level(project: &Project) -> usize {
    if project.parent_id.is_some() { 2 } else { 1 }
}

/// What a client reads after a project's name, beyond its selected state.
pub(super) fn project_description(
    missing: bool,
    remote: Option<SharedString>,
    activity: tab_activity::Status,
) -> Option<SharedString> {
    describe([
        missing.then(|| tr!("Project folder is missing")),
        remote,
        tab_sidebar::activity_description(activity),
    ])
}

pub(super) fn remote_description(project: &Project, model: &AppModel) -> Option<SharedString> {
    if project.home || project.server_id.is_local() {
        return None;
    }
    model
        .server_name(project.server_id)
        .map(|name| tr!("Remote project on %@", name))
}

pub(super) fn tile_size(model: &AppModel) -> Pixels {
    if model.appearance.sidebar_expanded {
        model.metrics.control_small()
    } else {
        model.metrics.icon_xxl()
    }
}

pub(crate) fn project_color(project: &Project, model: &AppModel) -> gpui::Rgba {
    if project.home && project.server_id.is_local() {
        model.theme.accent.to_rgb()
    } else {
        parse_hex(project.color.as_str()).unwrap_or(gpui::rgb(0x80_80_80))
    }
}

pub(crate) fn project_tile(
    project: &Project,
    model: &AppModel,
    size: Pixels,
    group: Option<SharedString>,
) -> AnyElement {
    let m = model.metrics;
    let id = project.id;
    let scale = f32::from(size) / f32::from(m.icon_xxl());
    let (background, foreground) = tile_colors(project_color(project, model), &model.theme);
    let glyph = if let Some((_, logo)) = model.project_logos.get(&project.id) {
        gpui::img(logo.clone())
            .size(size)
            .rounded(m.radius_md() * scale)
            .object_fit(gpui::ObjectFit::Cover)
            .into_any_element()
    } else if let Some(icon) = project
        .icon
        .as_ref()
        .filter(|icon| muxy_ui::symbols::PROJECT_ICONS || !icon.starts_with("sf:"))
    {
        if let Some(symbol) = icon.strip_prefix("sf:") {
            SymbolGlyph::new(symbol.to_owned(), m.font_title_large() * scale, foreground)
                .into_any_element()
        } else {
            div()
                .text_size(m.font_title_large() * scale)
                .child(icon.clone())
                .into_any_element()
        }
    } else if let Some(image) = model.project_icons.image(project) {
        gpui::img(image)
            .size(size)
            .rounded(m.radius_md() * scale)
            .object_fit(gpui::ObjectFit::Contain)
            .into_any_element()
    } else if project.home && project.server_id.is_local() {
        SymbolGlyph::new("house.fill", m.font_title_large() * scale, foreground).into_any_element()
    } else {
        div()
            .text_size(m.font_emphasis() * scale)
            .font_weight(FontWeight::BOLD)
            .text_color(foreground)
            .child(muxy_app_core::initial(&model.project_title(project)).to_uppercase())
            .into_any_element()
    };
    div()
        .debug_selector(move || format!("project-tile-{id}"))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size)
        .rounded(m.radius_md() * scale)
        .when(!model.project_logos.contains_key(&project.id), |tile| {
            tile.bg(background)
        })
        .overflow_hidden()
        .when_some(group, |tile, group| {
            tile.group_hover(group, |style| style.opacity(0.85))
        })
        .child(glyph)
        .into_any_element()
}

fn tile_colors(color: gpui::Rgba, theme: &muxy_ui::theme::Theme) -> (Hsla, Hsla) {
    const TINT: f32 = 0.2;
    const MARK_LIGHTNESS: f32 = 0.75;
    if theme.bg.l >= 0.5 {
        return (color.into(), contrasting_foreground(color).into());
    }
    let color = Hsla::from(color);
    let mark = Hsla {
        l: color.l.max(MARK_LIGHTNESS),
        ..color
    };
    (color.opacity(TINT), mark)
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_ui::theme::{ColorScheme, Theme};

    fn theme(background: &str) -> Theme {
        Theme::from_scheme(&ColorScheme::parse(&format!("background = {background}")))
    }

    #[test]
    fn dark_themes_tint_tiles_and_lighten_their_marks() {
        let red = gpui::rgb(0xe5_48_4d);
        let (background, mark) = tile_colors(red, &theme("#19171f"));
        assert!((background.a - 0.2).abs() < f32::EPSILON);
        assert!((background.h - Hsla::from(red).h).abs() < 0.001);
        assert!(mark.l >= 0.75 && (mark.a - 1.0).abs() < f32::EPSILON);

        let (background, mark) = tile_colors(red, &theme("#ffffff"));
        assert_eq!(background, Hsla::from(red));
        assert_eq!(mark, Hsla::from(contrasting_foreground(red)));
    }

    #[test]
    fn a_project_row_names_a_missing_folder_and_a_remote_computer() {
        assert_eq!(
            project_description(false, None, tab_activity::Status::None),
            None,
        );
        assert_eq!(
            project_description(
                true,
                Some("Remote project on nas".into()),
                tab_activity::Status::Blocked,
            ),
            Some(
                "Project folder is missing, Remote project on nas, \
                 An agent is waiting for your attention."
                    .into()
            ),
        );
    }

    #[test]
    fn a_project_row_reports_failed_work() {
        let failed = muxy_protocol::TerminalProgress {
            state: muxy_protocol::ProgressState::Error,
            percent: None,
        };
        assert_eq!(
            project_description(false, None, tab_activity::Status::Progress(failed)),
            Some("Work reported an error.".into()),
        );
    }
}
