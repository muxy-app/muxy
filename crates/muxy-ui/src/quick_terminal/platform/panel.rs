use crate::quick_terminal::panel::QuickTerminalConfiguration;
use gpui::Window;
use std::time::Duration;

#[cfg(target_os = "linux")]
use super::linux as backend;
#[cfg(target_os = "macos")]
use super::macos as backend;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use super::unsupported as backend;

#[derive(Debug)]
pub struct PanelAdapter(backend::PanelAdapter);

impl PanelAdapter {
    pub fn configure(window: &Window) -> Result<Self, String> {
        backend::PanelAdapter::configure(window).map(Self)
    }

    pub fn prepare(
        &mut self,
        configuration: QuickTerminalConfiguration,
        window: &mut Window,
    ) -> Result<(), String> {
        self.0.prepare(configuration, window)?;
        Ok(())
    }

    pub fn sync_bounds(&self, visible: bool) {
        self.0.sync_bounds(visible);
    }

    pub fn is_active(&self, window: &Window) -> bool {
        self.0.is_active(window.is_window_active())
    }

    pub fn show(&mut self, duration: Duration) {
        self.0.show(duration);
    }

    pub fn begin_hide(&mut self, duration: Duration) {
        self.0.begin_hide(duration);
    }

    pub fn finish_hide(&mut self, restores_focus: bool) {
        self.0.finish_hide(restores_focus);
    }

    pub fn set_pinned(&self, pinned: bool) {
        self.0.set_pinned(pinned);
    }

    /// A pinned panel outlives the focus it was opened from, so hiding it must not return there.
    pub fn forget_previous_focus(&mut self) {
        self.0.forget_previous_focus();
    }

    pub fn install_accessibility(&mut self, status: &str, shortcut: &str, pinned: bool) {
        self.0
            .install_accessibility(&crate::quick_terminal::view::bridge_accessibility_model(
                status, shortcut, pinned,
            ));
    }
}
