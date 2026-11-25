#![cfg(feature = "kitty-graphics")]

use std::{
    ffi::OsStr,
    mem::{ManuallyDrop, MaybeUninit},
    path::Path,
    sync::{Mutex, PoisonError},
};

use crate::{
    Terminal,
    alloc::{Allocator, Bytes, Object, Ref},
    error::{Error, Result, from_optional_result_uninit, from_result},
    ffi,
    selection::Selection,
};

#[doc(inline)]
pub use ffi::KittyGraphicsPlacementRenderInfo as PlacementRenderInfo;

#[derive(Debug)]
pub struct Graphics<'t> {
    inner: Ref<'t, ffi::KittyGraphicsImpl>,
}

#[derive(Debug)]
pub struct Image<'t> {
    inner: Ref<'t, ffi::KittyGraphicsImageImpl>,
}

#[derive(Debug)]
pub struct PlacementIterator<'alloc> {
    inner: Object<'alloc, ffi::KittyGraphicsPlacementIteratorImpl>,
}

#[derive(Debug)]
pub struct PlacementIteration<'t, 'alloc>(&'t mut PlacementIterator<'alloc>);

impl Terminal<'_, '_> {
    pub fn kitty_graphics(&self) -> Result<Graphics<'_>> {
        let inner = self.get::<ffi::KittyGraphics>(ffi::TerminalData::KITTY_GRAPHICS)?;
        Ok(Graphics {
            inner: Ref::new(inner)?,
        })
    }

    pub fn kitty_graphics_animation_tick(&mut self, now_ms: u64) -> Result<Option<u64>> {
        let Some(graphics) =
            self.get_optional::<ffi::KittyGraphics>(ffi::TerminalData::KITTY_GRAPHICS)?
        else {
            return Ok(None);
        };
        let mut next_ms = MaybeUninit::<u64>::zeroed();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_animation_tick(graphics, now_ms, next_ms.as_mut_ptr())
        };
        from_optional_result_uninit(result, next_ms)
    }

    pub fn kitty_image_storage_limit(&self) -> Result<u64> {
        self.get(ffi::TerminalData::KITTY_IMAGE_STORAGE_LIMIT)
    }
    pub fn is_kitty_image_from_file_allowed(&self) -> Result<bool> {
        self.get(ffi::TerminalData::KITTY_IMAGE_MEDIUM_FILE)
    }
    pub fn kitty_image_temp_file_dir(&self) -> Result<Option<&Path>> {
        let str = self.get::<ffi::String>(ffi::TerminalData::KITTY_IMAGE_MEDIUM_TEMP_FILE)?;
        if str.len == 0 {
            return Ok(None);
        }
        let bytes = unsafe { std::slice::from_raw_parts(str.ptr, str.len) };
        let os_str = unsafe { OsStr::from_encoded_bytes_unchecked(bytes) };
        Ok(Some(Path::new(os_str)))
    }
    pub fn is_kitty_image_from_shared_mem_allowed(&self) -> Result<bool> {
        self.get(ffi::TerminalData::KITTY_IMAGE_MEDIUM_SHARED_MEM)
    }
    pub fn set_kitty_image_storage_limit(&mut self, limit: u64) -> Result<&mut Self> {
        self.set(ffi::TerminalOption::KITTY_IMAGE_STORAGE_LIMIT, &limit)?;
        Ok(self)
    }
    pub fn set_kitty_image_from_file_allowed(&mut self, allowed: bool) -> Result<&mut Self> {
        self.set(ffi::TerminalOption::KITTY_IMAGE_MEDIUM_FILE, &allowed)?;
        Ok(self)
    }
    pub fn set_kitty_image_temp_file_dir(&mut self, dir: Option<&Path>) -> Result<&mut Self> {
        let dir = dir
            .map(|dir| dir.to_str().ok_or(Error::InvalidValue))
            .transpose()?
            .map(ffi::String::from);
        self.set_optional(
            ffi::TerminalOption::KITTY_IMAGE_MEDIUM_TEMP_FILE,
            dir.as_ref(),
        )?;
        Ok(self)
    }
    pub fn set_kitty_image_from_shared_mem_allowed(&mut self, allowed: bool) -> Result<&mut Self> {
        self.set(ffi::TerminalOption::KITTY_IMAGE_MEDIUM_SHARED_MEM, &allowed)?;
        Ok(self)
    }

    pub fn set_apc_max_bytes_kitty(&mut self, max: Option<usize>) -> Result<&mut Self> {
        self.set_optional(ffi::TerminalOption::APC_MAX_BYTES_KITTY, max.as_ref())?;
        Ok(self)
    }
}

