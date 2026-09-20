use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage,
    NSImageInterpolation,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

pub(crate) unsafe fn draw_into_bitmap(
    image: &NSImage,
    width: u32,
    height: u32,
) -> Option<Retained<NSBitmapImageRep>> {
    unsafe {
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
        NSBitmapImageRep::alloc(),
        std::ptr::null_mut(),
        isize::try_from(width).ok()?,
        isize::try_from(height).ok()?,
        8,
        4,
        true,
        false,
        NSDeviceRGBColorSpace,
        isize::try_from(width.checked_mul(4)?).ok()?,
        32,
    )?;

        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        context.setImageInterpolation(NSImageInterpolation::None);

        let rect = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(f64::from(width), f64::from(height)),
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

/// The bitmap's RGBA rows without their padding.
pub(crate) unsafe fn rgba_rows(rep: &NSBitmapImageRep, width: u32, height: u32) -> Option<Vec<u8>> {
    let data = rep.bitmapData();
    if data.is_null() {
        return None;
    }
    let bytes_per_row = usize::try_from(rep.bytesPerRow()).ok()?;
    let samples = usize::try_from(rep.samplesPerPixel()).ok()?;
    let columns = usize::try_from(width).ok()?;
    let rows = usize::try_from(height).ok()?;
    let row_bytes = columns.checked_mul(samples)?;
    let length = rows.checked_mul(bytes_per_row)?;
    if samples != 4
        || rep.isPlanar()
        || bytes_per_row < row_bytes
        || usize::try_from(rep.bytesPerPlane()).ok()? < length
    {
        return None;
    }
    let pixels = unsafe { std::slice::from_raw_parts(data, length) };
    let mut packed = Vec::with_capacity(row_bytes.checked_mul(rows)?);
    for row in pixels.chunks_exact(bytes_per_row).take(rows) {
        packed.extend_from_slice(&row[..row_bytes]);
    }
    Some(packed)
}
