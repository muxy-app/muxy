//! The sidebar's Remote section: other computers' servers with their state,
//! and the popover, form, password prompt, and list that manage them.

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, Hsla, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Subscription,
    Window, canvas, div, px,
};
use muxy_app_core::ServerId;
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::components::{ButtonInteraction, IconButton, IconGlyph};
use muxy_ui::controls::{self, Style};
use muxy_ui::icon::Icon;
use muxy_ui::l10n::tr_key;
use muxy_ui::text_input::{InputEvent, InputStyle, TextInput};
use muxy_ui::tr;

use super::command_palette::Handler;
use super::overlays::Overlay;
use crate::model::{AppModel, ConnectionTest, DeviceForm, RemoteServer, ServerStatus};

/// The Add or Edit form for one remote server.
pub(crate) struct ServerForm {
    /// `None` adds a server.
    device: Option<ServerId>,
    name: Entity<TextInput>,
    group: Entity<TextInput>,
    host: Entity<TextInput>,
    command: Entity<TextInput>,
    custom_command: bool,
    user: Entity<TextInput>,
    port: Entity<TextInput>,
    identity: Entity<TextInput>,
    automatic_identity: bool,
    identity_task: Option<gpui::Task<()>>,
    password: Entity<TextInput>,
    password_login: bool,
    error: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl ServerForm {
    fn advance_focus(&self, backwards: bool, window: &mut Window, cx: &gpui::App) {
        let inputs = [
            &self.name,
            &self.group,
            if self.custom_command {
                &self.command
            } else {
                &self.host
            },
            &self.user,
            &self.port,
            &self.identity,
            &self.password,
        ];
        let count = if self.custom_command {
            3
        } else if self.password_login {
            7
        } else {
            6
        };
        let inputs = &inputs[..count];
        let current = inputs
            .iter()
            .position(|input| input.focus_handle(cx).is_focused(window));
        let next = match current {
            Some(index) if backwards => (index + inputs.len() - 1) % inputs.len(),
            Some(index) => (index + 1) % inputs.len(),
            None if backwards => inputs.len() - 1,
            None => 0,
        };
        inputs[next].focus_handle(cx).focus(window);
    }

    fn submitted(&self, cx: &gpui::App) -> DeviceForm {
        let text = |input: &Entity<TextInput>| input.read(cx).text().to_owned();
        DeviceForm {
            name: text(&self.name),
            host: text(&self.host),
            ssh_command: self.custom_command.then(|| text(&self.command)),
            user: text(&self.user),
            port: text(&self.port),
            identity_file: if self.automatic_identity {
                String::new()
            } else {
                text(&self.identity)
            },
            password_login: self.password_login,
            password: text(&self.password),
            group: text(&self.group),
        }
    }
}

/// Asks for a server's password before connecting to it.
pub(crate) struct PasswordPrompt {
    server: ServerId,
    input: Entity<TextInput>,
    _subscription: Subscription,
}

pub(crate) fn status_color(server: &RemoteServer, model: &AppModel) -> Hsla {
    let theme = &model.theme;
    match server.status {
        ServerStatus::Connected => theme.accent,
        ServerStatus::Connecting | ServerStatus::Restarting | ServerStatus::Stopping => {
            theme.warning
        }
        ServerStatus::Disconnected if server.error.is_some() => theme.danger,
        ServerStatus::Disconnected => theme.fg_dim,
    }
}

pub(crate) fn status_label(server: &RemoteServer) -> SharedString {
    if server.needs_password && server.status == ServerStatus::Disconnected {
        tr!("Password needed")
    } else if server.install == Some(crate::model::Install::Running) {
        tr!("Installing…")
    } else {
        muxy_ui::l10n::translate(server.status.label())
    }
}

pub(crate) fn dot(color: Hsla, model: &AppModel) -> gpui::Div {
    div()
        .size(model.metrics.scaled(6.0))
        .flex_none()
        .rounded_full()
        .bg(color)
}

/// How many servers are offline, and whether one failed: the footer speaks
/// up only when something needs a look.
fn offline(servers: &[RemoteServer]) -> (usize, bool) {
    let offline: Vec<_> = servers
        .iter()
        .filter(|server| server.status == ServerStatus::Disconnected)
        .collect();
    let failed = offline.iter().any(|server| server.error.is_some());
    (offline.len(), failed)
}

/// The bottom of the sidebar, level with the status bar: the remote servers,
/// which open their popover above it.
pub(crate) fn section(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let wide = model.appearance.sidebar_expanded;
    let anchor = model.remote_anchor.clone();
    let open = matches!(model.overlay, Some(Overlay::RemoteServers));
    let (offline, failed) = offline(&model.remote_servers());
    let attention = if failed { theme.danger } else { theme.fg_dim };
    let summary = (offline > 0).then(|| tr!("%lld offline", offline));
    let tooltip = summary.as_ref().map_or_else(
        || tr!("Remote Servers").to_string(),
        |summary| format!("{} · {summary}", tr!("Remote Servers")),
    );
    div()
        .id("remote-servers-section")
        .debug_selector(|| "remote-servers-section".into())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .when(wide, |row| {
            row.mx(m.spacing3())
                .mb(m.spacing3())
                .px(m.spacing4())
                .h(m.control_large())
        })
        .when(!wide, |row| {
            row.mx_auto()
                .mb(m.spacing3())
                .justify_center()
                .size(m.scaled(34.0))
        })
        .rounded(m.radius_xl())
        .bg(if open { theme.hover } else { theme.surface })
        .text_color(if open { theme.fg } else { theme.fg_muted })
        .cursor_pointer()
        .hover(|style| style.bg(theme.hover).text_color(theme.fg))
        .when(!wide, |row| {
            row.tooltip(super::status_bar::tooltip(tooltip, model))
        })
        .button_interaction(cx.listener(|model, _, window, cx| {
            model.toggle_remote_popover(window, cx);
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
        .child(IconGlyph::new(
            Icon::Network,
            m.font_body(),
            if !wide && offline > 0 {
                attention
            } else {
                theme.fg_muted
            },
        ))
        .when(wide, |row| {
            row.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .truncate()
                    .text_size(m.font_body())
                    .font_weight(FontWeight::MEDIUM)
                    .child(tr!("Remote")),
            )
            .children(summary.map(|summary| {
                div()
                    .debug_selector(|| "remote-servers-offline".into())
                    .flex_none()
                    .text_size(m.font_footnote())
                    .text_color(attention)
                    .child(summary)
            }))
            .child(IconGlyph::new(
                if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronUp
                },
                m.font_caption(),
                theme.fg_dim,
            ))
        })
        .into_any_element()
}

/// One server in the popover: its state, then why it failed or how to fix it.
fn server_row(server: &RemoteServer, model: &AppModel, cx: &mut Context<AppModel>) -> gpui::Div {
    let m = model.metrics;
    let theme = &model.theme;
    let id = server.entry.id;
    let group = SharedString::from(format!("remote-server-{id}"));
    let error = match &server.install {
        Some(crate::model::Install::Failed(error)) => {
            Some(tr!("Install failed: %@", error).to_string())
        }
        _ => server.error.clone(),
    };
    let hint = (server.missing && crate::model::released_version().is_none())
        .then(|| tr!("Install a Linux build there by hand, from scripts/build-linux-dev.sh."));
    div()
        .flex()
        .flex_col()
        .child(
            muxy_ui::popover::row(theme, m, group.clone(), true, false)
                .group(group.clone())
                .debug_selector(move || format!("remote-server-{id}"))
                .on_click(cx.listener(move |model, _, window, cx| {
                    model.open_remote_servers(id, window, cx);
                }))
                .on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                        model.open_server_menu(id, false, event.position, window, cx);
                        cx.stop_propagation();
                    }),
                )
                .child(dot(status_color(server, model), model))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .truncate()
                        .child(server.entry.name.clone()),
                )
                .child(
                    div()
                        .relative()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_end()
                        .min_w(m.control_small())
                        .child(
                            div()
                                .text_size(m.font_footnote())
                                .text_color(theme.fg_muted)
                                .group_hover(group.clone(), |style| style.opacity(0.0))
                                .child(status_label(server)),
                        )
                        .child(edit_button(id, group, model, cx)),
                ),
        )
        .children(error.map(|error| {
            div()
                .debug_selector(move || format!("remote-server-error-{id}"))
                .px(m.scaled(muxy_ui::popover::ROW_PADDING) + m.scaled(12.0))
                .pb(m.spacing2())
                .text_size(m.font_footnote())
                .text_color(theme.danger)
                .child(error)
        }))
        .children(hint.map(|hint| {
            div()
                .debug_selector(move || format!("remote-server-hint-{id}"))
                .px(m.scaled(muxy_ui::popover::ROW_PADDING) + m.scaled(12.0))
                .pb(m.spacing2())
                .text_size(m.font_footnote())
                .text_color(theme.fg_muted)
                .child(hint)
        }))
}

