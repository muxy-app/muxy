pub use muxy_protocol::{
    CellSize, GraphicImage, GraphicPlacement, Graphics, MAX_GRAPHICS_BYTES, MAX_GRAPHICS_PLACEMENTS,
};

mod iterm;
mod sixel;
mod stream;

pub(crate) use stream::{Interceptor, Piece};

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use base64::Engine as _;
use image::RgbaImage;

use libghostty_vt::kitty::graphics::{self, DecodePng, DecodedImage, ImageFormat};
use libghostty_vt::render::{ImageIterator, ImageProtocol};
use libghostty_vt::terminal::{
    ConformanceLevel, DeviceAttributeFeature, DeviceAttributes, DeviceType,
    PrimaryDeviceAttributes, SecondaryDeviceAttributes, SizeReportSize, TertiaryDeviceAttributes,
};
use libghostty_vt::{
    RenderState, Terminal,
    alloc::{Allocator, Bytes},
};

type Result<T> = std::result::Result<T, libghostty_vt::Error>;

/// The character a program prints where an image with a virtual placement should appear.
pub(crate) const PLACEHOLDER: char = '\u{10EEEE}';
/// The widest or tallest image any decoder accepts.
const MAX_SIDE: u32 = 8192;
/// Memory a decoder may use for one image before it is shrunk to fit.
const MAX_DECODE_BYTES: u64 = 64 * 1024 * 1024;
/// Base64 bytes per Kitty command when showing a converted image.
const KITTY_CHUNK: usize = 4096;
/// IND: moves the cursor down a row, scrolling at the bottom, in its column.
const INDEX: &[u8] = b"\x1bD";

/// libghostty's default answers, plus Sixel, which is drawn as Kitty images.
const DEVICE_ATTRIBUTES: DeviceAttributes = DeviceAttributes {
    primary: PrimaryDeviceAttributes::new(
        ConformanceLevel::VT220,
        &[
            DeviceAttributeFeature::SIXEL,
            DeviceAttributeFeature::ANSI_COLOR,
        ],
    ),
    secondary: SecondaryDeviceAttributes {
        device_type: DeviceType::VT220,
        firmware_version: 0,
        rom_cartridge: 0,
    },
    tertiary: TertiaryDeviceAttributes { unit_id: 0 },
};

/// RGBA pixels decoded from a protocol libghostty doesn't implement.
#[derive(Debug)]
pub(crate) struct Picture {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

/// Cells a picture is stretched over; a missing side keeps the aspect ratio.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) columns: Option<u32>,
    pub(crate) rows: Option<u32>,
}

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
    // libghostty reads only regular files outside /proc, /sys and /dev, and
    // refuses FIFOs before opening them (libghostty-vt-sys
    // patches/kitty-image-file.patch). It deletes temporary files only inside a
    // temporary folder and named for the protocol, and unlinks shared memory
    // once read, as Kitty and Ghostty do.
    engine.set_kitty_image_from_file_allowed(true)?;
    let temporary = std::env::temp_dir();
    engine.set_kitty_image_temp_file_dir(temporary.to_str().map(Path::new))?;
    engine.set_kitty_image_from_shared_mem_allowed(true)?;
    engine.on_device_attributes(|_| Some(DEVICE_ATTRIBUTES))?;
    engine.on_size(|terminal| {
        let columns = terminal.cols().ok().filter(|n| *n > 0)?;
        let rows = terminal.rows().ok().filter(|n| *n > 0)?;
        Some(SizeReportSize {
            rows,
            columns,
            cell_width: terminal.width_px().ok()? / u32::from(columns),
            cell_height: terminal.height_px().ok()? / u32::from(rows),
        })
    })?;
    Ok(())
}

/// Shows a Sixel image at the cursor. As in xterm and foot, the cursor then
/// sits at the image's left edge, on the last row the image covers.
pub(crate) fn show_sixel(
    engine: &mut Terminal<'static, 'static>,
    cell: CellSize,
    params: &[u8],
    data: &[u8],
) {
    let background = engine
        .default_bg_color()
        .ok()
        .flatten()
        .map_or([0; 3], |color| [color.r, color.g, color.b]);
    let Some(picture) = sixel::decode(params, data, background) else {
        return;
    };
    let down = picture.height.div_ceil(u32::from(cell.height.max(1))) - 1;
    let Some((picture, placement)) = within_budget(picture, cell) else {
        return;
    };
    show(engine, &picture, &placement, false);
    engine.vt_write(&INDEX.repeat(down as usize));
}

