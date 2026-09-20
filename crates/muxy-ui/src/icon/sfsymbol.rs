use objc2::MainThreadMarker;
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSImage, NSImageSymbolConfiguration, NSImageSymbolScale};
use objc2_foundation::NSString;

use crate::bitmap;

#[derive(Debug)]
pub(super) struct Mask {
    pub width: u32,
    pub height: u32,
    pub logical_width: f32,
    pub logical_height: f32,
    pub alpha: Vec<u8>,
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Validated AppKit dimensions are converted to GPUI f32 coordinates."
)]
pub(super) fn rasterize(symbol: &str, point_size: f32, weight: f32, scale: f32) -> Option<Mask> {
    let _main_thread = MainThreadMarker::new()?;
    if !point_size.is_finite()
        || point_size <= 0.0
        || point_size > 256.0
        || !weight.is_finite()
        || !(-1.0..=1.0).contains(&weight)
        || !scale.is_finite()
        || !(0.5..=8.0).contains(&scale)
    {
        return None;
    }
    autoreleasepool(|_| {
        let name = NSString::from_str(symbol);
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&name, None)?;

        let config = NSImageSymbolConfiguration::configurationWithPointSize_weight_scale(
            f64::from(point_size),
            f64::from(weight),
            NSImageSymbolScale::Medium,
        );
        let image = image.imageWithSymbolConfiguration(&config)?;

        let natural = image.size();
        let width = dimension(natural.width * f64::from(scale))?;
        let height = dimension(natural.height * f64::from(scale))?;

        let alpha = bitmap::render_rgba(&image, width, height)?
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect();
        Some(Mask {
            width,
            height,
            logical_width: natural.width as f32,
            logical_height: natural.height as f32,
            alpha,
        })
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn dimension(value: f64) -> Option<u32> {
    if value.is_finite() && (0.0..=4096.0).contains(&value) && value > 0.0 {
        Some(value.round().max(1.0) as u32)
    } else {
        None
    }
}
