use objc2::AnyThread;
use objc2::rc::autoreleasepool;
use objc2_app_kit::{
    NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage,
    NSImageInterpolation,
};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGColorSpace, CGImageAlphaInfo, CGImageByteOrderInfo,
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
        draw(image, width, height, &context);
        Some(())
    })?;
    Some(pixels)
}

pub(crate) fn render_bgra(image: &NSImage, width: u32, height: u32) -> Option<Vec<u8>> {
    let bytes_per_row = usize::try_from(width).ok()?.checked_mul(4)?;
    let length = bytes_per_row.checked_mul(usize::try_from(height).ok()?)?;
    let mut pixels = vec![0_u8; length];
    autoreleasepool(|_| {
        let space = CGColorSpace::new_device_rgb()?;
        let bitmap = unsafe {
            CGBitmapContextCreate(
                pixels.as_mut_ptr().cast(),
                usize::try_from(width).ok()?,
                usize::try_from(height).ok()?,
                8,
                bytes_per_row,
                Some(&space),
                CGImageAlphaInfo::PremultipliedFirst.0 | CGImageByteOrderInfo::Order32Little.0,
            )
        }?;
        let context = NSGraphicsContext::graphicsContextWithCGContext_flipped(&bitmap, false);
        draw(image, width, height, &context);
        Some(())
    })?;
    Some(pixels)
}

fn draw(image: &NSImage, width: u32, height: u32, context: &NSGraphicsContext) {
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(context));
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
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use objc2::rc::Retained;
    use objc2_foundation::NSData;

    fn source() -> Retained<NSImage> {
        let pixels = vec![
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, 10, 20, 30, 255, 80, 40, 20, 128, 0, 0,
            255, 255,
        ];
        let source = image::RgbaImage::from_raw(3, 2, pixels).expect("source pixels");
        let mut png = std::io::Cursor::new(Vec::new());
        source
            .write_to(&mut png, image::ImageFormat::Png)
            .expect("PNG");
        let bitmap =
            NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(png.get_ref())).expect("bitmap");
        let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(3.0, 2.0));
        image.addRepresentation(&bitmap);
        image
    }

    #[test]
    fn direct_bgra_preserves_rows_scaling_and_premultiplied_alpha() {
        let image = source();
        for (width, height) in [(3, 2), (6, 4), (5, 3), (1, 1)] {
            let mut expected = render_rgba(&image, width, height).expect("RGBA");
            for pixel in expected.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            assert_eq!(render_bgra(&image, width, height).expect("BGRA"), expected);
        }
        let pixels = render_bgra(&image, 3, 2).expect("BGRA");
        assert_eq!(&pixels[0..4], &[0, 0, 255, 255]);
        assert_eq!(&pixels[4..8], &[0, 128, 0, 128]);
        assert_eq!(&pixels[8..12], &[0, 0, 0, 0]);
        assert_eq!(&pixels[20..24], &[255, 0, 0, 255]);
    }

    #[test]
    #[ignore = "manual headless benchmark of native snapshot pixel formats"]
    fn snapshot_pixel_format_benchmark() {
        use std::io::Write;
        let image = source();
        for direct in [false, true] {
            let started = std::time::Instant::now();
            for _ in 0..20 {
                let pixels = if direct {
                    render_bgra(&image, 1440, 900).expect("BGRA")
                } else {
                    let mut pixels = render_rgba(&image, 1440, 900).expect("RGBA");
                    for pixel in pixels.chunks_exact_mut(4) {
                        pixel.swap(0, 2);
                    }
                    pixels
                };
                std::hint::black_box(pixels);
            }
            let _ = writeln!(
                std::io::stderr(),
                "20 snapshots, 1440x900, direct_bgra={direct}: {:?}",
                started.elapsed(),
            );
        }
    }
}
