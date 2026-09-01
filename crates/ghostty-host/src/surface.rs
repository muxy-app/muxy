use std::ffi::{CString, c_void};
use std::fmt;
use std::num::NonZeroU64;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicU64, Ordering};

use ghostty_sys::ffi;
use thiserror::Error;

use crate::config::{ColorScheme, GhosttyConfig};
use crate::input::{KeyboardInput, Modifiers};
use crate::mouse::{MouseButton, MouseButtonState, MousePressureStage, ScrollMetadata};
use crate::runtime::{
    ClipboardRequestToken, GhosttyApp, MainThreadError, SurfaceUserdata, require_main_thread,
    surface_data_callback,
};

static NEXT_SURFACE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SurfaceId(NonZeroU64);

impl SurfaceId {
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    pub(crate) fn allocate() -> Self {
        let candidate = NEXT_SURFACE_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("exhausted all nonzero SurfaceId values");
        Self(NonZeroU64::new(candidate).expect("SurfaceId counter starts at one"))
    }
}

impl fmt::Display for SurfaceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get().fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SurfaceContext {
    #[default]
    Window,
    Tab,
    Split,
}

impl SurfaceContext {
    fn as_raw(self) -> ffi::ghostty_surface_context_e {
        match self {
            Self::Window => ffi::ghostty_surface_context_e_GHOSTTY_SURFACE_CONTEXT_WINDOW,
            Self::Tab => ffi::ghostty_surface_context_e_GHOSTTY_SURFACE_CONTEXT_TAB,
            Self::Split => ffi::ghostty_surface_context_e_GHOSTTY_SURFACE_CONTEXT_SPLIT,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceEnvironmentVariable {
    pub key: String,
    pub value: String,
}

impl SurfaceEnvironmentVariable {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceOptions {
    pub scale_factor: f64,
    pub font_size: f32,
    pub working_directory: PathBuf,
    pub command: Option<String>,
    pub environment: Vec<SurfaceEnvironmentVariable>,
    pub initial_input: Option<String>,
    pub wait_after_command: bool,
    pub context: SurfaceContext,
}

impl Default for SurfaceOptions {
    fn default() -> Self {
        Self {
            scale_factor: 1.0,
            font_size: 0.0,
            working_directory: std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/")),
            command: None,
            environment: Vec::new(),
            initial_input: None,
            wait_after_command: false,
            context: SurfaceContext::Window,
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SurfaceError {
    #[error(transparent)]
    MainThread(#[from] MainThreadError),
    #[error("surface scale factor must be finite and greater than zero, got {0}")]
    InvalidScaleFactor(f64),
    #[error("surface font size must be finite and nonnegative, got {0}")]
    InvalidFontSize(f32),
    #[error("surface {field} contains an interior NUL byte")]
    StringContainsNul { field: &'static str },
    #[error("surface environment key at index {index} contains an interior NUL byte")]
    EnvironmentKeyContainsNul { index: usize },
    #[error("surface environment value at index {index} contains an interior NUL byte")]
    EnvironmentValueContainsNul { index: usize },
    #[error("ghostty_surface_new returned null")]
    CreationReturnedNull,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SurfaceTextError {
    #[error("Ghostty returned a null pointer for nonempty {operation} text")]
    NullText { operation: &'static str },
    #[error("Ghostty returned invalid UTF-8 for {operation}")]
    InvalidUtf8 { operation: &'static str },
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ClipboardCompletionError {
    #[error("clipboard request belongs to surface {request_surface}, not {actual_surface}")]
    WrongSurface {
        request_surface: SurfaceId,
        actual_surface: SurfaceId,
    },
    #[error("clipboard request is no longer pending")]
    NotPending,
    #[error("clipboard completion text contains an interior NUL byte")]
    TextContainsNul,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BindingFlags(ffi::ghostty_binding_flags_e);

impl BindingFlags {
    pub const NONE: Self = Self(0);
    pub const CONSUMED: Self = Self(ffi::ghostty_binding_flags_e_GHOSTTY_BINDING_FLAGS_CONSUMED);
    pub const ALL: Self = Self(ffi::ghostty_binding_flags_e_GHOSTTY_BINDING_FLAGS_ALL);
    pub const GLOBAL: Self = Self(ffi::ghostty_binding_flags_e_GHOSTTY_BINDING_FLAGS_GLOBAL);
    pub const PERFORMABLE: Self =
        Self(ffi::ghostty_binding_flags_e_GHOSTTY_BINDING_FLAGS_PERFORMABLE);

    pub const fn from_raw(raw: ffi::ghostty_binding_flags_e) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> ffi::ghostty_binding_flags_e {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SurfaceSize {
    pub columns: u16,
    pub rows: u16,
    pub width_px: u32,
    pub height_px: u32,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
}

impl From<ffi::ghostty_surface_size_s> for SurfaceSize {
    fn from(value: ffi::ghostty_surface_size_s) -> Self {
        Self {
            columns: value.columns,
            rows: value.rows,
            width_px: value.width_px,
            height_px: value.height_px,
            cell_width_px: value.cell_width_px,
            cell_height_px: value.cell_height_px,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfacePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ImeRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceText {
    pub top_left: SurfacePoint,
    pub offset_start: u32,
    pub offset_len: u32,
    pub text: String,
}

struct OwnedEnvironmentVariable {
    key: CString,
    value: CString,
}

struct SurfaceStorage {
    working_directory: CString,
    command: Option<CString>,
    environment: Vec<OwnedEnvironmentVariable>,
    environment_ffi: Vec<ffi::ghostty_env_var_s>,
    initial_input: Option<CString>,
    userdata: Box<SurfaceUserdata>,
}

struct GhosttyStringGuard(ffi::ghostty_string_s);

impl Drop for GhosttyStringGuard {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_string_free(self.0) };
    }
}

struct GhosttyTextGuard {
    surface: ffi::ghostty_surface_t,
    text: *mut ffi::ghostty_text_s,
}

impl Drop for GhosttyTextGuard {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_surface_free_text(self.surface, self.text) };
    }
}

struct GhosttyCellsGuard {
    surface: ffi::ghostty_surface_t,
    cells: *mut ffi::ghostty_cells_s,
}

impl Drop for GhosttyCellsGuard {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_surface_free_cells(self.surface, self.cells) };
    }
}

pub struct GhosttySurface {
    raw: NonNull<c_void>,
    id: SurfaceId,
    storage: SurfaceStorage,
    app: GhosttyApp,
}

impl GhosttySurface {
    pub unsafe fn new(
        app: &GhosttyApp,
        ns_view: NonNull<c_void>,
        options: SurfaceOptions,
    ) -> Result<Self, SurfaceError> {
        require_main_thread()?;
        app.assert_owner_thread();
        if !options.scale_factor.is_finite() || options.scale_factor <= 0.0 {
            return Err(SurfaceError::InvalidScaleFactor(options.scale_factor));
        }
        if !options.font_size.is_finite() || options.font_size < 0.0 {
            return Err(SurfaceError::InvalidFontSize(options.font_size));
        }

        let working_directory = path_to_cstring(&options.working_directory)?;
        let command = optional_cstring(options.command.as_deref(), "command")?;
        let initial_input = optional_cstring(options.initial_input.as_deref(), "initial input")?;
        let mut environment = Vec::with_capacity(options.environment.len());
        for (index, variable) in options.environment.iter().enumerate() {
            let key = CString::new(variable.key.as_bytes())
                .map_err(|_| SurfaceError::EnvironmentKeyContainsNul { index })?;
            let value = CString::new(variable.value.as_bytes())
                .map_err(|_| SurfaceError::EnvironmentValueContainsNul { index })?;
            environment.push(OwnedEnvironmentVariable { key, value });
        }
        let environment_ffi = environment
            .iter()
            .map(|variable| ffi::ghostty_env_var_s {
                key: variable.key.as_ptr(),
                value: variable.value.as_ptr(),
            })
            .collect::<Vec<_>>();

        let id = SurfaceId::allocate();
        let userdata = Box::new(SurfaceUserdata::new(
            id,
            app.surface_event_sender(),
            app.surface_data_event_sender(),
        ));
        let mut storage = SurfaceStorage {
            working_directory,
            command,
            environment,
            environment_ffi,
            initial_input,
            userdata,
        };

        let mut config = unsafe { ffi::ghostty_surface_config_new() };
        config.platform_tag = ffi::ghostty_platform_e_GHOSTTY_PLATFORM_MACOS;
        config.platform = ffi::ghostty_platform_u {
            macos: ffi::ghostty_platform_macos_s {
                nsview: ns_view.as_ptr(),
            },
        };
        config.userdata = std::ptr::from_mut(storage.userdata.as_mut()).cast();
        config.scale_factor = options.scale_factor;
        config.font_size = options.font_size;
        config.working_directory = storage.working_directory.as_ptr();
        config.command = storage
            .command
            .as_ref()
            .map_or(ptr::null(), |command| command.as_ptr());
        config.env_vars = if storage.environment_ffi.is_empty() {
            ptr::null_mut()
        } else {
            storage.environment_ffi.as_mut_ptr()
        };
        config.env_var_count = storage.environment_ffi.len();
        config.initial_input = storage
            .initial_input
            .as_ref()
            .map_or(ptr::null(), |input| input.as_ptr());
        config.wait_after_command = options.wait_after_command;
        config.context = options.context.as_raw();

        let _environment_must_outlive_call = &storage.environment;
        let raw = unsafe { ffi::ghostty_surface_new(app.as_raw(), &config) };
        let raw = NonNull::new(raw).ok_or(SurfaceError::CreationReturnedNull)?;
        unsafe {
            ffi::ghostty_surface_set_data_callback(
                raw.as_ptr(),
                Some(surface_data_callback),
                std::ptr::from_mut(storage.userdata.as_mut()).cast(),
            )
        };

        Ok(Self {
            raw,
            id,
            storage,
            app: app.clone(),
        })
    }

    pub const fn id(&self) -> SurfaceId {
        self.id
    }

    pub fn set_size(&self, width: u32, height: u32) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_set_size(self.as_raw(), width, height) };
    }

    pub fn size(&self) -> SurfaceSize {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_size(self.as_raw()) }.into()
    }

    pub fn set_content_scale(&self, x: f64, y: f64) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_set_content_scale(self.as_raw(), x, y) };
    }

    pub fn set_focus(&self, focused: bool) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_set_focus(self.as_raw(), focused) };
    }

    pub fn set_occluded(&self, occluded: bool) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_set_occlusion(self.as_raw(), !occluded) };
    }

    pub fn refresh(&self) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_refresh(self.as_raw()) };
    }

    pub fn draw(&self) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_draw(self.as_raw()) };
    }

