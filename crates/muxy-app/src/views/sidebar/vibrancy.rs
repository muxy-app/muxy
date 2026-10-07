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
