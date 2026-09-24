use gpui::{Hsla, Window};
use objc2::{MainThreadMarker, MainThreadOnly, rc::Retained};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectView, NSWindowOrderingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::theme::Appearance;

/// A native background effect confined to the window's left sidebar.
#[derive(Debug)]
pub struct SidebarVibrancy {
    view: Retained<NSVisualEffectView>,
    width: f32,
    appearance: Appearance,
}

impl SidebarVibrancy {
    pub fn new(window: &Window, width: f32, background: Hsla) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let handle = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return None;
        };
        // SAFETY: GPUI lends its live AppKit view on the main thread.
        let gpui_view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        // SAFETY: This is GPUI's live main-thread view. Install the effect as a
        // sibling below it so the Metal surface and input handling stay on top.
        let content = unsafe { gpui_view.superview() }?;
        let frame = NSRect::new(
            NSPoint::ZERO,
            NSSize::new(f64::from(width), content.bounds().size.height),
        );
        let view = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), frame);
        let appearance = appearance(background);
        view.setAppearance(native_appearance(appearance).as_deref());
        view.setMaterial(NSVisualEffectMaterial::Sidebar);
        view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewHeightSizable);
        content.addSubview_positioned_relativeTo(
            &view,
            NSWindowOrderingMode::Below,
            Some(gpui_view),
        );
        Some(Self {
            view,
            width,
            appearance,
        })
    }

    pub fn set_width(&mut self, width: f32) {
        if self.width.to_bits() != width.to_bits() {
            self.width = width;
            self.view
                .setFrameSize(NSSize::new(f64::from(width), self.view.frame().size.height));
        }
    }

    pub fn set_appearance(&mut self, background: Hsla) {
        let appearance = appearance(background);
        if self.appearance != appearance {
            self.appearance = appearance;
            self.view
                .setAppearance(native_appearance(appearance).as_deref());
        }
    }
}

impl Drop for SidebarVibrancy {
    fn drop(&mut self) {
        self.view.removeFromSuperview();
    }
}

fn appearance(background: Hsla) -> Appearance {
    if background.l < 0.5 {
        Appearance::Dark
    } else {
        Appearance::Light
    }
}

fn native_appearance(appearance: Appearance) -> Option<Retained<NSAppearance>> {
    NSAppearance::appearanceNamed(unsafe {
        match appearance {
            Appearance::Light => NSAppearanceNameAqua,
            Appearance::Dark => NSAppearanceNameDarkAqua,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{ColorScheme, Theme};

    #[test]
    fn vibrancy_appearance_follows_the_theme_background() {
        for (source, expected) in [
            ("background = 19171f\nforeground = c9c2d9", Appearance::Dark),
            (
                "background = f0f0f5\nforeground = 1e1e2e",
                Appearance::Light,
            ),
            ("background = 123456\nforeground = abcdef", Appearance::Dark),
        ] {
            let theme = Theme::from_scheme(&ColorScheme::parse(source));
            assert_eq!(appearance(theme.bg), expected);
            let name = native_appearance(appearance(theme.bg)).map(|native| native.name());
            let expected_name = unsafe {
                match expected {
                    Appearance::Light => NSAppearanceNameAqua,
                    Appearance::Dark => NSAppearanceNameDarkAqua,
                }
            };
            assert_eq!(name.as_deref(), Some(expected_name));
        }
    }
}