    pub fn update_config(&self, config: &GhosttyConfig) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_update_config(self.as_raw(), config.as_raw()) };
    }

    pub fn needs_confirm_quit(&self) -> bool {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_needs_confirm_quit(self.as_raw()) }
    }

    pub fn process_exited(&self) -> bool {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_process_exited(self.as_raw()) }
    }

    pub fn foreground_pid(&self) -> Option<u64> {
        self.assert_owner_thread();
        NonZeroU64::new(unsafe { ffi::ghostty_surface_foreground_pid(self.as_raw()) })
            .map(NonZeroU64::get)
    }

    pub fn tty_name(&self) -> Result<Option<String>, SurfaceTextError> {
        self.assert_owner_thread();
        let raw = unsafe { ffi::ghostty_surface_tty_name(self.as_raw()) };
        let raw = GhosttyStringGuard(raw);
        unsafe { copy_ghostty_string(raw.0, "TTY name") }
    }

    pub fn set_color_scheme(&self, scheme: ColorScheme) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_set_color_scheme(self.as_raw(), scheme.as_raw()) };
    }

    pub fn key_translation_modifiers(&self, modifiers: Modifiers) -> Modifiers {
        self.assert_owner_thread();
        let raw =
            unsafe { ffi::ghostty_surface_key_translation_mods(self.as_raw(), modifiers.raw()) };
        Modifiers::from_raw(raw)
    }

    pub fn send_key(&self, input: &KeyboardInput) -> bool {
        self.assert_owner_thread();
        let event = input.as_ffi();
        unsafe { ffi::ghostty_surface_key(self.as_raw(), event) }
    }

    pub fn key_binding(&self, input: &KeyboardInput) -> Option<BindingFlags> {
        self.assert_owner_thread();
        let event = input.as_ffi();
        let mut flags = 0;
        let matched =
            unsafe { ffi::ghostty_surface_key_is_binding(self.as_raw(), event, &mut flags) };
        matched.then_some(BindingFlags::from_raw(flags))
    }

    pub fn send_text(&self, text: &str) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_text(self.as_raw(), text.as_ptr().cast(), text.len()) };
    }

    pub fn send_input_raw(&self, bytes: &[u8]) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_send_input_raw(self.as_raw(), bytes.as_ptr(), bytes.len()) };
    }

    pub fn set_preedit(&self, text: Option<&str>) {
        self.assert_owner_thread();
        let (pointer, len) =
            text.map_or((ptr::null(), 0), |text| (text.as_ptr().cast(), text.len()));
        unsafe { ffi::ghostty_surface_preedit(self.as_raw(), pointer, len) };
    }

    pub fn mouse_captured(&self) -> bool {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_mouse_captured(self.as_raw()) }
    }

    pub fn send_mouse_button(
        &self,
        state: MouseButtonState,
        button: MouseButton,
        modifiers: Modifiers,
    ) -> bool {
        self.assert_owner_thread();
        unsafe {
            ffi::ghostty_surface_mouse_button(
                self.as_raw(),
                state.as_raw(),
                button.as_raw(),
                modifiers.raw(),
            )
        }
    }

    pub fn set_mouse_position(&self, position: SurfacePoint, modifiers: Modifiers) {
        self.assert_owner_thread();
        unsafe {
            ffi::ghostty_surface_mouse_pos(self.as_raw(), position.x, position.y, modifiers.raw())
        };
    }

    pub fn send_mouse_scroll(&self, delta_x: f64, delta_y: f64, metadata: ScrollMetadata) {
        self.assert_owner_thread();
        unsafe {
            ffi::ghostty_surface_mouse_scroll(self.as_raw(), delta_x, delta_y, metadata.packed())
        };
    }

    pub fn send_mouse_pressure(&self, stage: MousePressureStage, pressure: f64) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_mouse_pressure(self.as_raw(), stage.as_raw(), pressure) };
    }

    pub fn ime_rect(&self) -> ImeRect {
        self.assert_owner_thread();
        let mut rect = ImeRect::default();
        unsafe {
            ffi::ghostty_surface_ime_point(
                self.as_raw(),
                &mut rect.x,
                &mut rect.y,
                &mut rect.width,
                &mut rect.height,
            )
        };
        rect
    }

    pub fn request_close(&self) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_request_close(self.as_raw()) };
    }

    pub fn perform_binding_action(&self, action: &str) -> bool {
        self.assert_owner_thread();
        unsafe {
            ffi::ghostty_surface_binding_action(self.as_raw(), action.as_ptr().cast(), action.len())
        }
    }

    pub fn complete_clipboard_request(
        &self,
        token: &ClipboardRequestToken,
        text: &str,
        confirmed: bool,
    ) -> Result<(), ClipboardCompletionError> {
        self.assert_owner_thread();
        if token.surface_id() != self.id {
            return Err(ClipboardCompletionError::WrongSurface {
                request_surface: token.surface_id(),
                actual_surface: self.id,
            });
        }
        let text = CString::new(text).map_err(|_| ClipboardCompletionError::TextContainsNul)?;
        if !self.storage.userdata.take_clipboard_request(token) {
            return Err(ClipboardCompletionError::NotPending);
        }
        unsafe {
            ffi::ghostty_surface_complete_clipboard_request(
                self.as_raw(),
                text.as_ptr(),
                token.state_ptr(),
                confirmed,
            )
        };
        Ok(())
    }

    pub fn has_selection(&self) -> bool {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_has_selection(self.as_raw()) }
    }

    pub fn read_selection(&self) -> Result<Option<SurfaceText>, SurfaceTextError> {
        self.read_surface_text("selection", |surface, output| unsafe {
            ffi::ghostty_surface_read_selection(surface, output)
        })
    }

    pub fn read_screen_text(&self, last_lines: usize) -> Option<String> {
        self.with_cells(|raw| {
            let cols = usize::try_from(raw.cols).ok()?;
            let rows = usize::try_from(raw.rows).ok()?;
            let len = cols.checked_mul(rows)?;
            if cols == 0 || rows == 0 || raw.cells.is_null() || len > raw.cells_len {
                return Some(String::new());
            }
            let cells = unsafe { std::slice::from_raw_parts(raw.cells, len) };
            Some(format_screen_cells(cells, cols, last_lines))
        })?
    }

    pub fn is_alternate_screen(&self) -> Option<bool> {
        self.with_cells(|raw| raw.alt_screen)
    }

    pub fn quicklook_word(&self) -> Result<Option<SurfaceText>, SurfaceTextError> {
        self.read_surface_text("quick-look word", |surface, output| unsafe {
            ffi::ghostty_surface_quicklook_word(surface, output)
        })
    }

    pub fn quicklook_font(&self) -> Option<NonNull<c_void>> {
        self.assert_owner_thread();
        NonNull::new(unsafe { ffi::ghostty_surface_quicklook_font(self.as_raw()) })
    }

    pub fn set_display_id(&self, display_id: u32) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_surface_set_display_id(self.as_raw(), display_id) };
    }

    fn read_surface_text(
        &self,
        operation: &'static str,
        read: impl FnOnce(ffi::ghostty_surface_t, *mut ffi::ghostty_text_s) -> bool,
    ) -> Result<Option<SurfaceText>, SurfaceTextError> {
        self.assert_owner_thread();
        let mut raw = ffi::ghostty_text_s::default();
        if !read(self.as_raw(), &mut raw) {
            return Ok(None);
        }
        let _guard = GhosttyTextGuard {
            surface: self.as_raw(),
            text: &mut raw,
        };
        unsafe { copy_surface_text(&raw, operation) }.map(Some)
    }

    fn with_cells<T>(&self, read: impl FnOnce(&ffi::ghostty_cells_s) -> T) -> Option<T> {
        self.assert_owner_thread();
        let mut raw = ffi::ghostty_cells_s::default();
        if !unsafe { ffi::ghostty_surface_read_cells(self.as_raw(), &mut raw) } {
            return None;
        }
        let guard = GhosttyCellsGuard {
            surface: self.as_raw(),
            cells: &mut raw,
        };
        Some(read(guard.as_ref()))
    }

    pub(crate) fn as_raw(&self) -> ffi::ghostty_surface_t {
        self.raw.as_ptr()
    }

    fn assert_owner_thread(&self) {
        self.app.assert_owner_thread();
    }
}

