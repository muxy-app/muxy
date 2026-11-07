use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Toggled, Window, canvas,
    div, px,
};
use muxy_app_core::ServerId;
use muxy_ui::accessibility;
use muxy_ui::components::{ButtonInteraction, IconGlyph};
use muxy_ui::controls::{self, Style};
use muxy_ui::icon::Icon;
use muxy_ui::l10n::translate;
use muxy_ui::popover;
use muxy_ui::tr;

use super::overlays::{Mark, OverlayView, Placement};
use crate::model::{AppModel, ServerStatus};

/// The server popover above the status bar's server item.
pub(crate) struct ServerPopover;

impl OverlayView for ServerPopover {
    fn render(
        &self,
        _: Option<Mark>,
        model: &AppModel,
        window: &Window,
        cx: &mut Context<AppModel>,
    ) -> AnyElement {
        render(model, window, cx)
    }

    fn placement(&self, model: &AppModel) -> Placement {
        Placement::above_while(model.appearance.status_bar_visible, model.server_anchor())
    }

    fn marks_bounds(&self, _: &AppModel) -> bool {
        true
    }
}

fn status_color(status: ServerStatus, model: &AppModel) -> Hsla {
    match status {
        ServerStatus::Connected => model.theme.accent,
        ServerStatus::Disconnected => model.theme.danger,
        ServerStatus::Connecting | ServerStatus::Restarting | ServerStatus::Stopping => {
            model.theme.warning
        }
    }
}

/// What the status bar's server item says: which server, and how it's doing.
fn control_text(remote: Option<&str>, status: ServerStatus) -> SharedString {
    let status = translate(status.label());
    match remote {
        Some(server) => format!("{server} · {status}"),
        None => format!("{} · {status}", tr!("Server")),
    }
    .into()
}