impl<'t> Graphics<'t> {
    fn get<T>(&self, tag: ffi::KittyGraphicsData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_get(self.inner.as_raw(), tag, value.as_mut_ptr().cast())
        };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }

    #[must_use]
    pub fn image(&self, id: u32) -> Option<Image<'t>> {
        let image = unsafe { ffi::ghostty_kitty_graphics_image(self.inner.as_raw(), id) };

        Some(Image {
            inner: Ref::new(image.cast_mut()).ok()?,
        })
    }

    pub fn generation(&self) -> Result<u64> {
        self.get(ffi::KittyGraphicsData::GENERATION)
    }
}

impl<'t> Image<'t> {
    fn get<T>(&self, tag: ffi::KittyGraphicsImageData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_image_get(
                self.inner.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }

    pub fn id(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::ID)
    }
    pub fn number(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::NUMBER)
    }
    pub fn width(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::WIDTH)
    }
    pub fn height(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::HEIGHT)
    }

    pub fn generation(&self) -> Result<u64> {
        self.get(ffi::KittyGraphicsImageData::GENERATION)
    }
    pub fn format(&self) -> Result<ImageFormat> {
        self.get::<ffi::KittyImageFormat::Type>(ffi::KittyGraphicsImageData::FORMAT)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }
    pub fn compression(&self) -> Result<Compression> {
        self.get::<ffi::KittyImageCompression::Type>(ffi::KittyGraphicsImageData::COMPRESSION)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }

    pub fn data(&self) -> Result<Option<&'t [u8]>> {
        let ptr = self.get::<*const u8>(ffi::KittyGraphicsImageData::DATA_PTR)?;
        if ptr.is_null() {
            return Ok(None);
        }

        let len = self.get::<usize>(ffi::KittyGraphicsImageData::DATA_LEN)?;

        Ok(Some(unsafe { std::slice::from_raw_parts(ptr, len) }))
    }

    pub fn frame_count(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::FRAME_COUNT)
    }
    pub fn animation_state(&self) -> Result<AnimationState> {
        self.get::<ffi::KittyAnimationState::Type>(ffi::KittyGraphicsImageData::ANIMATION_STATE)
            .and_then(|v| v.try_into().map_err(|_| Error::InvalidValue))
    }
    pub fn current_frame(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::CURRENT_FRAME)
    }
    pub fn loops(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsImageData::LOOPS)
    }

    pub fn frame(&self, number: u32) -> Result<Option<Frame<'t>>> {
        let mut frame = MaybeUninit::<ffi::KittyImageFrame>::zeroed();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_image_frame(self.inner.as_raw(), number, frame.as_mut_ptr())
        };
        let Some(frame) = from_optional_result_uninit(result, frame)? else {
            return Ok(None);
        };
        let data = if frame.data.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(frame.data, frame.data_len) }
        };
        Ok(Some(Frame {
            data,
            gap_ms: frame.gap_ms,
        }))
    }
}

