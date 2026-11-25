mod app_menu;
mod keyboard;
mod keystroke;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
mod linux;

#[cfg(target_os = "macos")]
mod mac;

#[cfg(any(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        any(feature = "x11", feature = "wayland")
    ),
    all(target_os = "macos", feature = "macos-blade")
))]
mod blade;

#[cfg(any(test, feature = "test-support"))]
mod test;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(all(
    feature = "screen-capture",
    any(
        target_os = "windows",
        all(
            any(target_os = "linux", target_os = "freebsd"),
            any(feature = "wayland", feature = "x11"),
        )
    )
))]
pub(crate) mod scap_screen_capture;

use crate::{
    Action, AnyWindowHandle, App, AsyncWindowContext, BackgroundExecutor, Bounds,
    DEFAULT_WINDOW_SIZE, DevicePixels, DispatchEventResult, FocusId, Font, FontId, FontMetrics,
    FontRun, ForegroundExecutor, GlyphId, GpuSpecs, ImageSource, Keymap, LineLayout, Pixels,
    PlatformInput, Point, RenderGlyphParams, RenderImage, RenderImageParams, RenderSvgParams,
    Scene, ShapedGlyph, ShapedRun, SharedString, Size, SvgRenderer, SvgSize, SystemWindowTab, Task,
    TaskLabel, Window, WindowControlArea, hash, point, px, size,
};
use anyhow::Result;
use async_task::Runnable;
use futures::channel::oneshot;
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder as _, Frame};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use schemars::JsonSchema;
use seahash::SeaHasher;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::ops;
use std::time::{Duration, Instant};
use std::{
    fmt::{self, Debug},
    ops::Range,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};
use strum::EnumIter;
use uuid::Uuid;

pub use app_menu::*;
pub use keyboard::*;
pub use keystroke::*;

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) use linux::*;
#[cfg(target_os = "macos")]
pub use mac::keystroke_from_native_event;
#[cfg(target_os = "macos")]
pub(crate) use mac::*;
pub use semantic_version::SemanticVersion;
#[cfg(any(test, feature = "test-support"))]
pub(crate) use test::*;
#[cfg(target_os = "windows")]
pub(crate) use windows::*;

#[cfg(any(test, feature = "test-support"))]
pub use test::{TestDispatcher, TestScreenCaptureSource, TestScreenCaptureStream};

pub fn background_executor() -> BackgroundExecutor {
    current_platform(true).background_executor()
}

#[cfg(target_os = "macos")]
pub(crate) fn current_platform(headless: bool) -> Rc<dyn Platform> {
    Rc::new(MacPlatform::new(headless))
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) fn current_platform(headless: bool) -> Rc<dyn Platform> {
    #[cfg(feature = "x11")]
    use anyhow::Context as _;

    if headless {
        return Rc::new(HeadlessClient::new());
    }

    match guess_compositor() {
        #[cfg(feature = "wayland")]
        "Wayland" => Rc::new(WaylandClient::new()),

        #[cfg(feature = "x11")]
        "X11" => Rc::new(
            X11Client::new()
                .context("Failed to initialize X11 client.")
                .unwrap(),
        ),

        "Headless" => Rc::new(HeadlessClient::new()),
        _ => unreachable!(),
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[inline]
pub fn guess_compositor() -> &'static str {
    if std::env::var_os("ZED_HEADLESS").is_some() {
        return "Headless";
    }

    #[cfg(feature = "wayland")]
    let wayland_display = std::env::var_os("WAYLAND_DISPLAY");
    #[cfg(not(feature = "wayland"))]
    let wayland_display: Option<std::ffi::OsString> = None;

    #[cfg(feature = "x11")]
    let x11_display = std::env::var_os("DISPLAY");
    #[cfg(not(feature = "x11"))]
    let x11_display: Option<std::ffi::OsString> = None;

    let use_wayland = wayland_display.is_some_and(|display| !display.is_empty());
    let use_x11 = x11_display.is_some_and(|display| !display.is_empty());

    if use_wayland {
        "Wayland"
    } else if use_x11 {
        "X11"
    } else {
        "Headless"
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn current_platform(_headless: bool) -> Rc<dyn Platform> {
    Rc::new(
        WindowsPlatform::new()
            .inspect_err(|err| show_error("Failed to launch", err.to_string()))
            .unwrap(),
    )
}

pub(crate) trait Platform: 'static {
    fn background_executor(&self) -> BackgroundExecutor;
    fn foreground_executor(&self) -> ForegroundExecutor;
    fn text_system(&self) -> Arc<dyn PlatformTextSystem>;

    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>);
    fn quit(&self);
    fn restart(&self, binary_path: Option<PathBuf>);
    fn activate(&self, ignoring_other_apps: bool);
    fn hide(&self);
    fn hide_other_apps(&self);
    fn unhide_other_apps(&self);

    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>>;
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>>;
    fn active_window(&self) -> Option<AnyWindowHandle>;
    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
        None
    }

    #[cfg(feature = "screen-capture")]
    fn is_screen_capture_supported(&self) -> bool;
    #[cfg(not(feature = "screen-capture"))]
    fn is_screen_capture_supported(&self) -> bool {
        false
    }
    #[cfg(feature = "screen-capture")]
    fn screen_capture_sources(&self)
    -> oneshot::Receiver<Result<Vec<Rc<dyn ScreenCaptureSource>>>>;
    #[cfg(not(feature = "screen-capture"))]
    fn screen_capture_sources(
        &self,
    ) -> oneshot::Receiver<anyhow::Result<Vec<Rc<dyn ScreenCaptureSource>>>> {
        let (sources_tx, sources_rx) = oneshot::channel();
        sources_tx
            .send(Err(anyhow::anyhow!(
                "gpui was compiled without the screen-capture feature"
            )))
            .ok();
        sources_rx
    }

    fn open_window(
        &self,
        handle: AnyWindowHandle,
        options: WindowParams,
    ) -> anyhow::Result<Box<dyn PlatformWindow>>;

    fn window_appearance(&self) -> WindowAppearance;

    fn open_url(&self, url: &str);
    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>);
    fn register_url_scheme(&self, url: &str) -> Task<Result<()>>;

    fn prompt_for_paths(
        &self,
        options: PathPromptOptions,
    ) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>>;
    fn prompt_for_new_path(
        &self,
        directory: &Path,
        suggested_name: Option<&str>,
    ) -> oneshot::Receiver<Result<Option<PathBuf>>>;
    fn can_select_mixed_files_and_dirs(&self) -> bool;
    fn reveal_path(&self, path: &Path);
    fn open_with_system(&self, path: &Path);

    fn on_quit(&self, callback: Box<dyn FnMut()>);
    fn on_reopen(&self, callback: Box<dyn FnMut()>);

    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap);
    fn get_menus(&self) -> Option<Vec<OwnedMenu>> {
        None
    }

    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap);
    fn perform_dock_menu_action(&self, _action: usize) {}
    fn add_recent_document(&self, _path: &Path) {}
    fn update_jump_list(
        &self,
        _menus: Vec<MenuItem>,
        _entries: Vec<SmallVec<[PathBuf; 2]>>,
    ) -> Vec<SmallVec<[PathBuf; 2]>> {
        Vec::new()
    }
    fn on_app_menu_action(&self, callback: Box<dyn FnMut(&dyn Action)>);
    fn on_will_open_app_menu(&self, callback: Box<dyn FnMut()>);
    fn on_validate_app_menu_command(&self, callback: Box<dyn FnMut(&dyn Action) -> bool>);

    fn compositor_name(&self) -> &'static str {
        ""
    }
    fn app_path(&self) -> Result<PathBuf>;
    fn path_for_auxiliary_executable(&self, name: &str) -> Result<PathBuf>;

    fn set_cursor_style(&self, style: CursorStyle);
    fn should_auto_hide_scrollbars(&self) -> bool;

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn write_to_primary(&self, item: ClipboardItem);
    fn write_to_clipboard(&self, item: ClipboardItem);
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    fn read_from_primary(&self) -> Option<ClipboardItem>;
    fn read_from_clipboard(&self) -> Option<ClipboardItem>;

    fn write_credentials(&self, url: &str, username: &str, password: &[u8]) -> Task<Result<()>>;
    fn read_credentials(&self, url: &str) -> Task<Result<Option<(String, Vec<u8>)>>>;
    fn delete_credentials(&self, url: &str) -> Task<Result<()>>;

    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout>;
    fn keyboard_mapper(&self) -> Rc<dyn PlatformKeyboardMapper>;
    fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>);
}

