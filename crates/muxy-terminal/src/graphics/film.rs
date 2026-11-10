//! Decodes image files, keeping every frame of an animated GIF.

use std::io::Cursor;

use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, ImageDecoder, ImageFormat, RgbaImage};

use super::{MAX_DECODE_BYTES, Picture, decode_limits, fitted};

/// Memory all the frames of one animation may use before they are shrunk.
pub(crate) const FILM_BUDGET: usize = 32 * 1024 * 1024;
/// Frames past this many are left out.
const MAX_FRAMES: usize = muxy_protocol::MAX_GRAPHICS_FRAMES;
/// Canvas bytes one file may have drawn, frame after frame, before the rest
/// of its frames are left out. It bounds the time a decode holds a terminal.
const DRAWN_BUDGET: u64 = 1024 * 1024 * 1024;
/// Browsers play a frame that asks for less than this as `SLOW_GAP`.
const SHORTEST_GAP: u32 = 20;
const SLOW_GAP: u32 = 100;

/// The frames of an image, all the same size. A still image has one.
#[derive(Debug)]
pub(crate) struct Film {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frames: Vec<Frame>,
}

#[derive(Debug)]
pub(crate) struct Frame {
    pub(crate) rgba: Vec<u8>,
    /// Milliseconds the frame shows before the next one.
    pub(crate) gap: u32,
}

impl Film {
    pub(crate) fn still(picture: Picture) -> Self {
        Self {
            width: picture.width,
            height: picture.height,
            frames: vec![Frame {
                rgba: picture.rgba,
                gap: 0,
            }],
        }
    }

    pub(crate) fn animated(&self) -> bool {
        self.frames.len() > 1
    }

    pub(crate) fn bytes(&self) -> usize {
        self.frames.iter().map(|frame| frame.rgba.len()).sum()
    }
}

/// Decodes any format the server can read, found from the data.
pub(crate) fn decode(bytes: &[u8]) -> Option<Film> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    if reader.format() == Some(ImageFormat::Gif)
        && let Some(film) = gif(bytes, FILM_BUDGET, decode_limits())
    {
        return Some(film);
    }
    let picture = super::decode_image(reader)?;
    Some(Film::still(Picture {
        width: picture.width(),
        height: picture.height(),
        rgba: picture.into_raw(),
    }))
}

/// Decodes the frames of a GIF, each drawn over the ones before it. Frames
/// are shrunk together as they arrive, so the film never passes `budget`
/// bytes by more than a frame. A damaged file keeps the frames before the
/// damage. `limits` also bound each frame's own size, which a file states
/// apart from the size of the whole picture.
fn gif(bytes: &[u8], budget: usize, limits: image::Limits) -> Option<Film> {
    let mut decoder = GifDecoder::new(Cursor::new(bytes)).ok()?;
    decoder.set_limits(limits).ok()?;
    let (width, height) = decoder.dimensions();
    let canvas = u64::from(width) * u64::from(height) * 4;
    if canvas == 0 || canvas > MAX_DECODE_BYTES {
        return None;
    }
    let most = usize::try_from(DRAWN_BUDGET / canvas)
        .unwrap_or(MAX_FRAMES)
        .clamp(1, MAX_FRAMES);
    let mut size = fitted(width, height);
    let mut frames: Vec<(RgbaImage, u32)> = Vec::new();
    for frame in decoder.into_frames().take(most) {
        let Ok(frame) = frame else {
            break;
        };
        let (numerator, denominator) = frame.delay().numer_denom_ms();
        let gap = numerator / denominator.max(1);
        let gap = if gap < SHORTEST_GAP { SLOW_GAP } else { gap };
        frames.push((shrink(frame.into_buffer(), size), gap));
        if frames.len() * bytes_of(size) > budget {
            size = smaller(size);
            for (picture, _) in &mut frames {
                *picture = shrink(std::mem::take(picture), size);
            }
        }
    }
    if frames.is_empty() {
        return None;
    }
    Some(Film {
        width: size.0,
        height: size.1,
        frames: frames
            .into_iter()
            .map(|(picture, gap)| Frame {
                rgba: picture.into_raw(),
                gap,
            })
            .collect(),
    })
}