impl<'alloc> PlacementIterator<'alloc> {
    pub fn new() -> Result<Self> {
        unsafe { Self::new_inner(std::ptr::null()) }
    }
    pub fn new_with_alloc<'ctx: 'alloc>(alloc: &'alloc Allocator<'ctx>) -> Result<Self> {
        unsafe { Self::new_inner(alloc.to_raw()) }
    }
    unsafe fn new_inner(alloc: *const ffi::Allocator) -> Result<Self> {
        let mut inner: ffi::KittyGraphicsPlacementIterator = std::ptr::null_mut();
        let result =
            unsafe { ffi::ghostty_kitty_graphics_placement_iterator_new(alloc, &raw mut inner) };
        from_result(result)?;
        Ok(Self {
            inner: Object::new(inner)?,
        })
    }

    pub fn update<'t>(
        &'t mut self,
        graphics: &'t Graphics<'_>,
    ) -> Result<PlacementIteration<'t, 'alloc>> {
        let result = unsafe {
            ffi::ghostty_kitty_graphics_get(
                graphics.inner.as_raw(),
                ffi::KittyGraphicsData::PLACEMENT_ITERATOR,
                (&raw mut self.inner).cast(),
            )
        };
        from_result(result)?;
        Ok(PlacementIteration(self))
    }
}

impl Drop for PlacementIterator<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::ghostty_kitty_graphics_placement_iterator_free(self.inner.as_raw());
        }
    }
}

