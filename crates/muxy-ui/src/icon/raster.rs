#[cfg(target_os = "macos")]
use super::Icon;
#[cfg(target_os = "macos")]
use super::sfsymbol;
use gpui::{App, Hsla, Pixels, RenderImage, Rgba, SharedString, px};
use image::{Frame, RgbaImage};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(target_os = "macos")]
const SEMIBOLD: f32 = 0.3;

#[cfg(target_os = "macos")]
const APPKIT_SYMBOL_CORRECTION: f32 = 0.97;

/// How much of each pixel a glyph covers, before it is tinted.
#[derive(Debug)]
pub(super) struct Mask {
    pub width: u32,
    pub height: u32,
    pub logical_width: f32,
    pub logical_height: f32,
    pub alpha: Vec<u8>,
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct MaskKey {
    symbol: SharedString,
    point_size: u32,
    weight: u32,
    scale: u32,
}

#[derive(PartialEq, Eq, Hash, Clone)]
struct TintKey {
    mask: MaskKey,
    color: u32,
}

thread_local! {
    static MASKS: RefCell<HashMap<MaskKey, Option<Arc<Mask>>>> = RefCell::new(HashMap::new());
    static TINTED: RefCell<HashMap<TintKey, Arc<RenderImage>>> = RefCell::new(HashMap::new());
}

#[derive(Debug)]
pub struct Glyph {
    pub image: Arc<RenderImage>,
    pub width: Pixels,
    pub height: Pixels,
}

#[cfg(target_os = "macos")]
pub fn tinted(icon: Icon, size: Pixels, color: Hsla, scale: f32) -> Option<Glyph> {
    tinted_symbol(&SharedString::from(icon.sf_symbol()), size, color, scale)
}

#[cfg(target_os = "macos")]
pub fn tinted_symbol(
    symbol: &SharedString,
    size: Pixels,
    color: Hsla,
    scale: f32,
) -> Option<Glyph> {
    tinted_symbol_weight(symbol, size, color, scale, SEMIBOLD)
}

#[cfg(target_os = "macos")]
pub fn tinted_symbol_weight(
    symbol: &SharedString,
    size: Pixels,
    color: Hsla,
    scale: f32,
    weight: f32,
) -> Option<Glyph> {
    let key = MaskKey {
        symbol: symbol.clone(),
        point_size: f32::from(size).to_bits(),
        weight: weight.to_bits(),
        scale: scale.to_bits(),
    };
    let tint = TintKey {
        mask: key.clone(),
        color: pack(color),
    };

    let cached = TINTED.with(|cache| cache.borrow().get(&tint).cloned());

    let mask = MASKS.with(|cache| {
        cache
            .borrow_mut()
            .entry(key.clone())
            .or_insert_with(|| {
                sfsymbol::rasterize(
                    &key.symbol,
                    f32::from(size) * APPKIT_SYMBOL_CORRECTION,
                    weight,
                    scale,
                )
                .map(Arc::new)
            })
            .clone()
    })?;

    let image = if let Some(image) = cached {
        image
    } else {
        let image = Arc::new(compose(&mask, color));
        TINTED.with(|cache| cache.borrow_mut().insert(tint, image.clone()));
        image
    };

    Some(Glyph {
        image,
        width: px(mask.logical_width),
        height: px(mask.logical_height),
    })
}

/// A template SVG tinted with `color`; `key` must change whenever the bytes do.
pub fn tinted_svg(
    key: &SharedString,
    bytes: &[u8],
    size: Pixels,
    color: Hsla,
    scale: f32,
    cx: &App,
) -> Option<Glyph> {
    let key = MaskKey {
        symbol: key.clone(),
        point_size: f32::from(size).to_bits(),
        weight: u32::MAX,
        scale: scale.to_bits(),
    };
    let tint = TintKey {
        mask: key.clone(),
        color: pack(color),
    };
    let mask = MASKS.with(|cache| {
        cache
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| rasterize_svg(bytes, f32::from(size), scale, cx).map(Arc::new))
            .clone()
    })?;
    let image = TINTED
        .with(|cache| cache.borrow().get(&tint).cloned())
        .unwrap_or_else(|| {
            let image = Arc::new(compose(&mask, color));
            TINTED.with(|cache| cache.borrow_mut().insert(tint, image.clone()));
            image
        });
    Some(Glyph {
        image,
        width: px(mask.logical_width),
        height: px(mask.logical_height),
    })
}

#[cfg(target_os = "macos")]
fn rasterize_svg(bytes: &[u8], point_size: f32, scale: f32, _cx: &App) -> Option<Mask> {
    sfsymbol::rasterize_svg(bytes, point_size, scale)
}

