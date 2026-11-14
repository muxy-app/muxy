//! Site icons for browser tabs.

use std::io::{Cursor, Read};
use std::sync::OnceLock;
use std::time::Duration;

use muxy_core::worker::WorkerPool;

const MAX_BYTES: u64 = 1024 * 1024;
const SIZE: u32 = 64;
/// The largest icon decoded, on each side. A page picks its icon, and a small
/// file can claim a huge image, so anything bigger is never decoded.
const MAX_SIDE: u32 = 1024;
/// What decoding one icon may allocate, its pixels included.
const MAX_DECODED_BYTES: u64 = 16 * 1024 * 1024;

/// Fetches and decodes the icon at `url` on a small pool of its own, so pages
/// can't make many large decodes run at once. `None` when the pool is busy:
/// the tab then shows no icon.
pub(crate) fn submit(url: String) -> Option<async_channel::Receiver<Option<gpui::RenderImage>>> {
    static WORKERS: OnceLock<Option<WorkerPool>> = OnceLock::new();
    let pool = WORKERS
        .get_or_init(|| WorkerPool::new("favicons", 2, 16).ok())
        .as_ref()?;
    let (send, receive) = async_channel::bounded(1);
    pool.try_spawn(move || {
        if !send.is_closed() {
            let _ = send.try_send(fetch(&url));
        }
    })
    .ok()?;
    Some(receive)
}

/// The icon at `url`, ready to draw, or none if it can't be read.
fn fetch(url: &str) -> Option<gpui::RenderImage> {
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
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    reader.limits(limits);
    let mut bgra = reader.decode().ok()?.thumbnail(SIZE, SIZE).into_rgba8();
    for pixel in bgra.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Some(gpui::RenderImage::new([image::Frame::new(bgra)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbaImage::new(width, height)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn icons_decode_only_up_to_a_small_size() {
        assert!(decode(&png(32, 32)).is_some());
        assert!(decode(&png(MAX_SIDE, 16)).is_some());
        // A few kilobytes of PNG that would decode to hundreds of megabytes.
        let huge = png(MAX_SIDE + 1, 1);
        assert!(decode(&huge).is_none());
        let tall = png(1, 20_000);
        assert!(tall.len() < 64 * 1024, "{}", tall.len());
        assert!(decode(&tall).is_none());
        assert!(decode(b"not an image").is_none());
    }
}