impl<'t> PlacementIteration<'t, '_> {
    #[expect(
        clippy::should_implement_trait,
        reason = "a lending iterator: each placement borrows the iteration"
    )]
    pub fn next(&mut self) -> Option<&Self> {
        if unsafe { ffi::ghostty_kitty_graphics_placement_next(self.0.inner.as_raw()) } {
            Some(self)
        } else {
            None
        }
    }

    fn set<T>(
        &self,
        tag: ffi::KittyGraphicsPlacementIteratorOption::Type,
        value: &T,
    ) -> Result<()> {
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_iterator_set(
                self.0.inner.as_raw(),
                tag,
                std::ptr::from_ref(value).cast(),
            )
        };
        from_result(result)
    }
    fn get<T>(&self, tag: ffi::KittyGraphicsPlacementData::Type) -> Result<T> {
        let mut value = MaybeUninit::<T>::zeroed();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_get(
                self.0.inner.as_raw(),
                tag,
                value.as_mut_ptr().cast(),
            )
        };
        from_result(result)?;
        Ok(unsafe { value.assume_init() })
    }

    pub fn set_layer(&self, layer: Layer) -> Result<()> {
        self.set::<ffi::KittyPlacementLayer::Type>(
            ffi::KittyGraphicsPlacementIteratorOption::LAYER,
            &layer.into(),
        )
    }

    pub fn pixel_size(
        &self,
        image: &Image<'t>,
        terminal: &'t Terminal<'_, '_>,
    ) -> Result<PixelSize> {
        let mut size = PixelSize::default();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_pixel_size(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                terminal.inner.as_raw(),
                &raw mut size.width,
                &raw mut size.height,
            )
        };
        from_result(result)?;
        Ok(size)
    }

    pub fn grid_size(&self, image: &Image<'t>, terminal: &'t Terminal<'_, '_>) -> Result<GridSize> {
        let mut size = GridSize::default();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_grid_size(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                terminal.inner.as_raw(),
                &raw mut size.cols,
                &raw mut size.rows,
            )
        };
        from_result(result)?;
        Ok(size)
    }

    pub fn viewport_pos(
        &self,
        image: &Image<'t>,
        terminal: &'t Terminal<'_, '_>,
    ) -> Result<Option<ViewportPos>> {
        let mut pos = ViewportPos::default();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_viewport_pos(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                terminal.inner.as_raw(),
                &raw mut pos.col,
                &raw mut pos.row,
            )
        };
        from_optional_result_uninit(result, MaybeUninit::new(pos))
    }

    pub fn source_rect(&self, image: &Image<'t>) -> Result<SourceRect> {
        let mut rect = SourceRect::default();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_source_rect(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                &raw mut rect.x,
                &raw mut rect.y,
                &raw mut rect.width,
                &raw mut rect.height,
            )
        };
        from_result(result)?;
        Ok(rect)
    }

    pub fn rect(&self, image: &Image<'t>, terminal: &'t Terminal<'_, '_>) -> Result<Selection<'t>> {
        let mut sel = MaybeUninit::<ffi::Selection>::zeroed();
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_rect(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                terminal.inner.as_raw(),
                sel.as_mut_ptr(),
            )
        };
        from_result(result)?;
        Ok(unsafe { Selection::from_raw(sel.assume_init()) })
    }

    pub fn placement_render_info(
        &self,
        image: &Image<'t>,
        terminal: &'t Terminal<'_, '_>,
    ) -> Result<PlacementRenderInfo> {
        let mut info = ffi::sized!(PlacementRenderInfo);
        let result = unsafe {
            ffi::ghostty_kitty_graphics_placement_render_info(
                self.0.inner.as_raw(),
                image.inner.as_raw(),
                terminal.inner.as_raw(),
                &raw mut info,
            )
        };
        from_result(result)?;
        Ok(info)
    }

    pub fn image_id(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::IMAGE_ID)
    }
    pub fn placement_id(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::PLACEMENT_ID)
    }
    pub fn is_virtual(&self) -> Result<bool> {
        self.get(ffi::KittyGraphicsPlacementData::IS_VIRTUAL)
    }
    pub fn x_offset(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::X_OFFSET)
    }
    pub fn y_offset(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::Y_OFFSET)
    }
    pub fn source_x(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::SOURCE_X)
    }
    pub fn source_y(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::SOURCE_Y)
    }
    pub fn source_width(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::SOURCE_WIDTH)
    }
    pub fn source_height(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::SOURCE_HEIGHT)
    }
    pub fn columns(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::COLUMNS)
    }
    pub fn rows(&self) -> Result<u32> {
        self.get(ffi::KittyGraphicsPlacementData::ROWS)
    }
    pub fn z(&self) -> Result<i32> {
        self.get(ffi::KittyGraphicsPlacementData::Z)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GridSize {
    pub cols: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewportPos {
    pub col: i32,
    pub row: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, int_enum::IntEnum)]
#[repr(i32)]
pub enum Layer {
    #[default]
    All = ffi::KittyPlacementLayer::ALL,
    BelowBg = ffi::KittyPlacementLayer::BELOW_BG,
    BelowText = ffi::KittyPlacementLayer::BELOW_TEXT,
    AboveText = ffi::KittyPlacementLayer::ABOVE_TEXT,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[repr(i32)]
pub enum ImageFormat {
    #[default]
    Rgb = ffi::KittyImageFormat::RGB,
    Rgba = ffi::KittyImageFormat::RGBA,
    Png = ffi::KittyImageFormat::PNG,
    GrayAlpha = ffi::KittyImageFormat::GRAY_ALPHA,
    Gray = ffi::KittyImageFormat::GRAY,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[repr(i32)]
pub enum Compression {
    #[default]
    None = ffi::KittyImageCompression::NONE,
    ZlibDeflate = ffi::KittyImageCompression::ZLIB_DEFLATE,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, int_enum::IntEnum)]
#[non_exhaustive]
#[repr(i32)]
pub enum AnimationState {
    #[default]
    Stopped = ffi::KittyAnimationState::STOPPED,
    Loading = ffi::KittyAnimationState::LOADING,
    Running = ffi::KittyAnimationState::RUNNING,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame<'t> {
    pub data: &'t [u8],
    pub gap_ms: u32,
}

static DECODE_PNG: Mutex<Option<Box<dyn DecodePng>>> = Mutex::new(None);

pub unsafe fn set_png_decoder(f: Option<Box<dyn DecodePng>>) -> Result<()> {
    unsafe extern "C" fn callback(
        _userdata: *mut std::ffi::c_void,
        allocator: *const ffi::Allocator,
        data: *const u8,
        data_len: usize,
        out: *mut ffi::SysImage,
    ) -> bool {
        let mut decoder = DECODE_PNG.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(decoder) = decoder.as_deref_mut() else {
            return false;
        };
        let alloc = unsafe { Allocator::from_raw(allocator) };
        let data = unsafe { std::slice::from_raw_parts(data, data_len) };

        match decoder.decode_png(&alloc, data) {
            Some(result) if result.data.allocator() != alloc.to_raw() => false,
            Some(result) => {
                let mut result = ManuallyDrop::new(result);
                unsafe {
                    *out = ffi::SysImage {
                        width: result.width,
                        height: result.height,
                        data: result.data.as_mut_ptr(),
                        data_len: result.data.len(),
                    }
                };
                true
            }
            None => false,
        }
    }

    let ptr: ffi::SysDecodePngFn = match f {
        None => None,
        Some(_) => Some(callback),
    };
    let _old = std::mem::replace(
        &mut *DECODE_PNG.lock().unwrap_or_else(PoisonError::into_inner),
        f,
    );

    crate::sys_set(
        ffi::SysOption::DECODE_PNG,
        ptr.map_or(std::ptr::null(), |p| p as *const std::ffi::c_void),
    )
}

pub trait DecodePng: Send + 'static {
    fn decode_png<'alloc>(
        &mut self,
        alloc: &'alloc Allocator<'_>,
        data: &[u8],
    ) -> Option<DecodedImage<'alloc>>;
}

