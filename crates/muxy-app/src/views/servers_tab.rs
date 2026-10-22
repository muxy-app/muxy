//! The Remote Servers tab: every remote server by group, to select and act
//! on one or many at once.

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, ClickEvent, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use muxy_app_core::{PaneId, ServerId};
use muxy_ui::components::{IconButton, IconGlyph};
use muxy_ui::controls::{self, Style};
use muxy_ui::icon::Icon;
use muxy_ui::l10n::tr_key;
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};
use muxy_ui::tr;

use super::remote_servers::{dot, status_color, status_label};
use super::server_menu::{Action, Servers, connect_choice, live};
use crate::model::{AppModel, RemoteServer, ServerStatus};

/// What the Remote Servers tab keeps between frames. Its selection is the
/// model's `remote_selection`.
pub(crate) struct ServersTab {
    pub(crate) focus: FocusHandle,
    search: Option<(Entity<TextInput>, Subscription)>,
}

impl ServersTab {
    pub(crate) fn new(cx: &mut Context<AppModel>) -> Self {
        Self {
            focus: cx.focus_handle(),
            search: None,
        }
    }

    pub(crate) fn search_input(&self) -> Option<&Entity<TextInput>> {
        self.search.as_ref().map(|(input, _)| input)
    }
}

/// `servers` by group, keeping those where each word of `query` is in the
/// name, destination, or group, ignoring case.
fn shown<'a>(
    servers: &'a [RemoteServer],
    query: &str,
) -> Vec<(Option<&'a str>, Vec<&'a RemoteServer>)> {
    let groups = crate::model::by_group(servers, |server| &server.entry);
    if query.trim().is_empty() {
        return groups;
    }
    let query = query.to_lowercase();
    let matches = |server: &RemoteServer| {
        let entry = &server.entry;
        let text = [
            entry.name.as_str(),
            entry.destination(),
            entry.group.as_deref().unwrap_or_default(),
        ]
        .join(" ")
        .to_lowercase();
        query.split_whitespace().all(|word| text.contains(word))
    };
    groups
        .into_iter()
        .map(|(group, members)| {
            (
                group,
                members
                    .into_iter()
                    .filter(|server| matches(server))
                    .collect::<Vec<_>>(),
            )
        })
        .filter(|(_, members)| !members.is_empty())
        .collect()
}

impl AppModel {
    /// The tab's search field, made once.
    pub(crate) fn ensure_servers_search(&mut self, cx: &mut Context<Self>) {
        if self.servers_tab.search.is_some() {
            return;
        }
        let style = InputStyle::compact(&self.theme, &self.metrics);
        let input =
            cx.new(|cx| TextInput::new(style, cx).with_placeholder_key(tr_key!("Search servers…")));
        let subscription = cx.subscribe(&input, |model, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                let shown = model.shown_server_ids(cx);
                model
                    .remote_selection
                    .retain(|selected| shown.contains(selected));
                cx.notify();
            } else if matches!(event, InputEvent::Cancelled) {
                model.clear_servers_search(cx);
            }
        });
        self.servers_tab.search = Some((input, subscription));
    }

    fn clear_servers_search(&mut self, cx: &mut Context<Self>) {
        if let Some((input, _)) = &self.servers_tab.search {
            input.update(cx, |input, cx| input.set_text("", cx));
        }
    }

    fn servers_query(&self, cx: &gpui::App) -> String {
        self.servers_tab
            .search_input()
            .map(|input| input.read(cx).text().to_owned())
            .unwrap_or_default()
    }

    /// The servers the tab shows now, in order.
    fn shown_server_ids(&self, cx: &gpui::App) -> Vec<ServerId> {
        let servers = self.remote_servers();
        shown(&servers, &self.servers_query(cx))
            .into_iter()
            .flat_map(|(_, members)| members)
            .map(|server| server.entry.id)
            .collect()
    }

    /// Ticks a server or clears its box; ⇧-click ticks every shown server
    /// from the last one ticked.
    fn tick_server(&mut self, id: ServerId, range: bool, cx: &mut Context<Self>) {
        let shown = self.shown_server_ids(cx);
        let position = |server| shown.iter().position(|listed| *listed == server);
        let anchor = self.remote_selection_anchor.and_then(position);
        if let (true, Some(anchor), Some(clicked)) = (range, anchor, position(id)) {
            let (first, last) = (anchor.min(clicked), anchor.max(clicked));
            for server in &shown[first..=last] {
                if !self.remote_selection.contains(server) {
                    self.remote_selection.push(*server);
                }
            }
        } else if self.remote_selection.contains(&id) {
            self.remote_selection.retain(|selected| *selected != id);
        } else {
            self.remote_selection.push(id);
        }
        self.remote_selection_anchor = Some(id);
        cx.notify();
    }

    /// Ticks every shown server, or clears them all once all are ticked.
    fn tick_all_servers(&mut self, cx: &mut Context<Self>) {
        let shown = self.shown_server_ids(cx);
        if !shown.is_empty() && shown.iter().all(|id| self.remote_selection.contains(id)) {
            self.remote_selection.clear();
        } else {
            for server in shown {
                if !self.remote_selection.contains(&server) {
                    self.remote_selection.push(server);
                }
            }
        }
        self.remote_selection_anchor = None;
        cx.notify();
    }
}

