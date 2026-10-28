pub use muxy_protocol::{
    CellSize, GraphicImage, GraphicPlacement, Graphics, MAX_GRAPHICS_BYTES, MAX_GRAPHICS_PLACEMENTS,
};

use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::{Arc, OnceLock};

use libghostty_vt::kitty::graphics::{
    self, DecodePng, DecodedImage, ImageFormat, PlacementIterator,
};
use libghostty_vt::{
    Terminal,
    alloc::{Allocator, Bytes},
};

type Result<T> = std::result::Result<T, libghostty_vt::Error>;

#[derive(Debug, Default)]
pub(crate) struct Cache {
    images: BTreeMap<u32, GraphicImage>,
}

pub(crate) fn configure(engine: &mut Terminal<'_, '_>) -> Result<()> {
    static PNG_DECODER: OnceLock<Result<()>> = OnceLock::new();
    #[allow(unsafe_code)]
    // SAFETY: The decoder is process-global and unsynchronized. It is installed
    // once, while creating the first terminal, and never replaced while other
    // terminals parse images.
    let installed = *PNG_DECODER
        .get_or_init(|| unsafe { graphics::set_png_decoder(Some(Box::new(PngDecoder))) });
    installed?;
    engine.set_kitty_image_storage_limit(MAX_GRAPHICS_BYTES as u64)?;
    engine.set_kitty_image_from_file_allowed(false)?;
    engine.set_kitty_image_temp_file_dir(None)?;
    engine.set_kitty_image_from_shared_mem_allowed(false)?;
    Ok(())
}

struct PngDecoder;

impl DecodePng for PngDecoder {
    fn decode_png<'alloc>(
        &mut self,
        alloc: &'alloc Allocator<'_>,
        data: &[u8],
    ) -> Option<DecodedImage<'alloc>> {
        let mut reader =
            image::ImageReader::with_format(Cursor::new(data), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(MAX_GRAPHICS_BYTES as u64);
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        reader.limits(limits);
        let decoded = reader.decode().ok()?.into_rgba8();
        if decoded.len() > MAX_GRAPHICS_BYTES {
            return None;
        }
        let mut bytes = Bytes::new_with_alloc(alloc, decoded.len()).ok()?;
        bytes.copy_from_slice(decoded.as_raw());
        Some(DecodedImage {
            width: decoded.width(),
            height: decoded.height(),
            data: bytes,
        })
    }
}

impl Cache {
    pub(crate) fn snapshot(
        &mut self,
        engine: &Terminal<'_, '_>,
        cell: CellSize,
    ) -> Result<Graphics> {
        let storage = engine.kitty_graphics()?;
        let mut iterator = PlacementIterator::new()?;
        let mut iter = iterator.update(&storage)?;
        let mut result = Graphics {
            cell,
            ..Graphics::default()
        };
        let mut used = std::collections::BTreeSet::new();
        let mut total = 0;
        while let Some(placement) = iter.next() {
            if result.placements.len() >= MAX_GRAPHICS_PLACEMENTS {
                break;
            }
            let id = placement.image_id()?;
            let Some(image) = storage.image(id) else {
                continue;
            };
            let Some(pos) = placement.viewport_pos(&image, engine)? else {
                continue;
            };
            let generation = image.generation()?;
            if !used.contains(&id) {
                if self
                    .images
                    .get(&id)
                    .is_none_or(|cached| cached.generation != generation)
                {
                    let width = image.width()?;
                    let height = image.height()?;
                    let Some(len) = u64::from(width)
                        .checked_mul(u64::from(height))
                        .and_then(|n| n.checked_mul(4))
                        .and_then(|n| usize::try_from(n).ok())
                        .filter(|n| *n <= MAX_GRAPHICS_BYTES)
                    else {
                        continue;
                    };
                    let Some(data) = image.data()? else {
                        continue;
                    };
                    let rgba: Arc<[u8]> = match image.format()? {
                        ImageFormat::Rgba if data.len() == len => data.into(),
                        ImageFormat::Rgb if data.len() == len / 4 * 3 => data
                            .chunks_exact(3)
                            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
                            .collect(),
                        ImageFormat::Gray if data.len() == len / 4 => data
                            .iter()
                            .flat_map(|&gray| [gray, gray, gray, 255])
                            .collect(),
                        ImageFormat::GrayAlpha if data.len() == len / 2 => data
                            .chunks_exact(2)
                            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
                            .collect(),
                        _ => continue,
                    };
                    self.images.insert(
                        id,
                        GraphicImage {
                            id,
                            generation,
                            width,
                            height,
                            rgba,
                        },
                    );
                }
                let cached = &self.images[&id];
                if total + cached.rgba.len() > MAX_GRAPHICS_BYTES {
                    continue;
                }
                total += cached.rgba.len();
                result.images.push(cached.clone());
                used.insert(id);
            }
            let size = placement.pixel_size(&image, engine)?;
            let source = placement.source_rect(&image)?;
            if source.width == 0 || source.height == 0 || size.width == 0 || size.height == 0 {
                continue;
            }
            result.placements.push(GraphicPlacement {
                image: id,
                id: placement.placement_id()?,
                column: pos.col,
                row: pos.row,
                offset: [placement.x_offset()?, placement.y_offset()?],
                size: [size.width, size.height],
                source: [source.x, source.y, source.width, source.height],
                z: placement.z()?,
            });
        }
        self.images.retain(|id, _| used.contains(id));
        result.placements.sort_by_key(|p| (p.z, p.image, p.id));
        result.images.sort_by_key(|image| image.id);
        Ok(result)
    }
}