#[cfg(all(feature = "kitty-graphics", feature = "png"))]
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct RustPngDecoder;
#[cfg(all(feature = "kitty-graphics", feature = "png"))]
impl RustPngDecoder {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
#[cfg(all(feature = "kitty-graphics", feature = "png"))]
impl DecodePng for RustPngDecoder {
    fn decode_png<'alloc>(
        &mut self,
        alloc: &'alloc Allocator<'_>,
        data: &[u8],
    ) -> Option<DecodedImage<'alloc>> {
        use png::{BitDepth, ColorType, Decoder, Transformations};
        use std::io::Cursor;

        let mut decoder = Decoder::new(Cursor::new(data));

        decoder.set_transformations(Transformations::ALPHA | Transformations::STRIP_16);

        let mut reader = decoder.read_info().ok()?;
        let (width, height) = reader.info().size();
        let pixels = (width as usize).checked_mul(height as usize)?;
        let rgba_len = pixels.checked_mul(4)?;

        let mut bytes = Bytes::new_with_alloc(alloc, rgba_len).ok()?;

        let (decoded_len, gray_alpha) = match reader.output_color_type() {
            (ColorType::Rgba, BitDepth::Eight) => (rgba_len, false),
            (ColorType::GrayscaleAlpha, BitDepth::Eight) => (pixels.checked_mul(2)?, true),
            _ => return None,
        };
        if reader.output_buffer_size()? != decoded_len {
            return None;
        }

        let info = reader.next_frame(&mut bytes[..decoded_len]).ok()?;
        if info.buffer_size() != decoded_len {
            return None;
        }
        reader.finish().ok()?;

        if gray_alpha {
            for i in (0..pixels).rev() {
                let (luma, alpha) = (bytes[2 * i], bytes[2 * i + 1]);
                bytes[4 * i..4 * i + 4].copy_from_slice(&[luma, luma, luma, alpha]);
            }
        }

        Some(DecodedImage {
            width,
            height,
            data: bytes,
        })
    }
}

#[derive(Debug)]
pub struct DecodedImage<'alloc> {
    pub width: u32,
    pub height: u32,
    pub data: Bytes<'alloc>,
}

#[cfg(all(test, not(miri)))]
mod animation_tests {
    use super::{AnimationState, Frame};
    use crate::{Terminal, ffi};

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    fn terminal() -> Terminal<'static, 'static> {
        let mut terminal = Terminal::new(80, 24).unwrap();
        terminal
            .set_kitty_image_storage_limit(64 * 1024 * 1024)
            .unwrap();
        terminal
    }

