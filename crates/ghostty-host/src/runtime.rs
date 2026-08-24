use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::ptr::{self, NonNull};
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread::{self, ThreadId};

use async_channel::{Receiver, Sender, TrySendError};
use ghostty_sys::ffi;
use thiserror::Error;

use crate::config::{ColorScheme, ConfigError, ConfigPaths, GhosttyConfig};
use crate::input::KeyboardInput;
use crate::mouse::{MouseShape, MouseVisibility};
use crate::surface::SurfaceId;

static INITIALIZED: OnceLock<Result<(), InitializationError>> = OnceLock::new();

unsafe extern "C" {
    fn pthread_main_np() -> c_int;
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InitializationError {
    #[error("process argument {index} contains an interior NUL byte")]
    ArgumentContainsNul { index: usize },
    #[error("ghostty_init failed with status {status}")]
    GhosttyInit { status: c_int },
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[error("Ghostty app and surface ownership is restricted to the main thread")]
pub struct MainThreadError;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AppError {
    #[error(transparent)]
    Initialization(#[from] InitializationError),
    #[error(transparent)]
    MainThread(#[from] MainThreadError),
    #[error("ghostty_app_new returned null")]
    CreationReturnedNull,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionTarget {
    App,
    Surface(Option<SurfaceId>),
    Unknown(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenUrlKind {
    Unknown,
    Text,
    Html,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenUrl {
    pub kind: OpenUrlKind,
    pub url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressState {
    Remove,
    Set,
    Error,
    Indeterminate,
    Pause,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressReport {
    pub state: ProgressState,
    pub percent: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scrollbar {
    pub total: u64,
    pub offset: u64,
    pub len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorKind {
    Foreground,
    Background,
    Cursor,
    Palette(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorChange {
    pub kind: ColorKind,
    pub color: RgbColor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    SetTitle(String),
    SetTabTitle(String),
    WorkingDirectory(PathBuf),
    Bell,
    MouseShape(MouseShape),
    MouseVisibility(MouseVisibility),
    MouseOverLink(Option<String>),
    OpenUrl(OpenUrl),
    SearchStart(String),
    SearchEnd,
    SearchTotal(Option<usize>),
    SearchSelected(Option<usize>),
    Progress(ProgressReport),
    Scrollbar(Scrollbar),
    ColorChange(ColorChange),
    ReloadConfig { soft: bool },
    Unsupported { tag: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionEvent {
    pub target: ActionTarget,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardLocation {
    Standard,
    Selection,
    Unknown(u32),
}

impl ClipboardLocation {
    fn from_raw(raw: ffi::ghostty_clipboard_e) -> Self {
        match raw {
            ffi::ghostty_clipboard_e_GHOSTTY_CLIPBOARD_STANDARD => Self::Standard,
            ffi::ghostty_clipboard_e_GHOSTTY_CLIPBOARD_SELECTION => Self::Selection,
            value => Self::Unknown(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipboardRequest {
    Paste,
    Osc52Read,
    Osc52Write,
    Unknown(u32),
}

impl ClipboardRequest {
    fn from_raw(raw: ffi::ghostty_clipboard_request_e) -> Self {
        match raw {
            ffi::ghostty_clipboard_request_e_GHOSTTY_CLIPBOARD_REQUEST_PASTE => Self::Paste,
            ffi::ghostty_clipboard_request_e_GHOSTTY_CLIPBOARD_REQUEST_OSC_52_READ => {
                Self::Osc52Read
            }
            ffi::ghostty_clipboard_request_e_GHOSTTY_CLIPBOARD_REQUEST_OSC_52_WRITE => {
                Self::Osc52Write
            }
            value => Self::Unknown(value),
        }
    }
}

#[derive(Eq, PartialEq)]
pub struct ClipboardRequestToken {
    surface_id: SurfaceId,
    state: NonZeroUsize,
}

impl fmt::Debug for ClipboardRequestToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClipboardRequestToken")
            .field("surface_id", &self.surface_id)
            .finish_non_exhaustive()
    }
}

impl ClipboardRequestToken {
    pub fn surface_id(&self) -> SurfaceId {
        self.surface_id
    }

    pub(crate) fn state_ptr(&self) -> *mut c_void {
        self.state.get() as *mut c_void
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClipboardContent {
    pub mime: Option<Vec<u8>>,
    pub data: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfaceProcessState {
    Alive,
    Exited,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RuntimeEvent {
    Action(ActionEvent),
    ClipboardRead {
        surface_id: SurfaceId,
        location: ClipboardLocation,
        token: ClipboardRequestToken,
    },
    ClipboardReadConfirmation {
        surface_id: SurfaceId,
        content: Option<Vec<u8>>,
        token: ClipboardRequestToken,
        request: ClipboardRequest,
    },
    ClipboardWrite {
        surface_id: SurfaceId,
        location: ClipboardLocation,
        contents: Vec<ClipboardContent>,
        confirm: bool,
    },
    Close {
        surface_id: SurfaceId,
        process: SurfaceProcessState,
    },
}

#[derive(Debug)]
struct RuntimeCallbackState {
    wakeups: Sender<()>,
    events: Sender<RuntimeEvent>,
}

#[derive(Debug)]
pub(crate) struct SurfaceUserdata {
    pub(crate) id: SurfaceId,
    events: Sender<RuntimeEvent>,
    clipboard_requests: Mutex<HashSet<NonZeroUsize>>,
}

impl SurfaceUserdata {
    pub(crate) fn new(id: SurfaceId, events: Sender<RuntimeEvent>) -> Self {
        Self {
            id,
            events,
            clipboard_requests: Mutex::new(HashSet::new()),
        }
    }

    fn clipboard_requests(&self) -> MutexGuard<'_, HashSet<NonZeroUsize>> {
        self.clipboard_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn begin_clipboard_request(&self, state: *mut c_void) -> Option<ClipboardRequestToken> {
        let state = NonZeroUsize::new(state as usize)?;
        self.clipboard_requests()
            .insert(state)
            .then_some(ClipboardRequestToken {
                surface_id: self.id,
                state,
            })
    }

    fn cancel_clipboard_request(&self, token: &ClipboardRequestToken) {
        if token.surface_id == self.id {
            self.clipboard_requests().remove(&token.state);
        }
    }

    pub(crate) fn take_clipboard_request(&self, token: &ClipboardRequestToken) -> bool {
        token.surface_id == self.id && self.clipboard_requests().remove(&token.state)
    }

    fn dispatch(&self, event: RuntimeEvent) -> bool {
        self.events.try_send(event).is_ok()
    }
}

#[derive(Debug)]
struct RuntimeBridge {
    callback_state: Box<RuntimeCallbackState>,
    wakeup_receiver: Receiver<()>,
    event_receiver: Receiver<RuntimeEvent>,
}

impl RuntimeBridge {
    fn new() -> Self {
        let (wakeup_sender, wakeup_receiver) = async_channel::bounded(1);
        let (event_sender, event_receiver) = async_channel::unbounded();
        Self {
            callback_state: Box::new(RuntimeCallbackState {
                wakeups: wakeup_sender,
                events: event_sender,
            }),
            wakeup_receiver,
            event_receiver,
        }
    }

    fn ffi_config(&self) -> ffi::ghostty_runtime_config_s {
        let userdata = std::ptr::from_ref(self.callback_state.as_ref())
            .cast_mut()
            .cast::<c_void>();
        ffi::ghostty_runtime_config_s {
            userdata,
            supports_selection_clipboard: true,
            wakeup_cb: Some(wakeup_callback),
            action_cb: Some(action_callback),
            read_clipboard_cb: Some(read_clipboard_callback),
            confirm_read_clipboard_cb: Some(confirm_read_clipboard_callback),
            write_clipboard_cb: Some(write_clipboard_callback),
            close_surface_cb: Some(close_surface_callback),
        }
    }

    fn wakeup_receiver(&self) -> Receiver<()> {
        self.wakeup_receiver.clone()
    }

    fn event_receiver(&self) -> Receiver<RuntimeEvent> {
        self.event_receiver.clone()
    }

    fn event_sender(&self) -> Sender<RuntimeEvent> {
        self.callback_state.events.clone()
    }
}

#[derive(Clone)]
pub struct GhosttyApp {
    inner: Rc<AppInner>,
    _main_thread_only: PhantomData<Rc<()>>,
}

struct AppInner {
    raw: NonNull<c_void>,
    config: RefCell<GhosttyConfig>,
    bridge: RuntimeBridge,
    owner_thread: ThreadId,
}

impl GhosttyApp {
    pub fn new(config: GhosttyConfig) -> Result<Self, AppError> {
        require_main_thread()?;
        initialize_ghostty()?;

        let bridge = RuntimeBridge::new();
        let runtime_config = bridge.ffi_config();
        let raw = unsafe { ffi::ghostty_app_new(&runtime_config, config.as_raw()) };
        let raw = NonNull::new(raw).ok_or(AppError::CreationReturnedNull)?;

        Ok(Self {
            inner: Rc::new(AppInner {
                raw,
                config: RefCell::new(config),
                bridge,
                owner_thread: thread::current().id(),
            }),
            _main_thread_only: PhantomData,
        })
    }

    pub fn wakeup_receiver(&self) -> Receiver<()> {
        self.assert_owner_thread();
        self.inner.bridge.wakeup_receiver()
    }

    pub fn event_receiver(&self) -> Receiver<RuntimeEvent> {
        self.assert_owner_thread();
        self.inner.bridge.event_receiver()
    }

    pub fn tick(&self) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_app_tick(self.as_raw()) };
    }

    pub fn set_focus(&self, focused: bool) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_app_set_focus(self.as_raw(), focused) };
    }

    pub fn send_key(&self, input: &KeyboardInput) -> bool {
        self.assert_owner_thread();
        let input = input.as_ffi();
        unsafe { ffi::ghostty_app_key(self.as_raw(), input) }
    }

    pub fn key_is_binding(&self, input: &KeyboardInput) -> bool {
        self.assert_owner_thread();
        let input = input.as_ffi();
        unsafe { ffi::ghostty_app_key_is_binding(self.as_raw(), input) }
    }

    pub fn keyboard_changed(&self) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_app_keyboard_changed(self.as_raw()) };
    }

    pub fn needs_confirm_quit(&self) -> bool {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_app_needs_confirm_quit(self.as_raw()) }
    }

    pub fn has_global_keybinds(&self) -> bool {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_app_has_global_keybinds(self.as_raw()) }
    }

    pub fn set_color_scheme(&self, scheme: ColorScheme) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_app_set_color_scheme(self.as_raw(), scheme.as_raw()) };
    }

    pub unsafe fn set_window_background_blur(&self, ns_window: NonNull<c_void>) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_set_window_background_blur(self.as_raw(), ns_window.as_ptr()) };
    }

    pub fn replace_config(&self, config: GhosttyConfig) {
        self.assert_owner_thread();
        unsafe { ffi::ghostty_app_update_config(self.as_raw(), config.as_raw()) };
        *self.inner.config.borrow_mut() = config;
    }

    pub fn reload_config(&self, paths: ConfigPaths) -> Result<Vec<String>, ConfigError> {
        self.assert_owner_thread();
        let config = GhosttyConfig::load(paths)?;
        let diagnostics = config.diagnostics().to_vec();
        self.replace_config(config);
        Ok(diagnostics)
    }

    pub fn clone_config(&self) -> Result<GhosttyConfig, ConfigError> {
        self.assert_owner_thread();
        self.inner.config.borrow().try_clone()
    }

    pub(crate) fn as_raw(&self) -> ffi::ghostty_app_t {
        self.inner.raw.as_ptr()
    }

    pub(crate) fn surface_event_sender(&self) -> Sender<RuntimeEvent> {
        self.inner.bridge.event_sender()
    }

    pub(crate) fn assert_owner_thread(&self) {
        assert_eq!(
            self.inner.owner_thread,
            thread::current().id(),
            "Ghostty app used outside its owning main thread"
        );
    }
}

impl Drop for AppInner {
    fn drop(&mut self) {
        debug_assert_eq!(self.owner_thread, thread::current().id());
        unsafe { ffi::ghostty_app_free(self.raw.as_ptr()) };
    }
}

pub fn initialize_ghostty() -> Result<(), InitializationError> {
    INITIALIZED.get_or_init(initialize_process).clone()
}

fn initialize_process() -> Result<(), InitializationError> {
    let args = std::env::args_os()
        .enumerate()
        .map(|(index, arg)| {
            CString::new(arg.as_os_str().as_bytes())
                .map_err(|_| InitializationError::ArgumentContainsNul { index })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let argc = args.len();
    let mut argv = args
        .iter()
        .map(|arg| arg.as_ptr().cast_mut())
        .collect::<Vec<*mut c_char>>();
    argv.push(ptr::null_mut());

    let status = unsafe { ffi::ghostty_init(argc, argv.as_mut_ptr()) };
    if status == ffi::GHOSTTY_SUCCESS as c_int {
        Ok(())
    } else {
        Err(InitializationError::GhosttyInit { status })
    }
}

pub(crate) fn require_main_thread() -> Result<(), MainThreadError> {
    let is_main = unsafe { pthread_main_np() } != 0;
    is_main.then_some(()).ok_or(MainThreadError)
}

unsafe extern "C" fn wakeup_callback(userdata: *mut c_void) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let Some(userdata) = NonNull::new(userdata).map(NonNull::cast::<RuntimeCallbackState>)
        else {
            return;
        };
        let state = unsafe { userdata.as_ref() };
        match state.wakeups.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) | Err(TrySendError::Closed(())) => {}
        }
    }));
}

unsafe extern "C" fn action_callback(
    app: ffi::ghostty_app_t,
    target: ffi::ghostty_target_s,
    action: ffi::ghostty_action_s,
) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        if app.is_null() {
            return false;
        }
        let userdata = unsafe { ffi::ghostty_app_userdata(app) };
        let Some(userdata) = NonNull::new(userdata).map(NonNull::cast::<RuntimeCallbackState>)
        else {
            return false;
        };
        let state = unsafe { userdata.as_ref() };
        unsafe { dispatch_action(&state.events, copy_action_target(target), action) }
    }))
    .unwrap_or(false)
}

enum DecodedAction {
    Supported(Action),
    Unsupported(u32),
    Invalid(u32),
}

unsafe fn dispatch_action(
    sender: &Sender<RuntimeEvent>,
    target: ActionTarget,
    raw: ffi::ghostty_action_s,
) -> bool {
    match unsafe { decode_action(raw) } {
        DecodedAction::Supported(action) => {
            let target_is_valid =
                matches!(target, ActionTarget::App | ActionTarget::Surface(Some(_)));
            let dispatched = sender
                .try_send(RuntimeEvent::Action(ActionEvent { target, action }))
                .is_ok();
            target_is_valid && dispatched
        }
        DecodedAction::Unsupported(tag) | DecodedAction::Invalid(tag) => {
            let _ = sender.try_send(RuntimeEvent::Action(ActionEvent {
                target,
                action: Action::Unsupported { tag },
            }));
            false
        }
    }
}

unsafe fn decode_action(raw: ffi::ghostty_action_s) -> DecodedAction {
    let action = match raw.tag {
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_SET_TITLE => {
            let payload = unsafe { raw.action.set_title };
            let Some(title) = (unsafe { copy_required_utf8_c_string(payload.title) }) else {
                return DecodedAction::Invalid(raw.tag);
            };
            Action::SetTitle(title)
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_SET_TAB_TITLE => {
            let payload = unsafe { raw.action.set_tab_title };
            let Some(title) = (unsafe { copy_required_utf8_c_string(payload.title) }) else {
                return DecodedAction::Invalid(raw.tag);
            };
            Action::SetTabTitle(title)
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_PWD => {
            let payload = unsafe { raw.action.pwd };
            let Some(path) = (unsafe { copy_required_path(payload.pwd) }) else {
                return DecodedAction::Invalid(raw.tag);
            };
            Action::WorkingDirectory(path)
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_RING_BELL => Action::Bell,
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_MOUSE_SHAPE => {
            let raw_shape = unsafe { raw.action.mouse_shape };
            let Some(shape) = MouseShape::from_raw(raw_shape) else {
                return DecodedAction::Invalid(raw.tag);
            };
            Action::MouseShape(shape)
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_MOUSE_VISIBILITY => {
            let raw_visibility = unsafe { raw.action.mouse_visibility };
            let Some(visibility) = MouseVisibility::from_raw(raw_visibility) else {
                return DecodedAction::Invalid(raw.tag);
            };
            Action::MouseVisibility(visibility)
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_MOUSE_OVER_LINK => {
            let payload = unsafe { raw.action.mouse_over_link };
            let link = if payload.len == 0 {
                None
            } else {
                let Some(link) = (unsafe { copy_utf8_slice(payload.url.cast(), payload.len) })
                else {
                    return DecodedAction::Invalid(raw.tag);
                };
                Some(link)
            };
            Action::MouseOverLink(link)
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_OPEN_URL => {
            let payload = unsafe { raw.action.open_url };
            let kind = match payload.kind {
                ffi::ghostty_action_open_url_kind_e_GHOSTTY_ACTION_OPEN_URL_KIND_UNKNOWN => {
                    OpenUrlKind::Unknown
                }
                ffi::ghostty_action_open_url_kind_e_GHOSTTY_ACTION_OPEN_URL_KIND_TEXT => {
                    OpenUrlKind::Text
                }
                ffi::ghostty_action_open_url_kind_e_GHOSTTY_ACTION_OPEN_URL_KIND_HTML => {
                    OpenUrlKind::Html
                }
                _ => return DecodedAction::Invalid(raw.tag),
            };
            if payload.len == 0 {
                return DecodedAction::Invalid(raw.tag);
            }
            let Some(url) = (unsafe { copy_utf8_slice(payload.url.cast(), payload.len) }) else {
                return DecodedAction::Invalid(raw.tag);
            };
            Action::OpenUrl(OpenUrl { kind, url })
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_START_SEARCH => {
            let payload = unsafe { raw.action.start_search };
            let Some(needle) = (unsafe { copy_required_utf8_c_string(payload.needle) }) else {
                return DecodedAction::Invalid(raw.tag);
            };
            Action::SearchStart(needle)
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_END_SEARCH => Action::SearchEnd,
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_SEARCH_TOTAL => {
            let value = unsafe { raw.action.search_total }.total;
            let Some(total) = decode_optional_index(value) else {
                return DecodedAction::Invalid(raw.tag);
            };
            Action::SearchTotal(total)
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_SEARCH_SELECTED => {
            let value = unsafe { raw.action.search_selected }.selected;
            let Some(selected) = decode_optional_index(value) else {
                return DecodedAction::Invalid(raw.tag);
            };
            Action::SearchSelected(selected)
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_PROGRESS_REPORT => {
            let payload = unsafe { raw.action.progress_report };
            let state = match payload.state {
                ffi::ghostty_action_progress_report_state_e_GHOSTTY_PROGRESS_STATE_REMOVE => {
                    ProgressState::Remove
                }
                ffi::ghostty_action_progress_report_state_e_GHOSTTY_PROGRESS_STATE_SET => {
                    ProgressState::Set
                }
                ffi::ghostty_action_progress_report_state_e_GHOSTTY_PROGRESS_STATE_ERROR => {
                    ProgressState::Error
                }
                ffi::ghostty_action_progress_report_state_e_GHOSTTY_PROGRESS_STATE_INDETERMINATE => {
                    ProgressState::Indeterminate
                }
                ffi::ghostty_action_progress_report_state_e_GHOSTTY_PROGRESS_STATE_PAUSE => {
                    ProgressState::Pause
                }
                _ => return DecodedAction::Invalid(raw.tag),
            };
            let percent = match payload.progress {
                -1 => None,
                0..=100 => Some(payload.progress as u8),
                _ => return DecodedAction::Invalid(raw.tag),
            };
            Action::Progress(ProgressReport { state, percent })
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_SCROLLBAR => {
            let payload = unsafe { raw.action.scrollbar };
            Action::Scrollbar(Scrollbar {
                total: payload.total,
                offset: payload.offset,
                len: payload.len,
            })
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_COLOR_CHANGE => {
            let payload = unsafe { raw.action.color_change };
            let kind = match payload.kind {
                ffi::ghostty_action_color_kind_e_GHOSTTY_ACTION_COLOR_KIND_FOREGROUND => {
                    ColorKind::Foreground
                }
                ffi::ghostty_action_color_kind_e_GHOSTTY_ACTION_COLOR_KIND_BACKGROUND => {
                    ColorKind::Background
                }
                ffi::ghostty_action_color_kind_e_GHOSTTY_ACTION_COLOR_KIND_CURSOR => {
                    ColorKind::Cursor
                }
                0..=255 => ColorKind::Palette(payload.kind as u8),
                _ => return DecodedAction::Invalid(raw.tag),
            };
            Action::ColorChange(ColorChange {
                kind,
                color: RgbColor {
                    red: payload.r,
                    green: payload.g,
                    blue: payload.b,
                },
            })
        }
        ffi::ghostty_action_tag_e_GHOSTTY_ACTION_RELOAD_CONFIG => {
            let soft = unsafe { raw.action.reload_config }.soft;
            Action::ReloadConfig { soft }
        }
        tag => return DecodedAction::Unsupported(tag),
    };
    DecodedAction::Supported(action)
}

fn decode_optional_index(value: isize) -> Option<Option<usize>> {
    match value {
        -1 => Some(None),
        0.. => usize::try_from(value).ok().map(Some),
        _ => None,
    }
}

fn copy_action_target(target: ffi::ghostty_target_s) -> ActionTarget {
    match target.tag {
        ffi::ghostty_target_tag_e_GHOSTTY_TARGET_APP => ActionTarget::App,
        ffi::ghostty_target_tag_e_GHOSTTY_TARGET_SURFACE => {
            let surface = unsafe { target.target.surface };
            let id = if surface.is_null() {
                None
            } else {
                let userdata = unsafe { ffi::ghostty_surface_userdata(surface) };
                NonNull::new(userdata)
                    .map(NonNull::cast::<SurfaceUserdata>)
                    .map(|userdata| unsafe { userdata.as_ref().id })
            };
            ActionTarget::Surface(id)
        }
        value => ActionTarget::Unknown(value),
    }
}

unsafe extern "C" fn read_clipboard_callback(
    userdata: *mut c_void,
    location: ffi::ghostty_clipboard_e,
    state: *mut c_void,
) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let Some(userdata) = NonNull::new(userdata).map(NonNull::cast::<SurfaceUserdata>) else {
            return false;
        };
        let userdata = unsafe { userdata.as_ref() };
        let Some(token) = userdata.begin_clipboard_request(state) else {
            return false;
        };
        let event = RuntimeEvent::ClipboardRead {
            surface_id: userdata.id,
            location: ClipboardLocation::from_raw(location),
            token,
        };
        match userdata.events.try_send(event) {
            Ok(()) => true,
            Err(error) => {
                let RuntimeEvent::ClipboardRead { token, .. } = error.into_inner() else {
                    unreachable!("clipboard read send returned a different event")
                };
                userdata.cancel_clipboard_request(&token);
                false
            }
        }
    }))
    .unwrap_or(false)
}

unsafe extern "C" fn confirm_read_clipboard_callback(
    userdata: *mut c_void,
    content: *const c_char,
    state: *mut c_void,
    request: ffi::ghostty_clipboard_request_e,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let Some(userdata) = NonNull::new(userdata).map(NonNull::cast::<SurfaceUserdata>) else {
            return;
        };
        let userdata = unsafe { userdata.as_ref() };
        let content = unsafe { copy_optional_c_string(content) };
        let Some(token) = userdata.begin_clipboard_request(state) else {
            return;
        };
        let event = RuntimeEvent::ClipboardReadConfirmation {
            surface_id: userdata.id,
            content,
            token,
            request: ClipboardRequest::from_raw(request),
        };
        if let Err(error) = userdata.events.try_send(event) {
            let RuntimeEvent::ClipboardReadConfirmation { token, .. } = error.into_inner() else {
                unreachable!("clipboard confirmation send returned a different event")
            };
            userdata.cancel_clipboard_request(&token);
        }
    }));
}

unsafe extern "C" fn write_clipboard_callback(
    userdata: *mut c_void,
    location: ffi::ghostty_clipboard_e,
    content: *const ffi::ghostty_clipboard_content_s,
    len: usize,
    confirm: bool,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let Some(userdata) = NonNull::new(userdata).map(NonNull::cast::<SurfaceUserdata>) else {
            return;
        };
        let userdata = unsafe { userdata.as_ref() };
        let Some(contents) = (unsafe { copy_clipboard_contents(content, len) }) else {
            return;
        };
        userdata.dispatch(RuntimeEvent::ClipboardWrite {
            surface_id: userdata.id,
            location: ClipboardLocation::from_raw(location),
            contents,
            confirm,
        });
    }));
}

unsafe extern "C" fn close_surface_callback(userdata: *mut c_void, process_alive: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let Some(userdata) = NonNull::new(userdata).map(NonNull::cast::<SurfaceUserdata>) else {
            return;
        };
        let userdata = unsafe { userdata.as_ref() };
        userdata.dispatch(RuntimeEvent::Close {
            surface_id: userdata.id,
            process: if process_alive {
                SurfaceProcessState::Alive
            } else {
                SurfaceProcessState::Exited
            },
        });
    }));
}

unsafe fn copy_required_utf8_c_string(value: *const c_char) -> Option<String> {
    if value.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(value) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

unsafe fn copy_required_path(value: *const c_char) -> Option<PathBuf> {
    if value.is_null() {
        return None;
    }
    let bytes = unsafe { CStr::from_ptr(value) }.to_bytes().to_vec();
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

unsafe fn copy_utf8_slice(value: *const u8, len: usize) -> Option<String> {
    if len == 0 {
        return Some(String::new());
    }
    if value.is_null() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(value, len) };
    std::str::from_utf8(bytes).ok().map(str::to_owned)
}

unsafe fn copy_optional_c_string(value: *const c_char) -> Option<Vec<u8>> {
    if value.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(value) }.to_bytes().to_vec())
    }
}