impl Drop for GhosttySurface {
    fn drop(&mut self) {
        self.assert_owner_thread();
        let _storage_must_outlive_surface = &self.storage;
        release_surface(
            self.as_raw(),
            |surface| unsafe {
                ffi::ghostty_surface_set_data_callback(surface, None, ptr::null_mut())
            },
            |surface| unsafe { ffi::ghostty_surface_free(surface) },
        );
    }
}

impl GhosttyCellsGuard {
    fn as_ref(&self) -> &ffi::ghostty_cells_s {
        unsafe { &*self.cells }
    }
}

fn release_surface<T: Copy>(surface: T, unregister: impl FnOnce(T), free: impl FnOnce(T)) {
    unregister(surface);
    free(surface);
}

fn format_screen_cells(cells: &[ffi::ghostty_cell_s], cols: usize, last_lines: usize) -> String {
    let mut lines = Vec::with_capacity(cells.len() / cols);
    for row in cells.chunks_exact(cols) {
        let line: String = row
            .iter()
            .map(|cell| {
                (cell.codepoint != 0)
                    .then(|| char::from_u32(cell.codepoint))
                    .flatten()
                    .unwrap_or(' ')
            })
            .collect();
        lines.push(line);
    }
    while lines
        .last()
        .is_some_and(|line| line.chars().all(|value| value == ' '))
    {
        lines.pop();
    }
    let start = lines.len().saturating_sub(last_lines);
    lines[start..]
        .iter()
        .map(|line| line.trim_end_matches(char::is_whitespace))
        .collect::<Vec<_>>()
        .join("\n")
}