    fn image(terminal: &Terminal<'_, '_>) -> ([u8; 4], u64) {
        let graphics = terminal.kitty_graphics().unwrap();
        let image = graphics.image(1).unwrap();
        let pixels = image.data().unwrap().unwrap();
        (pixels[..4].try_into().unwrap(), image.generation().unwrap())
    }

    #[test]
    fn nothing_playing_returns_none() {
        let mut terminal = terminal();

        assert_eq!(terminal.kitty_graphics_animation_tick(0).unwrap(), None);

        terminal.vt_write(b"\x1b_Ga=T,f=32,i=1,s=1,v=1,q=2;/wAA/w==\x1b\\");
        assert_eq!(terminal.kitty_graphics_animation_tick(0).unwrap(), None);
        assert_eq!(terminal.kitty_graphics_animation_tick(1000).unwrap(), None);
    }

    #[test]
    fn advances_one_frame_per_tick_and_reports_the_next_frame_time() {
        let mut terminal = terminal();

        terminal.vt_write(
            b"\x1b_Ga=T,f=32,i=1,s=1,v=1,q=2;/wAA/w==\x1b\\\
              \x1b_Ga=f,f=32,i=1,s=1,v=1,z=60,q=2;AAD//w==\x1b\\\
              \x1b_Ga=a,i=1,r=1,z=25,q=2\x1b\\\
              \x1b_Ga=a,i=1,s=3,q=2\x1b\\",
        );
        let (pixel, g0) = image(&terminal);
        assert_eq!(pixel, RED);

        let tick = |terminal: &mut Terminal<'_, '_>, now_ms| {
            terminal.kitty_graphics_animation_tick(now_ms).unwrap()
        };
        assert_eq!(tick(&mut terminal, 1000), Some(1025));
        assert_eq!(image(&terminal), (RED, g0));

        assert_eq!(tick(&mut terminal, 1010), Some(1025));
        assert_eq!(image(&terminal), (RED, g0));

        assert_eq!(tick(&mut terminal, 1025), Some(1085));
        let (pixel, g1) = image(&terminal);
        assert_eq!(pixel, BLUE);
        assert_ne!(g1, g0);

        assert_eq!(tick(&mut terminal, 5030), Some(5055));
        let (pixel, g2) = image(&terminal);
        assert_eq!(pixel, RED);
        assert_ne!(g2, g1);

        terminal.vt_write(b"\x1b_Ga=a,i=1,s=1,q=2\x1b\\");
        assert_eq!(tick(&mut terminal, 6000), None);
        assert_eq!(image(&terminal), (RED, g2));
    }

    #[test]
    fn reads_every_stored_frame_and_the_playback_state() {
        let mut terminal = Terminal::new(20, 5).unwrap();
        terminal.resize(20, 5, 10, 20).unwrap();

        terminal.vt_write(b"\x1b_Ga=T,i=1,f=32,s=1,v=1,q=2;/wAA/w==\x1b\\");
        terminal.vt_write(b"\x1b_Ga=f,i=1,f=32,s=1,v=1,z=100,q=2;AAD//w==\x1b\\");
        terminal.vt_write(b"\x1b_Ga=a,i=1,r=1,z=50,q=2\x1b\\");
        terminal.vt_write(b"\x1b_Ga=a,i=1,s=3,v=1,q=2\x1b\\");

        {
            let graphics = terminal.kitty_graphics().unwrap();
            let image = graphics.image(1).unwrap();
            assert_eq!(image.frame_count().unwrap(), 2);
            assert_eq!(
                image.frame(1).unwrap(),
                Some(Frame {
                    data: &RED,
                    gap_ms: 50
                })
            );
            assert_eq!(
                image.frame(2).unwrap(),
                Some(Frame {
                    data: &BLUE,
                    gap_ms: 100
                })
            );
            assert_eq!(image.frame(3).unwrap(), None);
            assert_eq!(image.frame(0).unwrap(), None);
            assert_eq!(image.animation_state().unwrap(), AnimationState::Running);
            assert_eq!(image.current_frame().unwrap(), 1);
            assert_eq!(image.loops().unwrap(), 0);
        }

        terminal.vt_write(b"\x1b_Ga=a,i=1,s=2,c=2,v=3,q=2\x1b\\");
        let graphics = terminal.kitty_graphics().unwrap();
        let image = graphics.image(1).unwrap();
        assert_eq!(image.animation_state().unwrap(), AnimationState::Loading);
        assert_eq!(image.current_frame().unwrap(), 2);
        assert_eq!(image.loops().unwrap(), 2);
        assert_eq!(image.frame(1).unwrap().unwrap().data, RED);
        assert_eq!(image.data().unwrap(), Some(&BLUE[..]));
        assert_eq!(image.frame_count().unwrap(), 2);
    }

