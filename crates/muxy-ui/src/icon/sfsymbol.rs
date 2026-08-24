use objc2::AnyThread;
use objc2::rc::{Retained, autoreleasepool};
use objc2_app_kit::{
    NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage,
    NSImageSymbolConfiguration, NSImageSymbolScale,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

pub struct Mask {
    pub width: u32,
    pub height: u32,
    pub logical_width: f32,
    pub logical_height: f32,
    pub alpha: Vec<u8>,
}

pub fn rasterize(symbol: &str, point_size: f32, weight: f32, scale: f32) -> Option<Mask> {
    autoreleasepool(|_| unsafe {
        let name = NSString::from_str(symbol);
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&name, None)?;

        let config = NSImageSymbolConfiguration::configurationWithPointSize_weight_scale(
            point_size as f64,
            weight as f64,
            NSImageSymbolScale::Medium,
        );
        let image = image.imageWithSymbolConfiguration(&config)?;

        let natural = image.size();
        let width = (natural.width * scale as f64).round().max(1.0) as u32;
        let height = (natural.height * scale as f64).round().max(1.0) as u32;

        let rep = draw_into_bitmap(&image, width, height)?;
        let data = rep.bitmapData();
        if data.is_null() {
            return None;
        }

        let bytes_per_row = rep.bytesPerRow() as usize;
        let samples = rep.samplesPerPixel() as usize;
        if samples < 4 {
            return None;
        }

        let mut alpha = vec![0u8; (width * height) as usize];
        for y in 0..height as usize {
            let row = data.add(y * bytes_per_row);
            for x in 0..width as usize {
                alpha[y * width as usize + x] = *row.add(x * samples + 3);
            }
        }
        Some(Mask {
            width,
            height,
            logical_width: natural.width as f32,
            logical_height: natural.height as f32,
            alpha,
        })
    })
}

unsafe fn draw_into_bitmap(
    image: &NSImage,
    width: u32,
    height: u32,
) -> Option<Retained<NSBitmapImageRep>> {
    unsafe {
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
        NSBitmapImageRep::alloc(),
        std::ptr::null_mut(),
        width as isize,
        height as isize,
        8,
        4,
        true,
        false,
        NSDeviceRGBColorSpace,
        (width * 4) as isize,
        32,
    )?;

        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));

        let rect = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width as f64, height as f64),
        );
        image.drawInRect_fromRect_operation_fraction(
            rect,
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
            NSCompositingOperation::SourceOver,
            1.0,
        );

        NSGraphicsContext::restoreGraphicsState_class();
        Some(rep)
    }
}
