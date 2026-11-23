use std::path::Path;

use objc2::rc::Retained;
use objc2::{AllocAnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSImage, NSWorkspace};
use objc2_foundation::{NSSize, NSString};

pub fn bgra(path: &Path, side: u32) -> Option<Vec<u8>> {
    if !(1..=512).contains(&side) {
        return None;
    }
    let path = NSString::from_str(path.to_str()?);
    let image = NSWorkspace::sharedWorkspace().iconForFile(&path);
    image.setSize(NSSize::new(f64::from(side), f64::from(side)));
    crate::bitmap::render_bgra(&image, side, side)
}

pub fn is_image(path: &Path) -> bool {
    load(path).is_some()
}

pub fn set_dock_icon(path: Option<&Path>) -> bool {
    let Some(main_thread) = MainThreadMarker::new() else {
        return false;
    };
    let image = path.and_then(load);
    let app = NSApplication::sharedApplication(main_thread);
    unsafe { app.setApplicationIconImage(image.as_deref()) };
    path.is_none() || image.is_some()
}

fn load(path: &Path) -> Option<Retained<NSImage>> {
    let file = NSString::from_str(&path.to_string_lossy());
    NSImage::initWithContentsOfFile(NSImage::alloc(), &file).filter(|image| image.isValid())
}