pub trait PlatformDisplay: Send + Sync + Debug {
    fn id(&self) -> DisplayId;

    fn uuid(&self) -> Result<Uuid>;

    fn bounds(&self) -> Bounds<Pixels>;

    fn default_bounds(&self) -> Bounds<Pixels> {
        let bounds = self.bounds();
        let center = bounds.center();
        let clipped_window_size = DEFAULT_WINDOW_SIZE.min(&bounds.size);

        let offset = clipped_window_size / 2.0;
        let origin = point(center.x - offset.width, center.y - offset.height);
        Bounds::new(origin, clipped_window_size)
    }
}

#[derive(Clone)]
pub struct SourceMetadata {
    pub id: u64,
    pub label: Option<SharedString>,
    pub is_main: Option<bool>,
    pub resolution: Size<DevicePixels>,
}

pub trait ScreenCaptureSource {
    fn metadata(&self) -> Result<SourceMetadata>;

    fn stream(
        &self,
        foreground_executor: &ForegroundExecutor,
        frame_callback: Box<dyn Fn(ScreenCaptureFrame) + Send>,
    ) -> oneshot::Receiver<Result<Box<dyn ScreenCaptureStream>>>;
}

pub trait ScreenCaptureStream {
    fn metadata(&self) -> Result<SourceMetadata>;
}

pub struct ScreenCaptureFrame(pub PlatformScreenCaptureFrame);

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct DisplayId(pub(crate) u32);

impl From<DisplayId> for u32 {
    fn from(id: DisplayId) -> Self {
        id.0
    }
}

