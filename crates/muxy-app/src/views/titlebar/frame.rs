use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Bounds, BoxShadow, CursorStyle, Decorations, Div, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, ResizeEdge, Size, Styled, Tiling, Window, div,
    point, px, size,
};
use muxy_ui::theme::Theme;

const INSET: f32 = 10.0;
const BORDER: f32 = 1.0;
const CONTENT_PADDING: f32 = 2.0;
const CORNER_RADIUS: f32 = 8.0;

fn tiling(window: &Window) -> Option<Tiling> {
    match window.window_decorations() {
        Decorations::Client { tiling } => {
            Some(if window.is_fullscreen() || window.is_maximized() {
                Tiling::tiled()
            } else {
                tiling
            })
        }
        Decorations::Server => None,
    }
}

fn content_bounds(size: Size<Pixels>, tiling: Tiling) -> Bounds<Pixels> {
    let rim = INSET + BORDER + CONTENT_PADDING;
    let left = px(if tiling.left { 0.0 } else { rim });
    let top = px(if tiling.top { 0.0 } else { INSET + BORDER });
    let right = px(if tiling.right { 0.0 } else { rim });
    let bottom = px(if tiling.bottom { 0.0 } else { rim });
    Bounds::new(
        point(left, top),
        gpui::size(
            (size.width - left - right).max(px(0.0)),
            (size.height - top - bottom).max(px(0.0)),
        ),
    )
}

pub(super) fn bounds(window: &Window) -> Bounds<Pixels> {
    content_bounds(
        window.viewport_size(),
        tiling(window).unwrap_or(Tiling::tiled()),
    )
}

pub(super) fn top_corners<T: Styled>(
    mut element: T,
    window: &Window,
    left: bool,
    right: bool,
) -> T {
    let Some(tiling) = tiling(window) else {
        return element;
    };
    let radius = px(CORNER_RADIUS - BORDER - CONTENT_PADDING);
    if left && !(tiling.top || tiling.left) {
        element = element.rounded_tl(radius);
    }
    if right && !(tiling.top || tiling.right) {
        element = element.rounded_tr(radius);
    }
    element
}

pub(super) fn overlay(element: impl IntoElement, window: &Window) -> AnyElement {
    let bounds = bounds(window);
    div()
        .absolute()
        .left(-bounds.origin.x)
        .top(-bounds.origin.y)
        .w(window.viewport_size().width)
        .h(window.viewport_size().height)
        .child(element)
        .into_any_element()
}

pub(crate) fn render(
    element: impl IntoElement,
    theme: &Theme,
    sidebar: Option<(Pixels, Hsla)>,
    dim: Option<f32>,
    window: &mut Window,
) -> AnyElement {
    let Some(tiling) = tiling(window) else {
        window.set_client_inset(px(0.0));
        return element.into_any_element();
    };
    window.set_client_inset(px(INSET));
    let rounding = px(CORNER_RADIUS);
    let surface = div()
        .relative()
        .size_full()
        .bg(theme.bg)
        .border_color(dim.map_or(theme.border, |alpha| {
            theme.border.blend(gpui::hsla(0.0, 0.0, 0.0, alpha))
        }))
        .when(!tiling.top, |surface| surface.border_t(px(BORDER)))
        .when(!tiling.bottom, |surface| surface.border_b(px(BORDER)))
        .when(!tiling.left, |surface| surface.border_l(px(BORDER)))
        .when(!tiling.right, |surface| surface.border_r(px(BORDER)))
        .when(!tiling.bottom, |surface| surface.pb(px(CONTENT_PADDING)))
        .when(!tiling.left, |surface| surface.pl(px(CONTENT_PADDING)))
        .when(!tiling.right, |surface| surface.pr(px(CONTENT_PADDING)))
        .when(!(tiling.top || tiling.left), |surface| {
            surface.rounded_tl(rounding)
        })
        .when(!(tiling.top || tiling.right), |surface| {
            surface.rounded_tr(rounding)
        })
        .when(!(tiling.bottom || tiling.left), |surface| {
            surface.rounded_bl(rounding)
        })
        .when(!(tiling.bottom || tiling.right), |surface| {
            surface.rounded_br(rounding)
        })
        .when(!tiling.is_tiled(), |surface| {
            surface.shadow(vec![BoxShadow {
                color: gpui::black().opacity(0.35),
                offset: point(px(0.0), px(2.0)),
                blur_radius: px(INSET / 2.0),
                spread_radius: px(0.0),
            }])
        })
        .overflow_hidden()
        .when_some(sidebar, |surface, (width, color)| {
            surface.child(sidebar_background(width, color, tiling))
        })
        .child(div().size_full().overflow_hidden().child(element))
        .when_some(dim, |surface, alpha| {
            surface.child(dimmed_rim(alpha, tiling))
        });
    div()
        .relative()
        .size_full()
        .when(!tiling.top, |frame| frame.pt(px(INSET)))
        .when(!tiling.bottom, |frame| frame.pb(px(INSET)))
        .when(!tiling.left, |frame| frame.pl(px(INSET)))
        .when(!tiling.right, |frame| frame.pr(px(INSET)))
        .child(surface)
        .children(
            resize_regions(window.viewport_size(), tiling)
                .into_iter()
                .map(|(edge, bounds)| {
                    let cursor = match edge {
                        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
                        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
                        ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                            CursorStyle::ResizeUpLeftDownRight
                        }
                        ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                            CursorStyle::ResizeUpRightDownLeft
                        }
                    };
                    div()
                        .absolute()
                        .left(bounds.origin.x)
                        .top(bounds.origin.y)
                        .w(bounds.size.width)
                        .h(bounds.size.height)
                        .cursor(cursor)
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            cx.stop_propagation();
                            window.start_window_resize(edge);
                        })
                }),
        )
        .into_any_element()
}