/// How wide the tab's content grows, so rows stay easy to scan.
const CONTENT_WIDTH: f32 = 960.0;

/// The Remote Servers tab in `pane`, laid out like the Extensions manager.
pub(crate) fn render(pane: PaneId, model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let theme = &model.theme;
    let servers = model.remote_servers();
    let shown = shown(&servers, &model.servers_query(cx));
    let everyone = crate::model::by_group(&servers, |server| &server.entry);
    let selected: Vec<_> = everyone
        .iter()
        .flat_map(|(_, members)| members)
        .copied()
        .filter(|server| model.remote_selection.contains(&server.entry.id))
        .collect();
    let selection: Vec<_> = selected.iter().map(|server| server.entry.id).collect();
    let groups: Vec<_> = shown
        .iter()
        .map(|(group, members)| {
            let whole = everyone
                .iter()
                .find(|(name, _)| name == group)
                .map_or(&[][..], |(_, members)| members.as_slice());
            let section = Section {
                group: *group,
                members,
                whole,
                selection: &selection,
            };
            group_section(&section, model, cx)
        })
        .collect();
    div()
        .id("servers-tab")
        .debug_selector(|| "servers-tab".into())
        .track_focus(&model.servers_tab.focus)
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.bg)
        .text_color(theme.fg)
        .text_size(model.metrics.font_body())
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |model, _, _, cx| model.focus_pane(pane, cx)),
        )
        .child(column(header(model, cx), model).flex_none())
        .child(list(&selected, &shown, groups, model, cx))
        .into_any_element()
}

fn style(model: &AppModel) -> Style<'_> {
    Style {
        theme: &model.theme,
        metrics: &model.metrics,
    }
}

/// `content` in a centered column no wider than `CONTENT_WIDTH`.
fn column(content: impl IntoElement, model: &AppModel) -> gpui::Div {
    div().flex().justify_center().px(px(32.0)).child(
        div()
            .w_full()
            .max_w(model.metrics.scaled(CONTENT_WIDTH))
            .child(content),
    )
}

fn icon_button(
    id: String,
    icon: Icon,
    tooltip: SharedString,
    model: &AppModel,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let theme = &model.theme;
    div()
        .debug_selector({
            let id = id.clone();
            move || id.clone()
        })
        .child(
            IconButton::new(
                SharedString::from(id),
                icon,
                model.metrics.icon_sm(),
                model.metrics.control_small(),
                theme.fg_muted,
                theme.fg,
            )
            .tooltip(tooltip, theme.raised(), theme.fg, theme.border, theme.bg)
            .on_click(on_click),
        )
        .into_any_element()
}

