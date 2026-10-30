use gpui::Window;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityFocusedUIElementChangedNotification,
    NSAccessibilityPostNotification, NSAccessibilityTextAreaRole, NSView,
};
use objc2_foundation::NSString;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Presents the window's content as a text area named `label` while typing
/// goes to it, so dictation and automation tools find a field to insert
/// into. `None` hides the content from accessibility clients again.
pub fn set_text_input(window: &Window, label: Option<&str>) {
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // SAFETY: GPUI's AppKit handle points at the window's live content view,
    // and GPUI windows are only touched on the main thread.
    let Some(view) = (unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }) else {
        return;
    };
    if let Some(label) = label {
        // SAFETY: The role is an AppKit constant and the view is retained.
        unsafe { view.setAccessibilityRole(Some(NSAccessibilityTextAreaRole)) };
        view.setAccessibilityLabel(Some(&NSString::from_str(label)));
    }
    view.setAccessibilityElement(label.is_some());
    // SAFETY: The notification is an AppKit constant posted for a retained view.
    unsafe {
        NSAccessibilityPostNotification(&view, NSAccessibilityFocusedUIElementChangedNotification);
    }
}