/// Rasterizes an SVG into an alpha mask that fits a `point_size` square. The
/// SVG is drawn as an image inside a document of that size, which fits it to
/// the square and keeps it from reading any other file.
#[cfg(not(target_os = "macos"))]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "The validated side is a small positive number of pixels."
)]
fn rasterize_svg(bytes: &[u8], point_size: f32, scale: f32, cx: &App) -> Option<Mask> {
    use std::fmt::Write as _;
    if !point_size.is_finite()
        || point_size <= 0.0
        || point_size > 256.0
        || !scale.is_finite()
        || !(0.5..=8.0).contains(&scale)
    {
        return None;
    }
    // Compressed SVG is not text, and is refused before it is expanded.
    std::str::from_utf8(bytes).ok()?;
    let side = (point_size * scale).round().max(1.0) as u32;
    let mut document = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{side}" height="{side}"><image width="{side}" height="{side}" href="data:image/svg+xml,"#
    );
    for byte in bytes {
        let _ = write!(document, "%{byte:02X}");
    }
    document.push_str(r#""/></svg>"#);
    let image = gpui::Image::from_bytes(gpui::ImageFormat::Svg, document.into_bytes())
        .to_image_data(cx.svg_renderer())
        .ok()?;
    let alpha: Vec<u8> = image
        .as_bytes(0)?
        .chunks_exact(4)
        .map(|pixel| pixel[3])
        .collect();
    // An SVG that could not be read draws nothing.
    (alpha.len() == (side * side) as usize && alpha.iter().any(|alpha| *alpha != 0)).then_some(
        Mask {
            width: side,
            height: side,
            logical_width: point_size,
            logical_height: point_size,
            alpha,
        },
    )
}

fn compose(mask: &Mask, color: Hsla) -> RenderImage {
    let rgba: Rgba = color.into();
    let (r, g, b) = (channel(rgba.r), channel(rgba.g), channel(rgba.b));
    let tint_alpha = rgba.a.clamp(0.0, 1.0);

    let mut buffer = RgbaImage::new(mask.width, mask.height);
    for (index, pixel) in buffer.pixels_mut().enumerate() {
        let coverage = (f32::from(mask.alpha[index]) / 255.0) * tint_alpha;
        pixel.0 = [b, g, r, channel(coverage)];
    }
    RenderImage::new([Frame::new(buffer)])
}

fn pack(color: Hsla) -> u32 {
    let rgba: Rgba = color.into();
    u32::from_be_bytes([
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b),
        channel(rgba.a),
    ])
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use gpui::{SharedString, TestAppContext, black, px};

    /// The rows of a 16 pixel square glyph that have ink in them.
    fn inked_rows(name: &str, svg: &str, cx: &mut TestAppContext) -> Option<Vec<usize>> {
        cx.update(|cx| {
            let key = SharedString::from(name.to_owned());
            let glyph = super::tinted_svg(&key, svg.as_bytes(), px(16.0), black(), 1.0, cx)?;
            let bytes = glyph.image.as_bytes(0)?;
            assert_eq!(bytes.len(), 16 * 16 * 4);
            Some(
                (0..16)
                    .filter(|row| {
                        bytes[row * 64..(row + 1) * 64]
                            .chunks_exact(4)
                            .any(|pixel| pixel[3] != 0)
                    })
                    .collect(),
            )
        })
    }

    #[gpui::test]
    fn an_svg_that_cannot_be_read_gives_no_glyph(cx: &mut TestAppContext) {
        assert_eq!(inked_rows("broken", "<svg", cx), None);
        assert_eq!(inked_rows("empty", "", cx), None);
        let compressed = cx.update(|cx| {
            let key = SharedString::from("compressed");
            super::tinted_svg(&key, &[0x1f, 0x8b, 8, 0], px(16.0), black(), 1.0, cx).is_none()
        });
        assert!(compressed);
    }

    #[gpui::test]
    fn an_svg_cannot_draw_another_file(cx: &mut TestAppContext) {
        let directory = std::env::temp_dir().join(format!("muxy-icon-{}", std::process::id()));
        let other = directory.join("other.svg");
        let written = std::fs::create_dir_all(&directory).is_ok()
            && std::fs::write(
                &other,
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect width="1" height="1"/></svg>"#,
            )
            .is_ok();
        assert!(written);
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><image width="1" height="1" href="{}"/></svg>"#,
            other.display()
        );
        // The renderer follows the reference when the SVG is the document.
        let followed = cx.update(|cx| {
            gpui::Image::from_bytes(gpui::ImageFormat::Svg, svg.clone().into_bytes())
                .to_image_data(cx.svg_renderer())
                .is_ok_and(|image| {
                    image
                        .as_bytes(0)
                        .is_some_and(|bytes| bytes.chunks_exact(4).any(|pixel| pixel[3] != 0))
                })
        });
        let drawn = inked_rows("reference", &svg, cx);
        let _ = std::fs::remove_dir_all(&directory);
        assert!(followed);
        assert_eq!(drawn, None);
    }
}
