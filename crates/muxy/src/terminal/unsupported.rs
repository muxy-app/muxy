use crate::terminal::surfaces::{AppSurfaceHandle, PaneLaunchContext, StandaloneLaunchContext};
use gpui::{App, Window};
use muxy_core::shortcuts::KeyCombo;
use muxy_core::workspace::TabId;
use muxy_terminal::backend::LaunchCommand;
use muxy_terminal::confirmation::{ConfirmationId, ConfirmationKind};
use std::path::PathBuf;

#[derive(Default)]
pub struct UnsupportedBackend;

impl UnsupportedBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn attach(
        &mut self,
        _combos: Vec<KeyCombo>,
        _mode: muxy_core::environment::BuildMode,
        _socket_path: &std::path::Path,
        _backdrop: gpui::Rgba,
        _window: &mut Window,
    ) -> Result<(), String> {
        Err("terminal surfaces are only available on macOS".to_owned())
    }

    pub fn attach_standalone(
        &mut self,
        _mode: muxy_core::environment::BuildMode,
        _socket_path: &std::path::Path,
        _window: &mut Window,
    ) -> Result<(), String> {
        Err("standalone terminal surfaces are only available on macOS".to_owned())
    }

    pub fn spawn(
        &mut self,
        _tab_id: &TabId,
        _directory: PathBuf,
        _command: Option<LaunchCommand>,
        _context: &PaneLaunchContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<Box<dyn AppSurfaceHandle>> {
        None
    }

    pub fn spawn_standalone(
        &mut self,
        _context: &StandaloneLaunchContext,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Result<Box<dyn AppSurfaceHandle>, String> {
        Err("standalone terminal surfaces are only available on macOS".to_owned())
    }

    pub fn set_shortcut_combos(&mut self, _combos: Vec<KeyCombo>) {}

    pub fn set_backdrop(&self, _backdrop: gpui::Rgba) {}

    pub fn tick(&self) {}

    pub fn reload_config(&mut self) {}

    pub fn active_confirmation(&self) -> Option<(TabId, ConfirmationId, ConfirmationKind)> {
        None
    }
}

impl UnsupportedBackend {
    pub fn set_window_active(&self, _active: bool) {}

    pub fn set_overlay_active(&self, _active: bool) {}
}
