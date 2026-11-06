use std::io::Cursor;

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
    let image = Image::new(bytes, &DecodeSettings::default()).ok()?;
    let (width, height) = (image.width(), image.height());
    if width == 0 || height == 0 || width > 1024 || height > 1024 {
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
