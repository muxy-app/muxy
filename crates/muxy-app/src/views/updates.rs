use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_ui::controls::{self, Style};

use crate::model::{AppModel, UpdateAction};

pub(crate) fn render(model: &AppModel, window: &Window, cx: &mut Context<AppModel>) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let details = model.update_details();
    let separate_server_action = model.server_update_pending()
        && details
            .as_ref()
            .and_then(|details| details.action)
            .is_none_or(|(action, _)| {
                !matches!(
                    action,
                    UpdateAction::RestartServer | UpdateAction::RetryServer | UpdateAction::Connect
                )
            });
    let (app, server) = model.update_versions();
    let width = m
        .scaled(360.0)
        .min((window.viewport_size().width - px(16.0)).max(px(0.0)));
    let mut panel = div()
        .id("update-popover")
        .debug_selector(|| "update-popover".into())
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
                        .child("Muxy updates"),
                )
                .child(controls::button(
                    Style { theme, metrics: &m },
                    "close-updates",
                    "Close",
                    true,
                    cx.listener(|model, _, _, cx| model.dismiss_overlay(cx)),
                )),
        )
        .child(version_row("App", app, model))
        .child(version_row("Server", server, model));
    if let Some(details) = details {
        panel = panel
            .child(div().font_weight(FontWeight::MEDIUM).child(details.label))
            .child(
                div()
                    .text_color(if details.failed {
                        theme.warning
                    } else {
                        theme.fg_muted
                    })
                    .child(details.description),
            );
        if let Some((action, label)) = details.action {
            panel = panel.child(
                div()
                    .flex()
                    .justify_end()
                    .child(action_button(action, label, model, cx)),
            );
        }
    } else {
        panel = panel.child(
            div()
                .text_color(theme.fg_muted)
                .child("No pending updates."),
        );
    }
    if separate_server_action {
        panel = panel.child(server_action(model, cx));
    }
    if model.update_scheduled() {
        panel = panel.child(div().flex().justify_end().child(action_button(
            UpdateAction::CancelSchedule,
            "Cancel scheduled update",
            model,
            cx,
        )));
    }
    panel.into_any_element()
}

fn server_action(model: &AppModel, cx: &mut Context<AppModel>) -> impl IntoElement {
    let m = model.metrics;
    let theme = &model.theme;
    let (action, label) = model.server_update_action();
    div().border_t_1().border_color(theme.border).pt(m.spacing6())
        .flex().flex_col().gap(m.spacing6())
        .child(div().text_size(m.font_footnote()).text_color(theme.fg_muted)
            .child("The installed app includes a server update. Restarting the server ends all terminal sessions."))
        .child(div().flex().justify_end().child(action_button(action, label, model, cx)))
}

fn action_button(
    action: UpdateAction,
    label: &'static str,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let id = match action {
        UpdateAction::RestartServer | UpdateAction::RetryServer => "server-update-action",
        UpdateAction::Connect => "connect-update-action",
        UpdateAction::CancelSchedule => "cancel-scheduled-update",
        _ => "update-action",
    };
    controls::button(
        Style {
            theme: &model.theme,
            metrics: &model.metrics,
        },
        id,
        label,
        model.update_action_enabled(action),
        cx.listener(move |model, _, _, cx| model.perform_update_action(action, cx)),
    )
    .debug_selector(move || id.into())
    .into_any_element()
}

fn version_row(label: &'static str, version: String, model: &AppModel) -> impl IntoElement {
    let m = model.metrics;
    div()
        .flex()
        .gap(m.spacing6())
        .border_t_1()
        .border_color(model.theme.border)
        .pt(m.spacing6())
        .child(div().w(m.scaled(44.0)).flex_none().child(label))
        .child(
            div()
                .min_w(px(0.0))
                .text_size(m.font_footnote())
                .text_color(model.theme.fg_muted)
                .child(version),
        )
}
