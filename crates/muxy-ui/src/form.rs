use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Div, FocusHandle, ParentElement, Styled, Window, div, px, relative};
use muxy_core::shortcuts::ShortcutId;

use crate::controls::Style;

gpui::actions!(form, [NextControl, PreviousControl]);

pub fn register_shortcuts(registry: &mut crate::shortcuts::Registry<'_>) {
    registry.register(ShortcutId::FormNextControl, &NextControl);
    registry.register(ShortcutId::FormPreviousControl, &PreviousControl);
}

/// Moves focus to the next field, wrapping at the ends. With none focused,
/// goes to the first field, or to the last when moving backwards.
pub fn advance_focus(fields: &[FocusHandle], backwards: bool, window: &mut Window) {
    let count = fields.len();
    let next = match fields.iter().position(|field| field.is_focused(window)) {
        Some(index) if backwards => (index + count - 1) % count,
        Some(index) => (index + 1) % count,
        None if backwards => count.wrapping_sub(1),
        None => 0,
    };
    if let Some(field) = fields.get(next) {
        field.focus(window);
    }
}

pub fn row(
    style: Style,
    label: &str,
    description: Option<&str>,
    control: AnyElement,
    stacked: bool,
) -> Div {
    let Style { theme, metrics } = style;
    let text = div()
        .flex_1()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(metrics.scaled(5.0))
        .child(
            div()
                .text_size(metrics.font_headline())
                .text_color(theme.fg)
                .child(label.to_owned()),
        )
        .when_some(description, |text, description| {
            text.child(
                div()
                    .text_size(metrics.font_emphasis())
                    .line_height(relative(1.55))
                    .text_color(theme.fg_muted)
                    .child(description.to_owned()),
            )
        });
    div()
        .flex()
        .min_w(px(0.0))
        .py(metrics.scaled(22.0))
        .gap(metrics.spacing9())
        .map(|row| {
            if stacked {
                row.flex_col().gap(metrics.spacing6())
            } else {
                row.items_center()
            }
        })
        .child(text)
        .child(div().flex_none().min_w(px(0.0)).child(control))
}

pub fn section_heading(style: Style, id: impl Into<gpui::ElementId>, title: &str) -> Div {
    let Style { theme, metrics } = style;
    div()
        .pt(metrics.spacing8())
        .pb(metrics.spacing6())
        .border_b_1()
        .border_color(theme.border)
        .font_family(".SystemUIFontMonospaced")
        .text_size(metrics.font_body())
        .text_color(theme.fg_muted)
        .child(crate::accessibility::label(id, title.to_owned()))
}

pub fn note(style: Style, text: &str, error: bool) -> Div {
    let Style { theme, metrics } = style;
    div()
        .py(metrics.spacing7())
        .text_size(metrics.font_footnote())
        .text_color(if error { theme.danger } else { theme.fg_muted })
        .child(text.to_owned())
}
