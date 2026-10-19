use crate::quick_terminal::ShortcutRecordingEvent;
use crate::quick_terminal::shortcut_service::{
    ShortcutBackend, ShortcutBackendFactory, ShortcutState,
};
use muxy_core::quick_terminal::QuickTerminalShortcut;
use std::rc::Rc;

pub const CAPABILITY: muxy_core::capability::Capability =
    muxy_core::capability::Capability::Unsupported;

#[derive(Debug)]
pub struct SystemObservers;

#[derive(Debug)]
pub struct ShortcutRecorder;

impl ShortcutRecorder {
    pub fn start(_sender: async_channel::Sender<ShortcutRecordingEvent>) -> Result<Self, String> {
        Err(muxy_core::capability::Feature::QuickTerminal
            .unsupported_reason()
            .into_owned())
    }
}

pub(super) struct UnsupportedShortcutBackendFactory;

impl ShortcutBackendFactory for UnsupportedShortcutBackendFactory {
    fn create(&mut self, shortcut: &QuickTerminalShortcut) -> Option<Box<dyn ShortcutBackend>> {
        (!matches!(shortcut, QuickTerminalShortcut::Unassigned))
            .then(|| Box::new(UnsupportedShortcutBackend) as Box<dyn ShortcutBackend>)
    }
}

struct UnsupportedShortcutBackend;

impl ShortcutBackend for UnsupportedShortcutBackend {
    fn start(&mut self, _trigger: Rc<dyn Fn()>) -> Result<(), String> {
        Ok(())
    }

    fn stop(&mut self) {}

    fn state(&self) -> ShortcutState {
        ShortcutState::Unavailable
    }
}

#[derive(Debug)]
pub(super) struct PanelAdapter;

#[allow(clippy::unused_self, reason = "Shared panel backend interface")]
impl PanelAdapter {
    pub(super) fn configure(_window: &gpui::Window) -> Result<Self, String> {
        Err(muxy_core::capability::Feature::QuickTerminal
            .unsupported_reason()
            .into_owned())
    }
    pub(super) fn prepare(
        &mut self,
        _configuration: crate::quick_terminal::panel::QuickTerminalConfiguration,
        _window: &mut gpui::Window,
    ) -> Result<(), String> {
        Err(muxy_core::capability::Feature::QuickTerminal
            .unsupported_reason()
            .into_owned())
    }
    pub(super) fn sync_bounds(&self, _visible: bool) {}
    pub(super) fn is_active(&self, active: bool) -> bool {
        active
    }
    pub(super) fn show(&mut self, _duration: std::time::Duration) {}
    pub(super) fn begin_hide(&mut self, _duration: std::time::Duration) {}
    pub(super) fn finish_hide(&mut self, _restores_focus: bool) {}
    pub(super) fn install_accessibility(
        &mut self,
        _nodes: &[crate::quick_terminal::view::AccessibilityNode],
    ) {
    }
}