/// A short status label, in accent while connected, as Extensions marks one
/// that is enabled.
fn badge(server: &RemoteServer, model: &AppModel) -> gpui::Div {
    let m = model.metrics;
    let theme = &model.theme;
    let connected = server.status == ServerStatus::Connected;
    div()
        .flex_none()
        .px(m.spacing3())
        .py(m.spacing1())
        .rounded(m.radius_sm())
        .text_size(m.font_footnote())
        .text_color(if connected {
            theme.accent
        } else {
            theme.fg_muted
        })
        .bg(if connected {
            theme.accent_soft
        } else {
            theme.hover
        })
        .child(status_label(server))
}

fn card(model: &AppModel) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .w_full()
        .min_w(px(0.0))
        .rounded(model.metrics.radius_lg())
        .bg(model.theme.surface)
        .border_1()
        .border_color(model.theme.border)
}

/// The icon, the title, and Add Server.
fn header(model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap(m.spacing6())
        .py(m.spacing7())
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .items_center()
                .gap(m.spacing4())
                .child(IconGlyph::new(Icon::Network, m.icon_md(), theme.fg_muted))
                .child(
                    div()
                        .text_size(m.font_title())
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr!("Remote Servers")),
                ),
        )
        .child(
            controls::button(
                style(model),
                "servers-add",
                &tr!("Add Server…"),
                true,
                cx.listener(|model, _, _, cx| model.open_server_form(None, cx)),
            )
            .debug_selector(|| "servers-add".into()),
        )
        .into_any_element()
}

/// The search, Select all with what the ticked servers can do, and the
/// groups.
fn list(
    selected: &[&RemoteServer],
    shown: &[(Option<&str>, Vec<&RemoteServer>)],
    groups: Vec<AnyElement>,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let count: usize = shown.iter().map(|(_, members)| members.len()).sum();
    let body = div()
        .flex()
        .flex_col()
        .gap(m.spacing7())
        .py(m.spacing7())
        .children(model.servers_tab.search_input().map(|input| {
            controls::search_field(style(model), "servers-search", input)
                .debug_selector(|| "servers-search".into())
        }))
        .when(count > 0, |body| {
            body.child(select_bar(selected, count, model, cx))
        })
        .when(count == 0, |body| body.child(empty_state(model, cx)))
        .children(groups);
    div()
        .id("servers-list")
        .flex_1()
        .min_h(px(0.0))
        .overflow_y_scroll()
        .child(column(body, model))
        .into_any_element()
}

/// Select all, then what the ticked servers can do once any is ticked. Its
/// box lines up with the rows' boxes.
fn select_bar(
    selected: &[&RemoteServer],
    shown: usize,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let any = !selected.is_empty();
    let all = any && selected.len() >= shown;
    let offline = selected
        .iter()
        .any(|server| server.status == ServerStatus::Disconnected);
    let connected = selected.iter().any(|server| live(server));
    let button = |id: &'static str, label: SharedString, enabled: bool, action: Action| {
        controls::button(
            style(model),
            id,
            &label,
            enabled,
            cx.listener(move |model, _, window, cx| {
                let position = window.mouse_position();
                model.perform_server_menu(Servers::Selected, action, position, window, cx);
            }),
        )
        .debug_selector(move || id.into())
    };
    div()
        .debug_selector(|| "servers-selection".into())
        .flex()
        .flex_wrap()
        .items_center()
        .gap(m.spacing3())
        .min_h(m.control_medium() + m.spacing3() * 2.0 + px(2.0))
        .px(m.spacing6())
        .py(m.spacing3())
        .rounded(m.radius_lg())
        .border_1()
        .border_color(if any { theme.accent_soft } else { theme.border })
        .bg(if any {
            theme.accent_soft
        } else {
            theme.surface
        })
        .child(controls::checkbox(
            style(model),
            "servers-select-all",
            all,
            any && !all,
            cx.listener(|model, _, _, cx| model.tick_all_servers(cx)),
        ))
        .child(
            div()
                .id("servers-select-all-label")
                .debug_selector(|| "servers-select-all-label".into())
                .flex_1()
                .min_w(px(0.0))
                .pl(m.spacing2())
                .when(any, |label| label.font_weight(FontWeight::MEDIUM))
                .when(!any, |label| {
                    label
                        .text_color(theme.fg_muted)
                        .cursor_pointer()
                        .on_click(cx.listener(|model, _, _, cx| model.tick_all_servers(cx)))
                })
                .child(if any {
                    tr!("%lld selected", selected.len())
                } else {
                    tr!("Select all")
                }),
        )
        .when(any, |bar| {
            bar.child(button(
                "servers-connect",
                tr!("Connect"),
                offline,
                Action::Connect,
            ))
            .child(button(
                "servers-disconnect",
                tr!("Disconnect"),
                connected,
                Action::Disconnect,
            ))
            .child(
                controls::button(
                    style(model),
                    "servers-move",
                    &tr!("Move to Group…"),
                    true,
                    cx.listener(|model, _, window, cx| {
                        let position = window.mouse_position();
                        model.open_move_menu(Servers::Selected, position, window, cx);
                    }),
                )
                .debug_selector(|| "servers-move".into()),
            )
            .child(button(
                "servers-remove",
                tr!("Remove…"),
                true,
                Action::Remove,
            ))
        })
        .into_any_element()
}

