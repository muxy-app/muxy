use objc2::AnyThread;
use objc2::rc::autoreleasepool;
use objc2_app_kit::{
    NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage,
    NSImageInterpolation,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

/// Draws the image straight into a packed RGBA buffer. Freed large buffers
/// stay resident on macOS, so every extra window-sized copy is a cost the
/// process keeps.
pub(crate) fn render_rgba(image: &NSImage, width: u32, height: u32) -> Option<Vec<u8>> {
    let bytes_per_row = usize::try_from(width).ok()?.checked_mul(4)?;
    let length = bytes_per_row.checked_mul(usize::try_from(height).ok()?)?;
    let mut pixels = vec![0_u8; length];
    let mut planes = [
        pixels.as_mut_ptr(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    ];
    autoreleasepool(|_| unsafe {
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            planes.as_mut_ptr(),
            isize::try_from(width).ok()?,
            isize::try_from(height).ok()?,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            isize::try_from(bytes_per_row).ok()?,
            32,
        )?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        context.setImageInterpolation(NSImageInterpolation::None);
        image.drawInRect_fromRect_operation_fraction(
            NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(f64::from(width), f64::from(height)),
            ),
            NSRect::ZERO,
            NSCompositingOperation::SourceOver,
            1.0,
        );
        NSGraphicsContext::restoreGraphicsState_class();
        Some(())
    })?;
    Some(pixels)
}
