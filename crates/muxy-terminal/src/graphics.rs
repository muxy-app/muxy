pub use muxy_protocol::{
    CellSize, GraphicFrame, GraphicImage, GraphicPlacement, Graphics, ImageFrame,
    MAX_GRAPHICS_BYTES, MAX_GRAPHICS_PLACEMENTS, Playback, PlaybackState,
};

mod film;
mod iterm;
mod sixel;
mod stream;

pub(crate) use stream::{Interceptor, Piece};

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

use base64::Engine as _;
use image::{DynamicImage, RgbaImage};
use muxy_protocol::MAX_GRAPHICS_FRAMES;

use film::Film;
use libghostty_vt::kitty::graphics::{
    self, AnimationState, DecodePng, DecodedImage, ImageFormat, PlacementIterator,
};
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
/// Each picture is shrunk to at most this, so two large pictures fit together
/// in one screen's list of images.
const PICTURE_BUDGET: usize = MAX_GRAPHICS_BYTES / 2;
/// Base64 bytes per Kitty command when showing a converted image.
const KITTY_CHUNK: usize = 4096;
/// Animations of the largest size that fit a terminal's image storage
/// together. One more makes libghostty drop the oldest image.
const STORED_FILMS: usize = 4;
/// libghostty keeps every frame of an animation in its image storage.
pub(crate) const STORAGE_LIMIT: usize = MAX_GRAPHICS_BYTES + STORED_FILMS * film::FILM_BUDGET;
/// Image numbers for the animations Muxy converts start here, out of the
/// way of the small numbers programs pick.
const FIRST_FILM_NUMBER: u32 = 1 << 31;

thread_local! {
    /// Animations decoded for libghostty during the write in progress on
    /// this thread. libghostty takes only a first frame from its decoder.
    static DECODED_FILMS: RefCell<Vec<Film>> = const { RefCell::new(Vec::new()) };
}
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

/// An image's size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Size {
    width: u32,
    height: u32,
}

/// Cells a picture is stretched over; a missing side keeps the aspect ratio.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Placement {
    pub(crate) columns: Option<u32>,
    pub(crate) rows: Option<u32>,
}

#[derive(Debug)]
pub(crate) struct Cache {
    images: BTreeMap<u32, Cached>,
    placements: ImageIterator<'static>,
    /// Whether an animation lists its frames. A saved screen keeps only the
    /// frame on show, so it doesn't read the others.
    films: bool,
}

#[derive(Debug)]
struct Cached {
    image: GraphicImage,
    /// The storage generation the image was read at.
    storage: u64,
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
    engine.set_kitty_image_storage_limit(STORAGE_LIMIT as u64)?;
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
    show(engine, &Film::still(picture), &placement, false);
    engine.vt_write(&INDEX.repeat(down as usize));
}