    #[test]
    fn image_without_animation_is_one_stopped_frame() {
        let mut terminal = Terminal::new(20, 5).unwrap();
        terminal.resize(20, 5, 10, 20).unwrap();
        terminal.vt_write(b"\x1b_Ga=T,i=1,f=32,s=1,v=1,q=2;/wAA/w==\x1b\\");

        let graphics = terminal.kitty_graphics().unwrap();
        let image = graphics.image(1).unwrap();
        assert_eq!(image.frame_count().unwrap(), 1);
        assert_eq!(image.animation_state().unwrap(), AnimationState::Stopped);
        assert_eq!(image.current_frame().unwrap(), 1);
        assert_eq!(image.loops().unwrap(), 0);
        assert_eq!(
            image.frame(1).unwrap(),
            Some(Frame {
                data: &RED,
                gap_ms: 0
            })
        );
        assert_eq!(image.frame(2).unwrap(), None);
    }

    #[test]
    fn frame_rejects_null_arguments() {
        let mut terminal = Terminal::new(20, 5).unwrap();
        terminal.resize(20, 5, 10, 20).unwrap();
        terminal.vt_write(b"\x1b_Ga=T,i=1,f=32,s=1,v=1,q=2;/wAA/w==\x1b\\");
        let graphics = terminal.kitty_graphics().unwrap();
        let image = graphics.image(1).unwrap();

        let mut frame = ffi::KittyImageFrame::default();
        let (null_image, null_out) = unsafe {
            (
                ffi::ghostty_kitty_graphics_image_frame(std::ptr::null(), 1, &raw mut frame),
                ffi::ghostty_kitty_graphics_image_frame(
                    image.inner.as_raw(),
                    1,
                    std::ptr::null_mut(),
                ),
            )
        };
        assert_eq!(null_image, ffi::Result::INVALID_VALUE);
        assert_eq!(null_out, ffi::Result::INVALID_VALUE);
    }
}

#[cfg(all(test, not(miri), feature = "kitty-graphics", feature = "png"))]
mod png_decoder_tests {
    use super::*;
    use crate::alloc::testing::Capped;

