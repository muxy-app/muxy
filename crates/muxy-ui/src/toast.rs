use gpui::prelude::FluentBuilder;
use gpui::{
    Div, FontWeight, InteractiveElement, ParentElement, Stateful, StatefulInteractiveElement,
    Styled, div, px,
};

use crate::{
    accessibility::LiveRegion,
    components::SymbolGlyph,
    theme::{Metrics, Theme},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToastKind {
    Success,
    Warning,
    Error,
}

/// `element`, a [`toast`] with an id, read out when it appears. Problems interrupt.
pub fn announced(
    element: Stateful<Div>,
    kind: ToastKind,
    message: &str,
    body: Option<&str>,
) -> LiveRegion {
    let text = match body {
        Some(body) => format!("{message}\n{body}"),
        None => message.to_owned(),
    };
    crate::accessibility::announced(element, text, kind != ToastKind::Success)
}

/// Success toasts stay compact; problems wrap so their full text stays readable.
pub fn toast(kind: ToastKind, message: &str, body: Option<&str>, theme: &Theme, m: Metrics) -> Div {
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
            // Block layout sets the width before wrapping; a flex column measures text unwrapped.
            div()
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
                            .mt(m.spacing1())
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Element;
    use gpui::accesskit::{Live, Node, Role};

    fn node_of(kind: ToastKind, body: Option<&str>) -> Node {
        let mut node = Node::new(Role::Unknown);
        announced(div().id("toast"), kind, "Saved", body).write_a11y_info(&mut node);
        node
    }

    #[test]
    fn problems_interrupt_and_read_their_body() {
        for kind in [ToastKind::Warning, ToastKind::Error] {
            let node = node_of(kind, Some("Disk full"));
            assert_eq!(node.live(), Some(Live::Assertive));
            assert_eq!(node.label(), Some("Saved\nDisk full"));
        }
    }

    #[test]
    fn a_success_waits_its_turn() {
        let node = node_of(ToastKind::Success, None);
        assert_eq!(node.live(), Some(Live::Polite));
        assert_eq!(node.label(), Some("Saved"));
    }
}