/// Shrinks a picture over the picture budget, placing it over the columns its
/// full size covers.
fn within_budget(picture: Picture, cell: CellSize) -> Option<(Picture, Placement)> {
    if picture.rgba.len() <= PICTURE_BUDGET {
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
    show(engine, &file.film, &placement, file.move_cursor);
}

/// Stores `film` with Kitty commands and places it at the cursor. An
/// animation plays without end, as in iTerm2.
fn show(
    engine: &mut Terminal<'static, 'static>,
    film: &Film,
    placement: &Placement,
    move_cursor: bool,
) {
    let Some(first) = film.frames.first() else {
        return;
    };
    let columns = placement
        .columns
        .map(|columns| format!("c={columns},"))
        .unwrap_or_default();
    let rows = placement
        .rows
        .map(|rows| format!("r={rows},"))
        .unwrap_or_default();
    let target = format!("I={}", film_number());
    let number = if film.animated() {
        format!("{target},")
    } else {
        String::new()
    };
    let keys = format!(
        "a=T,f=32,s={},v={},C={},{columns}{rows}{number}",
        film.width,
        film.height,
        u8::from(!move_cursor)
    );
    transmit(engine, &keys, &first.rgba);
    animate(engine, &target, film);
}

/// A number no other converted animation has, so the commands that add its
/// frames can name it, and reach nothing if its first frame was refused.
fn film_number() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    FIRST_FILM_NUMBER | NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Adds the frames after the first to the image `target` names, then plays
/// them.
fn animate(engine: &mut Terminal<'static, 'static>, target: &str, film: &Film) {
    let Some((first, rest)) = film.frames.split_first() else {
        return;
    };
    if rest.is_empty() {
        return;
    }
    for frame in rest {
        let keys = format!(
            "a=f,{target},f=32,s={},v={},z={},",
            film.width, film.height, frame.gap
        );
        transmit(engine, &keys, &frame.rgba);
    }
    let play = format!("\x1b_Ga=a,{target},r=1,z={},s=3,v=1,q=2\x1b\\", first.gap);
    engine.vt_write(play.as_bytes());
}

/// Writes one Kitty command that carries `pixels`, in chunks. `keys` ends
/// with a comma.
fn transmit(engine: &mut Terminal<'static, 'static>, keys: &str, pixels: &[u8]) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(pixels);
    let chunks = encoded.as_bytes().chunks(KITTY_CHUNK);
    let last = chunks.len().saturating_sub(1);
    let mut command = Vec::with_capacity(KITTY_CHUNK + keys.len() + 32);
    for (index, chunk) in chunks.enumerate() {
        command.clear();
        command.extend_from_slice(b"\x1b_G");
        if index == 0 {
            command.extend_from_slice(keys.as_bytes());
        }
        command.extend_from_slice(format!("q=2,m={};", u8::from(index < last)).as_bytes());
        command.extend_from_slice(chunk);
        command.extend_from_slice(b"\x1b\\");
        engine.vt_write(&command);
    }
}

/// Gives the animations libghostty's decoder met during the last write to
/// the images they belong to. The decoder isn't told which image it works
/// for, so the image is found by its first frame, and only once it is placed:
/// an animation sent now and placed by a later command stays a still image.
pub(crate) fn attach_films(engine: &mut Terminal<'static, 'static>) {
    for film in DECODED_FILMS.take() {
        if let Some(id) = decoded_image(engine, &film) {
            animate(engine, &format!("i={id}"), &film);
        }
    }
}

/// The newest placed still image holding the first frame of `film`.
fn decoded_image(engine: &Terminal<'static, 'static>, film: &Film) -> Option<u32> {
    let first = film.frames.first()?.rgba.as_slice();
    let storage = engine.kitty_graphics().ok()?;
    let mut iterator = PlacementIterator::new().ok()?;
    let mut placements = iterator.update(&storage).ok()?;
    let mut newest: Option<(u64, u32)> = None;
    while let Some(placement) = placements.next() {
        let id = placement.image_id().ok()?;
        let Some(image) = storage.image(id) else {
            continue;
        };
        if image.width().ok()? != film.width
            || image.height().ok()? != film.height
            || image.frame_count().ok()? != 1
            || image.data().ok()? != Some(first)
        {
            continue;
        }
        let generation = image.generation().ok()?;
        if newest.is_none_or(|(found, _)| generation > found) {
            newest = Some((generation, id));
        }
    }
    newest.map(|(_, id)| id)
}

/// Decodes an image file, shrinking a picture over the picture budget to fit
/// it, so large screenshots still show. The picture is shrunk in its decoded
/// pixel format, so only the shrunk copy is ever converted to RGBA.
pub(crate) fn decode_image(mut reader: image::ImageReader<Cursor<&[u8]>>) -> Option<RgbaImage> {
    reader.limits(decode_limits());
    let decoded = reader.decode().ok()?;
    let size = fitted(decoded.width(), decoded.height(), PICTURE_BUDGET);
    if size == (decoded.width(), decoded.height()) {
        return Some(decoded.into_rgba8());
    }
    Some(decoded.thumbnail_exact(size.0, size.1).into_rgba8())
}

/// What a decoder may allocate and the largest picture it reads.
fn decode_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits
}

