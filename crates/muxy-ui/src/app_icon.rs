//! The icons apps show in Finder.

use std::path::Path;

use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSSize, NSString};

/// The icon of the app at `path`, drawn `side` pixels square as
/// premultiplied BGRA.
pub fn bgra(path: &Path, side: u32) -> Option<Vec<u8>> {
    if !(1..=512).contains(&side) {
        return None;
    }
    let path = NSString::from_str(path.to_str()?);
    let image = NSWorkspace::sharedWorkspace().iconForFile(&path);
    image.setSize(NSSize::new(f64::from(side), f64::from(side)));
    crate::bitmap::render_bgra(&image, side, side)
}
