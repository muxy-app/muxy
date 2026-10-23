//! Site icons for browser tabs.

use std::io::Read;
use std::time::Duration;

const MAX_BYTES: u64 = 1024 * 1024;
const SIZE: u32 = 64;

/// The icon at `url`, ready to draw, or none if it can't be read.
pub(crate) fn fetch(url: &str) -> Option<gpui::RenderImage> {
    let client = crate::proxy::configure(
        reqwest::blocking::Client::builder().timeout(Duration::from_secs(5)),
    )
    .ok()?
    .build()
    .ok()?;
    let response = client.get(url).send().ok()?.error_for_status().ok()?;
    let mut bytes = Vec::new();
    response.take(MAX_BYTES).read_to_end(&mut bytes).ok()?;
    decode(&bytes)
}

fn decode(bytes: &[u8]) -> Option<gpui::RenderImage> {
    let mut bgra = image::load_from_memory(bytes)
        .ok()?
        .thumbnail(SIZE, SIZE)
        .into_rgba8();
    for pixel in bgra.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(gpui::RenderImage::new([image::Frame::new(bgra)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_decode_to_small_images_and_junk_does_not() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(128, 128, image::Rgba([255, 0, 0, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .expect("png");
        let icon = decode(png.get_ref()).expect("icon");
        assert!(icon.size(0).width.0 <= i32::try_from(SIZE).expect("size"));
        assert!(decode(b"<svg/>").is_none());
    }
}
