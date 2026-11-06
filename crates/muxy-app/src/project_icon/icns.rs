use std::io::Cursor;

const MAX_SIDE: u32 = 1024;
const MAX_TILES: u64 = 64;
const MAX_LAYERS: u16 = 32;
const MAX_CODE_BLOCKS: u64 = 32 * 1024;
const MAX_CODE_BLOCK_LAYERS: u64 = 256 * 1024;

pub(super) fn decode(bytes: &[u8]) -> Option<image::RgbaImage> {
    let length = u32::from_be_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
    if length != bytes.len() {
        return None;
    }
    let mut offset = 8;
    while offset < length {
        let size = u32::from_be_bytes(bytes.get(offset + 4..offset + 8)?.try_into().ok()?) as usize;
        if size < 8 || size > length - offset {
            return None;
        }
        offset += size;
    }
    let family = icns::IconFamily::read(Cursor::new(bytes)).ok()?;
    let mut icons = family.available_icons();
    icons.sort_by_key(|kind| kind.pixel_width().abs_diff(64));
    icons.into_iter().find_map(|kind| {
        let element = family
            .elements
            .iter()
            .find(|element| element.ostype == kind.ostype())?;
        if element.data.starts_with(b"\x89PNG") {
            return crate::views::project_editor::logo::decode(&element.data).ok();
        }
        if element.data.starts_with(b"\0\0\0\x0cjP  ") {
            return jpeg2000(&element.data);
        }
        let decoded = family
            .get_icon_with_type(kind)
            .ok()?
            .convert_to(icns::PixelFormat::RGBA);
        image::RgbaImage::from_raw(
            decoded.width(),
            decoded.height(),
            decoded.into_data().into_vec(),
        )
    })
}

fn jpeg2000(bytes: &[u8]) -> Option<image::RgbaImage> {
    use hayro_jpeg2000::{ColorSpace, DecodeSettings, Image};
    jpeg2000_layout(bytes)?;
    let image = Image::new(bytes, &DecodeSettings::default()).ok()?;
    let (width, height) = (image.width(), image.height());
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return None;
    }
    let channels = match image.color_space() {
        ColorSpace::Gray => 1,
        ColorSpace::RGB => 3,
        _ => return None,
    } + usize::from(image.has_alpha());
    let pixels = image.decode().ok()?;
    let decoded = match channels {
        1 => image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(width, height, pixels)?),
        2 => image::DynamicImage::ImageLumaA8(image::GrayAlphaImage::from_raw(
            width, height, pixels,
        )?),
        3 => image::DynamicImage::ImageRgb8(image::RgbImage::from_raw(width, height, pixels)?),
        4 => image::DynamicImage::ImageRgba8(image::RgbaImage::from_raw(width, height, pixels)?),
        _ => return None,
    };
    Some(decoded.into_rgba8())
}

fn jpeg2000_layout(mut bytes: &[u8]) -> Option<()> {
    let mut codestream = false;
    while !bytes.is_empty() {
        let length = u32::from_be_bytes(bytes.get(..4)?.try_into().ok()?);
        let kind = bytes.get(4..8)?;
        let (header, length) = match length {
            0 => (8, bytes.len()),
            1 => (
                16,
                usize::try_from(u64::from_be_bytes(bytes.get(8..16)?.try_into().ok()?)).ok()?,
            ),
            length => (8, usize::try_from(length).ok()?),
        };
        let data = bytes.get(header..length)?;
        if kind == b"jp2c" {
            jpeg2000_codestream_layout(data)?;
            codestream = true;
        }
        bytes = bytes.get(length..)?;
    }
    codestream.then_some(())
}

