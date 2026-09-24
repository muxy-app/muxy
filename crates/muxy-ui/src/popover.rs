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
        .when(enabled, |row| {
            row.cursor_pointer().hover(|style| style.bg(theme.hover))
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ColorScheme;
    use gpui::{Context, Render, TestAppContext};

    #[test]
    fn shell_and_rows_use_the_provider_menu_colors() {
        for background in [0x0019_171f, 0x00f8_f8f8] {
            let theme = Theme::from_scheme(&ColorScheme {
                background: Some(gpui::rgb(background)),
                ..ColorScheme::default()
            });
            let metrics = Metrics::new(1.0);
            assert_eq!(
                surface(&theme, metrics).style().background,
                Some(theme.bg.into())
            );
            assert_eq!(
                surface(&theme, metrics).style().box_shadow,
                Some(crate::theme::Elevation::Elevated.shadow(theme.bg))
            );
            assert_eq!(
                row(&theme, metrics, "selected", true, true)
                    .style()
                    .background,
                Some(theme.hover.into())
            );
            let mut disabled = row(&theme, metrics, "disabled", false, true);
            assert_eq!(disabled.style().background, None);
            assert_eq!(
                disabled.style().text.as_ref().unwrap().color,
                Some(theme.fg_dim)
            );
            assert_eq!(
                header(&theme, metrics).style().text.as_ref().unwrap().color,
                Some(theme.fg_muted)
            );
        }
    }

    struct TestPopover {
        metrics: Metrics,
        edge_to_edge: bool,
    }

    impl Render for TestPopover {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let theme = Theme::from_scheme(&ColorScheme::default());
            let m = self.metrics;
            let content = if self.edge_to_edge {
                PopoverSurface::new(
                    theme,
                    m,
                    240.0,
                    160.0,
                    div()
                        .size_full()
                        .debug_selector(|| "popover-content".into()),
                )
                .into_any_element()
            } else {
                surface(&theme, m)
                    .debug_selector(|| "popover-test".into())
                    .w(m.scaled(240.0))
                    .child(
                        row(&theme, m, "first", true, false)
                            .debug_selector(|| "popover-first".into())
                            .child("First"),
                    )
                    .child(
                        row(&theme, m, "second", true, true)
                            .debug_selector(|| "popover-second".into())
                            .child("Second"),
                    )
                    .into_any_element()
            };
            div().flex().items_start().child(content)
        }
    }

    #[gpui::test]
    fn shared_shell_keeps_compact_padding_row_height_and_gap_at_each_scale(
        cx: &mut TestAppContext,
    ) {
        for scale in [0.75, 1.0, 1.5, 2.0] {
            let m = Metrics::new(scale);
            let (_, cx) = cx.add_window_view(|_, _| TestPopover {
                metrics: m,
                edge_to_edge: false,
            });
            cx.run_until_parked();
            let panel = cx.debug_bounds("popover-test").unwrap();
            let first = cx.debug_bounds("popover-first").unwrap();
            let second = cx.debug_bounds("popover-second").unwrap();
            let near = |actual: Pixels, expected: Pixels| {
                assert!((f32::from(actual - expected)).abs() <= 1.0);
            };
            near(first.left() - panel.left(), m.scaled(4.0) + px(1.0));
            near(first.top() - panel.top(), m.scaled(4.0) + px(1.0));
            near(first.size.height, m.scaled(ROW_HEIGHT));
            near(second.top() - first.bottom(), m.scaled(ROW_GAP));
            near(panel.bottom() - second.bottom(), m.scaled(4.0) + px(1.0));
        }
    }

    #[gpui::test]
    fn fixed_size_content_wrapper_preserves_edge_to_edge_dimensions(cx: &mut TestAppContext) {
        let m = Metrics::new(1.5);
        let (_, cx) = cx.add_window_view(|_, _| TestPopover {
            metrics: m,
            edge_to_edge: true,
        });
        cx.run_until_parked();
        let content = cx.debug_bounds("popover-content").unwrap();
        assert_eq!(content.size.width, m.scaled(240.0) - px(2.0));
        assert_eq!(content.size.height, m.scaled(160.0) - px(2.0));
    }

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
