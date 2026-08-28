use crate::toast::{ToastContent, ToastPosition, ToastTone};
use crate::views::window::MainWindow;
use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, div, px,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::theme::{Metrics, Theme};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToastAccent {
    Success,
    Warning,
    Error,
    Info,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToastPresentation {
    pub position: ToastPosition,
    pub accent: ToastAccent,
    pub icon: Icon,
    pub interactive: bool,
    pub accessibility_label: String,
    pub enters_from_top: bool,
}

pub fn presentation(content: &ToastContent, position: ToastPosition) -> ToastPresentation {
    let (accent, icon) = match content.tone {
        ToastTone::Success => (ToastAccent::Success, Icon::Check),
        ToastTone::Warning => (ToastAccent::Warning, Icon::Lightbulb),
        ToastTone::Error => (ToastAccent::Error, Icon::CircleX),
        ToastTone::Info => (ToastAccent::Info, Icon::Bell),
    };
    ToastPresentation {
        position,
        accent,
        icon,
        interactive: content.action.is_some(),
        accessibility_label: content.accessibility_label(),
        enters_from_top: position.is_top(),
    }
}

pub fn layer(
    content: &ToastContent,
    generation: u64,
    position: ToastPosition,
    theme: &Theme,
    metrics: Metrics,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let model = presentation(content, position);
    let accessibility_copy = SharedString::from(model.accessibility_label.clone());
    let color = match model.accent {
        ToastAccent::Success => theme.accent,
        ToastAccent::Warning => theme.warning,
        ToastAccent::Error => theme.danger,
        ToastAccent::Info => theme.fg,
    };
    let mut card = div()
        .id(SharedString::from(format!("toast-{generation}")))
        .relative()
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing4())
        .max_w(metrics.scaled(360.0))
        .px(metrics.spacing5())
        .py(metrics.spacing4())
        .rounded_full()
        .bg(theme.raised())
        .border_1()
        .border_color(theme.border_solid())
        .shadow_lg()
        .child(IconGlyph::new(model.icon, metrics.font_headline(), color))
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(px(0.0))
                .gap(metrics.spacing1())
                .child(
                    div()
                        .truncate()
                        .text_size(metrics.font_body())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.fg)
                        .child(SharedString::from(content.title.clone())),
                )
                .children(content.body.as_ref().map(|body| {
                    div()
                        .line_clamp(2)
                        .text_size(metrics.font_footnote())
                        .text_color(theme.fg_muted)
                        .child(SharedString::from(body.clone()))
                })),
        )
        .child(
            div()
                .absolute()
                .size(px(1.0))
                .opacity(0.0)
                .child(accessibility_copy),
        );
    if model.interactive {
        card = card
            .cursor_pointer()
            .hover(|style| style.bg(theme.hover))
            .on_click(cx.listener(|window: &mut MainWindow, _, _, cx| {
                window.activate_toast(cx);
            }));
    }
    let enters_from_top = model.enters_from_top;
    let card = card.with_animation(
        SharedString::from(format!("toast-entry-{generation}")),
        Animation::new(Duration::from_millis(160)),
        move |card, delta| {
            let offset = if enters_from_top {
                -6.0 * (1.0 - delta)
            } else {
                6.0 * (1.0 - delta)
            };
            card.opacity(delta).mt(px(offset))
        },
    );
    let mut layer = div()
        .absolute()
        .left_0()
        .w_full()
        .flex()
        .px(metrics.spacing7());
    if position.is_top() {
        layer = layer.top(metrics.scaled(40.0));
    } else {
        layer = layer.bottom(metrics.spacing7());
    }
    if position.is_centered() {
        layer = layer.justify_center();
    } else if position.is_right() {
        layer = layer.justify_end();
    }
    layer.child(card).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toast::ToastAction;

    #[test]
    fn toast_presentation_maps_all_tones_and_action_semantics() {
        let cases = [
            (ToastTone::Success, ToastAccent::Success, Icon::Check),
            (ToastTone::Warning, ToastAccent::Warning, Icon::Lightbulb),
            (ToastTone::Error, ToastAccent::Error, Icon::CircleX),
            (ToastTone::Info, ToastAccent::Info, Icon::Bell),
        ];
        for (tone, accent, icon) in cases {
            let content = ToastContent::new("Title", "Body", tone, None);
            let model = presentation(&content, ToastPosition::BottomRight);
            assert_eq!(model.accent, accent);
            assert_eq!(model.icon, icon);
            assert!(!model.interactive);
            assert_eq!(model.accessibility_label, "Title, Body");
            assert!(!model.enters_from_top);
        }

        let content = ToastContent::new(
            "Open",
            "Target",
            ToastTone::Info,
            Some(ToastAction::NavigateNotification("ID".to_owned())),
        );
        assert!(presentation(&content, ToastPosition::TopCenter).interactive);
        assert!(presentation(&content, ToastPosition::TopCenter).enters_from_top);
    }

    #[test]
    fn toast_render_emits_the_combined_accessibility_copy() {
        let source = include_str!("toast.rs");
        let start = source.find("pub fn layer(").unwrap();
        let end = source.find("#[cfg(test)]").unwrap();
        assert!(
            source[start..end].contains("SharedString::from(model.accessibility_label.clone())")
        );
    }
}