fn jpeg2000_codestream_layout(bytes: &[u8]) -> Option<()> {
    if !bytes.starts_with(b"\xffO\xffQ") {
        return None;
    }
    let length = u16::from_be_bytes(bytes.get(4..6)?.try_into().ok()?);
    let components = u16::from_be_bytes(bytes.get(40..42)?.try_into().ok()?);
    if !(1..=4).contains(&components) || length != 38 + 3 * components {
        return None;
    }
    bytes.get(..usize::from(length) + 4)?;
    let read = |offset| {
        Some(u32::from_be_bytes(
            bytes.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    let columns = jpeg2000_tiles(read(8)?, read(16)?, read(24)?, read(32)?)?;
    let rows = jpeg2000_tiles(read(12)?, read(20)?, read(28)?, read(36)?)?;
    if columns * rows > MAX_TILES {
        return None;
    }
    jpeg2000_coding_layout(
        bytes.get(usize::from(length) + 4..)?,
        [read(8)?, read(12)?],
        components,
    )
}

fn jpeg2000_tiles(size: u32, offset: u32, tile_size: u32, tile_offset: u32) -> Option<u64> {
    if size == 0
        || size > MAX_SIDE
        || offset >= size
        || tile_size == 0
        || tile_offset > offset
        || tile_offset.checked_add(tile_size)? <= offset
    {
        return None;
    }
    let tiles = u64::from(size - tile_offset).div_ceil(u64::from(tile_size));
    u32::try_from(u64::from(tile_offset) + tiles * u64::from(tile_size))
        .is_ok()
        .then_some(tiles)
}

#[derive(Default)]
struct CodingBudget {
    max_component_blocks: u64,
    max_layers: u16,
}

fn jpeg2000_coding_layout(mut bytes: &[u8], size: [u32; 2], components: u16) -> Option<()> {
    let mut budget = CodingBudget::default();
    jpeg2000_header(&mut bytes, size, &mut budget, 0x90)?;
    let mut tile_parts = 0;
    while bytes.starts_with(b"\xff\x90") {
        if bytes.get(2..4)? != [0, 10] {
            return None;
        }
        let length = u32::from_be_bytes(bytes.get(6..10)?.try_into().ok()?);
        let length = if length == 0 {
            bytes.len()
        } else {
            usize::try_from(length).ok()?
        };
        let (part, rest) = bytes.split_at_checked(length)?;
        let mut header = part.get(12..)?;
        jpeg2000_header(&mut header, size, &mut budget, 0x93)?;
        bytes = rest;
        tile_parts += 1;
    }
    let blocks = budget.max_component_blocks * u64::from(components);
    (tile_parts != 0
        && (bytes.is_empty() || bytes.starts_with(b"\xff\xd9"))
        && blocks <= MAX_CODE_BLOCKS
        && blocks * u64::from(budget.max_layers) <= MAX_CODE_BLOCK_LAYERS)
        .then_some(())
}

fn jpeg2000_header(
    bytes: &mut &[u8],
    size: [u32; 2],
    budget: &mut CodingBudget,
    end: u8,
) -> Option<()> {
    loop {
        let marker = match bytes.get(..2)? {
            [0xff, marker] => *marker,
            _ => return None,
        };
        if marker == end {
            return Some(());
        }
        if (0x30..=0x3f).contains(&marker) {
            *bytes = bytes.get(2..)?;
            continue;
        }
        let length = usize::from(u16::from_be_bytes(bytes.get(2..4)?.try_into().ok()?)) + 2;
        let data = bytes.get(4..length)?;
        match marker {
            0x52 => {
                let layers = u16::from_be_bytes(data.get(2..4)?.try_into().ok()?);
                if !(1..=MAX_LAYERS).contains(&layers) {
                    return None;
                }
                budget.max_layers = budget.max_layers.max(layers);
                budget.max_component_blocks = budget
                    .max_component_blocks
                    .max(jpeg2000_coding_style(data.get(5..)?, data[0], size)?);
            }
            0x53 => {
                budget.max_component_blocks = budget
                    .max_component_blocks
                    .max(jpeg2000_coding_style(data.get(2..)?, *data.get(1)?, size)?);
            }
            0x5c | 0x5d => {
                let data = if marker == 0x5d { data.get(1..)? } else { data };
                let length = data.len().checked_sub(1)?;
                match data[0] & 0x1f {
                    0 => {}
                    1 if length == 2 => {}
                    2 if length.is_multiple_of(2) => {}
                    _ => return None,
                }
            }
            0x55 | 0x58 | 0x5e | 0x60 | 0x61 | 0x63 | 0x64 => {}
            _ => return None,
        }
        *bytes = bytes.get(length..)?;
    }
}

fn jpeg2000_coding_style(bytes: &[u8], flags: u8, size: [u32; 2]) -> Option<u64> {
    let levels = *bytes.first()?;
    let explicit = flags & 1 != 0;
    let length = 5 + if explicit { usize::from(levels) + 1 } else { 0 };
    if levels > 32 || bytes.len() != length {
        return None;
    }
    let block_width = bytes[1].checked_add(2)?;
    let block_height = bytes[2].checked_add(2)?;
    let mut blocks = 0;
    for resolution in 0..=levels {
        let precinct = if explicit {
            bytes[5 + usize::from(resolution)]
        } else {
            0xff
        };
        let high = u8::from(resolution != 0);
        let width = block_width.min((precinct & 0xf).checked_sub(high)?);
        let height = block_height.min((precinct >> 4).checked_sub(high)?);
        let scale = u32::from(levels - resolution + high);
        let columns = u64::from(size[0]).div_ceil(1_u64 << (scale + u32::from(width)));
        let rows = u64::from(size[1]).div_ceil(1_u64 << (scale + u32::from(height)));
        blocks += columns * rows * if resolution == 0 { 1 } else { 3 };
    }
    Some(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg2000_coding_budget_counts_blocks_and_layers() {
        assert_eq!(
            jpeg2000_coding_style(&[6, 2, 2, 0, 0], 0, [1024; 2]),
            Some(4096)
        );
        assert_eq!(
            jpeg2000_coding_style(&[0, 4, 4, 0, 0, 0], 1, [1024; 2]),
            Some(1024 * 1024)
        );
        assert!(jpeg2000_coding_style(&[33, 2, 2, 0, 0], 0, [16; 2]).is_none());
        assert!(jpeg2000_coding_style(&[0, 254, 2, 0, 0], 0, [16; 2]).is_none());
        assert!(jpeg2000_coding_style(&[1, 2, 2, 0, 0, 0xff, 0], 1, [16; 2]).is_none());
        let mut bytes = include_bytes!("fixtures/red.jp2").to_vec();
        let start = bytes
            .windows(4)
            .position(|bytes| bytes == b"\xffO\xffQ")
            .unwrap();
        for offset in [8, 12, 24, 28] {
            bytes[start + offset..start + offset + 4].copy_from_slice(&1024_u32.to_be_bytes());
        }
        assert!(
            jpeg2000_layout(&bytes).is_some(),
            "one layer with ordinary codeblocks"
        );
        let cod = bytes
            .windows(2)
            .position(|bytes| bytes == b"\xffR")
            .unwrap();
        bytes[cod + 6..cod + 8].copy_from_slice(&32_u16.to_be_bytes());
        assert!(
            jpeg2000_layout(&bytes).is_none(),
            "same codeblocks with 32 layers"
        );
    }

    #[test]
    fn jpeg2000_headers_reject_lengths_the_decoder_would_ignore() {
        for marker in [0x5c, 0x5d] {
            for style in [1, 2] {
                for size in [2_usize, 3] {
                    let mut data = vec![0xff, marker];
                    let component = usize::from(marker == 0x5d);
                    data.extend(u16::try_from(3 + component + size).unwrap().to_be_bytes());
                    if component != 0 {
                        data.push(0);
                    }
                    data.push(style);
                    data.extend(vec![0; size]);
                    data.extend(b"\xff\x90");
                    assert_eq!(
                        jpeg2000_header(
                            &mut data.as_slice(),
                            [16; 2],
                            &mut CodingBudget::default(),
                            0x90
                        )
                        .is_some(),
                        size == 2,
                    );
                }
            }
        }
        let original = include_bytes!("fixtures/red.jp2");
        let cod = original
            .windows(2)
            .position(|bytes| bytes == b"\xffR")
            .unwrap();
        for length in [0_u16, 11, 13, u16::MAX] {
            let mut bytes = original.to_vec();
            bytes[cod + 2..cod + 4].copy_from_slice(&length.to_be_bytes());
            assert!(jpeg2000_layout(&bytes).is_none());
        }
        let tile = original
            .windows(2)
            .position(|bytes| bytes == b"\xff\x90")
            .unwrap();
        for length in [1_u32, 11, 12, 13, u32::MAX] {
            let mut bytes = original.to_vec();
            bytes[tile + 6..tile + 10].copy_from_slice(&length.to_be_bytes());
            assert!(jpeg2000_layout(&bytes).is_none());
        }
        let mut bytes = original.to_vec();
        bytes[tile + 6..tile + 10].copy_from_slice(&0_u32.to_be_bytes());
        assert!(jpeg2000_layout(&bytes).is_some(), "tile part through EOF");
    }

    #[test]
    fn jpeg2000_layout_bounds_tiles_components_and_coordinates() {
        let original = include_bytes!("fixtures/red.jp2");
        let start = original
            .windows(4)
            .position(|bytes| bytes == b"\xffO\xffQ")
            .expect("codestream");
        assert!(jpeg2000_layout(original).is_some());
        for (offset, value) in [
            (8, 0_u32),
            (8, MAX_SIDE + 1),
            (12, 0),
            (12, MAX_SIDE + 1),
            (16, 16),
            (20, 16),
            (24, 0),
            (28, 0),
            (32, 1),
            (36, 1),
        ] {
            let mut bytes = original.to_vec();
            bytes[start + offset..start + offset + 4].copy_from_slice(&value.to_be_bytes());
            assert!(jpeg2000_layout(&bytes).is_none(), "{offset}: {value}");
        }
        for components in [0_u16, 5, u16::MAX] {
            let mut bytes = original.to_vec();
            bytes[start + 40..start + 42].copy_from_slice(&components.to_be_bytes());
            assert!(jpeg2000_layout(&bytes).is_none());
        }
        let mut bytes = original.to_vec();
        for offset in [24, 28] {
            bytes[start + offset..start + offset + 4].copy_from_slice(&2_u32.to_be_bytes());
        }
        assert!(jpeg2000_layout(&bytes).is_some(), "64 tiles");
        bytes[start + 24..start + 28].copy_from_slice(&1_u32.to_be_bytes());
        assert!(jpeg2000_layout(&bytes).is_none(), "128 tiles");
        assert_eq!(jpeg2000_tiles(16, 1, 16, 1), Some(1));
        assert_eq!(jpeg2000_tiles(16, 1, u32::MAX, 1), None);
        assert_eq!(jpeg2000_tiles(16, 1, 1, 0), None);
        let mut bytes = original.to_vec();
        bytes.extend(12_u32.to_be_bytes());
        bytes.extend(b"jp2c\xffO\xffQ");
        assert!(
            jpeg2000_layout(&bytes).is_none(),
            "all codestreams are checked"
        );
    }
}
