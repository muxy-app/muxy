mod panel;
mod shortcut;
pub(super) use panel::PanelAdapter;
pub(super) use shortcut::LinuxShortcutBackendFactory;

use async_channel::Sender;
use gpui::{Keystroke, Window};
use muxy_core::capability::Capability;
use muxy_core::quick_terminal::keys::{CONTROL, KeyCombo, OPTION, SHIFT, legacy_virtual_key_code};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use super::super::{ShortcutCapture, ShortcutRecordingEvent};
use crate::tr;

pub const CAPABILITY: Capability = Capability::Supported;

pub(super) fn wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|value| value == "wayland")
}

pub(super) fn capability(window: &Window) -> Capability {
    let x11 = HasWindowHandle::window_handle(window).is_ok_and(|handle| {
        matches!(
            handle.as_raw(),
            RawWindowHandle::Xcb(_) | RawWindowHandle::Xlib(_)
        )
    });
    availability(x11, wayland_session())
}

fn availability(x11: bool, wayland: bool) -> Capability {
    if wayland {
        Capability::Unavailable(tr!("Quick Terminal requires an X11 desktop session. Wayland dropdown placement, hiding and activation are not supported by the window backend yet.").to_string())
    } else if !x11 {
        Capability::Unavailable(tr!("Quick Terminal requires an X11 display.").to_string())
    } else {
        Capability::Supported
    }
}

#[derive(Debug)]
pub struct SystemObservers;

#[derive(Debug)]
pub struct ShortcutRecorder(Sender<ShortcutRecordingEvent>);

impl ShortcutRecorder {
    #[allow(
        clippy::unnecessary_wraps,
        reason = "Shared recorder backend interface"
    )]
    pub fn start(sender: Sender<ShortcutRecordingEvent>) -> Result<Self, String> {
        Ok(Self(sender))
    }

    pub(super) fn record(&self, keystroke: &Keystroke) {
        let _ = self.0.try_send(recording_event(keystroke));
    }
}

fn recording_event(keystroke: &Keystroke) -> ShortcutRecordingEvent {
    if keystroke.key == "escape" {
        return ShortcutRecordingEvent::Cancelled;
    }
    if keystroke.modifiers.platform || keystroke.modifiers.function {
        return ShortcutRecordingEvent::Rejected(
            tr!("Use Control or Alt with a supported key.").to_string(),
        );
    }
    let modifiers = (u64::from(keystroke.modifiers.control) * CONTROL)
        | (u64::from(keystroke.modifiers.alt) * OPTION)
        | (u64::from(keystroke.modifiers.shift) * SHIFT);
    let combo = KeyCombo::new(&keystroke.key, modifiers).canonicalized();
    let Some(virtual_key_code) =
        legacy_virtual_key_code(&combo.key).filter(|_| combo.is_supported_shortcut())
    else {
        return ShortcutRecordingEvent::Rejected(
            tr!("Use Control or Alt with a supported key.").to_string(),
        );
    };
    ShortcutRecordingEvent::Captured(ShortcutCapture {
        combo,
        virtual_key_code,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn imported_shortcuts_keep_their_key_name_and_macos_key_code() {
        let saved = muxy_core::quick_terminal::QuickTerminalShortcut::KeyCombo {
            key_combo: KeyCombo::new("q", CONTROL | OPTION),
            virtual_key_code: 0,
        };
        assert_eq!(super::super::canonicalize(&saved), Some(saved));
    }

    #[test]
    fn xwayland_never_claims_a_system_wide_shortcut() {
        assert!(!availability(true, true).is_supported());
        assert!(!availability(false, false).is_supported());
        assert!(availability(true, false).is_supported());
    }

    #[test]
    fn recording_rejects_typing_and_super_without_reinterpreting_them() {
        for key in ["a", "shift-a", "cmd-a"] {
            assert!(matches!(
                recording_event(&Keystroke::parse(key).unwrap()),
                ShortcutRecordingEvent::Rejected(_)
            ));
        }
        let ShortcutRecordingEvent::Captured(capture) =
            recording_event(&Keystroke::parse("ctrl-alt-space").unwrap())
        else {
            panic!("capture");
        };
        assert_eq!(capture.virtual_key_code, 49);
        assert_eq!(capture.combo.modifiers, CONTROL | OPTION);
        assert_eq!(
            recording_event(&Keystroke::parse("escape").unwrap()),
            ShortcutRecordingEvent::Cancelled
        );
    }
}
