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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{ColorScheme, Theme};

    #[test]
    fn elevated_shadows_use_two_compact_layers_for_both_appearances() {
        for (background, edge_opacity) in [(0x19_17_1f, 0.06), (0xf8_f8_f8, 0.03)] {
            let theme = Theme::from_scheme(&ColorScheme {
                background: Some(gpui::rgb(background)),
                ..ColorScheme::default()
            });
            assert_eq!(
                Elevation::Elevated.shadow(theme.bg),
                vec![layer(2.0, 3.0, 0.12), layer(1.0, 0.0, edge_opacity)]
            );
        }
    }

    #[test]
    fn modal_shadows_use_four_layers_with_lighter_light_appearance_opacity() {
        for (background, opacities) in [
            (gpui::black(), [0.12, 0.08, 0.04, 0.12]),
            (gpui::white(), [0.06, 0.06, 0.04, 0.04]),
        ] {
            assert_eq!(
                Elevation::Modal.shadow(background),
                vec![
                    layer(2.0, 3.0, opacities[0]),
                    layer(3.0, 6.0, opacities[1]),
                    layer(6.0, 12.0, opacities[2]),
                    layer(1.0, 0.0, opacities[3]),
                ]
            );
        }
    }

    #[test]
    fn shadow_layers_have_no_spread_or_horizontal_offset() {
        let shadow = layer(2.0, 3.0, 0.12);
        assert_eq!(shadow.offset, point(px(0.0), px(2.0)));
        assert_eq!(shadow.blur_radius, px(3.0));
        assert_eq!(shadow.spread_radius, px(0.0));
        assert_eq!(shadow.color, hsla(0.0, 0.0, 0.0, 0.12));
    }
}
