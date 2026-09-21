use gpui::{
    AnyElement, Context, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, StatefulInteractiveElement, Styled, Window, canvas, div, px,
};
use muxy_ui::components::ButtonInteraction;
use muxy_ui::controls::{self, Style};

use crate::model::{AppModel, ServerStatus};

fn status_color(status: ServerStatus, model: &AppModel) -> Hsla {
    match status {
        ServerStatus::Connected => model.theme.accent,
        ServerStatus::Disconnected => model.theme.danger,
        ServerStatus::Connecting | ServerStatus::Restarting | ServerStatus::Stopping => {
            model.theme.warning
        }
    }
}

pub(crate) fn control(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    let status = model.server_status();
    let anchor = model.server_anchor();
    div()
        .id("project-server-status")
        .debug_selector(|| "project-connection-status".into())
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
                .child(format!("Server · {}", status.label())),
        )
}

pub(crate) fn render(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let status = model.server_status();
    let width = m
        .scaled(360.0)
        .min((window.viewport_size().width - px(16.0)).max(px(0.0)));
    let panel = div()
        .id("server-popover")
        .debug_selector(|| "server-popover".into())
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .on_action(
            cx.listener(|model, _: &super::menu::DismissMenu, _, cx| model.dismiss_overlay(cx)),
        )
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .occlude()
        .flex()
        .flex_col()
        .w(width)
        .max_h((window.viewport_size().height - m.status_bar_height() - px(16.0)).max(px(0.0)))
        .overflow_y_scroll()
        .p(m.spacing7())
        .gap(m.spacing6())
        .rounded(m.radius_lg())
        .border_1()
        .border_color(theme.border)
        .bg(theme.raised())
        .shadow_lg()
        .text_color(theme.fg)
        .text_size(m.font_body())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Project server"),
                )
                .child(controls::button(
                    Style { theme, metrics: &m },
                    "close-server-status",
                    "Close",
                    true,
                    cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
                )),
        )
        .child(
            div()
                .border_t_1()
                .border_color(theme.border)
                .pt(m.spacing6())
                .flex()
                .justify_between()
                .gap(m.spacing6())
                .child("Current device")
                .child(div().text_color(status_color(status, model)).child(status.label())),
        )
        .child(
            div()
                .text_color(theme.fg_muted)
                .child(format!("Server for {}", model.state.current_project().name)),
        )
        .child(
            div()
                .text_color(theme.fg_muted)
                .child("Restarting or stopping this server ends all terminal sessions on this device, including sessions in other projects and clients."),
        );
    panel.child(actions(model, cx)).into_any_element()
}

fn actions(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    let status = model.server_status();
    let mut actions = div().flex().justify_end().gap(m.spacing6());
    if matches!(
        status,
        ServerStatus::Disconnected | ServerStatus::Connecting
    ) {
        actions = actions.child(
            controls::button(
                Style { theme, metrics: &m },
                "connect-server",
                "Connect",
                model.server_connect_enabled(),
                cx.listener(|model, _, _, cx| {
                    if model.server_connect_enabled() {
                        model.connect(cx);
                        cx.notify();
                    }
                }),
            )
            .debug_selector(|| "connect-server".into()),
        );
    } else {
        for (restart, id, label) in [
            (true, "restart-project-server", "Restart server…"),
            (false, "stop-project-server", "Stop server…"),
        ] {
            actions = actions.child(
                controls::button(
                    Style { theme, metrics: &m },
                    id,
                    label,
                    model.server_control_enabled(),
                    cx.listener(move |model, _, window, cx| {
                        model.confirm_server_control(restart, window.window_handle(), cx);
                    }),
                )
                .debug_selector(move || id.into()),
            );
        }
    }
    actions
}