fn shrink(picture: RgbaImage, size: (u32, u32)) -> RgbaImage {
    if picture.dimensions() == size {
        return picture;
    }
    image::imageops::thumbnail(&picture, size.0, size.1)
}

fn bytes_of(size: (u32, u32)) -> usize {
    size.0 as usize * size.1 as usize * 4
}

/// About half the pixels of `size`, keeping its shape.
fn smaller((width, height): (u32, u32)) -> (u32, u32) {
    let shorter = |length: u32| (length.saturating_mul(181) / 256).max(1);
    (shorter(width), shorter(height))
}

#[cfg(test)]
mod tests {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Rgba};

    use super::*;

    fn encode(width: u32, height: u32, delays: &[u32]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            let _ = encoder.set_repeat(Repeat::Infinite);
            for (index, &delay) in delays.iter().enumerate() {
                let shade = u8::try_from(index * 60).unwrap_or(u8::MAX);
                let picture = RgbaImage::from_pixel(width, height, Rgba([shade, 0, 255, 255]));
                let frame =
                    image::Frame::from_parts(picture, 0, 0, Delay::from_numer_denom_ms(delay, 1));
                let _ = encoder.encode_frame(frame);
            }
        }
        bytes
    }

    #[test]
    fn keeps_every_frame_and_its_gap() {
        let film = decode(&encode(4, 2, &[70, 0, 250]));
        let film = film.as_ref();
        assert_eq!(film.map(|film| (film.width, film.height)), Some((4, 2)));
        let gaps: Vec<u32> = film
            .into_iter()
            .flat_map(|film| film.frames.iter().map(|frame| frame.gap))
            .collect();
        assert_eq!(gaps, [70, SLOW_GAP, 250]);
        let reds: Vec<u8> = film
            .into_iter()
            .flat_map(|film| film.frames.iter().map(|frame| frame.rgba[0]))
            .collect();
        assert!(reds[0] < reds[1] && reds[1] < reds[2], "{reds:?}");
        assert!(film.is_some_and(Film::animated));
    }

    #[test]
    fn one_frame_and_other_formats_are_stills() {
        let film = decode(&encode(3, 3, &[100]));
        assert_eq!(film.map(|film| film.frames.len()), Some(1));
        let mut png = Vec::new();
        let picture = RgbaImage::from_pixel(2, 2, Rgba([0, 255, 0, 255]));
        let written = picture.write_to(&mut Cursor::new(&mut png), ImageFormat::Png);
        assert!(written.is_ok());
        let film = decode(&png);
        assert_eq!(film.map(|film| film.animated()), Some(false));
        assert!(decode(b"not an image").is_none());
    }

    #[test]
    fn long_films_are_shrunk_to_the_budget() {
        let budget = 16 * 1024;
        let film = gif(&encode(40, 30, &[40; 12]), budget, decode_limits());
        let film = film.as_ref();
        assert_eq!(film.map(|film| film.frames.len()), Some(12));
        let total: usize = film
            .into_iter()
            .flat_map(|film| film.frames.iter().map(|frame| frame.rgba.len()))
            .sum();
        assert!(total <= budget, "{total}");
        let size = film.map(|film| (film.width, film.height));
        assert!(
            size.is_some_and(|(width, height)| width < 40 && width.abs_diff(height * 4 / 3) <= 2),
            "{size:?}"
        );
        let sized = film.is_some_and(|film| {
            film.frames
                .iter()
                .all(|frame| frame.rgba.len() == bytes_of((film.width, film.height)))
        });
        assert!(sized);
    }

    #[test]
    fn a_frame_larger_than_the_decoder_may_allocate_is_refused() {
        // The file says its picture is one pixel, and its only frame 40 by 30.
        let mut file = encode(40, 30, &[40]);
        file[6..10].copy_from_slice(&[1, 0, 1, 0]);
        let mut small = decode_limits();
        small.max_alloc = Some(1024);
        assert!(gif(&file, FILM_BUDGET, small).is_none());
        assert!(gif(&file, FILM_BUDGET, decode_limits()).is_some());
    }
}