/// Edit…, shown in place of the status while the row is hovered.
fn edit_button(
    id: ServerId,
    group: SharedString,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    div()
        .debug_selector(move || format!("remote-server-edit-{id}"))
        .absolute()
        .top_0()
        .right_0()
        .h_full()
        .flex()
        .items_center()
        .opacity(0.0)
        .group_hover(group, |style| style.opacity(1.0))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            IconButton::new(
                SharedString::from(format!("remote-server-edit-{id}")),
                Icon::Pencil,
                model.metrics.icon_sm(),
                model.metrics.control_small(),
                theme.fg_muted,
                theme.fg,
            )
            .tooltip(
                tr!("Edit…"),
                theme.raised(),
                theme.fg,
                theme.border,
                theme.bg,
            )
            .on_click(cx.listener(move |model, _, _, cx| {
                cx.stop_propagation();
                model.open_server_form(Some(id), cx);
            })),
        )
        .into_any_element()
}

/// A group's heading in the popover; right-click it for the group's menu.
fn group_heading(group: Option<&str>, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let name = group.map(str::to_owned);
    let selector = format!("remote-group-{}", group.unwrap_or_default());
    muxy_ui::popover::header(&model.theme, m)
        .debug_selector(move || selector.clone())
        .pt(m.spacing3())
        .on_mouse_down(
            gpui::MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                model.open_group_menu(name.as_deref(), event.position, window, cx);
                cx.stop_propagation();
            }),
        )
        .child(group.map_or_else(|| tr!("Ungrouped"), |group| group.to_owned().into()))
        .into_any_element()
}

