//! Panels that float freely over the window: where each one is, how a drag
//! moves or resizes it, and which one is in front.

use crate::theme::{Metrics, Theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, CursorStyle, ElementId, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Pixels, Point, RenderOnce, Size, Styled, Window, div, px,
};
use std::rc::Rc;

/// A floating panel's place in window points, before the app's zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatingFrame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl FloatingFrame {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn from_array([x, y, width, height]: [f32; 4]) -> Self {
        Self::new(x, y, width, height)
    }

    pub const fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }

    pub fn is_finite(self) -> bool {
        self.to_array().iter().all(|value| value.is_finite())
    }

    /// Fits the frame inside `area` and `limits`, moving it before shrinking
    /// it. An area smaller than the minimum keeps the minimum at its origin.
    #[must_use]
    pub fn clamped(self, area: Size<f32>, limits: SizeLimits) -> Self {
        let fit = |origin: f32, extent: f32, area: f32, minimum: f32, maximum: f32| {
            let ceiling = maximum.min(area).max(minimum);
            let extent = if extent.is_finite() { extent } else { minimum };
            let extent = extent.clamp(minimum, ceiling);
            let origin = if origin.is_finite() { origin } else { 0.0 };
            (origin.clamp(0.0, (area - extent).max(0.0)), extent)
        };
        let (x, width) = fit(
            self.x,
            self.width,
            area.width,
            limits.min.width,
            limits.max.width,
        );
        let (y, height) = fit(
            self.y,
            self.height,
            area.height,
            limits.min.height,
            limits.max.height,
        );
        Self::new(x, y, width, height)
    }
}

/// The smallest and largest size a floating panel may take.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizeLimits {
    pub min: Size<f32>,
    pub max: Size<f32>,
}

impl SizeLimits {
    pub fn new(min: Size<f32>, max: Size<f32>) -> Self {
        let sane = |value: f32, fallback: f32| {
            if value.is_nan() {
                fallback
            } else {
                value.max(0.0)
            }
        };
        let min = Size {
            width: sane(min.width, 0.0).min(16384.0),
            height: sane(min.height, 0.0).min(16384.0),
        };
        let max = Size {
            width: sane(max.width, f32::INFINITY).max(min.width),
            height: sane(max.height, f32::INFINITY).max(min.height),
        };
        Self { min, max }
    }

    pub fn at_least(width: f32, height: f32) -> Self {
        Self::new(
            Size { width, height },
            Size {
                width: f32::INFINITY,
                height: f32::INFINITY,
            },
        )
    }
}

/// Which side of an axis a resize drag moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// The left or top edge.
    Start,
    /// The right or bottom edge.
    End,
}

/// The edges a resize drag moves, one per axis at most.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Edges {
    pub horizontal: Option<Side>,
    pub vertical: Option<Side>,
}

impl Edges {
    pub const LEFT: Self = Self::of(Some(Side::Start), None);
    pub const TOP: Self = Self::of(None, Some(Side::Start));
    pub const RIGHT: Self = Self::of(Some(Side::End), None);
    pub const BOTTOM: Self = Self::of(None, Some(Side::End));
    pub const TOP_LEFT: Self = Self::of(Some(Side::Start), Some(Side::Start));
    pub const TOP_RIGHT: Self = Self::of(Some(Side::End), Some(Side::Start));
    pub const BOTTOM_LEFT: Self = Self::of(Some(Side::Start), Some(Side::End));
    pub const BOTTOM_RIGHT: Self = Self::of(Some(Side::End), Some(Side::End));
    pub const ALL: [Self; 8] = [
        Self::LEFT,
        Self::TOP,
        Self::RIGHT,
        Self::BOTTOM,
        Self::TOP_LEFT,
        Self::TOP_RIGHT,
        Self::BOTTOM_LEFT,
        Self::BOTTOM_RIGHT,
    ];

