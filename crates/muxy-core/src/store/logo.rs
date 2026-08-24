use crate::prefs::app_support_dir;
use std::path::{Path, PathBuf};

const OUTPUT_SIZE: u32 = 128;

pub fn logos_dir() -> PathBuf {
    app_support_dir().join("logos")
}

pub fn logo_path(filename: &str) -> Option<PathBuf> {
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
        return None;
    }
    if filename == "." || filename == ".." {
        return None;
    }
    Some(logos_dir().join(filename))
}

pub fn store(source: &Path, project_id: &str) -> Option<String> {
    let image = decode(source)?;
    let cropped = center_square(image).resize_exact(
        OUTPUT_SIZE,
        OUTPUT_SIZE,
        image::imageops::FilterType::Lanczos3,
    );

    let filename = format!("{project_id}.png");
    let destination = logo_path(&filename)?;
    std::fs::create_dir_all(logos_dir()).ok()?;
    cropped
        .save_with_format(&destination, image::ImageFormat::Png)
        .ok()?;
    Some(filename)
}

pub fn remove(project_id: &str) {
    let Ok(entries) = std::fs::read_dir(logos_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let stem = path.file_stem().and_then(|value| value.to_str());
        if stem == Some(project_id) {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn decode(source: &Path) -> Option<image::DynamicImage> {
    let is_svg = source
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("svg"))
        .unwrap_or(false);
    if is_svg {
        return render_svg(source);
    }
    image::open(source).ok()
}

fn render_svg(source: &Path) -> Option<image::DynamicImage> {
    let data = std::fs::read(source).ok()?;
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_data(&data, &options).ok()?;

    let size = tree.size().to_int_size();
    let scale = OUTPUT_SIZE as f32 / size.width().min(size.height()) as f32;
    let width = ((size.width() as f32 * scale).round() as u32).max(1);
    let height = ((size.height() as f32 * scale).round() as u32).max(1);

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let buffer = image::RgbaImage::from_raw(width, height, pixmap.take())?;
    Some(image::DynamicImage::ImageRgba8(buffer))
}

fn center_square(image: image::DynamicImage) -> image::DynamicImage {
    use image::GenericImageView;
    let (width, height) = image.dimensions();
    let side = width.min(height);
    let x = (width - side) / 2;
    let y = (height - side) / 2;
    image.crop_imm(x, y, side, side)
}

#[cfg(test)]
mod tests {
    use image::GenericImageView;

    #[test]
    fn renders_svg_sources() {
        let dir = std::env::temp_dir().join("muxy-logo-tests");
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("logo.svg");
        std::fs::write(
            &path,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"64\" height=\"32\"><rect width=\"64\" height=\"32\" fill=\"red\"/></svg>",
        )
        .expect("write svg");

        let image = super::decode(&path).expect("decoded svg");
        assert_eq!(image.dimensions(), (256, 128));

        let square = super::center_square(image);
        assert_eq!(square.dimensions(), (128, 128));

        std::fs::remove_file(&path).ok();
    }
}