    struct Png<'a> {
        width: u32,
        height: u32,
        color: png::ColorType,
        depth: png::BitDepth,
        palette: Option<&'a [u8]>,
        trns: Option<&'a [u8]>,
        pixels: &'a [u8],
    }

    impl Png<'_> {
        fn encode(&self) -> Vec<u8> {
            let mut out = Vec::new();
            let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
            encoder.set_color(self.color);
            encoder.set_depth(self.depth);
            if let Some(palette) = self.palette {
                encoder.set_palette(palette);
            }
            if let Some(trns) = self.trns {
                encoder.set_trns(trns);
            }
            encoder
                .write_header()
                .unwrap()
                .write_image_data(self.pixels)
                .unwrap();
            out
        }
    }

    fn png(color: png::ColorType, depth: png::BitDepth, pixels: &[u8]) -> Png<'_> {
        Png {
            width: 1,
            height: 1,
            color,
            depth,
            palette: None,
            trns: None,
            pixels,
        }
    }

    fn decode(encoded: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
        RustPngDecoder
            .decode_png(&Allocator::GLOBAL, encoded)
            .map(|image| (image.width, image.height, image.data.to_vec()))
    }

    #[test]
    fn decodes_every_color_type_to_rgba8() {
        use png::{BitDepth, ColorType};

        let cases: [(&str, Png<'_>, [u8; 4]); 8] = [
            (
                "rgba8",
                png(ColorType::Rgba, BitDepth::Eight, &[255, 0, 128, 7]),
                [255, 0, 128, 7],
            ),
            (
                "rgb8 gains opaque alpha",
                png(ColorType::Rgb, BitDepth::Eight, &[1, 2, 3]),
                [1, 2, 3, 255],
            ),
            (
                "rgba16 is stripped to 8 bits",
                png(
                    ColorType::Rgba,
                    BitDepth::Sixteen,
                    &[0xAB, 0xCD, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC],
                ),
                [0xAB, 0x12, 0x56, 0x9A],
            ),
            (
                "gray8",
                png(ColorType::Grayscale, BitDepth::Eight, &[128]),
                [128, 128, 128, 255],
            ),
            (
                "gray2 is expanded to 8 bits",
                png(ColorType::Grayscale, BitDepth::Two, &[0b1000_0000]),
                [0xAA, 0xAA, 0xAA, 255],
            ),
            (
                "gray+alpha8",
                png(ColorType::GrayscaleAlpha, BitDepth::Eight, &[64, 96]),
                [64, 64, 64, 96],
            ),
            (
                "palette",
                Png {
                    palette: Some(&[0, 0, 0, 10, 20, 30]),
                    ..png(ColorType::Indexed, BitDepth::Eight, &[1])
                },
                [10, 20, 30, 255],
            ),
            (
                "palette with tRNS",
                Png {
                    palette: Some(&[0, 0, 0, 10, 20, 30]),
                    trns: Some(&[255, 42]),
                    ..png(ColorType::Indexed, BitDepth::Eight, &[1])
                },
                [10, 20, 30, 42],
            ),
        ];

        for (name, image, expected) in cases {
            assert_eq!(
                decode(&image.encode()),
                Some((1, 1, expected.to_vec())),
                "{name}"
            );
        }
    }

    #[test]
    fn expands_multi_pixel_grayscale_alpha_in_place() {
        let pixels = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let image = Png {
            width: 3,
            height: 2,
            ..png(
                png::ColorType::GrayscaleAlpha,
                png::BitDepth::Eight,
                &pixels,
            )
        };
        let expected: Vec<u8> = pixels
            .chunks_exact(2)
            .flat_map(|ga| [ga[0], ga[0], ga[0], ga[1]])
            .collect();
        assert_eq!(decode(&image.encode()), Some((3, 2, expected)));
    }

    #[test]
    fn rejects_malformed_input() {
        let encoded = png(png::ColorType::Rgba, png::BitDepth::Eight, &[1, 2, 3, 4]).encode();
        assert_eq!(decode(&[]), None);
        assert_eq!(decode(b"definitely not a png"), None);
        assert_eq!(decode(&encoded[..encoded.len() / 2]), None);
    }

    #[test]
    fn oversized_dimensions_are_rejected_by_the_provided_allocator() {
        let mut encoded = png(png::ColorType::Rgba, png::BitDepth::Eight, &[0; 4]).encode();
        encoded[16..24].copy_from_slice(&[0, 0, 0x3E, 0x80, 0, 0, 0x3E, 0x80]);
        let crc = crc32(&encoded[12..29]);
        encoded[29..33].copy_from_slice(&crc.to_be_bytes());

        let capped = Capped::new(1024 * 1024);
        assert!(
            RustPngDecoder
                .decode_png(&capped.allocator(), &encoded)
                .is_none()
        );
        assert_eq!(capped.largest_request(), 16000 * 16000 * 4);
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 0 {
                    crc >> 1
                } else {
                    0xEDB8_8320 ^ (crc >> 1)
                };
            }
        }
        !crc
    }
}
