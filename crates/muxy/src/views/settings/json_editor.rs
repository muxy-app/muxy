use super::SettingsModal;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};
use muxy_core::prefs::settings;
use muxy_ui::controls::{self, Choice};

pub const EDITOR_FIELD: &str = "json.user";
pub const PANE: &str = "json.pane";

const USER_DESCRIPTION: &str = "Edit your settings directly. Applying writes every recognised key back to the app and rewrites this file.";
const DEFAULTS_DESCRIPTION: &str = "The values Muxy ships with. This pane is read-only; copy anything you want into the User pane.";

pub fn is_user_pane(modal: &SettingsModal) -> bool {
    modal.selection(PANE).unwrap_or("user") == "user"
}

pub fn content(modal: &SettingsModal, cx: &mut Context<SettingsModal>) -> Vec<AnyElement> {
    let style = modal.style();
    let metrics = style.metrics;
    let user = is_user_pane(modal);

    let mut block = div()
        .flex()
        .flex_col()
        .flex_grow()
        .min_h(px(0.0))
        .gap(metrics.spacing4())
        .p(metrics.spacing6())
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(metrics.spacing5())
                .child(
                    div()
                        .flex_grow()
                        .text_size(metrics.font_display())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(style.theme.fg)
                        .child(SharedString::from("JSON Settings")),
                )
                .child(controls::segmented(
                    style,
                    PANE,
                    vec![
                        Choice::new("user", "User"),
                        Choice::new("defaults", "System Defaults"),
                    ],
                    if user { "user" } else { "defaults" },
                    cx.listener(|modal: &mut SettingsModal, value: &SharedString, _, cx| {
                        modal.set_selection(PANE, value, cx);
                    }),
                )),
        )
        .child(
            div()
                .text_size(metrics.font_footnote())
                .text_color(style.theme.fg_muted)
                .child(SharedString::from(if user {
                    USER_DESCRIPTION
                } else {
                    DEFAULTS_DESCRIPTION
                })),
        );

    if user {
        if let Some(field) = modal.field(EDITOR_FIELD) {
            block = block.child(controls::text_area(style, EDITOR_FIELD, field, None));
        }
    } else {
        block = block.child(
            div()
                .id("json-defaults")
                .flex()
                .flex_col()
                .flex_grow()
                .min_h(px(0.0))
                .p(metrics.spacing4())
                .rounded(metrics.radius_sm())
                .bg(style.theme.surface)
                .border_1()
                .border_color(style.theme.border)
                .overflow_y_scroll()
                .text_size(metrics.font_footnote())
                .text_color(style.theme.fg_muted)
                .font_family("Menlo")
                .child(SharedString::from(settings::system_defaults_text())),
        );
    }

    block = block.child(footer(modal, user, cx));
    vec![block.into_any_element()]
}

fn footer(modal: &SettingsModal, user: bool, cx: &mut Context<SettingsModal>) -> AnyElement {
    let style = modal.style();
    let metrics = style.metrics;
    let (message, color) = match (modal.json_error(), modal.json_status()) {
        (Some(error), _) => (error.to_owned(), style.theme.danger),
        (None, Some(status)) => (status.to_owned(), style.theme.fg_muted),
        (None, None) => (
            settings::user_path().to_string_lossy().into_owned(),
            style.theme.fg_muted,
        ),
    };

    let mut bar = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing4())
        .flex_none()
        .child(
            div()
                .flex_grow()
                .min_w(px(0.0))
                .truncate()
                .text_size(metrics.font_footnote())
                .text_color(color)
                .child(SharedString::from(message)),
        )
        .child(controls::button(
            style,
            "json-reload",
            "Reload",
            true,
            cx.listener(|modal: &mut SettingsModal, _, _, cx| modal.reload_json(cx)),
        ));

    if !user {
        return bar.into_any_element();
    }

    bar = bar
        .child(controls::button(
            style,
            "json-prettify",
            "Prettify",
            true,
            cx.listener(|modal: &mut SettingsModal, _, _, cx| modal.prettify_json(cx)),
        ))
        .child(controls::button(
            style,
            "json-reset",
            "Reset from Current Settings",
            true,
            cx.listener(|modal: &mut SettingsModal, _, _, cx| modal.reset_json(cx)),
        ))
        .child(
            div()
                .id("json-apply")
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .h(metrics.control_medium())
                .px(metrics.spacing5())
                .rounded(metrics.radius_sm())
                .cursor_pointer()
                .bg(style.theme.accent)
                .hover(|hover| hover.opacity(0.9))
                .text_size(metrics.font_footnote())
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.theme.accent_foreground)
                .child(SharedString::from("Apply"))
                .on_click(cx.listener(|modal: &mut SettingsModal, _, _, cx| modal.apply_json(cx))),
        );

    bar.into_any_element()
}