impl Debug for DisplayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DisplayId({})", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    TopLeft,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub enum WindowDecorations {
    #[default]
    Server,
    Client,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub enum Decorations {
    #[default]
    Server,
    Client {
        tiling: Tiling,
    },
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct WindowControls {
    pub fullscreen: bool,
    pub maximize: bool,
    pub minimize: bool,
    pub window_menu: bool,
}

impl Default for WindowControls {
    fn default() -> Self {
        Self {
            fullscreen: true,
            maximize: true,
            minimize: true,
            window_menu: true,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub struct Tiling {
    pub top: bool,
    pub left: bool,
    pub right: bool,
    pub bottom: bool,
}

impl Tiling {
    pub fn tiled() -> Self {
        Self {
            top: true,
            left: true,
            right: true,
            bottom: true,
        }
    }

    pub fn is_tiled(&self) -> bool {
        self.top || self.left || self.right || self.bottom
    }
}

pub(crate) struct A11yCallbacks {
    pub(crate) activation: Box<dyn Fn() -> Option<accesskit::TreeUpdate> + Send + 'static>,
    pub(crate) action: Box<dyn Fn(accesskit::ActionRequest) + Send + 'static>,
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub(crate) deactivation: Box<dyn Fn() + Send + 'static>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Default)]
pub(crate) struct RequestFrameOptions {
    pub(crate) require_presentation: bool,
    pub(crate) force_render: bool,
}

pub(crate) trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    fn bounds(&self) -> Bounds<Pixels>;
    fn is_maximized(&self) -> bool;
    fn window_bounds(&self) -> WindowBounds;
    fn content_size(&self) -> Size<Pixels>;
    fn resize(&mut self, size: Size<Pixels>);
    fn scale_factor(&self) -> f32;
    fn appearance(&self) -> WindowAppearance;
    fn display(&self) -> Option<Rc<dyn PlatformDisplay>>;
    fn mouse_position(&self) -> Point<Pixels>;
    fn modifiers(&self) -> Modifiers;
    fn capslock(&self) -> Capslock;
    fn set_input_handler(&mut self, input_handler: PlatformInputHandler);
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler>;
    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>>;
    fn activate(&self);
    fn is_active(&self) -> bool;
    fn is_hovered(&self) -> bool;
    fn set_title(&mut self, title: &str);
    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance);
    fn minimize(&self);
    fn zoom(&self);
    fn toggle_fullscreen(&self);
    fn is_fullscreen(&self) -> bool;
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>);
    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>);
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>);
    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>);
    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>);
    fn on_moved(&self, callback: Box<dyn FnMut()>);
    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>);
    fn on_hit_test_window_control(&self, callback: Box<dyn FnMut() -> Option<WindowControlArea>>);
    fn on_close(&self, callback: Box<dyn FnOnce()>);
    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>);
    fn draw(&self, scene: &Scene);
    fn completed_frame(&self) {}
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;

    fn get_title(&self) -> String {
        String::new()
    }
    fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        None
    }
    fn tab_bar_visible(&self) -> bool {
        false
    }
    fn set_edited(&mut self, _edited: bool) {}
    fn show_character_palette(&self) {}
    fn titlebar_double_click(&self) {}
    fn on_move_tab_to_new_window(&self, _callback: Box<dyn FnMut()>) {}
    fn on_merge_all_windows(&self, _callback: Box<dyn FnMut()>) {}
    fn on_select_previous_tab(&self, _callback: Box<dyn FnMut()>) {}
    fn on_select_next_tab(&self, _callback: Box<dyn FnMut()>) {}
    fn on_toggle_tab_bar(&self, _callback: Box<dyn FnMut()>) {}
    fn merge_all_windows(&self) {}
    fn move_tab_to_new_window(&self) {}
    fn toggle_window_tab_overview(&self) {}
    fn set_tabbing_identifier(&self, _identifier: Option<String>) {}

    #[cfg(target_os = "windows")]
    fn get_raw_handle(&self) -> windows::HWND;

    fn inner_window_bounds(&self) -> WindowBounds {
        self.window_bounds()
    }
    fn request_decorations(&self, _decorations: WindowDecorations) {}
    fn show_window_menu(&self, _position: Point<Pixels>) {}
    fn set_traffic_light_position(&self, _position: Point<Pixels>) {}
    fn start_window_move(&self) {}
    fn start_window_resize(&self, _edge: ResizeEdge) {}
    fn window_decorations(&self) -> Decorations {
        Decorations::Server
    }
    fn set_app_id(&mut self, _app_id: &str) {}
    fn map_window(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
    fn window_controls(&self) -> WindowControls {
        WindowControls::default()
    }
    fn set_client_inset(&self, _inset: Pixels) {}
    fn gpu_specs(&self) -> Option<GpuSpecs>;

    fn update_ime_position(&self, _bounds: Bounds<Pixels>);

    fn a11y_init(&self, _callbacks: A11yCallbacks) {}

    fn a11y_tree_update(&self, _tree_update: accesskit::TreeUpdate) {}

    fn a11y_update_window_bounds(&self) {}

    #[cfg(any(test, feature = "test-support"))]
    fn as_test(&mut self) -> Option<&mut TestWindow> {
        None
    }
}

#[doc(hidden)]
pub trait PlatformDispatcher: Send + Sync {
    fn is_main_thread(&self) -> bool;
    fn dispatch(&self, runnable: Runnable, label: Option<TaskLabel>);
    fn dispatch_on_main_thread(&self, runnable: Runnable);
    fn dispatch_after(&self, duration: Duration, runnable: Runnable);
    fn now(&self) -> Instant {
        Instant::now()
    }

    #[cfg(any(test, feature = "test-support"))]
    fn as_test(&self) -> Option<&TestDispatcher> {
        None
    }
}