unsafe fn copy_clipboard_contents(
    content: *const ffi::ghostty_clipboard_content_s,
    len: usize,
) -> Option<Vec<ClipboardContent>> {
    if len == 0 {
        return Some(Vec::new());
    }
    if content.is_null() {
        return None;
    }
    let content = unsafe { std::slice::from_raw_parts(content, len) };
    Some(
        content
            .iter()
            .map(|item| ClipboardContent {
                mime: unsafe { copy_optional_c_string(item.mime) },
                data: unsafe { copy_optional_c_string(item.data) },
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_channel::TryRecvError;

    fn raw_action(
        tag: ffi::ghostty_action_tag_e,
        action: ffi::ghostty_action_u,
    ) -> ffi::ghostty_action_s {
        ffi::ghostty_action_s { tag, action }
    }

    #[test]
    fn wakeups_coalesce_at_capacity_one() {
        let bridge = RuntimeBridge::new();
        let userdata = std::ptr::from_ref(bridge.callback_state.as_ref())
            .cast_mut()
            .cast::<c_void>();
        unsafe {
            wakeup_callback(userdata);
            wakeup_callback(userdata);
        }
        assert_eq!(bridge.wakeup_receiver.try_recv(), Ok(()));
        assert_eq!(bridge.wakeup_receiver.try_recv(), Err(TryRecvError::Empty));
    }

    #[test]
    fn supported_string_action_is_owned_and_handled() {
        let bridge = RuntimeBridge::new();
        let title = CString::new("shell title").expect("literal has no NUL");
        let raw = raw_action(
            ffi::ghostty_action_tag_e_GHOSTTY_ACTION_SET_TITLE,
            ffi::ghostty_action_u {
                set_title: ffi::ghostty_action_set_title_s {
                    title: title.as_ptr(),
                },
            },
        );
        assert!(unsafe { dispatch_action(&bridge.callback_state.events, ActionTarget::App, raw) });
        drop(title);
        assert_eq!(
            bridge.event_receiver.try_recv(),
            Ok(RuntimeEvent::Action(ActionEvent {
                target: ActionTarget::App,
                action: Action::SetTitle("shell title".to_owned()),
            }))
        );
    }

    #[test]
    fn unsupported_action_dispatches_diagnostic_and_returns_unhandled() {
        let bridge = RuntimeBridge::new();
        let tag = ffi::ghostty_action_tag_e_GHOSTTY_ACTION_NEW_WINDOW;
        let raw = raw_action(tag, ffi::ghostty_action_u::default());
        assert!(!unsafe { dispatch_action(&bridge.callback_state.events, ActionTarget::App, raw) });
        assert_eq!(
            bridge.event_receiver.try_recv(),
            Ok(RuntimeEvent::Action(ActionEvent {
                target: ActionTarget::App,
                action: Action::Unsupported { tag },
            }))
        );
    }

    #[test]
    fn invalid_target_is_observable_but_returns_unhandled() {
        let bridge = RuntimeBridge::new();
        let raw = raw_action(
            ffi::ghostty_action_tag_e_GHOSTTY_ACTION_RING_BELL,
            ffi::ghostty_action_u::default(),
        );
        let target = ActionTarget::Unknown(99);
        assert!(!unsafe { dispatch_action(&bridge.callback_state.events, target, raw) });
        assert_eq!(
            bridge.event_receiver.try_recv(),
            Ok(RuntimeEvent::Action(ActionEvent {
                target,
                action: Action::Bell,
            }))
        );
    }

    #[test]
    fn invalid_supported_payload_dispatches_diagnostic_and_is_unhandled() {
        let bridge = RuntimeBridge::new();
        let tag = ffi::ghostty_action_tag_e_GHOSTTY_ACTION_OPEN_URL;
        let raw = raw_action(
            tag,
            ffi::ghostty_action_u {
                open_url: ffi::ghostty_action_open_url_s {
                    kind: ffi::ghostty_action_open_url_kind_e_GHOSTTY_ACTION_OPEN_URL_KIND_UNKNOWN,
                    url: ptr::null(),
                    len: 4,
                },
            },
        );
        assert!(!unsafe { dispatch_action(&bridge.callback_state.events, ActionTarget::App, raw) });
        assert_eq!(
            bridge.event_receiver.try_recv(),
            Ok(RuntimeEvent::Action(ActionEvent {
                target: ActionTarget::App,
                action: Action::Unsupported { tag },
            }))
        );
    }

    #[test]
    fn progress_and_optional_search_values_are_validated() {
        let valid = raw_action(
            ffi::ghostty_action_tag_e_GHOSTTY_ACTION_PROGRESS_REPORT,
            ffi::ghostty_action_u {
                progress_report: ffi::ghostty_action_progress_report_s {
                    state: ffi::ghostty_action_progress_report_state_e_GHOSTTY_PROGRESS_STATE_SET,
                    progress: 100,
                },
            },
        );
        let decoded = unsafe { decode_action(valid) };
        assert!(matches!(
            decoded,
            DecodedAction::Supported(Action::Progress(ProgressReport {
                state: ProgressState::Set,
                percent: Some(100)
            }))
        ));

        let invalid = raw_action(
            ffi::ghostty_action_tag_e_GHOSTTY_ACTION_SEARCH_TOTAL,
            ffi::ghostty_action_u {
                search_total: ffi::ghostty_action_search_total_s { total: -2 },
            },
        );
        let decoded = unsafe { decode_action(invalid) };
        assert!(matches!(
            decoded,
            DecodedAction::Invalid(ffi::ghostty_action_tag_e_GHOSTTY_ACTION_SEARCH_TOTAL)
        ));
    }

    #[test]
    fn clipboard_request_requires_nonzero_unique_state() {
        let bridge = RuntimeBridge::new();
        let id = SurfaceId::allocate();
        let userdata = SurfaceUserdata::new(id, bridge.event_sender());
        assert!(userdata.begin_clipboard_request(ptr::null_mut()).is_none());

        let state = NonNull::<u8>::dangling().as_ptr().cast::<c_void>();
        let token = userdata
            .begin_clipboard_request(state)
            .expect("nonzero state is accepted");
        assert!(userdata.begin_clipboard_request(state).is_none());
        assert!(userdata.take_clipboard_request(&token));
        assert!(!userdata.take_clipboard_request(&token));
    }

    #[test]
    fn clipboard_read_callback_owns_token_and_returns_handled() {
        let bridge = RuntimeBridge::new();
        let id = SurfaceId::allocate();
        let mut userdata = SurfaceUserdata::new(id, bridge.event_sender());
        let state = NonNull::<u8>::dangling().as_ptr().cast::<c_void>();
        let handled = unsafe {
            read_clipboard_callback(
                std::ptr::from_mut(&mut userdata).cast(),
                ffi::ghostty_clipboard_e_GHOSTTY_CLIPBOARD_STANDARD,
                state,
            )
        };
        assert!(handled);
        let RuntimeEvent::ClipboardRead {
            surface_id,
            location,
            token,
        } = bridge.event_receiver.try_recv().expect("clipboard event")
        else {
            panic!("unexpected runtime event")
        };
        assert_eq!(surface_id, id);
        assert_eq!(location, ClipboardLocation::Standard);
        assert!(userdata.take_clipboard_request(&token));
        assert!(!userdata.take_clipboard_request(&token));
    }

    #[test]
    fn clipboard_content_array_is_deep_copied() {
        let mime = CString::new("text/plain").expect("literal has no NUL");
        let data = CString::new("copied text").expect("literal has no NUL");
        let raw = [ffi::ghostty_clipboard_content_s {
            mime: mime.as_ptr(),
            data: data.as_ptr(),
        }];
        let copied = unsafe { copy_clipboard_contents(raw.as_ptr(), raw.len()) }
            .expect("valid clipboard array");
        drop((mime, data));
        assert_eq!(
            copied,
            vec![ClipboardContent {
                mime: Some(b"text/plain".to_vec()),
                data: Some(b"copied text".to_vec()),
            }]
        );

        let empty = unsafe { copy_clipboard_contents(ptr::null(), 0) };
        assert_eq!(empty, Some(Vec::new()));
        let invalid = unsafe { copy_clipboard_contents(ptr::null(), 1) };
        assert_eq!(invalid, None);
    }

    #[test]
    fn confirmation_callback_copies_content_and_registers_token() {
        let bridge = RuntimeBridge::new();
        let id = SurfaceId::allocate();
        let mut userdata = SurfaceUserdata::new(id, bridge.event_sender());
        let content = CString::new("unsafe\npaste").expect("literal has no NUL");
        let state = NonNull::<u8>::dangling().as_ptr().cast::<c_void>();
        unsafe {
            confirm_read_clipboard_callback(
                std::ptr::from_mut(&mut userdata).cast(),
                content.as_ptr(),
                state,
                ffi::ghostty_clipboard_request_e_GHOSTTY_CLIPBOARD_REQUEST_PASTE,
            )
        };
        drop(content);
        let RuntimeEvent::ClipboardReadConfirmation {
            surface_id,
            content,
            token,
            request,
        } = bridge
            .event_receiver
            .try_recv()
            .expect("confirmation event")
        else {
            panic!("unexpected runtime event")
        };
        assert_eq!(surface_id, id);
        assert_eq!(content, Some(b"unsafe\npaste".to_vec()));
        assert_eq!(request, ClipboardRequest::Paste);
        assert!(userdata.take_clipboard_request(&token));
        assert!(!userdata.take_clipboard_request(&token));
    }

    #[test]
    fn callback_panics_are_contained() {
        let (sender, receiver) = async_channel::unbounded();
        drop(receiver);
        let userdata = SurfaceUserdata::new(SurfaceId::allocate(), sender);
        let pointer = std::ptr::from_ref(&userdata).cast_mut().cast();
        let result = catch_unwind(AssertUnwindSafe(|| {
            unsafe { close_surface_callback(pointer, true) };
        }));
        assert!(result.is_ok());
    }
}
