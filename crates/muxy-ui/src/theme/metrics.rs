use gpui::{Pixels, Window, px};

macro_rules! metrics {
    ($($name:ident = $value:expr;)*) => {
        $(pub fn $name(&self) -> Pixels { self.scaled($value) })*
    };
}

const ROOT_TEXT_SIZE: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    multiplier: f32,
}

#[allow(dead_code)]
impl Metrics {
    pub const fn new(multiplier: f32) -> Self {
        Self { multiplier }
    }

    /// The metrics a window was last given by `apply_to_window`.
    pub fn of_window(window: &Window) -> Self {
        Self::new(f32::from(window.rem_size()) / ROOT_TEXT_SIZE)
    }

    /// Scales text and sizes that are not set explicitly with `window`'s views.
    pub fn apply_to_window(&self, window: &mut Window) {
        window.set_rem_size(self.scaled(ROOT_TEXT_SIZE));
    }

    pub const fn multiplier(&self) -> f32 {
        self.multiplier
    }

    #[must_use]
    pub const fn times(&self, factor: f32) -> Self {
        Self::new(self.multiplier * factor)
    }

    pub fn scaled(&self, value: f32) -> Pixels {
        px(value * self.multiplier)
    }

    metrics! {
        font_micro = 8.0;
        font_xs = 9.0;
        font_caption = 10.0;
        font_footnote = 11.0;
        font_body = 12.0;
        font_emphasis = 13.0;
        font_headline = 14.0;
        font_title = 15.0;
        font_title_large = 16.0;
        font_display = 20.0;
        font_hero = 24.0;
        font_mega = 28.0;

        line_height_field = 18.0;
        line_height_compact = 16.0;

        spacing1 = 2.0;
        spacing2 = 4.0;
        spacing3 = 6.0;
        spacing4 = 8.0;
        spacing5 = 10.0;
        spacing6 = 12.0;
        spacing7 = 16.0;
        spacing8 = 20.0;
        spacing9 = 24.0;
        spacing10 = 32.0;

        icon_xs = 10.0;
        icon_sm = 12.0;
        icon_md = 14.0;
        icon_lg = 16.0;
        icon_xl = 20.0;
        icon_xxl = 28.0;

        control_small = 20.0;
        control_medium = 24.0;
        control_large = 32.0;
        resize_handle_hit_area = 10.0;

        radius_sm = 4.0;
        radius_md = 6.0;
        radius_lg = 8.0;
        radius_xl = 10.0;

        sidebar_collapsed_width = 44.0;
        sidebar_expanded_width = 270.0;
        sidebar_expanded_min_width = 180.0;
        sidebar_expanded_max_width = 480.0;

        tab_bar_height = 28.0;
        header_height = 36.0;
        title_bar_height = 32.0;
        status_bar_height = 28.0;

        traffic_light_width = 75.0;
        navigation_arrows_width = 78.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_size_keeps_every_value_and_others_multiply_it() {
        assert_eq!(Metrics::new(1.0).scaled(13.0), px(13.0));
        assert_eq!(Metrics::new(1.0).title_bar_height(), px(32.0));
        assert_eq!(Metrics::new(2.0).title_bar_height(), px(64.0));
        assert_eq!(Metrics::new(2.0).times(1.5), Metrics::new(3.0));
        assert_eq!(Metrics::new(1.5).multiplier().to_bits(), 1.5_f32.to_bits());
    }

    #[gpui::test]
    fn a_window_reports_the_metrics_it_was_given(cx: &mut gpui::TestAppContext) {
        let window = cx.add_empty_window();
        window.update(|window, _| {
            Metrics::new(1.5).apply_to_window(window);
            assert_eq!(window.rem_size(), px(24.0));
            assert_eq!(Metrics::of_window(window), Metrics::new(1.5));
        });
    }
}
