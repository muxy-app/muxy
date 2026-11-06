//! Decodes a Sixel image into RGBA pixels.
//!
//! Pixels use a 1:1 aspect ratio, as modern encoders expect. The palette
//! starts from the VT340 colors for every image.

use super::{MAX_DECODE_BYTES, MAX_SIDE, Picture};

const REGISTERS: usize = 1024;
/// Pixel writes allowed per image: twice the most pixels an image may have,
/// so data that paints the same pixels over and over can't stall the terminal.
const MAX_PAINTED: u64 = MAX_DECODE_BYTES / 2;
/// VT340 default colors, in percent.
const VT340: [[u8; 3]; 16] = [
    [0, 0, 0],
    [20, 20, 80],
    [80, 13, 13],
    [20, 80, 20],
    [80, 20, 80],
    [20, 80, 80],
    [80, 80, 20],
    [53, 53, 53],
    [26, 26, 26],
    [33, 33, 60],
    [60, 26, 26],
    [33, 60, 33],
    [60, 33, 60],
    [33, 60, 60],
    [60, 60, 33],
    [80, 80, 80],
];

/// Decodes the parameters and data between `ESC P` and `ST`.
///
/// Unset pixels are transparent when the second parameter is 1; otherwise
/// they take `background`, as on a VT340. The picture keeps its full size,
/// which may be over the image budget.
pub(crate) fn decode(params: &[u8], data: &[u8], background: [u8; 3]) -> Option<Picture> {
    let transparent = params.split(|&b| b == b';').nth(1) == Some(b"1");
    let extent = walk(data, Extent::new(!transparent, true), |_, _, _| {})?;
    let width = extent.raster[0].max(extent.right);
    let height = extent.raster[1].max(extent.bottom);
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return None;
    }
    let len = u64::from(width) * u64::from(height) * 4;
    if len > MAX_DECODE_BYTES {
        return None;
    }
    let len = usize::try_from(len).ok()?;
    let fill = if transparent {
        [0; 4]
    } else {
        [background[0], background[1], background[2], 255]
    };
    let mut rgba = fill.repeat(len / 4);
    let row = width as usize * 4;
    walk(data, Extent::new(!transparent, false), |x, y, color| {
        if x < width && y < height {
            let at = y as usize * row + x as usize * 4;
            rgba[at..at + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
        }
    })?;
    Some(Picture {
        width,
        height,
        rgba,
    })
}

#[derive(Debug, Default)]
struct Extent {
    /// Width and height from the raster attributes.
    raster: [u32; 2],
    /// One past the rightmost column and lowest row drawn.
    right: u32,
    bottom: u32,
    painted: u64,
    opaque: bool,
    /// Only measures the image, without drawing it.
    measuring: bool,
}

impl Extent {
    fn new(opaque: bool, measuring: bool) -> Self {
        Self {
            opaque,
            measuring,
            ..Self::default()
        }
    }

    /// An opaque image without a declared size paints each sixel's whole band.
    fn fills_bands(&self) -> bool {
        self.opaque && self.raster == [0, 0]
    }
}

/// Runs the sixel commands, calling `draw` for every set pixel.
fn walk(
    data: &[u8],
    mut extent: Extent,
    mut draw: impl FnMut(u32, u32, [u8; 3]),
) -> Option<Extent> {
    let mut palette = vec![[0_u8; 3]; REGISTERS];
    for (register, color) in palette.iter_mut().zip(VT340) {
        *register = color.map(|value| percent(u32::from(value)));
    }
    let mut color = 0;
    let (mut x, mut y) = (0_u32, 0_u32);
    let mut i = 0;
    while i < data.len() {
        let byte = data[i];
        i += 1;
        match byte {
            b'"' => {
                let (values, next) = numbers(data, i);
                i = next;
                extent.raster = [values[2], values[3]];
            }
            b'#' => {
                let (values, next) = numbers(data, i);
                i = next;
                let register = values[0] as usize;
                if register >= REGISTERS {
                    continue;
                }
                color = register;
                match values[1] {
                    1 => palette[register] = hls(values[2], values[3], values[4]),
                    2 => palette[register] = [values[2], values[3], values[4]].map(percent),
                    _ => {}
                }
            }
            b'!' => {
                let (values, next) = numbers(data, i);
                i = next;
                if let Some(&sixel @ b'?'..=b'~') = data.get(i) {
                    i += 1;
                    let count = values[0].max(1);
                    paint(
                        &mut x,
                        y,
                        count,
                        sixel,
                        palette[color],
                        &mut extent,
                        &mut draw,
                    )?;
                }
            }
            b'$' => x = 0,
            b'-' => {
                x = 0;
                y = y.checked_add(6).filter(|y| *y < MAX_SIDE)?;
            }
            b'?'..=b'~' => paint(&mut x, y, 1, byte, palette[color], &mut extent, &mut draw)?,
            _ => {}
        }
    }
    Some(extent)
}

fn paint(
    x: &mut u32,
    y: u32,
    count: u32,
    sixel: u8,
    color: [u8; 3],
    extent: &mut Extent,
    draw: &mut impl FnMut(u32, u32, [u8; 3]),
) -> Option<()> {
    let end = x.checked_add(count).filter(|end| *end <= MAX_SIDE)?;
    let bits = sixel - b'?';
    if extent.fills_bands() {
        extent.right = extent.right.max(end);
        extent.bottom = extent.bottom.max(y + 6);
    }
    if bits != 0 {
        for bit in 0..6 {
            if bits & (1 << bit) != 0 {
                extent.painted += u64::from(count);
                if extent.painted > MAX_PAINTED {
                    return None;
                }
                if !extent.measuring {
                    for column in *x..end {
                        draw(column, y + bit, color);
                    }
                }
                extent.bottom = extent.bottom.max(y + bit + 1);
            }
        }
        extent.right = extent.right.max(end);
    }
    *x = end;
    Some(())
}

