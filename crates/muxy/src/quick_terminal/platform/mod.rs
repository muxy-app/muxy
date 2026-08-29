#[cfg(target_os = "macos")]
pub(crate) mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;

use super::ShortcutRecordingEvent;
use super::shortcut_service::ShortcutBackendFactory;
use async_channel::Sender;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemMutation {
    Accessibility,
    KeyboardLayout,
    Screens,
}

#[cfg(target_os = "macos")]
pub type SystemObservers = macos::SystemObservers;
#[cfg(not(target_os = "macos"))]
pub type SystemObservers = unsupported::SystemObservers;

#[cfg(target_os = "macos")]
pub type ShortcutRecorder = macos::ShortcutRecorder;
#[cfg(not(target_os = "macos"))]
pub type ShortcutRecorder = unsupported::ShortcutRecorder;

#[cfg(target_os = "macos")]
pub fn factory() -> Box<dyn ShortcutBackendFactory> {
    Box::new(macos::MacShortcutBackendFactory::new())
}

#[cfg(not(target_os = "macos"))]
pub fn factory() -> Box<dyn ShortcutBackendFactory> {
    Box::new(unsupported::UnsupportedShortcutBackendFactory)
}

#[cfg(target_os = "macos")]
pub fn resolve_key(virtual_key_code: u16) -> Option<String> {
    macos::resolve_key(virtual_key_code)
}

#[cfg(not(target_os = "macos"))]
pub fn resolve_key(virtual_key_code: u16) -> Option<String> {
    muxy_core::shortcuts::legacy_key_for_virtual_key_code(virtual_key_code).map(str::to_owned)
}

#[cfg(target_os = "macos")]
pub fn start_shortcut_recorder(
    sender: Sender<ShortcutRecordingEvent>,
) -> Result<ShortcutRecorder, String> {
    macos::ShortcutRecorder::start(sender)
}

#[cfg(not(target_os = "macos"))]
pub fn start_shortcut_recorder(
    sender: Sender<ShortcutRecordingEvent>,
) -> Result<ShortcutRecorder, String> {
    unsupported::ShortcutRecorder::start(sender)
}

#[cfg(target_os = "macos")]
pub fn observe_system_mutations(sender: Sender<SystemMutation>) -> Result<SystemObservers, String> {
    macos::SystemObservers::start(sender)
}

#[cfg(not(target_os = "macos"))]
pub fn observe_system_mutations(
    _sender: Sender<SystemMutation>,
) -> Result<SystemObservers, String> {
    Ok(unsupported::SystemObservers)
}
