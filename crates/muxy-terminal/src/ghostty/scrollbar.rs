use crate::scrollbar::ScrollbarMetrics;
use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{NSEvent, NSScrollView, NSScrollerStyle, NSView};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
use std::cell::Cell;
use std::time::{Duration, Instant};

const REVEAL_DURATION: Duration = Duration::from_millis(1_250);

pub struct NativeScrollbar {
    scroll_view: Retained<NSScrollView>,
    document_view: Retained<NSView>,
    metrics: Cell<ScrollbarMetrics>,
    cell_height: Cell<f64>,
    dragging: Cell<bool>,
    last_sent_row: Cell<Option<u64>>,
    revealed_until: Cell<Option<Instant>>,
}

impl NativeScrollbar {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
        let scroll_view = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), frame);
        let document_view = NSView::initWithFrame(NSView::alloc(mtm), frame);
        scroll_view.setHasVerticalScroller(true);
        scroll_view.setHasHorizontalScroller(false);
        scroll_view.setAutohidesScrollers(true);
        scroll_view.setScrollerStyle(NSScrollerStyle::Overlay);
        scroll_view.setDrawsBackground(false);
        scroll_view.contentView().setDrawsBackground(false);
        scroll_view.contentView().setClipsToBounds(false);
        scroll_view.setDocumentView(Some(&document_view));
        Self {
            scroll_view,
            document_view,
            metrics: Cell::new(ScrollbarMetrics::default()),
            cell_height: Cell::new(0.0),
            dragging: Cell::new(false),
            last_sent_row: Cell::new(None),
            revealed_until: Cell::new(None),
        }
    }

    pub fn view(&self) -> &NSView {
        &self.scroll_view
    }

    pub fn content_view(&self) -> Retained<objc2_app_kit::NSClipView> {
        self.scroll_view.contentView()
    }

    pub fn layout(&self, bounds: NSRect) {
        self.scroll_view.setFrame(bounds);
        self.synchronize();
    }

    pub fn update(&self, metrics: ScrollbarMetrics, cell_height: f64) {
        self.metrics.set(metrics);
        self.cell_height.set(cell_height);
        self.synchronize();
    }

    pub fn update_cell_height(&self, cell_height: f64) {
        self.cell_height.set(cell_height);
        self.synchronize();
    }

    pub fn flash(&self) {
        if !self.has_scrollable_content() {
            return;
        }
        self.reveal();
        self.scroll_view.flashScrollers();
    }

    pub fn extend_reveal(&self, pointer_x: f64, width: f64) {
        if !self.has_scrollable_content() || !self.allows_hit() {
            return;
        }
        let Some(scroller) = self.scroll_view.verticalScroller() else {
            return;
        };
        let scroller_width = scroller.frame().size.width.max(0.0);
        if !pointer_x.is_finite() || !width.is_finite() || pointer_x < width - scroller_width * 2.0
        {
            return;
        }
        self.reveal();
        self.scroll_view.flashScrollers();
    }

    pub fn handle_mouse_down(&self, event: &NSEvent) -> bool {
        if !self.has_scrollable_content() || !self.allows_hit() {
            return false;
        }
        let Some(scroller) = self.scroll_view.verticalScroller() else {
            return false;
        };
        if scroller.isHidden() {
            return false;
        }
        let point = scroller.convertPoint_fromView(event.locationInWindow(), None);
        if !contains(scroller.bounds(), point) {
            return false;
        }
        self.dragging.set(true);
        self.last_sent_row.set(Some(self.metrics.get().offset));
        scroller.mouseDown(event);
        self.dragging.set(false);
        self.reveal();
        true
    }

    pub fn live_row(&self) -> Option<u64> {
        if !self.dragging.get() || !self.has_scrollable_content() {
            return None;
        }
        let visible = self.scroll_view.contentView().documentVisibleRect();
        let height = self.document_view.frame().size.height;
        let scroll_offset = height - visible.origin.y - visible.size.height;
        let row = row_for_scroll_offset(
            scroll_offset,
            self.cell_height.get(),
            self.metrics.get().maximum_offset(),
        )?;
        if self.last_sent_row.replace(Some(row)) == Some(row) {
            None
        } else {
            Some(row)
        }
    }

    fn synchronize(&self) {
        let content_size = self.scroll_view.contentSize();
        let metrics = self.metrics.get();
        let cell_height = self.cell_height.get();
        let height = if self.has_scrollable_content() {
            document_height(metrics, cell_height, content_size.height)
        } else {
            content_size.height
        };
        self.document_view
            .setFrameSize(NSSize::new(content_size.width, height));
        let clip = self.scroll_view.contentView();
        if !self.has_scrollable_content() {
            self.scroll_view.reflectScrolledClipView(&clip);
            return;
        }
        if self.dragging.get() {
            self.scroll_view.reflectScrolledClipView(&clip);
            return;
        }
        let rows_from_newest = metrics
            .total
            .saturating_sub(metrics.offset)
            .saturating_sub(metrics.visible);
        let offset_y = rows_from_newest as f64 * cell_height;
        clip.scrollToPoint(NSPoint::new(0.0, offset_y));
        self.last_sent_row.set(Some(metrics.offset));
        self.scroll_view.reflectScrolledClipView(&clip);
    }

    fn has_scrollable_content(&self) -> bool {
        let metrics = self.metrics.get();
        self.cell_height.get().is_finite()
            && self.cell_height.get() > 0.0
            && metrics.is_scrollable()
    }

    fn reveal(&self) {
        self.revealed_until
            .set(Instant::now().checked_add(REVEAL_DURATION));
    }

    fn allows_hit(&self) -> bool {
        self.dragging.get()
            || self
                .revealed_until
                .get()
                .is_some_and(|until| Instant::now() < until)
    }
}

fn document_height(metrics: ScrollbarMetrics, cell_height: f64, content_height: f64) -> f64 {
    let grid_height = metrics.total as f64 * cell_height;
    let visible_height = metrics.visible as f64 * cell_height;
    let height = grid_height + content_height - visible_height;
    if height.is_finite() {
        height.max(content_height)
    } else {
        content_height
    }
}

fn row_for_scroll_offset(scroll_offset: f64, cell_height: f64, maximum: u64) -> Option<u64> {
    if !scroll_offset.is_finite() || !cell_height.is_finite() || cell_height <= 0.0 {
        return None;
    }
    let row = (scroll_offset / cell_height)
        .round()
        .clamp(0.0, maximum as f64);
    Some(row as u64)
}

fn contains(rect: NSRect, point: NSPoint) -> bool {
    point.x >= rect.origin.x
        && point.y >= rect.origin.y
        && point.x < rect.origin.x + rect.size.width
        && point.y < rect.origin.y + rect.size.height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_height_preserves_visible_padding() {
        let metrics = ScrollbarMetrics::new(100, 80, 20);
        assert_eq!(document_height(metrics, 10.0, 240.0), 1_040.0);
    }

    #[test]
    fn live_row_rounds_and_clamps() {
        assert_eq!(row_for_scroll_offset(44.0, 10.0, 80), Some(4));
        assert_eq!(row_for_scroll_offset(46.0, 10.0, 80), Some(5));
        assert_eq!(row_for_scroll_offset(1_000.0, 10.0, 80), Some(80));
        assert_eq!(row_for_scroll_offset(-20.0, 10.0, 80), Some(0));
    }

    #[test]
    fn live_row_rejects_invalid_geometry() {
        assert_eq!(row_for_scroll_offset(10.0, 0.0, 80), None);
        assert_eq!(row_for_scroll_offset(f64::NAN, 10.0, 80), None);
    }
}
