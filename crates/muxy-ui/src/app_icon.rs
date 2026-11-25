use std::path::Path;

use objc2::rc::Retained;
use objc2::{AllocAnyThread, MainThreadMarker};
use objc2_app_kit::{NSApplication, NSImage, NSWorkspace};
use objc2_foundation::{
    NSBundle, NSData, NSDistributedNotificationCenter, NSSize, NSString, NSUserDefaults,
};

const DOCK_ICON_KEY: &str = "MuxyDockIcon";

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
    let icon = path.and_then(load);
    let app = NSApplication::sharedApplication(main_thread);
    unsafe { app.setApplicationIconImage(icon.as_ref().map(|(_, image)| &**image)) };
    share_with_dock_tile(icon.as_ref().map(|(data, _)| &**data));
    path.is_none() || icon.is_some()
}

fn load(path: &Path) -> Option<(Retained<NSData>, Retained<NSImage>)> {
    let data = NSData::with_bytes(&std::fs::read(path).ok()?);
    let image = NSImage::initWithData(NSImage::alloc(), &data)?;
    image.isValid().then_some((data, image))
}

fn share_with_dock_tile(data: Option<&NSData>) {
    let Some(identifier) = NSBundle::mainBundle().bundleIdentifier() else {
        return;
    };
    let defaults = NSUserDefaults::standardUserDefaults();
    let key = NSString::from_str(DOCK_ICON_KEY);
    match data {
        Some(data) => unsafe { defaults.setObject_forKey(Some(data), &key) },
        None => defaults.removeObjectForKey(&key),
    }
    let name = NSString::from_str(&format!("{identifier}.DockIconChanged"));
    let center = NSDistributedNotificationCenter::defaultCenter();
    unsafe {
        center.postNotificationName_object_userInfo_deliverImmediately(&name, None, None, true);
    };
}