/// No servers yet, or none matching the search, as Extensions says so.
fn empty_state(model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let none = model.settings.servers.is_empty();
    let (title, description) = if none {
        (
            tr!("No remote servers yet"),
            tr!("Add a computer you reach over SSH to work with its projects here."),
        )
    } else {
        (
            tr!("No matching servers"),
            tr!("Try a different name or clear your search."),
        )
    };
    card(model)
        .items_center()
        .gap(m.spacing6())
        .p(m.spacing9())
        .child(IconGlyph::new(Icon::Network, m.icon_xxl(), theme.fg_muted))
        .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
        .child(
            div()
                .text_color(theme.fg_muted)
                .text_center()
                .child(description),
        )
        .when(none, |empty| {
            empty.child(controls::button(
                style(model),
                "servers-empty-add",
                &tr!("Add Server…"),
                true,
                cx.listener(|model, _, _, cx| model.open_server_form(None, cx)),
            ))
        })
        .into_any_element()
}

/// One group as the tab lists it.
struct Section<'a> {
    group: Option<&'a str>,
    /// The servers the search shows.
    members: &'a [&'a RemoteServer],
    /// Every server in the group, which its buttons act on.
    whole: &'a [&'a RemoteServer],
    selection: &'a [ServerId],
}

/// A group's title and actions over a card of its servers.
fn group_section(section: &Section<'_>, model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let selected = |server: &RemoteServer| section.selection.contains(&server.entry.id);
    let rows = section
        .members
        .iter()
        .enumerate()
        .map(|(index, server)| row(server, index == 0, selected(server), model, cx));
    div()
        .flex()
        .flex_col()
        .gap(model.metrics.spacing4())
        .child(heading(section, model, cx))
        .child(card(model).overflow_hidden().children(rows))
        .into_any_element()
}

/// A group's title, its count, Connect, and its menu.
fn heading(section: &Section<'_>, model: &AppModel, cx: &Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let group = section.group;
    let key = group.unwrap_or_default().to_owned();
    let servers = model.group_servers(group).unwrap_or(Servers::Group(None));
    let (_, action) = connect_choice(section.whole);
    let connect = if matches!(action, Action::Reconnect) {
        tr!("Reconnect")
    } else {
        tr!("Connect")
    };
    let (menu_name, more_name) = (group.map(str::to_owned), group.map(str::to_owned));
    div()
        .id(SharedString::from(format!("servers-group-{key}")))
        .debug_selector({
            let key = key.clone();
            move || format!("servers-group-{key}")
        })
        .flex()
        .items_center()
        .gap(m.spacing4())
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                model.open_group_menu(menu_name.as_deref(), event.position, window, cx);
                cx.stop_propagation();
            }),
        )
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .text_size(m.font_emphasis())
                .font_weight(FontWeight::SEMIBOLD)
                .child(group.map_or_else(|| tr!("Ungrouped"), |group| group.to_owned().into())),
        )
        .child(
            div()
                .flex_none()
                .text_size(m.font_footnote())
                .text_color(theme.fg_muted)
                .child(section.members.len().to_string()),
        )
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(m.spacing2())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(controls::button(
                    style(model),
                    &format!("servers-group-connect-{key}"),
                    &connect,
                    !section.whole.is_empty(),
                    cx.listener(move |model, _, window, cx| {
                        let position = window.mouse_position();
                        model.perform_server_menu(servers, action, position, window, cx);
                    }),
                ))
                .child(icon_button(
                    format!("servers-group-more-{key}"),
                    Icon::Ellipsis,
                    tr!("Group Actions"),
                    model,
                    cx.listener(move |model, _, window, cx| {
                        let position = window.mouse_position();
                        model.open_group_menu(more_name.as_deref(), position, window, cx);
                    }),
                )),
        )
        .into_any_element()
}

