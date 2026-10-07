use crate::theme::{Metrics, Theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, ElementId, FontWeight, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, RenderOnce, Size, Styled, Window, canvas, div,
    point, px, size,
};
use std::{cell::Cell, rc::Rc};

pub type PopoverAnchor = Rc<Cell<Option<Bounds<Pixels>>>>;

pub const PADDING: f32 = 4.0;
pub const ROW_HEIGHT: f32 = 24.0;
pub const ROW_GAP: f32 = 1.0;
pub const ROW_PADDING: f32 = 6.0;

pub fn surface(theme: &Theme, metrics: Metrics) -> gpui::Div {
    div()
        .occlude()
        .flex()
        .flex_col()
        .rounded(metrics.radius_lg())
        .border_1()
        .border_color(theme.border)
        .bg(theme.bg)
        .p(metrics.scaled(PADDING))
        .gap(metrics.scaled(ROW_GAP))
        .shadow(crate::theme::Elevation::Elevated.shadow(theme.bg))
        .text_color(theme.fg)
        .text_size(metrics.font_body())
}

pub fn row(
    theme: &Theme,
    metrics: Metrics,
    id: impl Into<ElementId>,
    enabled: bool,
    highlighted: bool,
) -> gpui::Stateful<gpui::Div> {
    menu_row(theme, metrics, id, enabled, highlighted)
        .when(enabled, |row| row.hover(|style| style.bg(theme.hover)))
}

/// A row lit only by `highlighted`, for menus that move the highlight with the pointer themselves.
pub fn menu_row(
    theme: &Theme,
    metrics: Metrics,
    id: impl Into<ElementId>,
    enabled: bool,
    highlighted: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(metrics.scaled(ROW_PADDING))
        .px(metrics.scaled(ROW_PADDING))
        .h(metrics.scaled(ROW_HEIGHT))
        .rounded(metrics.radius_sm())
        .text_size(metrics.font_body())
        .text_color(if enabled { theme.fg } else { theme.fg_dim })
        .when(enabled, Styled::cursor_pointer)
        .when(enabled && highlighted, |row| row.bg(theme.hover))
}

pub fn header(theme: &Theme, metrics: Metrics) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .px(metrics.scaled(ROW_PADDING))
        .pb(metrics.spacing2())
        .gap(metrics.spacing3())
        .text_size(metrics.font_footnote())
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.fg_muted)
}

pub fn body(metrics: Metrics) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .flex_col()
        .px(metrics.scaled(ROW_PADDING))
        .py(metrics.spacing2())
        .gap(metrics.spacing4())
}

pub fn footer(theme: &Theme, metrics: Metrics) -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_end()
        .px(metrics.scaled(ROW_PADDING))
        .pt(metrics.spacing2())
        .gap(metrics.spacing3())
        .border_t_1()
        .border_color(theme.border)
}

pub fn divider(theme: &Theme, metrics: Metrics) -> gpui::Div {
    div()
        .flex_none()
        .h(px(1.0))
        .my(metrics.spacing2())
        .bg(theme.border)
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
            .p_0()
            .gap_0()
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