pub(crate) trait PlatformTextSystem: Send + Sync {
    fn add_fonts(&self, fonts: Vec<Cow<'static, [u8]>>) -> Result<()>;
    fn all_font_names(&self) -> Vec<String>;
    fn font_id(&self, descriptor: &Font) -> Result<FontId>;
    fn font_metrics(&self, font_id: FontId) -> FontMetrics;
    fn typographic_bounds(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Bounds<f32>>;
    fn advance(&self, font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>>;
    fn glyph_for_char(&self, font_id: FontId, ch: char) -> Option<GlyphId>;
    fn glyph_raster_bounds(&self, params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>>;
    fn rasterize_glyph(
        &self,
        params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)>;
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout;
}

pub(crate) struct NoopTextSystem;

impl NoopTextSystem {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self
    }
}

impl PlatformTextSystem for NoopTextSystem {
    fn add_fonts(&self, _fonts: Vec<Cow<'static, [u8]>>) -> Result<()> {
        Ok(())
    }

    fn all_font_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn font_id(&self, _descriptor: &Font) -> Result<FontId> {
        Ok(FontId(1))
    }

    fn font_metrics(&self, _font_id: FontId) -> FontMetrics {
        FontMetrics {
            units_per_em: 1000,
            ascent: 1025.0,
            descent: -275.0,
            line_gap: 0.0,
            underline_position: -95.0,
            underline_thickness: 60.0,
            cap_height: 698.0,
            x_height: 516.0,
            bounding_box: Bounds {
                origin: Point {
                    x: -260.0,
                    y: -245.0,
                },
                size: Size {
                    width: 1501.0,
                    height: 1364.0,
                },
            },
        }
    }

    fn typographic_bounds(&self, _font_id: FontId, _glyph_id: GlyphId) -> Result<Bounds<f32>> {
        Ok(Bounds {
            origin: Point { x: 54.0, y: 0.0 },
            size: size(392.0, 528.0),
        })
    }

    fn advance(&self, _font_id: FontId, glyph_id: GlyphId) -> Result<Size<f32>> {
        Ok(size(600.0 * glyph_id.0 as f32, 0.0))
    }

    fn glyph_for_char(&self, _font_id: FontId, ch: char) -> Option<GlyphId> {
        Some(GlyphId(ch.len_utf16() as u32))
    }

    fn glyph_raster_bounds(&self, _params: &RenderGlyphParams) -> Result<Bounds<DevicePixels>> {
        Ok(Default::default())
    }

    fn rasterize_glyph(
        &self,
        _params: &RenderGlyphParams,
        raster_bounds: Bounds<DevicePixels>,
    ) -> Result<(Size<DevicePixels>, Vec<u8>)> {
        Ok((raster_bounds.size, Vec::new()))
    }

    fn layout_line(&self, text: &str, font_size: Pixels, _runs: &[FontRun]) -> LineLayout {
        let mut position = px(0.);
        let metrics = self.font_metrics(FontId(0));
        let em_width = font_size
            * self
                .advance(FontId(0), self.glyph_for_char(FontId(0), 'm').unwrap())
                .unwrap()
                .width
            / metrics.units_per_em as f32;
        let mut glyphs = Vec::new();
        for (ix, c) in text.char_indices() {
            if let Some(glyph) = self.glyph_for_char(FontId(0), c) {
                glyphs.push(ShapedGlyph {
                    id: glyph,
                    position: point(position, px(0.)),
                    index: ix,
                    is_emoji: glyph.0 == 2,
                });
                if glyph.0 == 2 {
                    position += em_width * 2.0;
                } else {
                    position += em_width;
                }
            } else {
                position += em_width
            }
        }
        let mut runs = Vec::default();
        if !glyphs.is_empty() {
            runs.push(ShapedRun {
                font_id: FontId(0),
                glyphs,
            });
        } else {
            position = px(0.);
        }

        LineLayout {
            font_size,
            width: position,
            ascent: font_size * (metrics.ascent / metrics.units_per_em as f32),
            descent: font_size * (metrics.descent / metrics.units_per_em as f32),
            runs,
            len: text.len(),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub(crate) enum AtlasKey {
    Glyph(RenderGlyphParams),
    Svg(RenderSvgParams),
    Image(RenderImageParams),
}

impl AtlasKey {
    #[cfg_attr(
        all(
            any(target_os = "linux", target_os = "freebsd"),
            not(any(feature = "x11", feature = "wayland"))
        ),
        allow(dead_code)
    )]
    pub(crate) fn texture_kind(&self) -> AtlasTextureKind {
        match self {
            AtlasKey::Glyph(params) => {
                if params.is_emoji {
                    AtlasTextureKind::Polychrome
                } else {
                    AtlasTextureKind::Monochrome
                }
            }
            AtlasKey::Svg(_) => AtlasTextureKind::Monochrome,
            AtlasKey::Image(_) => AtlasTextureKind::Polychrome,
        }
    }
}

impl From<RenderGlyphParams> for AtlasKey {
    fn from(params: RenderGlyphParams) -> Self {
        Self::Glyph(params)
    }
}

impl From<RenderSvgParams> for AtlasKey {
    fn from(params: RenderSvgParams) -> Self {
        Self::Svg(params)
    }
}

impl From<RenderImageParams> for AtlasKey {
    fn from(params: RenderImageParams) -> Self {
        Self::Image(params)
    }
}

pub(crate) trait PlatformAtlas: Send + Sync {
    fn get_or_insert_with<'a>(
        &self,
        key: &AtlasKey,
        build: &mut dyn FnMut() -> Result<Option<(Size<DevicePixels>, Cow<'a, [u8]>)>>,
    ) -> Result<Option<AtlasTile>>;
    fn remove(&self, key: &AtlasKey);
}

struct AtlasTextureList<T> {
    textures: Vec<Option<T>>,
    free_list: Vec<usize>,
}

impl<T> Default for AtlasTextureList<T> {
    fn default() -> Self {
        Self {
            textures: Vec::default(),
            free_list: Vec::default(),
        }
    }
}

impl<T> ops::Index<usize> for AtlasTextureList<T> {
    type Output = Option<T>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.textures[index]
    }
}

impl<T> AtlasTextureList<T> {
    #[allow(unused)]
    fn drain(&mut self) -> std::vec::Drain<'_, Option<T>> {
        self.free_list.clear();
        self.textures.drain(..)
    }

    #[allow(dead_code)]
    fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut T> {
        self.textures.iter_mut().flatten()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(C)]
pub(crate) struct AtlasTile {
    pub(crate) texture_id: AtlasTextureId,
    pub(crate) tile_id: TileId,
    pub(crate) padding: u32,
    pub(crate) bounds: Bounds<DevicePixels>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
pub(crate) struct AtlasTextureId {
    pub(crate) index: u32,
    pub(crate) kind: AtlasTextureKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(C)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
pub(crate) enum AtlasTextureKind {
    Monochrome = 0,
    Polychrome = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub(crate) struct TileId(pub(crate) u32);

impl From<etagere::AllocId> for TileId {
    fn from(id: etagere::AllocId) -> Self {
        Self(id.serialize())
    }
}

impl From<TileId> for etagere::AllocId {
    fn from(id: TileId) -> Self {
        Self::deserialize(id.0)
    }
}

pub(crate) struct PlatformInputHandler {
    cx: AsyncWindowContext,
    handler: Box<dyn InputHandler>,
    focus: Option<FocusId>,
}

#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
impl PlatformInputHandler {
    pub fn new(cx: AsyncWindowContext, handler: Box<dyn InputHandler>) -> Self {
        Self {
            cx,
            handler,
            focus: None,
        }
    }

    pub fn with_focus(mut self, focus: FocusId) -> Self {
        self.focus = Some(focus);
        self
    }

    pub fn focus_id(&self) -> Option<FocusId> {
        self.focus
    }

    fn selected_text_range(&mut self, ignore_disabled_input: bool) -> Option<UTF16Selection> {
        self.cx
            .update(|window, cx| {
                self.handler
                    .selected_text_range(ignore_disabled_input, window, cx)
            })
            .ok()
            .flatten()
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    fn marked_text_range(&mut self) -> Option<Range<usize>> {
        self.cx
            .update(|window, cx| self.handler.marked_text_range(window, cx))
            .ok()
            .flatten()
    }

    #[cfg_attr(
        any(target_os = "linux", target_os = "freebsd", target_os = "windows"),
        allow(dead_code)
    )]
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
    ) -> Option<String> {
        self.cx
            .update(|window, cx| {
                self.handler
                    .text_for_range(range_utf16, adjusted, window, cx)
            })
            .ok()
            .flatten()
    }

    fn replace_text_in_range(&mut self, replacement_range: Option<Range<usize>>, text: &str) {
        self.cx
            .update(|window, cx| {
                self.handler
                    .replace_text_in_range(replacement_range, text, window, cx);
            })
            .ok();
    }

    pub fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
    ) {
        self.cx
            .update(|window, cx| {
                self.handler.replace_and_mark_text_in_range(
                    range_utf16,
                    new_text,
                    new_selected_range,
                    window,
                    cx,
                )
            })
            .ok();
    }

    #[cfg_attr(target_os = "windows", allow(dead_code))]
    fn unmark_text(&mut self) {
        self.cx
            .update(|window, cx| self.handler.unmark_text(window, cx))
            .ok();
    }

    fn bounds_for_range(&mut self, range_utf16: Range<usize>) -> Option<Bounds<Pixels>> {
        self.cx
            .update(|window, cx| self.handler.bounds_for_range(range_utf16, window, cx))
            .ok()
            .flatten()
    }

    #[allow(dead_code)]
    fn apple_press_and_hold_enabled(&mut self) -> bool {
        self.handler.apple_press_and_hold_enabled()
    }

    pub(crate) fn dispatch_input(&mut self, input: &str, window: &mut Window, cx: &mut App) {
        self.handler.replace_text_in_range(None, input, window, cx);
    }

    pub fn selected_bounds(&mut self, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        let marked_range = self.handler.marked_text_range(window, cx);
        let selection = self.handler.selected_text_range(true, window, cx)?;
        compute_ime_candidate_bounds(marked_range, &selection, |range| {
            self.handler.bounds_for_range(range, window, cx)
        })
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub fn ime_candidate_bounds(&mut self) -> Option<Bounds<Pixels>> {
        let marked_range = self.marked_text_range();
        let selection = self.selected_text_range(true)?;
        compute_ime_candidate_bounds(marked_range, &selection, |range| {
            self.bounds_for_range(range)
        })
    }

    #[allow(unused)]
    pub fn character_index_for_point(&mut self, point: Point<Pixels>) -> Option<usize> {
        self.cx
            .update(|window, cx| self.handler.character_index_for_point(point, window, cx))
            .ok()
            .flatten()
    }
}

fn compute_ime_candidate_bounds(
    marked_range: Option<Range<usize>>,
    selection: &UTF16Selection,
    mut bounds_for_range: impl FnMut(Range<usize>) -> Option<Bounds<Pixels>>,
) -> Option<Bounds<Pixels>> {
    if let Some(marked_range) = marked_range {
        let mut line_start = marked_range.start;
        let caret = selection.range.end;
        if let Some(caret_bounds) = bounds_for_range(caret..caret) {
            for index in (marked_range.start..caret).rev() {
                if let Some(bounds) = bounds_for_range(index..index)
                    && (bounds.origin.y - caret_bounds.origin.y).abs() > px(0.1)
                {
                    line_start = index + 1;
                    break;
                }
            }
        }
        bounds_for_range(line_start..line_start)
    } else {
        let offset = if selection.reversed {
            selection.range.start
        } else {
            selection.range.end
        };
        bounds_for_range(offset..offset)
    }
}

#[derive(Debug)]
pub struct UTF16Selection {
    pub range: Range<usize>,
    pub reversed: bool,
}

pub trait InputHandler: 'static {
    fn selected_text_range(
        &mut self,
        ignore_disabled_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection>;

    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>>;

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String>;

    fn replace_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    );

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    );

    fn unmark_text(&mut self, window: &mut Window, cx: &mut App);

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Bounds<Pixels>>;

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize>;

    #[allow(dead_code)]
    fn apple_press_and_hold_enabled(&mut self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct WindowOptions {
    pub window_bounds: Option<WindowBounds>,

    pub titlebar: Option<TitlebarOptions>,

    pub focus: bool,

    pub show: bool,

    pub kind: WindowKind,

    pub is_movable: bool,

    pub is_resizable: bool,

    pub is_minimizable: bool,

    pub display_id: Option<DisplayId>,

    pub window_background: WindowBackgroundAppearance,

    pub app_id: Option<String>,

    pub window_min_size: Option<Size<Pixels>>,

    pub window_decorations: Option<WindowDecorations>,

    pub tabbing_identifier: Option<String>,
}

#[derive(Debug)]
#[cfg_attr(
    all(
        any(target_os = "linux", target_os = "freebsd"),
        not(any(feature = "x11", feature = "wayland"))
    ),
    allow(dead_code)
)]
pub(crate) struct WindowParams {
    pub bounds: Bounds<Pixels>,

    #[cfg_attr(feature = "wayland", allow(dead_code))]
    pub titlebar: Option<TitlebarOptions>,

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub kind: WindowKind,

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub is_movable: bool,

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub is_resizable: bool,

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub is_minimizable: bool,

    #[cfg_attr(
        any(target_os = "linux", target_os = "freebsd", target_os = "windows"),
        allow(dead_code)
    )]
    pub focus: bool,

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub show: bool,