/// Shrinks `picture` to the picture budget, keeping its aspect ratio.
fn fit(picture: RgbaImage) -> RgbaImage {
    let (width, height) = fitted(picture.width(), picture.height(), PICTURE_BUDGET);
    if (width, height) == picture.dimensions() {
        return picture;
    }
    image::imageops::thumbnail(&picture, width, height)
}

/// The size a `width` by `height` picture takes within `bytes` of RGBA.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn fitted(width: u32, height: u32, bytes: usize) -> (u32, u32) {
    let pixels = u64::from(width) * u64::from(height);
    let budget = (bytes / 4) as u64;
    if pixels <= budget {
        return (width, height);
    }
    let scale = (budget as f64 / pixels as f64).sqrt();
    (
        ((f64::from(width) * scale) as u32).max(1),
        ((f64::from(height) * scale) as u32).max(1),
    )
}

/// Decodes Kitty `f=100` images. Kitty defines only PNG; as in some other
/// terminals, any format the server can read is accepted, found from the data.
/// libghostty takes the first frame; an animation's other frames wait in
/// `DECODED_FILMS` for `attach_films`.
struct PngDecoder;

impl DecodePng for PngDecoder {
    fn decode_png<'alloc>(
        &mut self,
        alloc: &'alloc Allocator<'_>,
        data: &[u8],
    ) -> Option<DecodedImage<'alloc>> {
        let film = film::decode(data)?;
        let first = &film.frames.first()?.rgba;
        let mut bytes = Bytes::new_with_alloc(alloc, first.len()).ok()?;
        bytes.copy_from_slice(first);
        let decoded = DecodedImage {
            width: film.width,
            height: film.height,
            data: bytes,
        };
        if film.animated() {
            DECODED_FILMS.with_borrow_mut(|films| {
                let mut held = film.bytes() + films.iter().map(Film::bytes).sum::<usize>();
                while held > film::FILM_BUDGET && !films.is_empty() {
                    held -= films.remove(0).bytes();
                }
                films.push(film);
            });
        }
        Some(decoded)
    }
}

