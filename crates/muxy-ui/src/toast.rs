use gpui::prelude::FluentBuilder;
use gpui::{
    FontWeight, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div, px,
};

use crate::{
    components::SymbolGlyph,
    theme::{Metrics, Theme},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToastKind {
    Success,
    Warning,
    Error,
}

/// Success toasts stay compact; problems wrap so their full text stays readable.
pub fn toast(
    kind: ToastKind,
    message: &str,
    body: Option<&str>,
    theme: &Theme,
    m: Metrics,
) -> gpui::Div {
    let (symbol, color) = match kind {
        ToastKind::Success => ("checkmark.circle.fill", theme.diff_add),
        ToastKind::Warning => ("exclamationmark.triangle.fill", theme.warning),
        ToastKind::Error => ("xmark.octagon.fill", theme.danger),
    };
    let problem = kind != ToastKind::Success;
    div()
        .debug_selector(|| "workspace-toast".into())
        .relative()
        .flex()
        .items_center()
        .when(body.is_some() || problem, Styled::items_start)
        .gap(m.spacing3())
        .max_w_full()
        .px(m.scaled(14.0))
        .py(m.spacing4())
        .map(|toast| {
            if problem {
                toast.rounded(m.radius_lg())
            } else {
                toast.rounded_full()
            }
        })
        .bg(theme.bg)
        .border_1()
        .border_color(if problem { color } else { theme.border })
        .child(SymbolGlyph::new(symbol, m.font_body(), color))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(m.spacing1())
                .min_w(px(0.0))
                .max_w(m.scaled(if problem { 420.0 } else { 360.0 }))
                .child(
                    div()
                        .debug_selector(|| "workspace-toast-message".into())
                        .text_size(m.font_body())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.fg)
                        .map(|message| {
                            if problem {
                                message.line_clamp(3)
                            } else {
                                message.truncate().line_clamp(1)
                            }
                        })
                        .child(message.replace(['\r', '\n'], " ")),
                )
                .when_some(body, |content, body| {
                    content.child(
                        div()
                            .id("workspace-toast-body")
                            .debug_selector(|| "workspace-toast-body".into())
                            .text_size(m.font_footnote())
                            .text_color(theme.fg_muted)
                            .map(|text| {
                                if problem {
                                    text.max_h(m.scaled(120.0)).overflow_y_scroll()
                                } else {
                                    text.line_clamp(2)
                                }
                            })
                            .child(if problem {
                                body.to_owned()
                            } else {
                                body.replace(['\r', '\n'], " ")
                            }),
                    )
                }),
        )
}