    const fn of(horizontal: Option<Side>, vertical: Option<Side>) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }

    pub fn cursor(self) -> CursorStyle {
        match (self.horizontal, self.vertical) {
            (Some(_), None) => CursorStyle::ResizeLeftRight,
            (None, Some(_)) => CursorStyle::ResizeUpDown,
            (Some(horizontal), Some(vertical)) if horizontal == vertical => {
                CursorStyle::ResizeUpLeftDownRight
            }
            _ => CursorStyle::ResizeUpRightDownLeft,
        }
    }

    fn name(self) -> &'static str {
        match (self.horizontal, self.vertical) {
            (Some(Side::Start), None) => "left",
            (Some(Side::End), None) => "right",
            (None, Some(Side::Start)) => "top",
            (None, Some(Side::End)) => "bottom",
            (Some(Side::Start), Some(Side::Start)) => "top-left",
            (Some(Side::End), Some(Side::Start)) => "top-right",
            (Some(Side::Start), Some(Side::End)) => "bottom-left",
            _ => "bottom-right",
        }
    }
}

/// A drag that moves a floating panel, or resizes it by some of its edges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatingDrag {
    pointer: Point<f32>,
    start: FloatingFrame,
    edges: Option<Edges>,
}

impl FloatingDrag {
    pub fn moving(pointer: Point<Pixels>, start: FloatingFrame) -> Self {
        Self {
            pointer: point_of(pointer),
            start,
            edges: None,
        }
    }

    pub fn resizing(pointer: Point<Pixels>, start: FloatingFrame, edges: Edges) -> Self {
        Self {
            pointer: point_of(pointer),
            start,
            edges: Some(edges),
        }
    }

    pub fn is_resize(&self) -> bool {
        self.edges.is_some()
    }

    /// The frame once the pointer is at `pointer`. `scale` turns window
    /// pixels into frame points. An edge stops at the area's border, and the
    /// opposite edge stays put while the panel is at a size limit.
    pub fn frame_at(
        &self,
        pointer: Point<Pixels>,
        scale: f32,
        area: Size<f32>,
        limits: SizeLimits,
    ) -> FloatingFrame {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let pointer = point_of(pointer);
        let dx = (pointer.x - self.pointer.x) / scale;
        let dy = (pointer.y - self.pointer.y) / scale;
        let start = self.start;
        let Some(edges) = self.edges else {
            return FloatingFrame::new(start.x + dx, start.y + dy, start.width, start.height)
                .clamped(area, limits);
        };
        let axis = |(origin, extent): (f32, f32),
                    delta: f32,
                    side: Option<Side>,
                    area: f32,
                    (minimum, maximum): (f32, f32)| match side {
            Some(Side::End) => {
                let ceiling = maximum.min(area - origin).max(minimum);
                (origin, (extent + delta).clamp(minimum, ceiling))
            }
            Some(Side::Start) => {
                let end = origin + extent;
                let ceiling = maximum.min(end).max(minimum);
                let extent = (extent - delta).clamp(minimum, ceiling);
                (end - extent, extent)
            }
            None => (origin, extent),
        };
        let (x, width) = axis(
            (start.x, start.width),
            dx,
            edges.horizontal,
            area.width,
            (limits.min.width, limits.max.width),
        );
        let (y, height) = axis(
            (start.y, start.height),
            dy,
            edges.vertical,
            area.height,
            (limits.min.height, limits.max.height),
        );
        FloatingFrame::new(x, y, width, height).clamped(area, limits)
    }
}

fn point_of(pointer: Point<Pixels>) -> Point<f32> {
    Point::new(f32::from(pointer.x), f32::from(pointer.y))
}

/// Which floating panel is in front of which, bottom first.
#[derive(Debug, Clone)]
pub struct FloatingStack<K> {
    order: Vec<K>,
}

impl<K> Default for FloatingStack<K> {
    fn default() -> Self {
        Self { order: Vec::new() }
    }
}

impl<K: PartialEq + Clone> FloatingStack<K> {
    /// Brings `key` to the front, reporting whether the order changed.
    pub fn raise(&mut self, key: &K) -> bool {
        if self.order.last() == Some(key) {
            return false;
        }
        self.order.retain(|entry| entry != key);
        self.order.push(key.clone());
        true
    }