impl Cache {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            images: BTreeMap::new(),
            placements: ImageIterator::new()?,
            films: true,
        })
    }

    /// Lists every image as a still one from now on.
    pub(crate) fn stills_only(&mut self) {
        self.films = false;
        self.images.clear();
    }

    /// Lists placements from the render state, which also places images shown with placeholder cells.
    /// When the images on screen pass what one screen lists, the newest are kept, and only
    /// they are converted. One image larger than a whole screen's share is scaled down to it.
    pub(crate) fn snapshot(
        &mut self,
        engine: &Terminal<'static, 'static>,
        render: &mut RenderState<'static>,
        cell: CellSize,
    ) -> Result<Graphics> {
        let storage = engine.kitty_graphics()?;
        let stored = storage.generation()?;
        let snapshot = render.update(engine)?;
        let mut iter = self.placements.update(&snapshot)?;
        let mut found = Vec::new();
        let mut sizes = BTreeMap::new();
        while let Some(placement) = iter.next() {
            if found.len() >= MAX_GRAPHICS_PLACEMENTS {
                break;
            }
            if !matches!(placement.protocol(), Ok(ImageProtocol::Kitty)) {
                continue;
            }
            let id = placement.image_id()?;
            let Some(image) = storage.image(id) else {
                continue;
            };
            let full = Size {
                width: image.width()?,
                height: image.height()?,
            };
            let size = [placement.pixel_width()?, placement.pixel_height()?];
            // Placeholder rows round their slice of the image, which can pass its edge.
            let x = placement.source_x()?.min(full.width);
            let y = placement.source_y()?.min(full.height);
            let source = [
                x,
                y,
                placement.source_width()?.min(full.width - x),
                placement.source_height()?.min(full.height - y),
            ];
            if size.contains(&0) || source[2] == 0 || source[3] == 0 {
                continue;
            }
            sizes.insert(id, (image.generation()?, full));
            found.push((
                full,
                GraphicPlacement {
                    image: id,
                    id: placement.kitty_placement_id()?.unwrap_or_default(),
                    column: placement.viewport_col()?,
                    row: placement.viewport_row()?,
                    offset: [placement.cell_offset_x()?, placement.cell_offset_y()?],
                    size,
                    source,
                    z: placement.z()?,
                },
            ));
        }
        let listed = within_screen(&sizes);
        self.images.retain(|id, _| listed.contains(id));
        let mut images = Vec::new();
        for &id in &listed {
            let Some(image) = storage.image(id) else {
                continue;
            };
            if let Some(cached) = cached(&mut self.images, id, &image, stored, self.films)? {
                images.push(cached.clone());
            }
        }
        images.sort_by_key(|image| image.id);
        let mut placements: Vec<GraphicPlacement> = found
            .into_iter()
            .filter_map(|(full, mut placement)| {
                let image = images
                    .binary_search_by_key(&placement.image, |image| image.id)
                    .ok()
                    .map(|index| &images[index])?;
                placement.source = scaled_source(placement.source, full, image);
                Some(placement)
            })
            .collect();
        placements.sort_by_key(|p| (p.z, p.image, p.id));
        Ok(Graphics {
            cell,
            images,
            placements,
        })
    }

    /// The pixels of the frames of an image on screen that `keys` name, in
    /// that order, as far as one reply holds them. Frames that are gone are
    /// left out. A still image has one frame, and answers with the pixels it
    /// has now under their key, so an app that asks late still gets a picture.
    pub(crate) fn frames(
        &self,
        engine: &Terminal<'static, 'static>,
        id: u32,
        keys: &[u64],
    ) -> Result<Vec<ImageFrame>> {
        let Some(cached) = self.images.get(&id).map(|cached| &cached.image) else {
            return Ok(Vec::new());
        };
        let storage = engine.kitty_graphics()?;
        let mut frames = Vec::new();
        let mut total = 0;
        if cached.frames.is_empty() {
            return Ok(vec![ImageFrame {
                key: cached.generation,
                width: cached.width,
                height: cached.height,
                rgba: cached.rgba.clone(),
            }]);
        }
        let Some(image) = storage.image(id) else {
            return Ok(Vec::new());
        };
        for &key in keys {
            let Some(rgba) = frame_pixels(&image, cached, key)? else {
                continue;
            };
            if !frames.is_empty() && total + rgba.len() > MAX_GRAPHICS_BYTES {
                break;
            }
            total += rgba.len();
            frames.push(ImageFrame {
                key,
                width: cached.width,
                height: cached.height,
                rgba,
            });
        }
        Ok(frames)
    }
}

/// The image as the screen lists it, read again when the storage changed.
/// `None` for an image whose pixels can't be shown.
fn cached<'a>(
    images: &'a mut BTreeMap<u32, Cached>,
    id: u32,
    image: &graphics::Image<'_>,
    stored: u64,
    films: bool,
) -> Result<Option<&'a GraphicImage>> {
    if images
        .get(&id)
        .is_none_or(|cached| cached.storage != stored)
    {
        let generation = image.generation()?;
        let kept = images
            .get(&id)
            .filter(|cached| cached.image.generation == generation)
            .map(|cached| Stored {
                width: cached.image.width,
                height: cached.image.height,
                rgba: cached.image.rgba.clone(),
            });
        let Some(kept) = kept.map_or_else(|| stored_pixels(image), |kept| Ok(Some(kept)))? else {
            images.remove(&id);
            return Ok(None);
        };
        let whole = kept.width == image.width()? && kept.height == image.height()?;
        let (frames, playback) = if films && whole {
            animation(image)?
        } else {
            (Vec::new(), Playback::default())
        };
        images.insert(
            id,
            Cached {
                image: GraphicImage {
                    id,
                    generation,
                    width: kept.width,
                    height: kept.height,
                    rgba: kept.rgba,
                    frames,
                    playback,
                },
                storage: stored,
            },
        );
    }
    Ok(images.get(&id).map(|cached| &cached.image))
}