/// The popover above the section, which has the keyboard.
pub(crate) fn popover(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    list(model, window, cx)
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .on_action(cx.listener(|model, _: &super::menu::DismissMenu, _, cx| {
            model.dismiss_overlay(cx);
        }))
        .into_any_element()
}

/// The popover's servers by group, then Add and Manage. It also shows under
/// a menu opened from it.
pub(crate) fn list(
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> gpui::Stateful<gpui::Div> {
    let m = model.metrics;
    let theme = &model.theme;
    let servers = model.remote_servers();
    let width = m
        .scaled(300.0)
        .min((window.viewport_size().width - px(16.0)).max(px(0.0)));
    let groups = crate::model::by_group(&servers, |server| &server.entry);
    let headed = groups.iter().any(|(group, _)| group.is_some());
    let mut rows = Vec::new();
    for (group, members) in groups {
        if headed {
            rows.push(group_heading(group, model, cx));
        }
        rows.extend(
            members
                .into_iter()
                .map(|server| server_row(server, model, cx).into_any_element()),
        );
    }
    let action = |id: &'static str, icon: Icon, label: SharedString| {
        muxy_ui::popover::row(theme, m, id, true, false)
            .debug_selector(move || id.into())
            .child(IconGlyph::new(icon, m.font_body(), theme.fg_muted))
            .child(label)
    };
    muxy_ui::popover::surface(theme, m)
        .id("remote-servers-popover")
        .debug_selector(|| "remote-servers-popover".into())
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .w(width)
        .max_h((window.viewport_size().height - m.status_bar_height() - px(16.0)).max(px(0.0)))
        .overflow_y_scroll()
        .child(muxy_ui::popover::header(theme, m).child(tr!("Remote Servers")))
        .when(servers.is_empty(), |list| {
            list.child(
                div()
                    .px(m.scaled(muxy_ui::popover::ROW_PADDING))
                    .pb(m.spacing2())
                    .text_color(theme.fg_muted)
                    .child(tr!("No remote servers yet.")),
            )
        })
        .children(rows)
        .child(muxy_ui::popover::divider(theme, m))
        .child(
            action("remote-add-server", Icon::Plus, tr!("Add Server…"))
                .on_click(cx.listener(|model, _, _, cx| model.open_server_form(None, cx))),
        )
        .child(
            action(
                "remote-manage-servers",
                Icon::Settings,
                tr!("Manage Servers…"),
            )
            .on_click(cx.listener(|model, _, window, cx| {
                model.open_servers_tab(window, cx);
            })),
        )
}

/// "Remote Servers…" in the command palette: Add Server…, then each server.
pub(crate) fn command(model: &AppModel, cx: &Context<AppModel>) -> Command<Handler> {
    let _ = model;
    let model = cx.weak_entity();
    Command::list("remote_servers", tr!("Remote Servers…"), move |cx| {
        let mut servers = Registry::default();
        let add: Handler = Rc::new(|model, _, cx| model.open_server_form(None, cx));
        servers.register(
            Command::new("remote-add-server", tr!("Add Server…"), add).keywords("Add Server"),
        );
        let manage: Handler = Rc::new(AppModel::open_servers_tab);
        servers.register(
            Command::new("remote-manage-servers", tr!("Manage Servers…"), manage)
                .keywords("Manage Servers"),
        );
        let Some(model) = model.upgrade() else {
            return servers;
        };
        for server in model.read(cx).remote_servers() {
            servers.register(server_command(&server, model.downgrade()));
        }
        servers
    })
    .keywords("Remote Servers")
}

/// One server's page: what can be done with it now.
#[allow(
    clippy::too_many_lines,
    reason = "One palette page of a server's actions"
)]
fn server_command(server: &RemoteServer, model: gpui::WeakEntity<AppModel>) -> Command<Handler> {
    let id = server.entry.id;
    let title = format!("{}  ·  {}", server.entry.name, status_label(server));
    Command::list(format!("remote-{id}"), title, move |cx| {
        let mut actions = Registry::default();
        let Some(server) = model.upgrade().and_then(|model| {
            model
                .read(cx)
                .remote_servers()
                .into_iter()
                .find(|server| server.entry.id == id)
        }) else {
            return actions;
        };
        let handler =
            |action: fn(&mut AppModel, ServerId, &mut Window, &mut Context<AppModel>)| -> Handler {
                Rc::new(move |model, window, cx| action(model, id, window, cx))
            };
        actions.register(
            Command::new(
                "new-tab",
                tr!("New Tab"),
                handler(|model, id, _, cx| model.new_remote_home_tab(id, cx)),
            )
            .keywords("New Tab"),
        );
        actions.register(
            Command::new(
                "add-project",
                tr!("Add Project…"),
                handler(|model, id, _, cx| model.open_remote_project_picker(id, cx)),
            )
            .keywords("Add Project"),
        );
        let connected = matches!(
            server.status,
            ServerStatus::Connected | ServerStatus::Connecting
        );
        actions.register(
            Command::new(
                "connect",
                if connected {
                    tr!("Reconnect")
                } else {
                    tr!("Connect")
                },
                handler(|model, id, _, cx| model.reconnect_remote_server(id, cx)),
            )
            .keywords(if connected { "Reconnect" } else { "Connect" }),
        );
        if connected {
            actions.register(
                Command::new(
                    "disconnect",
                    tr!("Disconnect"),
                    handler(|model, id, _, cx| model.disconnect_remote_servers(&[id], cx)),
                )
                .keywords("Disconnect"),
            );
        }
        actions.register(
            Command::new(
                "edit",
                tr!("Edit…"),
                handler(|model, id, _, cx| model.open_server_form(Some(id), cx)),
            )
            .keywords("Edit"),
        );
        if server.missing && crate::model::released_version().is_some() {
            actions.register(
                Command::new(
                    "install",
                    tr!("Install Muxy…"),
                    handler(|model, id, window, cx| {
                        model.confirm_install_server(id, window.window_handle(), cx);
                    }),
                )
                .keywords("Install Muxy"),
            );
        }
        if server.incompatible && crate::model::released_version().is_some() {
            actions.register(
                Command::new(
                    "copy-update-command",
                    tr!("Copy Update Command"),
                    handler(|model, _, _, cx| model.copy_update_command(cx)),
                )
                .keywords("Copy Update Command"),
            );
        }
        actions.register(
            Command::new(
                "remove",
                tr!("Remove…"),
                handler(|model, id, window, cx| {
                    model.confirm_forget_servers(vec![id], window.window_handle(), cx);
                }),
            )
            .keywords("Remove"),
        );
        actions
    })
    .keywords(
        [
            server.entry.destination(),
            server.entry.group.as_deref().unwrap_or_default(),
        ]
        .join(" "),
    )
}

