//! The terminal's own scrollbar, for panes the system gives none.

use std::time::Instant;

use gpui::{Bounds, Context, Pixels, Task, point, px, size};
use muxy_ui::scrollbar::{
    MINIMUM_THUMB_LENGTH, REVEAL_DURATION, ScrollbarRevealState, ThumbGeometry, WIDTH,
};

use super::pane::TerminalPane;

/// The gap between the thumb and the pane's right edge.
const EDGE_GAP: f32 = 2.0;

#[derive(Debug)]
pub(crate) struct Scrollbar {
    reveal: ScrollbarRevealState,
    /// Reveal times count from here.
    epoch: Instant,
    hide: Option<Task<()>>,
    /// Where a drag holds the thumb, in pixels below its top.
    grab: Option<f32>,
    /// Where the pane was last drawn.
    track: Option<Bounds<Pixels>>,
}

struct Geometry {
    track: Bounds<Pixels>,
    thumb: ThumbGeometry,
    /// The rows the pane can scroll through.
    scrollable: f64,
}

impl Geometry {
    fn thumb_bounds(&self) -> Bounds<Pixels> {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "Thumb geometry is bounded by the track's f32 pixels"
        )]
        let (origin, length) = (self.thumb.origin as f32, self.thumb.length as f32);
        Bounds::new(
            point(
                self.track.right() - px(WIDTH + EDGE_GAP),
                self.track.top() + px(origin),
            ),
            size(px(WIDTH), px(length)),
        )
    }

    /// How far above the bottom the view is when the thumb's top sits `origin`
    /// pixels down the track.
    fn rows_from_bottom(&self, origin: f32) -> f64 {
        let travel = f64::from(f32::from(self.track.size.height)) - self.thumb.length;
        if travel <= 0.0 {
            return 0.0;
        }
        self.scrollable * (1.0 - (f64::from(origin) / travel).clamp(0.0, 1.0))
    }
}

impl Scrollbar {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            reveal: ScrollbarRevealState::default(),
            epoch: now,
            hide: None,
            grab: None,
            track: None,
        }
    }

    pub(super) fn place(&mut self, track: Bounds<Pixels>) {
        self.track = Some(track);
    }

    fn shown(&self, now: Instant) -> bool {
        self.reveal.allows_hit(now.duration_since(self.epoch))
    }
}

impl TerminalPane {
    fn scrollbar_geometry(&self) -> Option<Geometry> {
        #[cfg(target_os = "macos")]
        if self.native_scroll.is_some() {
            return None;
        }
        if !self.scrolls_history() {
            return None;
        }
        let track = self.scrollbar.track?;
        let rows = self
            .viewport()
            .or(self.grid.as_ref().map(|grid| grid.size))?
            .rows;
        let scrollable = self.scrollable_rows(rows);
        let thumb = ThumbGeometry::from_lengths(
            scrollable + f64::from(rows),
            f64::from(rows),
            scrollable - self.scroll.requested_rows(),
            f64::from(f32::from(track.size.height)),
            MINIMUM_THUMB_LENGTH,
        )?;
        Some(Geometry {
            track,
            thumb,
            scrollable,
        })
    }

    /// The thumb, while the scrollbar shows.
    pub(super) fn scrollbar_thumb(&self, now: Instant) -> Option<Bounds<Pixels>> {
        self.scrollbar
            .shown(now)
            .then(|| self.scrollbar_geometry())
            .flatten()
            .map(|geometry| geometry.thumb_bounds())
    }

    pub(super) fn scrollbar_dragging(&self) -> bool {
        self.scrollbar.grab.is_some()
    }

    /// Shows the scrollbar for a moment, as scrolling and pointing at it do.
    pub(super) fn reveal_scrollbar(&mut self, cx: &mut Context<Self>) {
        if self.scrollbar_geometry().is_none() {
            return;
        }
        let executor = cx.background_executor().clone();
        let now = executor.now();
        let shown = self.scrollbar.shown(now);
        self.scrollbar
            .reveal
            .reveal(now.duration_since(self.scrollbar.epoch));
        self.scrollbar.hide = Some(cx.spawn(async move |pane, cx| {
            executor.timer(REVEAL_DURATION).await;
            let _ = pane.update(cx, |pane, cx| {
                pane.scrollbar.hide = None;
                cx.notify();
            });
        }));
        if !shown {
            cx.notify();
        }
    }

    pub(super) fn hover_scrollbar(
        &mut self,
        position: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self
            .scrollbar
            .track
            .is_some_and(|track| near_thumb(track, position))
        {
            self.reveal_scrollbar(cx);
        }
    }

    /// Starts dragging the thumb when the press lands on the shown scrollbar.
    pub(super) fn press_scrollbar(
        &mut self,
        position: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.scrollbar.shown(cx.background_executor().now()) {
            return false;
        }
        let Some(geometry) = self.scrollbar_geometry() else {
            return false;
        };
        if !near_thumb(geometry.track, position) {
            return false;
        }
        let thumb = geometry.thumb_bounds();
        let grab = if thumb.top() <= position.y && position.y <= thumb.bottom() {
            position.y - thumb.top()
        } else {
            thumb.size.height / 2.0
        };
        self.scrollbar.grab = Some(f32::from(grab));
        self.scrollbar.reveal.begin_drag();
        self.prepare_saved_history(cx);
        self.drag_scrollbar(position, cx);
        true
    }

    pub(super) fn drag_scrollbar(&mut self, position: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(grab), Some(geometry)) = (self.scrollbar.grab, self.scrollbar_geometry()) else {
            return;
        };
        let origin = f32::from(position.y - geometry.track.top()) - grab;
        self.scroll_to(geometry.rows_from_bottom(origin), cx);
    }

    /// Ends a drag of the thumb, if one was under way.
    pub(super) fn release_scrollbar(&mut self, cx: &mut Context<Self>) -> bool {
        if self.scrollbar.grab.take().is_none() {
            return false;
        }
        let now = cx.background_executor().now();
        self.scrollbar
            .reveal
            .end_drag(now.duration_since(self.scrollbar.epoch));
        self.reveal_scrollbar(cx);
        cx.notify();
        true
    }
}

/// Whether `position` is over the strip of `track` the thumb moves in.
fn near_thumb(track: Bounds<Pixels>, position: gpui::Point<Pixels>) -> bool {
    track.contains(&position) && position.x >= track.right() - px(WIDTH * 2.0)
}