/// One server in its group's card: its box, its name and state, where it is,
/// why it failed, and its actions on hover.
fn row(
    server: &RemoteServer,
    first: bool,
    selected: bool,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let id = server.entry.id;
    let group = SharedString::from(format!("servers-row-{id}"));
    div()
        .id(group.clone())
        .group(group.clone())
        .debug_selector(move || format!("servers-row-{id}"))
        .flex()
        .items_start()
        .gap(m.spacing5())
        .px(m.spacing6())
        .py(m.spacing5())
        .when(!first, |row| row.border_t_1().border_color(theme.border))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                model.open_server_menu(id, true, event.position, window, cx);
                cx.stop_propagation();
            }),
        )
        .child(
            div()
                .flex_none()
                .h(m.control_small())
                .flex()
                .items_center()
                .gap(m.spacing5())
                .child(controls::checkbox(
                    style(model),
                    &format!("server-{id}"),
                    selected,
                    false,
                    cx.listener(move |model, event: &ClickEvent, _, cx| {
                        model.tick_server(id, event.modifiers().shift, cx);
                    }),
                ))
                .child(dot(status_color(server, model), model)),
        )
        .child(row_text(server, model))
        .child(row_actions(id, group, model, cx))
        .into_any_element()
}

/// A server's name and state, where it is, and why it failed.
fn row_text(server: &RemoteServer, model: &AppModel) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let error = match &server.install {
        Some(crate::model::Install::Failed(error)) => {
            Some(tr!("Install failed: %@", error).to_string())
        }
        _ => server.error.clone(),
    };
    div()
        .flex_1()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(m.spacing1())
        .child(
            div()
                .h(m.control_small())
                .flex()
                .items_center()
                .gap(m.spacing4())
                .child(
                    div()
                        .min_w(px(0.0))
                        .truncate()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(server.entry.name.clone()),
                )
                .child(badge(server, model)),
        )
        .child(
            div()
                .truncate()
                .text_size(m.font_footnote())
                .text_color(theme.fg_muted)
                .child(server.entry.destination().to_owned()),
        )
        .children(error.map(|error| {
            div()
                .text_size(m.font_footnote())
                .text_color(theme.danger)
                .line_clamp(2)
                .child(error)
        }))
        .into_any_element()
}

/// New Tab, Edit, and more, shown on a hovered row.
fn row_actions(
    id: ServerId,
    group: SharedString,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    div()
        .debug_selector(move || format!("servers-row-actions-{id}"))
        .flex()
        .flex_none()
        .items_center()
        .gap(model.metrics.spacing1())
        .opacity(0.0)
        .group_hover(group, |style| style.opacity(1.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(icon_button(
            format!("servers-new-tab-{id}"),
            Icon::Terminal,
            tr!("New Tab"),
            model,
            cx.listener(move |model, _, _, cx| model.new_remote_home_tab(id, cx)),
        ))
        .child(icon_button(
            format!("servers-edit-{id}"),
            Icon::Pencil,
            tr!("Edit…"),
            model,
            cx.listener(move |model, _, _, cx| model.open_server_form(Some(id), cx)),
        ))
        .child(icon_button(
            format!("servers-more-{id}"),
            Icon::Ellipsis,
            tr!("More Actions"),
            model,
            cx.listener(move |model, _, window, cx| {
                let position = window.mouse_position();
                model.open_server_menu(id, false, position, window, cx);
            }),
        ))
        .into_any_element()
}