fn input(
    model: &AppModel,
    cx: &mut Context<AppModel>,
    placeholder: &'static str,
    text: &str,
) -> Entity<TextInput> {
    let style = InputStyle::field(&model.theme, &model.metrics);
    let text = text.to_owned();
    cx.new(|cx| {
        TextInput::new(style, cx)
            .with_placeholder_key(placeholder)
            .with_text(text)
    })
}

fn default_identity(home: &std::path::Path) -> Option<String> {
    [
        "id_ed25519",
        "id_ed25519_sk",
        "id_ecdsa",
        "id_ecdsa_sk",
        "id_rsa",
    ]
    .into_iter()
    .map(|name| home.join(".ssh").join(name))
    .find(|path| path.is_file())
    .map(|path| path.to_string_lossy().into_owned())
}

impl AppModel {
    /// Focuses `focus` once the current update is done, from wherever the
    /// overlay was opened, even outside the window.
    pub(crate) fn focus_later(&self, focus: gpui::FocusHandle, cx: &mut Context<Self>) {
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, _| focus.focus(window));
        });
    }

    pub(crate) fn toggle_remote_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if matches!(self.overlay, Some(Overlay::RemoteServers)) {
            self.dismiss_overlay(cx);
            return;
        }
        self.dismiss_overlay(cx);
        self.overlay = Some(Overlay::RemoteServers);
        self.overlay_focus.focus(window);
        cx.notify();
    }

    /// One server's actions in the command palette.
    pub(crate) fn open_remote_servers(
        &mut self,
        server: ServerId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(listed) = self
            .remote_servers()
            .into_iter()
            .find(|listed| listed.entry.id == server)
        else {
            return;
        };
        let mut registry = Registry::default();
        registry.register(command(self, cx));
        registry.register(server_command(&listed, cx.weak_entity()));
        self.show_palette(registry, Some(&format!("remote-{server}")), window, cx);
    }

    pub(crate) fn open_server_form(&mut self, device: Option<ServerId>, cx: &mut Context<Self>) {
        self.clear_server_probe();
        let entry = device.and_then(|id| self.settings.server(id).cloned());
        let custom_command = entry
            .as_ref()
            .is_some_and(|entry| entry.ssh_command.is_some());
        let automatic_identity = entry
            .as_ref()
            .is_none_or(|entry| entry.identity_file.is_none());
        let (host, user, port) = entry
            .as_ref()
            .map(|entry| crate::model::split_destination(&entry.ssh))
            .unwrap_or_default();
        let name = input(
            self,
            cx,
            tr_key!("Shown in the sidebar"),
            entry.as_ref().map_or("", |entry| &entry.name),
        );
        let group = input(
            self,
            cx,
            tr_key!("optional"),
            entry
                .as_ref()
                .and_then(|entry| entry.group.as_deref())
                .unwrap_or_default(),
        );
        let host = input(
            self,
            cx,
            tr_key!("host, address, or ~/.ssh/config alias"),
            &host,
        );
        let command = input(
            self,
            cx,
            "swiftvm ssh my-vm",
            entry
                .as_ref()
                .and_then(|entry| entry.ssh_command.as_deref())
                .unwrap_or_default(),
        );
        let user = input(self, cx, tr_key!("optional"), &user);
        let port = input(self, cx, "22", &port);
        let default_identity = automatic_identity
            .then(|| std::env::home_dir().and_then(|home| default_identity(&home)))
            .flatten()
            .unwrap_or_default();
        let identity = input(
            self,
            cx,
            tr_key!("Use SSH config or agent"),
            entry
                .as_ref()
                .and_then(|entry| entry.identity_file.as_deref())
                .unwrap_or(&default_identity),
        );
        let style = InputStyle::field(&self.theme, &self.metrics);
        let password = cx.new(|cx| {
            TextInput::new(style, cx)
                .secure()
                .with_placeholder_key(tr_key!("Asked when connecting if empty"))
        });
        let subscriptions = [
            &name, &group, &host, &command, &user, &port, &identity, &password,
        ]
        .into_iter()
        .map(|input| {
            cx.subscribe(input, |model, input, event, cx| match event {
                InputEvent::Submitted => model.submit_server_form(cx),
                InputEvent::Cancelled => model.dismiss_overlay(cx),
                InputEvent::Changed => model.server_form_changed(&input, cx),
            })
        })
        .collect();
        let focus = if custom_command { &command } else { &host };
        self.focus_later(focus.focus_handle(cx), cx);
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::ServerForm(Box::new(ServerForm {
            device,
            name,
            group,
            host,
            command,
            custom_command,
            user,
            port,
            identity,
            automatic_identity,
            identity_task: None,
            password,
            password_login: entry.as_ref().is_some_and(|entry| entry.password_login),
            error: None,
            _subscriptions: subscriptions,
        })));
        self.autofill_server_identity(cx);
        cx.notify();
    }

    /// An edit clears the last error and the last Test Connection.
    fn server_form_changed(&mut self, input: &Entity<TextInput>, cx: &mut Context<Self>) {
        let Some(Overlay::ServerForm(form)) = &mut self.overlay else {
            return;
        };
        form.error = None;
        if *input == form.identity {
            form.automatic_identity = false;
            form.identity_task = None;
        }
        let addressed = *input == form.host || *input == form.user || *input == form.port;
        self.clear_server_probe();
        if addressed {
            self.autofill_server_identity(cx);
        }
        cx.notify();
    }

    fn autofill_server_identity(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::ServerForm(form)) = &mut self.overlay else {
            return;
        };
        if form.custom_command || !form.automatic_identity {
            return;
        }
        form.identity_task = None;
        let submitted = form.submitted(cx);
        let Ok(destination) =
            crate::model::join_destination(&submitted.host, &submitted.user, &submitted.port)
        else {
            return;
        };
        let Ok(target) = muxy_client::SshTarget::new(&destination) else {
            return;
        };
        let identity = form.identity.clone();
        form.identity_task = Some(cx.spawn(async move |model, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(250))
                .await;
            let path = cx
                .background_executor()
                .spawn(async move { target.default_identity().ok().flatten() })
                .await;
            let _ = model.update(cx, |model, cx| {
                model.apply_server_identity(&identity, path.as_deref(), cx);
            });
        }));
    }

    fn apply_server_identity(
        &mut self,
        identity: &Entity<TextInput>,
        path: Option<&std::path::Path>,
        cx: &mut Context<Self>,
    ) {
        let Some(Overlay::ServerForm(form)) = &mut self.overlay else {
            return;
        };
        if form.custom_command || !form.automatic_identity || form.identity != *identity {
            return;
        }
        form.identity_task = None;
        let text = path
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        if form.identity.read(cx).text() == text {
            cx.notify();
            return;
        }
        form.identity
            .update(cx, |input, cx| input.set_text(text, cx));
        form.error = None;
        self.clear_server_probe();
        cx.notify();
    }

    fn submit_server_form(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::ServerForm(form)) = &self.overlay else {
            return;
        };
        if form.identity_task.is_some() {
            return;
        }
        let (device, submitted) = (form.device, form.submitted(cx));
        if self.server_form_installing(&submitted) {
            return;
        }
        match self.save_remote_server(device, &submitted, cx) {
            Ok(_) if matches!(self.overlay, Some(Overlay::ServerForm(_))) => {
                self.dismiss_overlay(cx);
            }
            Ok(_) => {}
            Err(error) => {
                if let Some(Overlay::ServerForm(form)) = &mut self.overlay {
                    form.error = Some(error);
                }
                cx.notify();
            }
        }
    }

    fn test_server_form(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::ServerForm(form)) = &self.overlay {
            if form.identity_task.is_some() {
                return;
            }
            let (device, submitted) = (form.device, form.submitted(cx));
            if self.server_form_installing(&submitted) {
                return;
            }
            self.test_remote_server(device, &submitted, cx);
            cx.notify();
        }
    }

    fn trust_server_form(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::ServerForm(form)) = &self.overlay {
            let (device, submitted) = (form.device, form.submitted(cx));
            self.trust_test_server(device, &submitted, cx);
            cx.notify();
        }
    }

    fn toggle_custom_command(&mut self, cx: &mut Context<Self>) {
        self.clear_server_probe();
        if let Some(Overlay::ServerForm(form)) = &mut self.overlay {
            form.custom_command = !form.custom_command;
            form.identity_task = None;
            form.error = None;
            let focus = if form.custom_command {
                &form.command
            } else {
                &form.host
            }
            .focus_handle(cx);
            self.focus_later(focus, cx);
            self.autofill_server_identity(cx);
            cx.notify();
        }
    }

    fn toggle_password_login(&mut self, cx: &mut Context<Self>) {
        self.clear_server_probe();
        if let Some(Overlay::ServerForm(form)) = &mut self.overlay {
            form.password_login = !form.password_login;
            cx.notify();
        }
    }

    fn server_form_installing(&self, form: &DeviceForm) -> bool {
        form.destination().ok().is_some_and(|destination| {
            self.server_probe(&destination) == Some(&ConnectionTest::Installing)
        })
    }

    fn confirm_install_server_form(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(Overlay::ServerForm(form)) = &self.overlay else {
            return;
        };
        if !self.accepts_prompts() {
            return;
        }
        let (device, submitted, host) = (form.device, form.submitted(cx), form.host.clone());
        let Ok(destination) = submitted.destination() else {
            return;
        };
        if !matches!(
            self.server_probe(&destination),
            Some(ConnectionTest::NotInstalled(_) | ConnectionTest::InstallFailed(_))
        ) {
            return;
        }
        let window = window.window_handle();
        let source = crate::remote_install::Source::current();
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response =
                super::confirm::prompt_install_server(window, &destination, &source, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                let current = matches!(&model.overlay, Some(Overlay::ServerForm(form))
                    if form.host == host && form.submitted(cx) == submitted);
                if current {
                    match response {
                        Ok(true) => model.install_test_server(device, &submitted, source, cx),
                        Ok(false) => {}
                        Err(error) => {
                            if let Some(Overlay::ServerForm(form)) = &mut model.overlay {
                                form.error = Some(error);
                            }
                        }
                    }
                }
                cx.notify();
            });
        }));
    }

    /// A file panel at `~/.ssh` that shows hidden files, for a key file.
    fn choose_server_identity(&mut self, cx: &mut Context<Self>) {
        let (sender, receiver) = async_channel::bounded(1);
        let directory = std::env::home_dir().unwrap_or_default().join(".ssh");
        let dialog = muxy_ui::dialog::choose_file(
            cx,
            &tr!("Choose a private key"),
            &directory,
            move |path| {
                let _ = sender.try_send(path);
            },
        );
        let dialog = match dialog {
            Ok(dialog) => dialog,
            Err(error) => {
                if let Some(Overlay::ServerForm(form)) = &mut self.overlay {
                    form.error =
                        Some(tr!("Could not choose a key file: %@", error.to_string()).to_string());
                }
                cx.notify();
                return;
            }
        };
        cx.spawn(async move |model, cx| {
            let answer = receiver.recv().await.unwrap_or(Ok(None));
            drop(dialog);
            let _ = model.update(cx, |model, cx| match answer {
                Ok(Some(path)) => model.set_server_identity(&path, cx),
                Ok(None) => {}
                Err(error) => {
                    if let Some(Overlay::ServerForm(form)) = &mut model.overlay {
                        form.error = Some(
                            tr!("Could not choose a key file: %@", error.to_string()).to_string(),
                        );
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn set_server_identity(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        let Some(Overlay::ServerForm(form)) = &mut self.overlay else {
            return;
        };
        form.automatic_identity = false;
        form.identity_task = None;
        form.identity.update(cx, |input, cx| {
            input.set_text(path.to_string_lossy().into_owned(), cx);
        });
        form.error = None;
        self.clear_server_probe();
        cx.notify();
    }

    /// Asks for `server`'s password; connecting goes on once it is typed.
    pub(crate) fn ask_password(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if !self.accepts_prompts() {
            return;
        }
        if matches!(&self.overlay, Some(Overlay::Password(prompt)) if prompt.server == server) {
            return;
        }
        let style = InputStyle::field(&self.theme, &self.metrics);
        let input = cx.new(|cx| TextInput::new(style, cx).secure());
        let subscription = cx.subscribe(&input, |model, _, event, cx| match event {
            InputEvent::Submitted => model.submit_password(cx),
            InputEvent::Cancelled => model.dismiss_overlay(cx),
            InputEvent::Changed => {}
        });
        self.focus_later(input.focus_handle(cx), cx);
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::Password(Box::new(PasswordPrompt {
            server,
            input,
            _subscription: subscription,
        })));
        cx.notify();
    }

    fn submit_password(&mut self, cx: &mut Context<Self>) {
        let Some(Overlay::Password(prompt)) = &self.overlay else {
            return;
        };
        let server = prompt.server;
        let password = prompt.input.read(cx).text().to_owned();
        if password.is_empty() {
            return;
        }
        let pending = self.pending_remote;
        self.dismiss_overlay(cx);
        self.pending_remote = pending;
        self.remember_password(server, password, cx);
    }
}

fn field(label: &str, input: AnyElement, model: &AppModel) -> gpui::Div {
    super::git::form_field(label, input, model)
}

fn modal(model: &AppModel, window: &Window, id: &'static str) -> gpui::Stateful<gpui::Div> {
    let m = model.metrics;
    let theme = &model.theme;
    muxy_ui::popover::surface(theme, m)
        .shadow(muxy_ui::theme::Elevation::Modal.shadow(theme.bg))
        .id(id)
        .debug_selector(move || id.into())
        .w(m.scaled(460.0)
            .min((window.viewport_size().width - px(16.0)).max(px(0.0))))
        .max_h((window.viewport_size().height - m.scaled(80.0)).max(px(0.0)))
        .overflow_y_scroll()
        .p(m.spacing8())
        .gap(m.scaled(14.0))
        .text_color(theme.fg)
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

fn title(text: &str, model: &AppModel) -> gpui::Div {
    div()
        .text_size(model.metrics.font_headline())
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.to_owned())
}

fn primary(
    id: &'static str,
    label: SharedString,
    enabled: bool,
    model: &AppModel,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    div()
        .id(id)
        .debug_selector(move || id.into())
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
                .button_interaction(on_click)
        })
        .child(label)
        .into_any_element()
}

fn connection_status(
    probe: Option<&ConnectionTest>,
    finding_identity: bool,
    custom_command: bool,
    theme: &muxy_ui::theme::Theme,
) -> (SharedString, Hsla) {
    match probe {
        None if finding_identity => (tr!("Finding SSH key…"), theme.fg_muted),
        None if custom_command => (
            tr!(
                "Runs a local program with Muxy's remote command appended. It must connect without prompts or a TTY. Use an absolute program path if it isn't on Muxy's PATH."
            ),
            theme.fg_muted,
        ),
        None => (
            tr!("Muxy uses your SSH config, keys, and agent. A password is never saved."),
            theme.fg_muted,
        ),
        Some(ConnectionTest::Testing) => (tr!("Testing connection…"), theme.fg_muted),
        Some(ConnectionTest::UntrustedHost(prompt)) => {
            let details = prompt
                .split("\nAre you sure you want to continue connecting")
                .next()
                .unwrap_or(prompt);
            (
                tr!(
                    "%@\n\nVerify this fingerprint with your server administrator before trusting it.",
                    details
                ),
                theme.warning,
            )
        }
        Some(ConnectionTest::Installing) => (
            tr!("Installing muxy-server, then testing connection…"),
            theme.fg_muted,
        ),
        Some(ConnectionTest::Succeeded(version)) => {
            (tr!("Connection succeeded · Muxy %@", version), theme.accent)
        }
        Some(ConnectionTest::NotInstalled(_)) => (
            tr!("SSH connected. Install muxy-server to finish setting up this server."),
            theme.fg_muted,
        ),
        Some(ConnectionTest::Failed(error)) => (error.clone().into(), theme.danger),
        Some(ConnectionTest::InstallFailed(error)) => {
            (tr!("Install failed: %@", error), theme.danger)
        }
    }
}

pub(crate) fn render_form(
    form: &ServerForm,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let style = Style { theme, metrics: &m };
    let submitted = form.submitted(cx);
    let destination = submitted.probe_key();
    let probe = model.server_probe(&destination);
    let installing = probe == Some(&ConnectionTest::Installing);
    let testing = installing || probe == Some(&ConnectionTest::Testing);
    let installable = matches!(
        probe,
        Some(ConnectionTest::NotInstalled(_) | ConnectionTest::InstallFailed(_))
    );
    let has_destination = !submitted
        .ssh_command
        .as_deref()
        .unwrap_or(&submitted.host)
        .trim()
        .is_empty();
    let (status, color) = connection_status(
        probe,
        form.identity_task.is_some(),
        form.custom_command,
        theme,
    );
    let text = |id: &str, input: &Entity<TextInput>| controls::text_field(style, id, input, None);
    modal(model, window, "server-form")
        .on_key_down(
            cx.listener(|model, event: &gpui::KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if event.keystroke.key == "tab"
                    && !modifiers.control
                    && !modifiers.alt
                    && !modifiers.platform
                    && !modifiers.function
                {
                    if let Some(Overlay::ServerForm(form)) = &model.overlay {
                        form.advance_focus(modifiers.shift, window, cx);
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }),
        )
        .child(title(
            &if form.device.is_some() {
                tr!("Edit Remote Server")
            } else {
                tr!("Add Remote Server")
            },
            model,
        ))
        .child(field(&tr!("Name"), text("server-name", &form.name), model))
        .child(field(
            &tr!("Group"),
            text("server-group", &form.group),
            model,
        ))
        .child(connection_fields(form, model, cx))
        .child(
            div()
                .debug_selector(|| "server-form-probe".into())
                .text_size(m.font_footnote())
                .text_color(color)
                .child(status),
        )
        .children(form.error.clone().map(|error| {
            div()
                .debug_selector(|| "server-form-error".into())
                .text_size(m.font_footnote())
                .text_color(theme.danger)
                .child(error)
        }))
        .when(installable, |form| form.child(install_offer(model, cx)))
        .when(
            matches!(probe, Some(ConnectionTest::UntrustedHost(_))),
            |form| {
                form.child(
                    controls::button(
                        style,
                        "server-trust",
                        &tr!("Trust & Test"),
                        true,
                        cx.listener(|model, _, _, cx| model.trust_server_form(cx)),
                    )
                    .debug_selector(|| "server-trust".into()),
                )
            },
        )
        .child(form_actions(
            has_destination && !testing && form.identity_task.is_none(),
            has_destination && !installing && form.identity_task.is_none(),
            model,
            cx,
        ))
        .into_any_element()
}

fn install_offer(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    controls::button(
        Style {
            theme: &model.theme,
            metrics: &model.metrics,
        },
        "server-install",
        &tr!("Install muxy-server & Test Connection"),
        true,
        cx.listener(|model, _, window, cx| model.confirm_install_server_form(window, cx)),
    )
    .debug_selector(|| "server-install".into())
    .into_any_element()
}

fn connection_fields(form: &ServerForm, model: &AppModel, cx: &mut Context<AppModel>) -> gpui::Div {
    let m = model.metrics;
    let style = Style {
        theme: &model.theme,
        metrics: &m,
    };
    let text = |id: &str, input: &Entity<TextInput>| controls::text_field(style, id, input, None);
    div()
        .flex()
        .flex_col()
        .gap(m.scaled(14.0))
        .child(
            div()
                .flex()
                .items_center()
                .gap(m.spacing3())
                .child(controls::toggle(
                    style,
                    "server-custom-command",
                    form.custom_command,
                    cx.listener(|model, _, _, cx| model.toggle_custom_command(cx)),
                ))
                .child(
                    div()
                        .text_size(m.font_footnote())
                        .child(tr!("Custom command")),
                ),
        )
        .when(form.custom_command, |view| {
            view.child(field(
                &tr!("SSH command"),
                text("server-command", &form.command),
                model,
            ))
        })
        .when(!form.custom_command, |view| {
            view.child(field(
                &tr!("SSH host"),
                text("server-host", &form.host),
                model,
            ))
            .child(login(form, model, cx))
        })
}

/// User, port, key file, and password login.
fn login(form: &ServerForm, model: &AppModel, cx: &mut Context<AppModel>) -> gpui::Div {
    let m = model.metrics;
    let theme = &model.theme;
    let style = Style { theme, metrics: &m };
    let text = |id: &str, input: &Entity<TextInput>| controls::text_field(style, id, input, None);
    div()
        .flex()
        .flex_col()
        .gap(m.scaled(14.0))
        .child(
            div()
                .flex()
                .gap(m.spacing4())
                .child(field(&tr!("User"), text("server-user", &form.user), model).flex_1())
                .child(
                    field(&tr!("Port"), text("server-port", &form.port), model).w(m.scaled(96.0)),
                ),
        )
        .child(field(
            &if form.automatic_identity {
                tr!("Identity file (automatic)")
            } else {
                tr!("Identity file")
            },
            div()
                .flex()
                .gap(m.spacing3())
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(text("server-identity", &form.identity)),
                )
                .child(controls::button(
                    style,
                    "server-identity-browse",
                    &tr!("Browse…"),
                    true,
                    cx.listener(|model, _, _, cx| model.choose_server_identity(cx)),
                ))
                .into_any_element(),
            model,
        ))
        .when(form.automatic_identity, |fields| {
            fields.child(
                div()
                    .text_size(m.font_footnote())
                    .text_color(theme.fg_muted)
                    .child(tr!(
                        "SSH can also try your other keys. Edit or browse to use a specific key."
                    )),
            )
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(m.spacing3())
                .child(controls::toggle(
                    style,
                    "server-password-login",
                    form.password_login,
                    cx.listener(|model, _, _, cx| model.toggle_password_login(cx)),
                ))
                .child(
                    div()
                        .text_size(m.font_footnote())
                        .child(tr!("Log in with a password")),
                ),
        )
        .when(form.password_login, |view| {
            view.child(field(
                &tr!("Password"),
                text("server-password", &form.password),
                model,
            ))
        })
}

/// Test Connection, Cancel, and Save.
fn form_actions(
    testable: bool,
    savable: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let style = Style {
        theme: &model.theme,
        metrics: &m,
    };
    div()
        .flex()
        .items_center()
        .gap(m.spacing3())
        .pt(m.spacing3())
        .child(
            controls::button(
                style,
                "server-test",
                &tr!("Test Connection"),
                testable,
                cx.listener(|model, _, _, cx| model.test_server_form(cx)),
            )
            .debug_selector(|| "server-test".into()),
        )
        .child(div().flex_1())
        .child(controls::button(
            style,
            "server-cancel",
            &tr!("Cancel"),
            true,
            cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
        ))
        .child(primary(
            "server-save",
            tr!("Save"),
            savable,
            model,
            cx.listener(|model, _, _, cx| model.submit_server_form(cx)),
        ))
        .into_any_element()
}

pub(crate) fn render_password(
    prompt: &PasswordPrompt,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let style = Style { theme, metrics: &m };
    let server = model
        .remote_servers()
        .into_iter()
        .find(|server| server.entry.id == prompt.server);
    let (name, destination) = server.as_ref().map_or_else(Default::default, |server| {
        (server.entry.name.clone(), server.entry.ssh.clone())
    });
    let error = server
        .and_then(|server| server.error)
        .filter(|_| model.server_failed_login(prompt.server));
    modal(model, window, "server-password")
        .child(title(&tr!("Password for %@", &name), model))
        .child(
            div()
                .text_size(m.font_footnote())
                .text_color(theme.fg_muted)
                .child(tr!(
                    "%@ · Muxy keeps it in memory until it quits.",
                    &destination
                )),
        )
        .child(controls::text_field(
            style,
            "server-password-input",
            &prompt.input,
            None,
        ))
        .children(error.map(|error| {
            div()
                .text_size(m.font_footnote())
                .text_color(theme.danger)
                .child(error)
        }))
        .child(
            div()
                .flex()
                .justify_end()
                .gap(m.spacing3())
                .pt(m.spacing3())
                .child(controls::button(
                    style,
                    "server-password-cancel",
                    &tr!("Cancel"),
                    true,
                    cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
                ))
                .child(primary(
                    "server-password-connect",
                    tr!("Connect"),
                    true,
                    model,
                    cx.listener(|model, _, _, cx| model.submit_password(cx)),
                )),
        )
        .into_any_element()
}
