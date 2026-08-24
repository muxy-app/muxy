use super::Rect;

pub const MIN_TAB_WIDTH: f32 = 44.0;
pub const DEFAULT_MAX_TAB_WIDTH: f32 = 200.0;
pub const NEW_TAB_BUTTON_WIDTH: f32 = 28.0;
pub const TAB_TITLE_THRESHOLD: f32 = 80.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabStripMetrics {
    pub max_tab_width: f32,
}

impl Default for TabStripMetrics {
    fn default() -> Self {
        Self {
            max_tab_width: DEFAULT_MAX_TAB_WIDTH,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabStripLayout {
    pub bounds: Rect,
    pub frames: Vec<Rect>,
    pub new_button_frame: Rect,
    pub ideal_tab_width: f32,
    pub tab_width: f32,
    pub content_width: f32,
    pub viewport_width: f32,
    pub scrolls: bool,
    pub pins_new_tab_button: bool,
    pub shows_titles: bool,
}

impl TabStripLayout {
    pub fn calculate(
        bounds: Rect,
        tab_count: usize,
        _pinned_count: usize,
        metrics: TabStripMetrics,
    ) -> Self {
        let width = bounds.width.max(0.0);
        let button_width = NEW_TAB_BUTTON_WIDTH.min(width);
        let viewport_width = (width - button_width).max(0.0);
        let configured_max = finite_non_negative(metrics.max_tab_width);
        let ideal_tab_width = if tab_count == 0 {
            DEFAULT_MAX_TAB_WIDTH
        } else {
            viewport_width / tab_count as f32
        };
        let capped_width = if configured_max > 0.0 {
            configured_max.min(ideal_tab_width)
        } else {
            ideal_tab_width
        };
        let tab_width = MIN_TAB_WIDTH.max(capped_width);
        let content_width = tab_width * tab_count as f32;
        let scrolls = content_width > viewport_width;
        let pins_new_tab_button = ideal_tab_width < MIN_TAB_WIDTH;
        let shows_titles = tab_width >= TAB_TITLE_THRESHOLD;
        let frames = (0..tab_count)
            .map(|index| {
                Rect::new(
                    bounds.x + tab_width * index as f32,
                    bounds.y,
                    tab_width,
                    bounds.height,
                )
            })
            .collect();
        let new_button_frame = Rect::new(
            bounds.max_x() - button_width,
            bounds.y,
            button_width,
            bounds.height,
        );
        Self {
            bounds,
            frames,
            new_button_frame,
            ideal_tab_width,
            tab_width,
            content_width,
            viewport_width,
            scrolls,
            pins_new_tab_button,
            shows_titles,
        }
    }

    pub fn frame(&self, index: usize) -> Option<Rect> {
        self.frames.get(index).copied()
    }

    pub fn tab_index_at(&self, x: f32) -> Option<usize> {
        self.frames
            .iter()
            .position(|frame| x >= frame.x && x <= frame.max_x())
    }

    pub fn insertion_index(&self, x: f32) -> usize {
        self.frames
            .iter()
            .position(|frame| x < frame.mid_x())
            .unwrap_or(self.frames.len())
    }

    pub fn visible_range(&self, clip: Rect) -> std::ops::Range<usize> {
        let start = self
            .frames
            .iter()
            .position(|frame| frame.max_x() >= clip.x)
            .unwrap_or(self.frames.len());
        let end = self
            .frames
            .iter()
            .rposition(|frame| frame.x <= clip.max_x())
            .map(|index| index + 1)
            .unwrap_or(start);
        start..end.max(start)
    }
}

fn finite_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_swift_width_formula_and_reserves_new_button() {
        let layout = TabStripLayout::calculate(
            Rect::new(0.0, 0.0, 428.0, 32.0),
            2,
            0,
            TabStripMetrics::default(),
        );
        assert_eq!(layout.viewport_width, 400.0);
        assert_eq!(layout.ideal_tab_width, 200.0);
        assert_eq!(layout.tab_width, 200.0);
        assert_eq!(layout.new_button_frame.width, 28.0);
        assert!(layout.shows_titles);
        assert!(!layout.scrolls);
    }

    #[test]
    fn scrolls_without_compressing_and_pins_new_button_below_minimum() {
        let layout = TabStripLayout::calculate(
            Rect::new(0.0, 0.0, 200.0, 32.0),
            5,
            2,
            TabStripMetrics::default(),
        );
        assert_eq!(layout.ideal_tab_width, 34.4);
        assert_eq!(layout.tab_width, 44.0);
        assert_eq!(layout.content_width, 220.0);
        assert!(layout.scrolls);
        assert!(layout.pins_new_tab_button);
        assert!(!layout.shows_titles);

        let no_pins = TabStripLayout::calculate(
            Rect::new(0.0, 0.0, 200.0, 32.0),
            5,
            0,
            TabStripMetrics::default(),
        );
        assert!(no_pins.pins_new_tab_button);
    }

    #[test]
    fn respects_configured_max_and_eighty_point_title_threshold() {
        let metrics = TabStripMetrics {
            max_tab_width: 79.0,
        };
        let layout = TabStripLayout::calculate(Rect::new(0.0, 0.0, 500.0, 32.0), 2, 0, metrics);
        assert_eq!(layout.tab_width, 79.0);
        assert!(!layout.shows_titles);
    }
}
