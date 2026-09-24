use gpui::Hsla;
#[cfg(all(target_os = "macos", not(test)))]
use gpui::Window;
use muxy_app_core::settings::Appearance;
use muxy_ui::theme::Theme;

use crate::model::AppModel;

impl AppModel {
    #[cfg(all(target_os = "macos", not(test)))]
    pub(crate) fn sync_sidebar_vibrancy(&mut self, window: &Window) {
        if enabled(&self.appearance) {
            let width = self.sidebar_width();
            if let Some(effect) = &mut self.sidebar_vibrancy {
                effect.set_width(width);
                effect.set_appearance(self.theme.bg);
            } else {
                self.sidebar_vibrancy =
                    muxy_ui::vibrancy::SidebarVibrancy::new(window, width, self.theme.bg);
            }
        } else {
            self.sidebar_vibrancy = None;
        }
    }

    pub(crate) fn sidebar_background(&self) -> Hsla {
        #[cfg(target_os = "macos")]
        let available = self.sidebar_vibrancy.is_some();
        #[cfg(not(target_os = "macos"))]
        let available = false;
        background(&self.appearance, &self.theme, available)
    }
}

fn enabled(appearance: &Appearance) -> bool {
    appearance.sidebar_expanded
        && appearance.sidebar_vibrancy
        && appearance.sidebar_vibrancy_level > 0
}

fn background(appearance: &Appearance, theme: &Theme, available: bool) -> Hsla {
    if !appearance.sidebar_expanded {
        return theme.bg;
    }
    if !available || !enabled(appearance) {
        return theme.raised();
    }
    let mut color = theme.bg;
    color.a = 1.0 - f32::from(appearance.sidebar_vibrancy_level.min(100)) / 100.0;
    color
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_app_core::settings::{AppLayout, SidebarCollapsedStyle};
    use muxy_ui::theme::ColorScheme;

    #[test]
    fn collapsed_modes_are_solid_in_both_layouts() {
        let theme = Theme::from_scheme(&ColorScheme::default());
        for layout in [AppLayout::ProjectFocused, AppLayout::TabFocused] {
            for style in [SidebarCollapsedStyle::Icons, SidebarCollapsedStyle::Hidden] {
                let appearance = Appearance {
                    layout,
                    sidebar_collapsed_style: style,
                    sidebar_vibrancy_level: 100,
                    ..Appearance::default()
                };
                assert!(!enabled(&appearance));
                assert_eq!(background(&appearance, &theme, true), theme.bg);
            }
        }
    }

    #[test]
    fn expanded_vibrancy_uses_the_theme_background_without_a_foreground_tint() {
        for source in [
            "background = 19171f\nforeground = c9c2d9",
            "background = f0f0f5\nforeground = 1e1e2e",
        ] {
            let theme = Theme::from_scheme(&ColorScheme::parse(source));
            for level in [0, 30, 50, 70, 100, 255] {
                let appearance = Appearance {
                    sidebar_expanded: true,
                    sidebar_vibrancy_level: level,
                    ..Appearance::default()
                };
                let mut expected = if level == 0 { theme.raised() } else { theme.bg };
                expected.a = 1.0 - f32::from(level.min(100)) / 100.0;
                assert_eq!(background(&appearance, &theme, true), expected);
                assert_eq!(background(&appearance, &theme, false), theme.raised());
                assert_eq!(
                    background(
                        &Appearance {
                            sidebar_vibrancy: false,
                            ..appearance
                        },
                        &theme,
                        true
                    ),
                    theme.raised()
                );
            }
        }
    }

    #[test]
    fn expanded_vibrancy_respects_level_toggle_and_native_availability() {
        let theme = Theme::from_scheme(&ColorScheme::default());
        let mut appearance = Appearance {
            sidebar_expanded: true,
            ..Appearance::default()
        };
        for level in [0, 30, 70, 100, 255] {
            appearance.sidebar_vibrancy_level = level;
            let color = background(&appearance, &theme, true);
            assert!((color.a - (1.0 - f32::from(level.min(100)) / 100.0)).abs() < f32::EPSILON);
            assert_eq!(background(&appearance, &theme, false), theme.raised());
            assert_eq!(enabled(&appearance), level > 0);
        }
        appearance.sidebar_vibrancy = false;
        assert!(!enabled(&appearance));
        assert_eq!(background(&appearance, &theme, true), theme.raised());
    }
}