    #[cfg_attr(feature = "wayland", allow(dead_code))]
    pub display_id: Option<DisplayId>,

    pub window_min_size: Option<Size<Pixels>>,
    #[cfg(target_os = "macos")]
    pub tabbing_identifier: Option<String>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum WindowBounds {
    Windowed(Bounds<Pixels>),
    Maximized(Bounds<Pixels>),
    Fullscreen(Bounds<Pixels>),
}

impl Default for WindowBounds {
    fn default() -> Self {
        WindowBounds::Windowed(Bounds::default())
    }
}

impl WindowBounds {
    pub fn get_bounds(&self) -> Bounds<Pixels> {
        match self {
            WindowBounds::Windowed(bounds) => *bounds,
            WindowBounds::Maximized(bounds) => *bounds,
            WindowBounds::Fullscreen(bounds) => *bounds,
        }
    }

    pub fn centered(size: Size<Pixels>, cx: &App) -> Self {
        WindowBounds::Windowed(Bounds::centered(None, size, cx))
    }
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            window_bounds: None,
            titlebar: Some(TitlebarOptions {
                title: Default::default(),
                appears_transparent: Default::default(),
                traffic_light_position: Default::default(),
            }),
            focus: true,
            show: true,
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            display_id: None,
            window_background: WindowBackgroundAppearance::default(),
            app_id: None,
            window_min_size: None,
            window_decorations: None,
            tabbing_identifier: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct TitlebarOptions {
    pub title: Option<SharedString>,

    pub appears_transparent: bool,

    pub traffic_light_position: Option<Point<Pixels>>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WindowKind {
    Normal,

    PopUp,

    Floating,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WindowAppearance {
    Light,

    VibrantLight,

    Dark,

    VibrantDark,
}

impl Default for WindowAppearance {
    fn default() -> Self {
        Self::Light
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub enum WindowBackgroundAppearance {
    #[default]
    Opaque,
    Transparent,
    Blurred,
}

#[derive(Clone, Debug)]
pub struct PathPromptOptions {
    pub files: bool,
    pub directories: bool,
    pub multiple: bool,
    pub prompt: Option<SharedString>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PromptLevel {
    Info,

    Warning,

    Critical,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PromptButton {
    Ok(SharedString),
    Cancel(SharedString),
    Other(SharedString),
}

impl PromptButton {
    pub fn new(label: impl Into<SharedString>) -> Self {
        PromptButton::Other(label.into())
    }

    pub fn ok(label: impl Into<SharedString>) -> Self {
        PromptButton::Ok(label.into())
    }

    pub fn cancel(label: impl Into<SharedString>) -> Self {
        PromptButton::Cancel(label.into())
    }

    #[allow(dead_code)]
    pub(crate) fn is_cancel(&self) -> bool {
        matches!(self, PromptButton::Cancel(_))
    }

    pub fn label(&self) -> &SharedString {
        match self {
            PromptButton::Ok(label) => label,
            PromptButton::Cancel(label) => label,
            PromptButton::Other(label) => label,
        }
    }
}

impl From<&str> for PromptButton {
    fn from(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "ok" => PromptButton::Ok("Ok".into()),
            "cancel" => PromptButton::Cancel("Cancel".into()),
            _ => PromptButton::Other(SharedString::from(value.to_owned())),
        }
    }
}

#[doc = " The style of the cursor (pointer)"]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum CursorStyle {
    #[doc = " The default cursor"]
    Arrow,

    #[doc = " A text input cursor"]
    #[doc = " corresponds to the CSS cursor value `text`"]
    IBeam,

    #[doc = " A crosshair cursor"]
    #[doc = " corresponds to the CSS cursor value `crosshair`"]
    Crosshair,

    #[doc = " A closed hand cursor"]
    #[doc = " corresponds to the CSS cursor value `grabbing`"]
    ClosedHand,

    #[doc = " An open hand cursor"]
    #[doc = " corresponds to the CSS cursor value `grab`"]
    OpenHand,

    #[doc = " A pointing hand cursor"]
    #[doc = " corresponds to the CSS cursor value `pointer`"]
    PointingHand,

    #[doc = " A resize left cursor"]
    #[doc = " corresponds to the CSS cursor value `w-resize`"]
    ResizeLeft,

    #[doc = " A resize right cursor"]
    #[doc = " corresponds to the CSS cursor value `e-resize`"]
    ResizeRight,

    #[doc = " A resize cursor to the left and right"]
    #[doc = " corresponds to the CSS cursor value `ew-resize`"]
    ResizeLeftRight,

    #[doc = " A resize up cursor"]
    #[doc = " corresponds to the CSS cursor value `n-resize`"]
    ResizeUp,

    #[doc = " A resize down cursor"]
    #[doc = " corresponds to the CSS cursor value `s-resize`"]
    ResizeDown,

    #[doc = " A resize cursor directing up and down"]
    #[doc = " corresponds to the CSS cursor value `ns-resize`"]
    ResizeUpDown,

    #[doc = " A resize cursor directing up-left and down-right"]
    #[doc = " corresponds to the CSS cursor value `nesw-resize`"]
    ResizeUpLeftDownRight,

    #[doc = " A resize cursor directing up-right and down-left"]
    #[doc = " corresponds to the CSS cursor value `nwse-resize`"]
    ResizeUpRightDownLeft,

    #[doc = " A cursor indicating that the item/column can be resized horizontally."]
    #[doc = " corresponds to the CSS cursor value `col-resize`"]
    ResizeColumn,

    #[doc = " A cursor indicating that the item/row can be resized vertically."]
    #[doc = " corresponds to the CSS cursor value `row-resize`"]
    ResizeRow,

    #[doc = " A text input cursor for vertical layout"]
    #[doc = " corresponds to the CSS cursor value `vertical-text`"]
    IBeamCursorForVerticalLayout,

    #[doc = " A cursor indicating that the operation is not allowed"]
    #[doc = " corresponds to the CSS cursor value `not-allowed`"]
    OperationNotAllowed,

    #[doc = " A cursor indicating that the operation will result in a link"]
    #[doc = " corresponds to the CSS cursor value `alias`"]
    DragLink,

    #[doc = " A cursor indicating that the operation will result in a copy"]
    #[doc = " corresponds to the CSS cursor value `copy`"]
    DragCopy,

    #[doc = " A cursor indicating that the operation will result in a context menu"]
    #[doc = " corresponds to the CSS cursor value `context-menu`"]
    ContextualMenu,

    #[doc = " Hide the cursor"]
    None,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self::Arrow
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardItem {
    entries: Vec<ClipboardEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClipboardEntry {
    String(ClipboardString),
    Image(Image),
    ExternalPaths(crate::ExternalPaths),
}

impl ClipboardItem {
    pub fn new_string(text: String) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(ClipboardString::new(text))],
        }
    }

    pub fn new_string_with_metadata(text: String, metadata: String) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(ClipboardString {
                text,
                metadata: Some(metadata),
            })],
        }
    }

    pub fn new_string_with_json_metadata<T: Serialize>(text: String, metadata: T) -> Self {
        Self {
            entries: vec![ClipboardEntry::String(
                ClipboardString::new(text).with_json_metadata(metadata),
            )],
        }
    }

    pub fn new_image(image: &Image) -> Self {
        Self {
            entries: vec![ClipboardEntry::Image(image.clone())],
        }
    }

    pub fn text(&self) -> Option<String> {
        let mut answer = String::new();
        let mut any_entries = false;

        for entry in self.entries.iter() {
            if let ClipboardEntry::String(ClipboardString { text, metadata: _ }) = entry {
                answer.push_str(text);
                any_entries = true;
            }
        }

        if any_entries { Some(answer) } else { None }
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub fn metadata(&self) -> Option<&String> {
        match self.entries().first() {
            Some(ClipboardEntry::String(clipboard_string)) if self.entries.len() == 1 => {
                clipboard_string.metadata.as_ref()
            }
            _ => None,
        }
    }

    pub fn entries(&self) -> &[ClipboardEntry] {
        &self.entries
    }

    pub fn into_entries(self) -> impl Iterator<Item = ClipboardEntry> {
        self.entries.into_iter()
    }
}

impl From<ClipboardString> for ClipboardEntry {
    fn from(value: ClipboardString) -> Self {
        Self::String(value)
    }
}

impl From<String> for ClipboardEntry {
    fn from(value: String) -> Self {
        Self::from(ClipboardString::from(value))
    }
}

impl From<Image> for ClipboardEntry {
    fn from(value: Image) -> Self {
        Self::Image(value)
    }
}

impl From<ClipboardEntry> for ClipboardItem {
    fn from(value: ClipboardEntry) -> Self {
        Self {
            entries: vec![value],
        }
    }
}

impl From<String> for ClipboardItem {
    fn from(value: String) -> Self {
        Self::from(ClipboardEntry::from(value))
    }
}

impl From<Image> for ClipboardItem {
    fn from(value: Image) -> Self {
        Self::from(ClipboardEntry::from(value))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, EnumIter, Hash)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
    Svg,
    Bmp,
    Tiff,
}

impl ImageFormat {
    pub const fn mime_type(self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Webp => "image/webp",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Svg => "image/svg+xml",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Tiff => "image/tiff",
        }
    }

    pub fn from_mime_type(mime_type: &str) -> Option<Self> {
        match mime_type {
            "image/png" => Some(Self::Png),
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            "image/webp" => Some(Self::Webp),
            "image/gif" => Some(Self::Gif),
            "image/svg+xml" => Some(Self::Svg),
            "image/bmp" => Some(Self::Bmp),
            "image/tiff" | "image/tif" => Some(Self::Tiff),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub format: ImageFormat,
    pub bytes: Vec<u8>,
    id: u64,
}

impl Hash for Image {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.id);
    }
}

impl Image {
    pub fn empty() -> Self {
        Self::from_bytes(ImageFormat::Png, Vec::new())
    }