/// Reads `;`-separated numbers starting at `i`, up to five; missing ones are 0.
fn numbers(data: &[u8], mut i: usize) -> ([u32; 5], usize) {
    let mut values = [0_u32; 5];
    let mut index = 0;
    while let Some(&byte) = data.get(i) {
        match byte {
            b'0'..=b'9' => {
                if let Some(value) = values.get_mut(index) {
                    *value = value
                        .saturating_mul(10)
                        .saturating_add(u32::from(byte - b'0'));
                }
            }
            b';' => index += 1,
            _ => break,
        }
        i += 1;
    }
    (values, i)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn percent(value: u32) -> u8 {
    (f64::from(value.min(100)) * 255.0 / 100.0).round() as u8
}

/// DEC hue puts blue at 0°, red at 120° and green at 240°.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn hls(hue: u32, lightness: u32, saturation: u32) -> [u8; 3] {
    let turn = f64::from((hue % 360 + 240) % 360) / 360.0;
    let light = f64::from(lightness.min(100)) / 100.0;
    let chroma = f64::from(saturation.min(100)) / 100.0;
    if chroma == 0.0 {
        let gray = (light * 255.0).round() as u8;
        return [gray; 3];
    }
    let high = if light < 0.5 {
        light * (1.0 + chroma)
    } else {
        light + chroma - light * chroma
    };
    let low = 2.0 * light - high;
    let channel = |offset: f64| {
        let at = offset.rem_euclid(1.0);
        let value = if at < 1.0 / 6.0 {
            low + (high - low) * 6.0 * at
        } else if at < 0.5 {
            high
        } else if at < 2.0 / 3.0 {
            low + (high - low) * (2.0 / 3.0 - at) * 6.0
        } else {
            low
        };
        (value * 255.0).round() as u8
    };
    [
        channel(turn + 1.0 / 3.0),
        channel(turn),
        channel(turn - 1.0 / 3.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(picture: &Picture, x: u32, y: u32) -> [u8; 4] {
        let at = (y * picture.width + x) as usize * 4;
        let mut out = [0; 4];
        out.copy_from_slice(&picture.rgba[at..at + 4]);
        out
    }

    #[test]
    fn draws_colors_repeats_and_bands() -> Result<(), &'static str> {
        // Red 2x6 block, then a new band with one green pixel at the top.
        let picture =
            decode(b"0;1;0", b"#1;2;100;0;0#1!2~-#2;2;0;100;0@", [0; 3]).ok_or("decode")?;
        assert_eq!((picture.width, picture.height), (2, 7));
        assert_eq!(pixel(&picture, 1, 5), [255, 0, 0, 255]);
        assert_eq!(pixel(&picture, 0, 6), [0, 255, 0, 255]);
        assert_eq!(pixel(&picture, 1, 6), [0, 0, 0, 0]);
        Ok(())
    }

    #[test]
    fn carriage_return_overlays_and_raster_sets_size() -> Result<(), &'static str> {
        let picture =
            decode(b"", b"\"1;1;4;3#1;2;0;0;100@$#2;2;100;100;100A", [9, 9, 9]).ok_or("decode")?;
        assert_eq!((picture.width, picture.height), (4, 3));
        assert_eq!(pixel(&picture, 0, 0), [0, 0, 255, 255]);
        assert_eq!(pixel(&picture, 0, 1), [255, 255, 255, 255]);
        // Opaque background where nothing was drawn.
        assert_eq!(pixel(&picture, 3, 2), [9, 9, 9, 255]);
        Ok(())
    }

    #[test]
    fn hls_hue_starts_at_blue() {
        assert_eq!(hls(0, 50, 100), [0, 0, 255]);
        assert_eq!(hls(120, 50, 100), [255, 0, 0]);
        assert_eq!(hls(240, 50, 100), [0, 255, 0]);
        assert_eq!(hls(0, 100, 0), [255; 3]);
        assert_eq!(hls(u32::MAX, 50, 100), hls(u32::MAX % 360, 50, 100));
    }

    #[test]
    fn blank_sixels_still_size_an_opaque_image() -> Result<(), &'static str> {
        let picture = decode(b"", b"#1;2;100;0;0~??", [9, 9, 9]).ok_or("decode")?;
        assert_eq!((picture.width, picture.height), (3, 6));
        assert_eq!(pixel(&picture, 2, 5), [9, 9, 9, 255]);
        let blank = decode(b"", b"??", [9, 9, 9]).ok_or("blank")?;
        assert_eq!((blank.width, blank.height), (2, 6));
        let transparent = decode(b"0;1", b"~??", [9, 9, 9]).ok_or("transparent")?;
        assert_eq!((transparent.width, transparent.height), (1, 6));
        Ok(())
    }

    #[test]
    fn rejects_empty_and_oversized_images() {
        assert!(decode(b"", b"#1", [0; 3]).is_none());
        assert!(decode(b"", b"!9000~", [0; 3]).is_none());
        assert!(decode(b"", b"\"1;1;8192;8192~", [0; 3]).is_none());
    }

    #[test]
    fn rejects_data_that_repaints_the_same_pixels_without_end() {
        let overdrawn = b"!8192~$".repeat(1000);
        let started = std::time::Instant::now();
        assert!(decode(b"", &overdrawn, [0; 3]).is_none());
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        assert!(decode(b"", &b"!8192~$".repeat(100), [0; 3]).is_some());
    }
}