/// A dialog dimming the window stops at the content, which clips it. This
/// darkens the padding around the content to match.
fn dimmed_rim(alpha: f32, tiling: Tiling) -> Div {
    let padding = px(CONTENT_PADDING);
    let radius = px(CORNER_RADIUS - BORDER);
    div()
        .absolute()
        .inset_0()
        .border_color(gpui::hsla(0.0, 0.0, 0.0, alpha))
        .when(!tiling.bottom, |rim| rim.border_b(padding))
        .when(!tiling.left, |rim| rim.border_l(padding))
        .when(!tiling.right, |rim| rim.border_r(padding))
        .when(!(tiling.top || tiling.left), |rim| rim.rounded_tl(radius))
        .when(!(tiling.top || tiling.right), |rim| rim.rounded_tr(radius))
        .when(!(tiling.bottom || tiling.left), |rim| {
            rim.rounded_bl(radius)
        })
        .when(!(tiling.bottom || tiling.right), |rim| {
            rim.rounded_br(radius)
        })
}

fn sidebar_background(width: Pixels, color: Hsla, tiling: Tiling) -> Div {
    let padding = if tiling.left { 0.0 } else { CONTENT_PADDING };
    div()
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .w(width + px(padding))
        .bg(color)
        .when(!(tiling.top || tiling.left), |background| {
            background.rounded_tl(px(CORNER_RADIUS - BORDER))
        })
        .when(!(tiling.bottom || tiling.left), |background| {
            background.rounded_bl(px(CORNER_RADIUS - BORDER))
        })
}

fn resize_regions(viewport: Size<Pixels>, tiling: Tiling) -> Vec<(ResizeEdge, Bounds<Pixels>)> {
    let inset = px(INSET);
    let left = if tiling.left { px(0.0) } else { inset };
    let right = if tiling.right { px(0.0) } else { inset };
    let top = if tiling.top { px(0.0) } else { inset };
    let bottom = if tiling.bottom { px(0.0) } else { inset };
    let mut regions = Vec::with_capacity(8);
    for (enabled, edge, x, y, width, height) in [
        (
            !tiling.top,
            ResizeEdge::Top,
            left,
            px(0.0),
            viewport.width - left - right,
            inset,
        ),
        (
            !tiling.bottom,
            ResizeEdge::Bottom,
            left,
            viewport.height - inset,
            viewport.width - left - right,
            inset,
        ),
        (
            !tiling.left,
            ResizeEdge::Left,
            px(0.0),
            top,
            inset,
            viewport.height - top - bottom,
        ),
        (
            !tiling.right,
            ResizeEdge::Right,
            viewport.width - inset,
            top,
            inset,
            viewport.height - top - bottom,
        ),
        (
            !(tiling.top || tiling.left),
            ResizeEdge::TopLeft,
            px(0.0),
            px(0.0),
            inset,
            inset,
        ),
        (
            !(tiling.top || tiling.right),
            ResizeEdge::TopRight,
            viewport.width - inset,
            px(0.0),
            inset,
            inset,
        ),
        (
            !(tiling.bottom || tiling.left),
            ResizeEdge::BottomLeft,
            px(0.0),
            viewport.height - inset,
            inset,
            inset,
        ),
        (
            !(tiling.bottom || tiling.right),
            ResizeEdge::BottomRight,
            viewport.width - inset,
            viewport.height - inset,
            inset,
            inset,
        ),
    ] {
        if enabled {
            regions.push((edge, Bounds::new(point(x, y), size(width, height))));
        }
    }
    regions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_targets_stay_outside_content_and_cover_each_free_edge() {
        let viewport = size(px(800.0), px(600.0));
        for tiling in [
            Tiling::default(),
            Tiling {
                top: true,
                left: true,
                ..Tiling::default()
            },
            Tiling::tiled(),
        ] {
            let content = content_bounds(viewport, tiling);
            for (edge, bounds) in resize_regions(viewport, tiling) {
                let center = bounds.center();
                assert!(!content.contains(&center), "{edge:?}");
                match edge {
                    ResizeEdge::Top => assert!(!tiling.top),
                    ResizeEdge::Bottom => assert!(!tiling.bottom),
                    ResizeEdge::Left => assert!(!tiling.left),
                    ResizeEdge::Right => assert!(!tiling.right),
                    ResizeEdge::TopLeft => assert!(!(tiling.top || tiling.left)),
                    ResizeEdge::TopRight => assert!(!(tiling.top || tiling.right)),
                    ResizeEdge::BottomLeft => assert!(!(tiling.bottom || tiling.left)),
                    ResizeEdge::BottomRight => assert!(!(tiling.bottom || tiling.right)),
                }
            }
        }
        assert_eq!(resize_regions(viewport, Tiling::default()).len(), 8);
        assert!(resize_regions(viewport, Tiling::tiled()).is_empty());
        assert_eq!(content_bounds(viewport, Tiling::tiled()).size, viewport);
        let snapped = content_bounds(
            viewport,
            Tiling {
                left: true,
                top: true,
                bottom: true,
                right: false,
            },
        );
        assert_eq!(snapped.origin, point(px(0.0), px(0.0)));
        assert_eq!(snapped.size, size(px(787.0), px(600.0)));
    }
}