unsafe fn copy_surface_text(
    raw: &ffi::ghostty_text_s,
    operation: &'static str,
) -> Result<SurfaceText, SurfaceTextError> {
    let text = if raw.text_len == 0 {
        String::new()
    } else {
        if raw.text.is_null() {
            return Err(SurfaceTextError::NullText { operation });
        }
        let bytes = unsafe { std::slice::from_raw_parts(raw.text.cast(), raw.text_len) };
        std::str::from_utf8(bytes)
            .map_err(|_| SurfaceTextError::InvalidUtf8 { operation })?
            .to_owned()
    };
    Ok(SurfaceText {
        top_left: SurfacePoint {
            x: raw.tl_px_x,
            y: raw.tl_px_y,
        },
        offset_start: raw.offset_start,
        offset_len: raw.offset_len,
        text,
    })
}

unsafe fn copy_ghostty_string(
    raw: ffi::ghostty_string_s,
    operation: &'static str,
) -> Result<Option<String>, SurfaceTextError> {
    if raw.ptr.is_null() {
        return if raw.len == 0 {
            Ok(None)
        } else {
            Err(SurfaceTextError::NullText { operation })
        };
    }
    let bytes = unsafe { std::slice::from_raw_parts(raw.ptr.cast(), raw.len) };
    let text = std::str::from_utf8(bytes)
        .map_err(|_| SurfaceTextError::InvalidUtf8 { operation })?
        .to_owned();
    Ok(Some(text))
}