pub(crate) fn control(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    let status = model.server_status();
    let anchor = model.server_anchor();
    let server = model.status_server();
    let remote = (!server.is_local()).then(|| model.server_label(server));
    let text = control_text(remote.as_deref(), status);
    div()
        .id("project-server-status")
        .role(Role::Button)
        .accessibility_id("project-connection-status")
        .aria_label(text.clone())
        .debug_selector(|| "project-connection-status".into())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .h_full()
        .gap(m.scaled(6.0))
        .pl(m.scaled(8.0))
        .border_l_1()
        .border_color(theme.border)
        .text_color(theme.fg_muted)
        .cursor_pointer()
        .hover(|style| style.text_color(theme.fg))
        .focus(|style| style.bg(theme.hover))
        .button_interaction(
            cx.listener(|model, _, window, cx| model.toggle_server_popover(window, cx)),
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
                .bg(status_color(status, model)),
        )
        .child(
            div()
                .text_size(m.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .child(text),
        )
}

fn render(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let status = model.server_status();
    let server = model.status_server();
    let label = model.server_label(server);
    let version = model.server_version(server);
    let error = model.server_error(server);
    let width = m
        .scaled(360.0)
        .min((window.viewport_size().width - px(16.0)).max(px(0.0)));
    let content = popover::body(m)
        .child(
            div()
                .flex()
                .justify_between()
                .gap(m.spacing6())
                .child(
                    div()
                        .debug_selector(|| "server-popover-name".into())
                        .child(accessibility::label("server-popover-name", label.clone())),
                )
                .child(
                    div()
                        .text_color(status_color(status, model))
                        .child(accessibility::label(
                            "server-popover-status",
                            translate(status.label()),
                        )),
                ),
        )
        .children(version.map(|version| {
            div()
                .text_color(theme.fg_muted)
                .child(accessibility::label(
                    "server-popover-version",
                    tr!("Muxy %@", version),
                ))
        }))
        .children(error.map(|error| {
            div()
                .debug_selector(|| "server-popover-error".into())
                .text_color(theme.danger)
                .child(accessibility::label(
                    "server-popover-error",
                    error.to_owned(),
                ))
        }))
        .child(
            div()
                .text_color(theme.fg_muted)
                .child(accessibility::label("server-popover-warning", if server.is_local() {
                    tr!("Restarting or stopping this server ends all terminal sessions on this computer, including sessions in other projects and clients.")
                } else {
                    tr!("Restarting or stopping this server ends all terminal sessions on %@, including sessions in other projects and clients.", &label)
                })),
        );
    let panel = popover::surface(theme, m)
        .id("server-popover")
        .role(Role::Dialog)
        .accessibility_id("server-popover")
        .aria_label(tr!("Server"))
        .debug_selector(|| "server-popover".into())
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .on_action(
            cx.listener(|model, _: &super::menu::DismissMenu, _, cx| model.dismiss_overlay(cx)),
        )
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .w(width)
        .max_h((window.viewport_size().height - m.status_bar_height() - px(16.0)).max(px(0.0)))
        .overflow_y_scroll()
        .child(
            popover::header(theme, m)
                .justify_between()
                .child(tr!("Server"))
                .child(controls::button(
                    Style { theme, metrics: &m },
                    "close-server-status",
                    controls::Button::secondary(&tr!("Close")),
                    cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
                )),
        )
        .child(picker(model, cx))
        .child(popover::divider(theme, m))
        .child(content);
    panel.child(actions(model, cx)).into_any_element()
}

/// Every server the app can work with, the default one ticked. Picking one
/// switches the whole app to it.
fn picker(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let servers = std::iter::once(ServerId::local()).chain(model.listed_servers());
    div()
        .id("server-picker")
        .role(Role::RadioGroup)
        .accessibility_id("server-picker")
        .aria_label(tr!("Default server"))
        .debug_selector(|| "server-picker".into())
        .flex()
        .flex_col()
        .gap(model.metrics.scaled(popover::ROW_GAP))
        .px(model.metrics.spacing2())
        .py(model.metrics.spacing2())
        .children(servers.map(|server| choice(server, model, cx)))
}

fn choice(server: ServerId, model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let id = format!("server-choice-{server}");
    let selected = server == model.default_server();
    let status = model.status_of(server);
    let name = model.server_label(server);
    let state = model
        .remote_servers()
        .into_iter()
        .find(|remote| remote.entry.id == server)
        .map_or_else(
            || translate(status.label()),
            |remote| super::remote_servers::status_label(&remote),
        );
    popover::row(theme, m, SharedString::from(id.clone()), true, false)
        .role(Role::RadioButton)
        .accessibility_id(SharedString::from(id.clone()))
        .aria_label(name.clone())
        .aria_description(state.clone())
        .aria_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        })
        .debug_selector(move || id.clone())
        .when(selected, |row| row.bg(theme.hover))
        .on_click(cx.listener(move |model, _, _, cx| model.set_default_server(server, cx)))
        .child(
            div()
                .size(m.scaled(6.0))
                .flex_none()
                .rounded_full()
                .bg(status_color(status, model)),
        )
        .child(div().flex_1().min_w(px(0.0)).truncate().child(name))
        .child(
            div()
                .flex_none()
                .text_size(m.font_footnote())
                .text_color(theme.fg_muted)
                .child(state),
        )
        .child(div().flex_none().w(m.icon_sm()).when(selected, |slot| {
            slot.child(IconGlyph::new(Icon::Check, m.icon_sm(), theme.accent))
        }))
        .into_any_element()
}

fn actions(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    let status = model.server_status();
    let mut actions = popover::footer(theme, m);
    if matches!(
        status,
        ServerStatus::Disconnected | ServerStatus::Connecting
    ) {
        actions = actions.child(
            controls::button(
                Style { theme, metrics: &m },
                "connect-server",
                controls::Button::secondary(&tr!("Connect"))
                    .enabled(model.server_connect_enabled()),
                cx.listener(|model, _, _, cx| {
                    if model.server_connect_enabled() {
                        model.connect_server(model.status_server(), cx);
                        cx.notify();
                    }
                }),
            )
            .debug_selector(|| "connect-server".into()),
        );
    } else {
        let restartable = model.restartable(model.status_server());
        for (restart, id, label) in [
            (true, "restart-project-server", tr!("Restart server…")),
            (false, "stop-project-server", tr!("Stop server…")),
        ]
        .into_iter()
        .filter(|(restart, ..)| restartable || !restart)
        {
            actions = actions.child(
                controls::button(
                    Style { theme, metrics: &m },
                    id,
                    controls::Button::secondary(&label).enabled(model.server_control_enabled()),
                    cx.listener(move |model, _, window, cx| {
                        model.confirm_server_control(
                            model.status_server(),
                            restart,
                            window.window_handle(),
                            cx,
                        );
                    }),
                )
                .debug_selector(move || id.into()),
            );
        }
    }
    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_item_names_a_remote_server_and_says_how_it_is() {
        assert_eq!(
            control_text(None, ServerStatus::Connected).as_ref(),
            "Server · Connected",
        );
        assert_eq!(
            control_text(Some("nas"), ServerStatus::Disconnected).as_ref(),
            "nas · Disconnected",
        );
    }
}