/// The frames of an animated image and how it plays. A still image has no
/// frames.
fn animation(image: &graphics::Image<'_>) -> Result<(Vec<GraphicFrame>, Playback)> {
    let count = image.frame_count()?;
    if count < 2 {
        return Ok((Vec::new(), Playback::default()));
    }
    let mut frames = Vec::new();
    for number in (1..=count).take(MAX_GRAPHICS_FRAMES) {
        let Some(frame) = image.frame(number)? else {
            break;
        };
        frames.push(GraphicFrame {
            key: fingerprint(frame.data),
            gap: frame.gap_ms,
        });
    }
    if frames.len() < 2 {
        return Ok((Vec::new(), Playback::default()));
    }
    let state = match image.animation_state()? {
        AnimationState::Running => PlaybackState::Running,
        AnimationState::Loading => PlaybackState::Loading,
        _ => PlaybackState::Stopped,
    };
    let shown = image.current_frame()?.saturating_sub(1);
    let playback = Playback {
        state,
        frame: if (shown as usize) < frames.len() {
            shown
        } else {
            0
        },
        loops: image.loops()?,
    };
    Ok((frames, playback))
}

/// The pixels of the frame `key` names, unless they changed since the image
/// was listed.
fn frame_pixels(
    image: &graphics::Image<'_>,
    listed: &GraphicImage,
    key: u64,
) -> Result<Option<Arc<[u8]>>> {
    let Some(index) = listed.frames.iter().position(|frame| frame.key == key) else {
        return Ok(None);
    };
    let Ok(number) = u32::try_from(index + 1) else {
        return Ok(None);
    };
    let Some(frame) = image.frame(number)? else {
        return Ok(None);
    };
    if fingerprint(frame.data) != key {
        return Ok(None);
    }
    // Only the first frame can be in a format other than RGBA.
    let format = if index == 0 {
        image.format()?
    } else {
        ImageFormat::Rgba
    };
    Ok(pixels(format, listed.width, listed.height, frame.data))
}

/// Tells frames apart by their pixels, so a frame keeps its key until a
/// program changes it. Not for untrusted comparisons.
fn fingerprint(data: &[u8]) -> u64 {
    const MULTIPLIER: u64 = 0x517c_c1b7_2722_0a95;
    let mix = |hash: u64, word: u64| (hash.rotate_left(5) ^ word).wrapping_mul(MULTIPLIER);
    let mut words = data.chunks_exact(8);
    let mut hash = mix(0, data.len() as u64);
    for word in &mut words {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(word);
        hash = mix(hash, u64::from_le_bytes(bytes));
    }
    for &byte in words.remainder() {
        hash = mix(hash, u64::from(byte));
    }
    hash
}

/// An image's pixels as a screen lists them: RGBA, scaled down to fit a
/// whole screen's share when larger.
struct Stored {
    width: u32,
    height: u32,
    rgba: Arc<[u8]>,
}

fn stored_pixels(image: &graphics::Image<'_>) -> Result<Option<Stored>> {
    let Some(data) = image.data()? else {
        return Ok(None);
    };
    let (format, width, height) = (image.format()?, image.width()?, image.height()?);
    let (fit_width, fit_height) = fitted(width, height, MAX_GRAPHICS_BYTES);
    if (fit_width, fit_height) == (width, height) {
        return Ok(pixels(format, width, height, data).map(|rgba| Stored {
            width,
            height,
            rgba,
        }));
    }
    Ok(
        shrunk(format, width, height, data, fit_width, fit_height).map(|image| Stored {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw().into(),
        }),
    )
}

