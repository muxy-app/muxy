//! Listing icons and screenshots shown in Settings → Extensions.

use std::io::Read;
use std::path::{Path, PathBuf};

/// How large a picture is drawn, which bounds how much of it stays decoded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Size {
    Icon,
    /// A screenshot on a grid card or in the gallery's thumbnail strip.
    Card,
    /// The screenshot the gallery shows large.
    Full,
}

impl Size {
    fn side(self) -> u32 {
        match self {
            Self::Icon => 128,
            Self::Card => 720,
            Self::Full => 1024,
        }
    }

    fn limit(self) -> u64 {
        match self {
            Self::Icon => 1024 * 1024,
            Self::Card | Self::Full => 8 * 1024 * 1024,
        }
    }
}

/// Where a listing picture is read from.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum Source {
    /// A marketplace URL, for extensions that aren't installed.
    Marketplace(String),
    /// A file inside an installed extension, already resolved within it.
    File(PathBuf),
}

/// The picture at `source`, scaled down to `size`, ready to draw.
pub(crate) fn load(source: &Source, size: Size) -> Result<gpui::RenderImage, String> {
    let bytes = match source {
        Source::Marketplace(url) => super::marketplace::picture(url, size.limit())?,
        Source::File(path) => read(path, size.limit())?,
    };
    render(&bytes, size)
}

fn read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("picture is too large".into());
    }
    Ok(bytes)
}

fn render(bytes: &[u8], size: Size) -> Result<gpui::RenderImage, String> {
    let image = crate::views::project_editor::logo::decode(bytes)?;
    let longest = size.side();
    let mut bgra = if image.width() > longest || image.height() > longest {
        image::DynamicImage::ImageRgba8(image)
            .thumbnail(longest, longest)
            .into_rgba8()
    } else {
        image
    };
    for pixel in bgra.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Ok(gpui::RenderImage::new([image::Frame::new(bgra)]))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "Tests fail immediately on fixture errors"
    )]
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbaImage::from_pixel(width, height, image::Rgba([255, 0, 0, 255]))
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    #[test]
    fn pictures_are_scaled_to_where_they_are_drawn() {
        let card = render(&png(1600, 1000), Size::Card).unwrap();
        assert_eq!((card.size(0).width.0, card.size(0).height.0), (720, 450));
        let icon = render(&png(32, 32), Size::Icon).unwrap();
        assert_eq!((icon.size(0).width.0, icon.size(0).height.0), (32, 32));
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect width="24" height="24" fill="red"/></svg>"#;
        let icon = render(svg, Size::Icon).unwrap();
        assert_eq!((icon.size(0).width.0, icon.size(0).height.0), (128, 128));
        assert!(render(b"not a picture", Size::Icon).is_err());
    }

    #[test]
    fn files_over_the_limit_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("icon.png");
        std::fs::write(&path, vec![0_u8; 1024 * 1024 + 1]).unwrap();
        assert_eq!(
            load(&Source::File(path), Size::Icon).unwrap_err(),
            "picture is too large"
        );
    }
}
