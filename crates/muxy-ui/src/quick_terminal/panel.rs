use gpui::WindowBackgroundAppearance;
use muxy_core::quick_terminal::geometry::{
    Rect, Size, collapsed_rect, cutout_rect, panel_frame, preferred_screen_index,
};
use muxy_core::quick_terminal::presentation::{
    PresentationState, PresentationTransition, should_capture_focus, should_restore_focus,
};
use std::time::Duration;

pub const SHOW_DURATION: Duration = Duration::from_millis(340);
pub const HIDE_DURATION: Duration = Duration::from_millis(180);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AccessibilityPreferences {
    pub reduce_motion: bool,
    pub reduce_transparency: bool,
    pub increase_contrast: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuickTerminalConfiguration {
    pub enabled: bool,
    pub width: i64,
    pub height: i64,
    pub transparency: i64,
    pub blur: i64,
}

impl Default for QuickTerminalConfiguration {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 720,
            height: 430,
            transparency: 18,
            blur: 70,
        }
    }
}

impl QuickTerminalConfiguration {
    #[must_use]
    pub fn normalized(self) -> Self {
        Self {
            enabled: self.enabled,
            width: self.width.clamp(480, 1200),
            height: self.height.clamp(280, 800),
            transparency: self.transparency.clamp(0, 55),
            blur: self.blur.clamp(0, 100),
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        reason = "Dimensions are clamped to at most 1200 pixels"
    )]
    pub fn preferred_size(self) -> Size {
        let configuration = self.normalized();
        Size {
            width: configuration.width as f64,
            height: configuration.height as f64,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectiveAppearance {
    pub background: WindowBackgroundAppearance,
    pub tint_alpha_percent: u8,
    pub effective_transparency: i64,
    pub effective_blur: i64,
}

pub fn effective_appearance(
    configuration: QuickTerminalConfiguration,
    accessibility: AccessibilityPreferences,
) -> EffectiveAppearance {
    let configuration = configuration.normalized();
    let forced_opaque = accessibility.reduce_transparency || accessibility.increase_contrast;
    let effective_transparency = if forced_opaque {
        0
    } else {
        configuration.transparency
    };
    let effective_blur = if forced_opaque { 0 } else { configuration.blur };
    let background = if effective_transparency == 0 {
        WindowBackgroundAppearance::Opaque
    } else if effective_blur == 0 {
        WindowBackgroundAppearance::Transparent
    } else {
        WindowBackgroundAppearance::Blurred
    };
    EffectiveAppearance {
        background,
        tint_alpha_percent: u8::try_from(100 - effective_transparency).unwrap_or(100),
        effective_transparency,
        effective_blur,
    }
}

pub fn transition_duration(shows_panel: bool, reduce_motion: bool) -> Duration {
    if reduce_motion {
        Duration::ZERO
    } else if shows_panel {
        SHOW_DURATION
    } else {
        HIDE_DURATION
    }
}

#[derive(Debug, Default)]
pub struct PanelPresentation {
    state: PresentationState,
}

impl PanelPresentation {
    pub fn request(&mut self, visible: bool) -> Option<PresentationTransition> {
        self.state.request_visibility(visible)
    }

    pub fn complete(&mut self, transition: PresentationTransition) -> bool {
        self.state.complete(transition)
    }

    pub fn target_is_visible(&self) -> bool {
        self.state.target_is_visible
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PanelGeometryRequest<'a> {
    pub mouse: muxy_core::quick_terminal::geometry::Point,
    pub screens: &'a [Rect],
    pub visible_frames: &'a [Rect],
    pub key_window: Option<usize>,
    pub main_window: Option<usize>,
    pub main_screen: Option<usize>,
    pub preferred_size: Size,
    pub safe_area_top: f64,
    pub auxiliary_widths: Option<(f64, f64)>,
}

pub fn resolve_panel_geometry(
    request: PanelGeometryRequest<'_>,
) -> Option<(usize, Rect, Option<Rect>)> {
    let index = preferred_screen_index(
        request.mouse,
        request.screens,
        request.key_window,
        request.main_window,
        request.main_screen,
    )?;
    let screen = *request.screens.get(index)?;
    let visible = *request.visible_frames.get(index)?;
    let frame = panel_frame(screen, visible, request.preferred_size);
    let panel_top = frame.origin.y + frame.size.height;
    let screen_top = screen.origin.y + screen.size.height;
    let collapsed = ((panel_top - screen_top).abs() < f64::EPSILON)
        .then_some(request.auxiliary_widths)
        .flatten()
        .and_then(|(left, right)| {
            cutout_rect(screen, request.safe_area_top, Some(left), Some(right))
        })
        .map(|cutout| collapsed_rect(cutout, frame));
    Some((index, frame, collapsed))
}

pub fn capture_focus(has_snapshot: bool, panel_is_key: bool) -> bool {
    should_capture_focus(has_snapshot, panel_is_key)
}

pub fn restore_focus(requested: bool, panel_is_key: bool) -> bool {
    should_restore_focus(requested, panel_is_key)
}