/// Scales pixels in `format` straight to RGBA of the given size, without a
/// full-size RGBA copy first.
fn shrunk(
    format: ImageFormat,
    width: u32,
    height: u32,
    data: &[u8],
    to_width: u32,
    to_height: u32,
) -> Option<RgbaImage> {
    fn scale<P: image::Pixel<Subpixel = u8> + 'static>(
        width: u32,
        height: u32,
        data: &[u8],
        to_width: u32,
        to_height: u32,
    ) -> Option<image::ImageBuffer<P, Vec<u8>>> {
        let source = image::ImageBuffer::<P, &[u8]>::from_raw(width, height, data)?;
        Some(image::imageops::thumbnail(&source, to_width, to_height))
    }
    let image = match format {
        ImageFormat::Rgba => {
            DynamicImage::ImageRgba8(scale(width, height, data, to_width, to_height)?)
        }
        ImageFormat::Rgb => {
            DynamicImage::ImageRgb8(scale(width, height, data, to_width, to_height)?)
        }
        ImageFormat::Gray => {
            DynamicImage::ImageLuma8(scale(width, height, data, to_width, to_height)?)
        }
        ImageFormat::GrayAlpha => {
            DynamicImage::ImageLumaA8(scale(width, height, data, to_width, to_height)?)
        }
        _ => return None,
    };
    Some(image.into_rgba8())
}

/// The images to list, newest first, while their pixels fit one screen.
fn within_screen(sizes: &BTreeMap<u32, (u64, Size)>) -> BTreeSet<u32> {
    let mut newest: Vec<_> = sizes.iter().collect();
    newest.sort_by_key(|(_, (generation, _))| std::cmp::Reverse(*generation));
    let mut total = 0;
    let mut listed = BTreeSet::new();
    for (&id, (_, size)) in newest {
        let (width, height) = fitted(size.width, size.height, MAX_GRAPHICS_BYTES);
        let bytes = width as usize * height as usize * 4;
        if total + bytes > MAX_GRAPHICS_BYTES {
            continue;
        }
        total += bytes;
        listed.insert(id);
    }
    listed
}

/// The part of `image` a placement shows, given as a part of the image at
/// its `full` size, for an image that was scaled down.
fn scaled_source(source: [u32; 4], full: Size, image: &GraphicImage) -> [u32; 4] {
    if (image.width, image.height) == (full.width, full.height) {
        return source;
    }
    let [x, y, width, height] = source;
    let (x, x_end) = scaled_span(x, width, full.width, image.width);
    let (y, y_end) = scaled_span(y, height, full.height, image.height);
    [x, y, x_end - x, y_end - y]
}

/// A span starting at `start` of `length` along a side `from` long, on a side
/// `to` long: at least one pixel, inside the side.
#[allow(clippy::cast_possible_truncation)]
fn scaled_span(start: u32, length: u32, from: u32, to: u32) -> (u32, u32) {
    let scale = |value: u32, up: bool| {
        let product = u64::from(value) * u64::from(to);
        let divisor = u64::from(from.max(1));
        let scaled = if up {
            product.div_ceil(divisor)
        } else {
            product / divisor
        };
        scaled.min(u64::from(to)) as u32
    };
    let begin = scale(start, false).min(to.saturating_sub(1));
    let end = scale(start + length, true).max(begin + 1);
    (begin, end)
}

fn pixels(format: ImageFormat, width: u32, height: u32, data: &[u8]) -> Option<Arc<[u8]>> {
    let len = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| *n <= MAX_GRAPHICS_BYTES)?;
    let channels = match format {
        ImageFormat::Rgba => 4,
        ImageFormat::Rgb => 3,
        ImageFormat::GrayAlpha => 2,
        ImageFormat::Gray => 1,
        _ => return None,
    };
    if data.len() != len / 4 * channels {
        return None;
    }
    if channels == 4 {
        return Some(data.into());
    }
    let mut rgba = vec![255; len];
    for (to, from) in rgba.chunks_exact_mut(4).zip(data.chunks_exact(channels)) {
        match *from {
            [red, green, blue] => to[..3].copy_from_slice(&[red, green, blue]),
            [gray, alpha] => to.copy_from_slice(&[gray, gray, gray, alpha]),
            [gray] => to[..3].fill(gray),
            _ => {}
        }
    }
    Some(rgba.into())
}