    pub fn remove(&mut self, key: &K) {
        self.order.retain(|entry| entry != key);
    }

    pub fn retain(&mut self, keep: impl FnMut(&K) -> bool) {
        self.order.retain(keep);
    }

    pub fn rank(&self, key: &K) -> Option<usize> {
        self.order.iter().position(|entry| entry == key)
    }

    pub fn frontmost(&self) -> Option<&K> {
        self.order.last()
    }

    pub fn iter(&self) -> impl Iterator<Item = &K> {
        self.order.iter()
    }

    /// Orders `items` bottom first. Items the stack doesn't know go on top,
    /// in the order given.
    pub fn sort<T>(&self, items: &mut [T], key: impl Fn(&T) -> &K) {
        items.sort_by_key(|item| self.rank(key(item)).unwrap_or(usize::MAX));
    }
}

type DragHandler = Rc<dyn Fn(Option<Edges>, Point<Pixels>, &mut Window, &mut App)>;
type GripFactory = Rc<dyn Fn(CursorStyle) -> AnyElement>;

const RESIZE_EDGE: f32 = 6.0;
const RESIZE_CORNER: f32 = 12.0;

/// A floating panel's box: drawn as a raised card, or left bare so the page
/// draws everything itself. Its edges and corners resize it.
#[derive(IntoElement)]
#[must_use]
pub struct FloatingCard {
    id: ElementId,
    frame: FloatingFrame,
    unit: f32,
    framed: bool,
    resizable: bool,
    interactive: bool,
    children: Vec<AnyElement>,
    on_drag: Option<DragHandler>,
    grip: Option<GripFactory>,
    theme: Theme,
    metrics: Metrics,
}

impl FloatingCard {
    pub fn new(
        id: impl Into<ElementId>,
        frame: FloatingFrame,
        theme: Theme,
        metrics: Metrics,
    ) -> Self {
        Self {
            id: id.into(),
            unit: metrics.multiplier(),
            frame,
            framed: true,
            resizable: true,
            interactive: true,
            children: Vec::new(),
            on_drag: None,
            grip: None,
            theme,
            metrics,
        }
    }

    /// Window pixels per frame point. The app's zoom by default; `1.0` for
    /// a frame in the pixels its page lays out in.
    pub fn unit(mut self, unit: f32) -> Self {
        self.unit = unit;
        self
    }