/// Shrinks a picture over the image budget, placing it over the columns its
/// full size covers.
fn within_budget(picture: Picture, cell: CellSize) -> Option<(Picture, Placement)> {
    if picture.rgba.len() <= MAX_GRAPHICS_BYTES {
        return Some((picture, Placement::default()));
    }
    let columns = picture.width.div_ceil(u32::from(cell.width.max(1)));
    let image = fit(RgbaImage::from_raw(
        picture.width,
        picture.height,
        picture.rgba,
    )?);
    let picture = Picture {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    };
    let placement = Placement {
        columns: Some(columns),
        rows: None,
    };
    Some((picture, placement))
}

/// Shows an iTerm2 inline image at the cursor.
pub(crate) fn show_iterm(engine: &mut Terminal<'static, 'static>, cell: CellSize, file: &[u8]) {
    let Some(file) = iterm::parse(file) else {
        return;
    };
    let (Ok(columns), Ok(rows)) = (engine.cols(), engine.rows()) else {
        return;
    };
    let placement = file.placement(columns, rows, cell);
    show(engine, &file.picture, &placement, file.move_cursor);
}

/// Stores `picture` with Kitty commands and places it at the cursor.
fn show(
    engine: &mut Terminal<'static, 'static>,
    picture: &Picture,
    placement: &Placement,
    move_cursor: bool,
) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(&picture.rgba);
    let chunks = encoded.as_bytes().chunks(KITTY_CHUNK);
    let last = chunks.len().saturating_sub(1);
    let mut command = Vec::with_capacity(KITTY_CHUNK + 128);
    for (index, chunk) in chunks.enumerate() {
        command.clear();
        command.extend_from_slice(b"\x1b_G");
        if index == 0 {
            let columns = placement
                .columns
                .map(|columns| format!("c={columns},"))
                .unwrap_or_default();
            let rows = placement
                .rows
                .map(|rows| format!("r={rows},"))
                .unwrap_or_default();
            let keys = format!(
                "a=T,f=32,s={},v={},C={},{columns}{rows}",
                picture.width,
                picture.height,
                u8::from(!move_cursor)
            );
            command.extend_from_slice(keys.as_bytes());
        }
        command.extend_from_slice(format!("q=2,m={};", u8::from(index < last)).as_bytes());
        command.extend_from_slice(chunk);
        command.extend_from_slice(b"\x1b\\");
        engine.vt_write(&command);
    }
}

/// Decodes an image file, shrinking a picture over the image budget to fit
/// it, so large screenshots still show.
pub(crate) fn decode_image(mut reader: image::ImageReader<Cursor<&[u8]>>) -> Option<RgbaImage> {
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    reader.limits(limits);
    Some(fit(reader.decode().ok()?.into_rgba8()))
}

/// Shrinks `picture` to at most `MAX_GRAPHICS_BYTES`, keeping its aspect ratio.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn fit(picture: RgbaImage) -> RgbaImage {
    let pixels = u64::from(picture.width()) * u64::from(picture.height());
    let budget = (MAX_GRAPHICS_BYTES / 4) as u64;
    if pixels <= budget {
        return picture;
    }
    let scale = (budget as f64 / pixels as f64).sqrt();
    let width = ((f64::from(picture.width()) * scale) as u32).max(1);
    let height = ((f64::from(picture.height()) * scale) as u32).max(1);
    image::imageops::thumbnail(&picture, width, height)
}

/// Decodes Kitty `f=100` images. Kitty defines only PNG; as in some other
/// terminals, any format the server can read is accepted, found from the data.
struct PngDecoder;

impl DecodePng for PngDecoder {
    fn decode_png<'alloc>(
        &mut self,
        alloc: &'alloc Allocator<'_>,
        data: &[u8],
    ) -> Option<DecodedImage<'alloc>> {
        let reader = image::ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .ok()?;
        let decoded = decode_image(reader)?;
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