fn path_to_cstring(path: &Path) -> Result<CString, SurfaceError> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| SurfaceError::StringContainsNul {
        field: "working directory",
    })
}

fn optional_cstring(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<CString>, SurfaceError> {
    value
        .map(|value| CString::new(value).map_err(|_| SurfaceError::StringContainsNul { field }))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_id_is_stable_unique_and_nonzero() {
        let id = SurfaceId::allocate();
        let next = SurfaceId::allocate();
        assert_ne!(id.get(), 0);
        assert_ne!(id, next);
        assert_eq!(id.to_string(), id.get().to_string());
    }

    #[test]
    fn surface_size_copies_every_abi_field() {
        let size = SurfaceSize::from(ffi::ghostty_surface_size_s {
            columns: 80,
            rows: 24,
            width_px: 800,
            height_px: 480,
            cell_width_px: 10,
            cell_height_px: 20,
        });
        assert_eq!(
            size,
            SurfaceSize {
                columns: 80,
                rows: 24,
                width_px: 800,
                height_px: 480,
                cell_width_px: 10,
                cell_height_px: 20,
            }
        );
    }

    #[test]
    fn binding_flags_preserve_and_query_abi_bits() {
        let flags =
            BindingFlags::from_raw(BindingFlags::CONSUMED.raw() | BindingFlags::PERFORMABLE.raw());
        assert!(flags.contains(BindingFlags::CONSUMED));
        assert!(flags.contains(BindingFlags::PERFORMABLE));
        assert!(!flags.contains(BindingFlags::GLOBAL));
    }

    #[test]
    fn surface_text_copy_handles_lengths_and_utf8() {
        let text = "selected text";
        let raw = ffi::ghostty_text_s {
            tl_px_x: 12.5,
            tl_px_y: 24.0,
            offset_start: 2,
            offset_len: 4,
            text: text.as_ptr().cast(),
            text_len: text.len(),
        };
        let copied = unsafe { copy_surface_text(&raw, "test") }.expect("valid text");
        assert_eq!(copied.text, text);
        assert_eq!(copied.top_left, SurfacePoint { x: 12.5, y: 24.0 });
        assert_eq!(copied.offset_start, 2);
        assert_eq!(copied.offset_len, 4);

        let null = ffi::ghostty_text_s {
            text: ptr::null(),
            text_len: 1,
            ..ffi::ghostty_text_s::default()
        };
        let result = unsafe { copy_surface_text(&null, "test") };
        assert_eq!(
            result,
            Err(SurfaceTextError::NullText { operation: "test" })
        );
    }

    #[test]
    fn screen_cells_trim_rows_and_keep_only_the_requested_suffix() {
        let values = [
            'a' as u32, ' ' as u32, 0, 'b' as u32, 'c' as u32, ' ' as u32, 0, 0, 0, 0, 0, 0,
        ];
        let cells: Vec<ffi::ghostty_cell_s> = values
            .into_iter()
            .map(|codepoint| ffi::ghostty_cell_s {
                codepoint,
                ..ffi::ghostty_cell_s::default()
            })
            .collect();
        assert_eq!(format_screen_cells(&cells, 3, 50), "a\nbc");
        assert_eq!(format_screen_cells(&cells, 3, 1), "bc");
    }

    #[test]
    fn release_unregisters_the_data_callback_before_freeing_the_surface() {
        let order = std::cell::RefCell::new(Vec::new());
        release_surface(
            7,
            |surface| order.borrow_mut().push(("unregister", surface)),
            |surface| order.borrow_mut().push(("free", surface)),
        );
        assert_eq!(order.into_inner(), [("unregister", 7), ("free", 7)]);
    }

    #[test]
    fn cross_thread_data_callback_quiesces_before_surface_storage_is_released() {
        static OUTPUT: &[u8] = b"shell output";

        let (events, _event_receiver) = async_channel::unbounded();
        let (data_events, data_event_receiver) = async_channel::bounded(1);
        let mut userdata = Box::new(SurfaceUserdata::new(
            SurfaceId::allocate(),
            events,
            data_events,
        ));
        let userdata_address = std::ptr::from_mut(userdata.as_mut()) as usize;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let callback_barrier = barrier.clone();
        let mut callback = Some(std::thread::spawn(move || {
            callback_barrier.wait();
            unsafe {
                surface_data_callback(
                    userdata_address as *mut c_void,
                    OUTPUT.as_ptr(),
                    OUTPUT.len(),
                )
            };
        }));

        release_surface(
            7,
            |_| {
                barrier.wait();
                callback
                    .take()
                    .expect("callback thread exists")
                    .join()
                    .expect("callback thread finishes");
            },
            |_| {
                assert!(matches!(
                    data_event_receiver.try_recv(),
                    Ok(crate::runtime::RuntimeEvent::Data { bytes, .. }) if bytes == OUTPUT
                ));
            },
        );
        drop(userdata);
    }
}
