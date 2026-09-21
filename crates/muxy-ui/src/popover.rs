use crate::theme::{Metrics, Theme};
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, InteractiveElement, IntoElement, ParentElement,
    Pixels, Point, RenderOnce, Size, Styled, Window, canvas, div, point, px, size,
};
use std::{cell::Cell, rc::Rc};

pub type PopoverAnchor = Rc<Cell<Option<Bounds<Pixels>>>>;

pub fn surface(theme: &Theme, metrics: Metrics) -> gpui::Div {
    div()
        .occlude()
        .flex()
        .flex_col()
        .rounded(metrics.radius_lg())
        .border_1()
        .border_color(theme.border)
        .bg(theme.raised())
        .shadow_lg()
        .text_color(theme.fg)
        .text_size(metrics.font_body())
}

pub fn header(theme: &Theme, metrics: Metrics) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(metrics.control_large())
        .px(metrics.spacing4())
        .gap(metrics.spacing3())
        .border_b_1()
        .border_color(theme.border)
}

pub fn body(metrics: Metrics) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .flex_col()
        .p(metrics.spacing6())
        .gap(metrics.spacing4())
}

pub fn footer(theme: &Theme, metrics: Metrics) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_end()
        .p(metrics.spacing3())
        .gap(metrics.spacing2())
        .border_t_1()
        .border_color(theme.border)
}

pub fn anchored_popover(
    anchor: PopoverAnchor,
    content: AnyElement,
    on_anchor_lost: impl FnOnce(&mut Window, &mut App) + 'static,
) -> AnyElement {
    positioned_popover(anchor, content, dropdown_origin, on_anchor_lost)
}

pub fn anchored_popover_above(
    anchor: PopoverAnchor,
    content: AnyElement,
    on_anchor_lost: impl FnOnce(&mut Window, &mut App) + 'static,
) -> AnyElement {
    positioned_popover(anchor, content, above_origin, on_anchor_lost)
}

fn positioned_popover(
    anchor: PopoverAnchor,
    mut content: AnyElement,
    position: fn(Bounds<Pixels>, Size<Pixels>, Size<Pixels>) -> Point<Pixels>,
    on_anchor_lost: impl FnOnce(&mut Window, &mut App) + 'static,
) -> AnyElement {
    canvas(
        move |_, window, cx| {
            let Some(anchor) = anchor.get() else {
                window.defer(cx, on_anchor_lost);
                return None;
            };
            let size = content.layout_as_root(
                size(AvailableSpace::MinContent, AvailableSpace::MinContent),
                window,
                cx,
            );
            let origin = position(anchor, size, window.viewport_size());
            content.prepaint_at(origin, window, cx);
            Some(content)
        },
        |_, content, window, cx| {
            if let Some(mut content) = content {
                content.paint(window, cx);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

fn above_origin(
    anchor: Bounds<Pixels>,
    panel: Size<Pixels>,
    viewport: Size<Pixels>,
) -> Point<Pixels> {
    clamp_to_viewport(
        point(anchor.left(), anchor.top() - panel.height - px(4.0)),
        panel,
        viewport,
    )
}

fn dropdown_origin(
    anchor: Bounds<Pixels>,
    panel: Size<Pixels>,
    viewport: Size<Pixels>,
) -> Point<Pixels> {
    let left = if anchor.left() + panel.width > viewport.width - px(8.0) {
        anchor.right() - panel.width
    } else {
        anchor.left()
    };
    let below = anchor.bottom() + px(4.0);
    let above = anchor.top() - panel.height - px(4.0);
    let top = if below + panel.height > viewport.height - px(8.0) && above >= px(8.0) {
        above
    } else {
        below
    };
    clamp_to_viewport(point(left, top), panel, viewport)
}

pub fn clamp_to_viewport(
    origin: Point<Pixels>,
    panel: Size<Pixels>,
    viewport: Size<Pixels>,
) -> Point<Pixels> {
    point(
        origin
            .x
            .max(px(8.0))
            .min((viewport.width - panel.width - px(8.0)).max(px(8.0))),
        origin
            .y
            .max(px(8.0))
            .min((viewport.height - panel.height - px(8.0)).max(px(8.0))),
    )
}

#[derive(IntoElement)]
pub struct PopoverSurface {
    theme: Theme,
    metrics: Metrics,
    width: f32,
    height: f32,
    content: AnyElement,
}

impl PopoverSurface {
    pub fn new(
        theme: Theme,
        metrics: Metrics,
        width: f32,
        height: f32,
        content: impl IntoElement,
    ) -> Self {
        Self {
            theme,
            metrics,
            width,
            height,
            content: content.into_any_element(),
        }
    }
}

impl RenderOnce for PopoverSurface {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        surface(&self.theme, self.metrics)
            .w(self.metrics.scaled(self.width))
            .h(self.metrics.scaled(self.height))
            .overflow_hidden()
            .child(self.content)
    }
}

impl std::fmt::Debug for PopoverSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PopoverSurface")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropdown_flips_above_and_aligns_right_when_near_window_edges() {
        let viewport = size(px(800.0), px(600.0));
        let panel = size(px(300.0), px(200.0));
        let anchor = Bounds::new(point(px(650.0), px(500.0)), size(px(100.0), px(30.0)));
        let bounds = Bounds::new(dropdown_origin(anchor, panel, viewport), panel);
        assert_eq!(bounds.right(), anchor.right());
        assert!(bounds.bottom() <= anchor.top());
        assert!(bounds.top() >= px(0.0));
        let anchor = Bounds::new(point(px(50.0), px(50.0)), size(px(100.0), px(30.0)));
        let bounds = Bounds::new(dropdown_origin(anchor, panel, viewport), panel);
        assert_eq!(bounds.left(), anchor.left());
        assert!(bounds.top() >= anchor.bottom());
        assert!(bounds.bottom() <= viewport.height);
    }

    #[test]
    fn oversized_popovers_keep_their_origin_inside_the_window() {
        let viewport = size(px(800.0), px(600.0));
        let origin = clamp_to_viewport(
            point(px(-20.0), px(700.0)),
            size(px(900.0), px(700.0)),
            viewport,
        );
        assert!(origin.x >= px(0.0) && origin.x < viewport.width);
        assert!(origin.y >= px(0.0) && origin.y < viewport.height);
    }
}