    pub fn from_bytes(format: ImageFormat, bytes: Vec<u8>) -> Self {
        Self {
            id: hash(&bytes),
            format,
            bytes,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn use_render_image(
        self: Arc<Self>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<RenderImage>> {
        ImageSource::Image(self)
            .use_data(None, window, cx)
            .and_then(|result| result.ok())
    }

    pub fn get_render_image(
        self: Arc<Self>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Arc<RenderImage>> {
        ImageSource::Image(self)
            .get_data(None, window, cx)
            .and_then(|result| result.ok())
    }

    pub fn remove_asset(self: Arc<Self>, cx: &mut App) {
        ImageSource::Image(self).remove_asset(cx);
    }

    pub fn to_image_data(&self, svg_renderer: SvgRenderer) -> Result<Arc<RenderImage>> {
        fn frames_for_image(
            bytes: &[u8],
            format: image::ImageFormat,
        ) -> Result<SmallVec<[Frame; 1]>> {
            let mut data = image::load_from_memory_with_format(bytes, format)?.into_rgba8();

            for pixel in data.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }

            Ok(SmallVec::from_elem(Frame::new(data), 1))
        }

        let frames = match self.format {
            ImageFormat::Gif => {
                let decoder = GifDecoder::new(Cursor::new(&self.bytes))?;
                let mut frames = SmallVec::new();

                for frame in decoder.into_frames() {
                    let mut frame = frame?;
                    for pixel in frame.buffer_mut().chunks_exact_mut(4) {
                        pixel.swap(0, 2);
                    }
                    frames.push(frame);
                }

                frames
            }
            ImageFormat::Png => frames_for_image(&self.bytes, image::ImageFormat::Png)?,
            ImageFormat::Jpeg => frames_for_image(&self.bytes, image::ImageFormat::Jpeg)?,
            ImageFormat::Webp => frames_for_image(&self.bytes, image::ImageFormat::WebP)?,
            ImageFormat::Bmp => frames_for_image(&self.bytes, image::ImageFormat::Bmp)?,
            ImageFormat::Tiff => frames_for_image(&self.bytes, image::ImageFormat::Tiff)?,
            ImageFormat::Svg => {
                let pixmap = svg_renderer.render_pixmap(&self.bytes, SvgSize::ScaleFactor(1.0))?;

                let buffer =
                    image::ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take())
                        .unwrap();

                SmallVec::from_elem(Frame::new(buffer), 1)
            }
        };

        Ok(Arc::new(RenderImage::new(frames)))
    }

    pub fn format(&self) -> ImageFormat {
        self.format
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardString {
    pub(crate) text: String,
    pub(crate) metadata: Option<String>,
}

impl ClipboardString {
    pub fn new(text: String) -> Self {
        Self {
            text,
            metadata: None,
        }
    }

    pub fn with_json_metadata<T: Serialize>(mut self, metadata: T) -> Self {
        self.metadata = Some(serde_json::to_string(&metadata).unwrap());
        self
    }

    pub fn text(&self) -> &String {
        &self.text
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub fn metadata_json<T>(&self) -> Option<T>
    where
        T: for<'a> Deserialize<'a>,
    {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
    }

    #[cfg_attr(any(target_os = "linux", target_os = "freebsd"), allow(dead_code))]
    pub(crate) fn text_hash(text: &str) -> u64 {
        let mut hasher = SeaHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }
}

impl From<String> for ClipboardString {
    fn from(value: String) -> Self {
        Self {
            text: value,
            metadata: None,
        }
    }
}

#[cfg(test)]
mod ime_candidate_bounds_tests {
    use super::{UTF16Selection, compute_ime_candidate_bounds};
    use crate::{Bounds, Pixels, point, px, size};
    use std::ops::Range;

    fn selection(range: Range<usize>) -> UTF16Selection {
        UTF16Selection {
            range,
            reversed: false,
        }
    }

    fn at(index: usize, y: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(index as f32), px(y)), size(px(0.), px(16.)))
    }

    #[test]
    fn wrapped_preedit_anchors_at_the_caret_line() {
        let bounds = compute_ime_candidate_bounds(Some(2..8), &selection(8..8), |range| {
            let y = if range.start >= 5 { 20. } else { 0. };
            Some(at(range.start, y))
        })
        .unwrap();
        assert_eq!(bounds.origin, point(px(5.), px(20.)));
    }

    #[test]
    fn single_line_preedit_anchors_at_the_marked_start() {
        let bounds = compute_ime_candidate_bounds(Some(2..8), &selection(8..8), |range| {
            Some(at(range.start, 0.))
        })
        .unwrap();
        assert_eq!(bounds.origin, point(px(2.), px(0.)));
    }

    #[test]
    fn caret_is_the_anchor_without_a_composition() {
        let bounds =
            compute_ime_candidate_bounds(None, &selection(4..4), |range| Some(at(range.start, 0.)))
                .unwrap();
        assert_eq!(bounds.origin, point(px(4.), px(0.)));
    }
}
