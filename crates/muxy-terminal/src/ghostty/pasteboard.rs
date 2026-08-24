use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::{MainThreadMarker, NSString};

pub fn read_text() -> Option<String> {
    MainThreadMarker::new()?;
    let pasteboard = NSPasteboard::generalPasteboard();
    let value = unsafe { pasteboard.stringForType(NSPasteboardTypeString) }?;
    Some(value.to_string())
}

pub fn write_text(text: &str) -> bool {
    if MainThreadMarker::new().is_none() {
        return false;
    }
    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    unsafe { pasteboard.setString_forType(&NSString::from_str(text), NSPasteboardTypeString) }
}
