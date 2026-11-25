use image::{DynamicImage, ImageFormat};
use std::io::Cursor;

#[derive(Debug)]
pub struct PreparedImageSource {
    contents: Vec<u8>,
    pub(super) extension: &'static str,
}

impl PreparedImageSource {
    pub fn contents(&self) -> &[u8] {
        &self.contents
    }
}

pub const MAX_ENCODED_IMAGE_BYTES: usize = 25 * 1024 * 1024;
pub const MAX_DECODED_IMAGE_PIXELS: u64 = 64_000_000;

pub fn prepare_image_source(contents: Vec<u8>) -> std::io::Result<PreparedImageSource> {
    let (format, extension) = source_format(&contents)?;
    decode_image(&contents, format)?;
    Ok(PreparedImageSource {
        contents,
        extension,
    })
}

pub fn normalize_png(contents: &[u8]) -> std::io::Result<Vec<u8>> {
    let (format, _) = source_format(contents)?;
    let image = decode_image(contents, format)?;
    let mut normalized = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut normalized), ImageFormat::Png)
        .map_err(std::io::Error::other)?;
    Ok(normalized)
}

fn source_format(contents: &[u8]) -> std::io::Result<(ImageFormat, &'static str)> {
    if contents.is_empty() || contents.len() > MAX_ENCODED_IMAGE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Composer image source size is invalid",
        ));
    }
    let format = image::guess_format(contents).map_err(std::io::Error::other)?;
    let extension = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Gif => "gif",
        ImageFormat::Tiff => "tiff",
        ImageFormat::WebP => "webp",
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "unsupported Composer image format",
            ));
        }
    };
    Ok((format, extension))
}

fn decode_image(contents: &[u8], format: ImageFormat) -> std::io::Result<DynamicImage> {
    let reader = image::ImageReader::with_format(Cursor::new(contents), format);
    let (width, height) = reader.into_dimensions().map_err(std::io::Error::other)?;
    validate_dimensions(width, height)?;
    image::ImageReader::with_format(Cursor::new(contents), format)
        .decode()
        .map_err(std::io::Error::other)
}

fn validate_dimensions(width: u32, height: u32) -> std::io::Result<()> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels == 0 || pixels > MAX_DECODED_IMAGE_PIXELS {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Composer image dimensions are invalid",
        ));
    }
    Ok(())
}
