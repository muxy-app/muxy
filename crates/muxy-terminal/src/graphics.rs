pub use muxy_protocol::{
    CellSize, GraphicImage, GraphicPlacement, Graphics, MAX_GRAPHICS_BYTES, MAX_GRAPHICS_PLACEMENTS,
};

use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::{Arc, OnceLock};

use libghostty_vt::kitty::graphics::{self, DecodePng, DecodedImage, ImageFormat};
use libghostty_vt::render::{ImageIterator, ImageProtocol};
use libghostty_vt::{
    RenderState, Terminal,
    alloc::{Allocator, Bytes},
};

type Result<T> = std::result::Result<T, libghostty_vt::Error>;

/// The character a program prints where an image with a virtual placement should appear.
pub(crate) const PLACEHOLDER: char = '\u{10EEEE}';

#[derive(Debug)]
pub(crate) struct Cache {
    images: BTreeMap<u32, GraphicImage>,
    placements: ImageIterator<'static>,
}

/// Must run before this process makes any other libghostty call.
pub(crate) fn install_png_decoder() -> Result<()> {
    static PNG_DECODER: OnceLock<Result<()>> = OnceLock::new();
    #[allow(unsafe_code)]
    // SAFETY: The decoder is process-global and unsynchronized. Every libghostty
    // call in this process goes through a terminal, and `Terminal::new` calls this
    // first. A concurrent first call waits on the lock until the install is done.
    *PNG_DECODER.get_or_init(|| unsafe { graphics::set_png_decoder(Some(Box::new(PngDecoder))) })
}

pub(crate) fn configure(engine: &mut Terminal<'_, '_>) -> Result<()> {
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
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            images: BTreeMap::new(),
            placements: ImageIterator::new()?,
        })
    }

    /// Lists placements from the render state, which also places images shown with placeholder cells.
    pub(crate) fn snapshot(
        &mut self,
        engine: &Terminal<'static, 'static>,
        render: &mut RenderState<'static>,
        cell: CellSize,
    ) -> Result<Graphics> {
        let storage = engine.kitty_graphics()?;
        let snapshot = render.update(engine)?;
        let mut iter = self.placements.update(&snapshot)?;
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
            if !matches!(placement.protocol(), Ok(ImageProtocol::Kitty)) {
                continue;
            }
            let id = placement.image_id()?;
            let Some(image) = storage.image(id) else {
                continue;
            };
            let generation = image.generation()?;
            if !used.contains(&id) {
                if self
                    .images
                    .get(&id)
                    .is_none_or(|cached| cached.generation != generation)
                {
                    let Some(rgba) = rgba(&image)? else {
                        continue;
                    };
                    self.images.insert(
                        id,
                        GraphicImage {
                            id,
                            generation,
                            width: image.width()?,
                            height: image.height()?,
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
            let size = [placement.pixel_width()?, placement.pixel_height()?];
            // Placeholder rows round their slice of the image, which can pass its edge.
            let cached = &self.images[&id];
            let x = placement.source_x()?.min(cached.width);
            let y = placement.source_y()?.min(cached.height);
            let source = [
                x,
                y,
                placement.source_width()?.min(cached.width - x),
                placement.source_height()?.min(cached.height - y),
            ];
            if size.contains(&0) || source[2] == 0 || source[3] == 0 {
                continue;
            }
            result.placements.push(GraphicPlacement {
                image: id,
                id: placement.kitty_placement_id()?.unwrap_or_default(),
                column: placement.viewport_col()?,
                row: placement.viewport_row()?,
                offset: [placement.cell_offset_x()?, placement.cell_offset_y()?],
                size,
                source,
                z: placement.z()?,
            });
        }
        self.images.retain(|id, _| used.contains(id));
        result.placements.sort_by_key(|p| (p.z, p.image, p.id));
        result.images.sort_by_key(|image| image.id);
        Ok(result)
    }
}

fn rgba(image: &graphics::Image<'_>) -> Result<Option<Arc<[u8]>>> {
    let Some(len) = u64::from(image.width()?)
        .checked_mul(u64::from(image.height()?))
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n <= MAX_GRAPHICS_BYTES)
    else {
        return Ok(None);
    };
    let Some(data) = image.data()? else {
        return Ok(None);
    };
    Ok(match image.format()? {
        ImageFormat::Rgba if data.len() == len => Some(data.into()),
        ImageFormat::Rgb if data.len() == len / 4 * 3 => Some(
            data.chunks_exact(3)
                .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
                .collect(),
        ),
        ImageFormat::Gray if data.len() == len / 4 => Some(
            data.iter()
                .flat_map(|&gray| [gray, gray, gray, 255])
                .collect(),
        ),
        ImageFormat::GrayAlpha if data.len() == len / 2 => Some(
            data.chunks_exact(2)
                .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
                .collect(),
        ),
        _ => None,
    })
}
