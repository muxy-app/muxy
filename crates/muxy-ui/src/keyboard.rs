use gpui::Modifiers;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};

pub fn option_sides() -> (bool, bool) {
    let Some(mtm) = MainThreadMarker::new() else {
        return (false, false);
    };
    let Some(event) = NSApplication::sharedApplication(mtm).currentEvent() else {
        return (false, false);
    };
    if !matches!(event.r#type(), NSEventType::KeyDown | NSEventType::KeyUp) {
        return (false, false);
    }
    let flags = event.modifierFlags().0;
    (flags & 0x20 != 0, flags & 0x40 != 0)
}

/// The modifiers held right now, even when no event reported them.
pub fn held_modifiers() -> Modifiers {
    let flags = NSEvent::modifierFlags_class();
    Modifiers {
        control: flags.contains(NSEventModifierFlags::Control),
        alt: flags.contains(NSEventModifierFlags::Option),
        shift: flags.contains(NSEventModifierFlags::Shift),
        platform: flags.contains(NSEventModifierFlags::Command),
        function: flags.contains(NSEventModifierFlags::Function),
    }
}
