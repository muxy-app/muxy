use gpui::{BoxShadow, Hsla, hsla, point, px};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    Elevated,
    Modal,
}

impl Elevation {
    pub fn shadow(self, background: Hsla) -> Vec<BoxShadow> {
        let light = background.l >= 0.5;
        match self {
            Self::Elevated => vec![
                layer(2.0, 3.0, 0.12),
                layer(1.0, 0.0, if light { 0.03 } else { 0.06 }),
            ],
            Self::Modal => vec![
                layer(2.0, 3.0, if light { 0.06 } else { 0.12 }),
                layer(3.0, 6.0, if light { 0.06 } else { 0.08 }),
                layer(6.0, 12.0, 0.04),
                layer(1.0, 0.0, if light { 0.04 } else { 0.12 }),
            ],
        }
    }
}

fn layer(offset: f32, blur: f32, opacity: f32) -> BoxShadow {
    BoxShadow {
        color: hsla(0.0, 0.0, 0.0, opacity),
        offset: point(px(0.0), px(offset)),
        blur_radius: px(blur),
        spread_radius: px(0.0),
    }
}
