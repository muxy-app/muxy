use gpui::{Pixels, px};

macro_rules! metrics {
    ($($name:ident = $value:expr;)*) => {
        $(pub fn $name(&self) -> Pixels { self.scaled($value) })*
    };
}

#[derive(Debug, Clone, Copy)]
pub struct Metrics {
    multiplier: f32,
}

#[allow(dead_code)]
impl Metrics {
    pub fn new(multiplier: f32) -> Self {
        Self { multiplier }
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
        sidebar_expanded_width = 220.0;
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
