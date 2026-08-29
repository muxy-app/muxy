use crate::quick_terminal::ShortcutRecordingEvent;
use crate::quick_terminal::shortcut_service::{
    MonitoringState, ShortcutBackend, ShortcutBackendFactory,
};
use muxy_core::quick_terminal::QuickTerminalShortcut;
use std::rc::Rc;

pub struct SystemObservers;

pub struct ShortcutRecorder;

impl ShortcutRecorder {
    pub fn start(_sender: async_channel::Sender<ShortcutRecordingEvent>) -> Result<Self, String> {
        Err("Quick Terminal shortcut recording is unavailable on this platform".to_owned())
    }
}

pub struct UnsupportedShortcutBackendFactory;

impl ShortcutBackendFactory for UnsupportedShortcutBackendFactory {
    fn create(&mut self, shortcut: &QuickTerminalShortcut) -> Option<Box<dyn ShortcutBackend>> {
        (!matches!(shortcut, QuickTerminalShortcut::Unassigned))
            .then(|| Box::new(UnsupportedShortcutBackend) as Box<dyn ShortcutBackend>)
    }

    fn request_input_monitoring_access(&mut self) -> bool {
        false
    }
}

struct UnsupportedShortcutBackend;

impl ShortcutBackend for UnsupportedShortcutBackend {
    fn start(&mut self, _trigger: Rc<dyn Fn()>) -> Result<(), String> {
        Ok(())
    }

    fn stop(&mut self) {}

    fn monitoring_state(&self) -> MonitoringState {
        MonitoringState::Unavailable
    }
}