    /// Whether the card draws its background, border, and shadow.
    pub fn framed(mut self, framed: bool) -> Self {
        self.framed = framed;
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Whether the card takes the clicks on it. One that doesn't lets them
    /// reach what is under it.
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Called when a resize handle is pressed, with the edges it moves.
    pub fn on_resize(
        mut self,
        handler: impl Fn(Option<Edges>, Point<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_drag = Some(Rc::new(handler));
        self
    }

    /// Adds `grip(cursor)` inside each resize handle, such as a layer that
    /// lets native views under the handle pass clicks to it.
    pub fn grip(mut self, grip: impl Fn(CursorStyle) -> AnyElement + 'static) -> Self {
        self.grip = Some(Rc::new(grip));
        self
    }
}

impl ParentElement for FloatingCard {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for FloatingCard {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let m = self.metrics;
        let FloatingFrame {
            x,
            y,
            width,
            height,
        } = self.frame;
        let handles = self
            .on_drag
            .filter(|_| self.resizable)
            .map(|handler| {
                Edges::ALL
                    .into_iter()
                    .map(|edges| resize_handle(&self.id, edges, &handler, self.grip.as_ref(), m))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let selector = self.id.to_string();
        div()
            .id(self.id)
            .debug_selector(move || selector.clone())
            .absolute()
            .left(px(x * self.unit))
            .top(px(y * self.unit))
            .w(px(width * self.unit))
            .h(px(height * self.unit))
            .flex()
            .flex_col()
            .when(self.framed, |card| {
                card.rounded(m.radius_xl())
                    .border_1()
                    .border_color(self.theme.border)
                    .bg(self.theme.raised())
                    .shadow_lg()
            })
            .when(self.interactive, |card| {
                card.occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            })
            .children(self.children)
            .children(handles)
    }
}

fn resize_handle(
    card: &ElementId,
    edges: Edges,
    handler: &DragHandler,
    grip: Option<&GripFactory>,
    m: Metrics,
) -> AnyElement {
    let edge = m.scaled(RESIZE_EDGE);
    let corner = m.scaled(RESIZE_CORNER);
    let handler = handler.clone();
    let cursor = edges.cursor();
    let corner_handle = edges.horizontal.is_some() && edges.vertical.is_some();
    let name = format!("{card}-resize-{}", edges.name());
    let selector = name.clone();
    div()
        .id(ElementId::Name(name.into()))
        .debug_selector(move || selector.clone())
        .absolute()
        .occlude()
        .cursor(cursor)
        .map(|handle| {
            if corner_handle {
                handle.size(corner)
            } else if edges.horizontal.is_some() {
                handle.top(corner).bottom(corner).w(edge)
            } else {
                handle.left(corner).right(corner).h(edge)
            }
        })
        .map(|handle| match edges.horizontal {
            Some(Side::Start) => handle.left(px(0.0)),
            Some(Side::End) => handle.right(px(0.0)),
            None => handle,
        })
        .map(|handle| match edges.vertical {
            Some(Side::Start) => handle.top(px(0.0)),
            Some(Side::End) => handle.bottom(px(0.0)),
            None => handle,
        })
        .on_mouse_down(
            MouseButton::Left,
            move |event: &MouseDownEvent, window, cx| {
                handler(Some(edges), event.position, window, cx);
                window.prevent_default();
                cx.stop_propagation();
            },
        )
        .children(grip.map(|grip| grip(cursor)))
        .into_any_element()
}

impl std::fmt::Debug for FloatingCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FloatingCard")
            .field("id", &self.id)
            .field("frame", &self.frame)
            .field("framed", &self.framed)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: f32, height: f32) -> Size<f32> {
        Size { width, height }
    }

    fn at(x: f32, y: f32) -> Point<Pixels> {
        Point::new(px(x), px(y))
    }

    #[test]
    fn a_frame_moves_back_inside_before_it_shrinks() {
        let limits = SizeLimits::at_least(100.0, 50.0);
        let frame =
            FloatingFrame::new(900.0, -20.0, 300.0, 200.0).clamped(area(1000.0, 800.0), limits);
        assert_eq!(frame, FloatingFrame::new(700.0, 0.0, 300.0, 200.0));
        let frame =
            FloatingFrame::new(10.0, 10.0, 3000.0, 20.0).clamped(area(1000.0, 800.0), limits);
        assert_eq!(frame, FloatingFrame::new(0.0, 10.0, 1000.0, 50.0));
    }

    #[test]
    fn a_window_smaller_than_the_minimum_keeps_the_minimum() {
        let frame = FloatingFrame::new(30.0, 30.0, 300.0, 300.0)
            .clamped(area(100.0, 100.0), SizeLimits::at_least(200.0, 150.0));
        assert_eq!(frame, FloatingFrame::new(0.0, 0.0, 200.0, 150.0));
    }

    #[test]
    fn broken_numbers_fall_back_to_the_limits() {
        let frame = FloatingFrame::new(f32::NAN, 5.0, f32::INFINITY, f32::NAN)
            .clamped(area(1000.0, 800.0), SizeLimits::at_least(80.0, 60.0));
        assert_eq!(frame, FloatingFrame::new(0.0, 5.0, 80.0, 60.0));
        let limits = SizeLimits::new(
            Size {
                width: -4.0,
                height: f32::NAN,
            },
            Size {
                width: 10.0,
                height: -1.0,
            },
        );
        assert_eq!(limits.min, area(0.0, 0.0));
        assert_eq!(limits.max, area(10.0, 0.0));
    }

    #[test]
    fn moving_follows_the_pointer_at_the_window_scale() {
        let drag = FloatingDrag::moving(
            at(100.0, 100.0),
            FloatingFrame::new(50.0, 50.0, 200.0, 100.0),
        );
        let frame = drag.frame_at(
            at(140.0, 80.0),
            2.0,
            area(1000.0, 800.0),
            SizeLimits::at_least(10.0, 10.0),
        );
        assert_eq!(frame, FloatingFrame::new(70.0, 40.0, 200.0, 100.0));
        let frame = drag.frame_at(
            at(-5000.0, 5000.0),
            1.0,
            area(1000.0, 800.0),
            SizeLimits::at_least(10.0, 10.0),
        );
        assert_eq!(frame, FloatingFrame::new(0.0, 700.0, 200.0, 100.0));
    }

    #[test]
    fn resizing_by_the_far_edges_stops_at_the_window() {
        let drag = FloatingDrag::resizing(
            at(0.0, 0.0),
            FloatingFrame::new(700.0, 100.0, 200.0, 100.0),
            Edges::BOTTOM_RIGHT,
        );
        let frame = drag.frame_at(
            at(500.0, 40.0),
            1.0,
            area(1000.0, 800.0),
            SizeLimits::at_least(50.0, 50.0),
        );
        assert_eq!(frame, FloatingFrame::new(700.0, 100.0, 300.0, 140.0));
    }

    #[test]
    fn resizing_by_the_near_edges_keeps_the_far_edges() {
        let drag = FloatingDrag::resizing(
            at(0.0, 0.0),
            FloatingFrame::new(100.0, 100.0, 200.0, 200.0),
            Edges::TOP_LEFT,
        );
        let limits = SizeLimits::at_least(50.0, 50.0);
        let frame = drag.frame_at(at(-40.0, 30.0), 1.0, area(1000.0, 800.0), limits);
        assert_eq!(frame, FloatingFrame::new(60.0, 130.0, 240.0, 170.0));
        let frame = drag.frame_at(at(400.0, -400.0), 1.0, area(1000.0, 800.0), limits);
        assert_eq!(frame, FloatingFrame::new(250.0, 0.0, 50.0, 300.0));
    }

    #[test]
    fn resizing_respects_the_largest_size() {
        let drag = FloatingDrag::resizing(
            at(0.0, 0.0),
            FloatingFrame::new(0.0, 0.0, 100.0, 100.0),
            Edges::RIGHT,
        );
        let limits = SizeLimits::new(area(50.0, 50.0), area(150.0, 150.0));
        let frame = drag.frame_at(at(400.0, 400.0), 1.0, area(1000.0, 800.0), limits);
        assert_eq!(frame, FloatingFrame::new(0.0, 0.0, 150.0, 100.0));
    }

    #[test]
    fn the_stack_raises_and_orders_bottom_first() {
        let mut stack = FloatingStack::default();
        assert!(stack.raise(&"a"));
        assert!(stack.raise(&"b"));
        assert!(!stack.raise(&"b"));
        assert!(stack.raise(&"a"));
        assert_eq!(stack.frontmost(), Some(&"a"));
        let mut items = vec!["new", "a", "b"];
        stack.sort(&mut items, |item| item);
        assert_eq!(items, ["b", "a", "new"]);
        stack.remove(&"a");
        assert_eq!(stack.iter().collect::<Vec<_>>(), [&"b"]);
    }

    #[test]
    fn each_edge_has_its_cursor() {
        assert_eq!(Edges::LEFT.cursor(), CursorStyle::ResizeLeftRight);
        assert_eq!(Edges::BOTTOM.cursor(), CursorStyle::ResizeUpDown);
        assert_eq!(Edges::TOP_LEFT.cursor(), CursorStyle::ResizeUpLeftDownRight);
        assert_eq!(
            Edges::BOTTOM_LEFT.cursor(),
            CursorStyle::ResizeUpRightDownLeft
        );
    }
}
