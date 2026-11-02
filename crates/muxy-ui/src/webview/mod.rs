//! Extension pages. A backend shows a page and reports what it does; what a
//! page is allowed to do is the same for every backend and lives in `rules`.

pub mod assets;
#[cfg(target_os = "macos")]
pub(crate) mod host;
mod rules;
#[cfg(target_os = "macos")]
pub(crate) use macos::parent_view;

use std::path::Path;

use async_channel::Receiver;
use gpui::{Bounds, CursorStyle, Keystroke, Modifiers, MouseButton, Pixels, Point, Rgba, Window};
use muxy_core::capability::Capability;

use assets::Source;

#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod unsupported;
#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use unsupported as backend;

pub use backend::CAPABILITY;

/// Premultiplied BGRA pixels of a page at the display's scale.
pub struct Snapshot {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Snapshot({}x{})", self.width, self.height)
    }
}
type AssetResult = Result<(Vec<u8>, &'static str), String>;

/// What the pointer did over a page the app draws, in points from the page's
/// top left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pointer {
    Moved {
        position: Point<Pixels>,
        pressed: Option<MouseButton>,
        modifiers: Modifiers,
    },
    Pressed {
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    },
    Released {
        position: Point<Pixels>,
        button: MouseButton,
        modifiers: Modifiers,
    },
    /// `delta` is how far the content moves, as GPUI reports it.
    Scrolled {
        position: Point<Pixels>,
        delta: Point<Pixels>,
        modifiers: Modifiers,
    },
    Left,
}

/// One row of a `<select>` a page asked to open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuOption {
    pub label: String,
    pub enabled: bool,
    pub selected: bool,
    /// The title of a group of options, which cannot be chosen.
    pub heading: bool,
}

/// A `<select>` a page asked to open, anchored to the control in points from
/// the page's top left corner.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionMenu {
    pub options: Vec<MenuOption>,
    pub anchor: Bounds<Pixels>,
}

#[derive(Debug)]
pub enum Event {
    Message {
        id: u64,
        generation: u64,
        json: String,
    },
    Navigated,
    Loaded,
    Failed(String),
    Escape,
    Shortcut(Keystroke),
    Snapshot {
        id: u64,
        generation: u64,
        image: Snapshot,
    },
    Asset {
        id: u64,
        result: AssetResult,
    },
    /// The pointer's shape over a page the app draws changed.
    Cursor(CursorStyle),
    /// A page the app draws put text on the clipboard.
    Copied(String),
    /// A page the app draws opened a `<select>`, or closed the one it had open.
    Menu(Option<OptionMenu>),
}

/// Whether what the backend loads at run time is there to show pages.
/// Loading it can take a moment, so ask off the main thread.
pub fn probe() -> Capability {
    backend::probe()
}

/// Where pages keep what they store, for a backend that does not keep it with
/// the app's own data. Set it before the first page opens.
pub fn set_data_directory(directory: &Path) {
    backend::set_data_directory(directory);
}

/// One extension page and the engine showing it.
#[derive(Debug)]
pub struct NativeWebview(backend::NativeWebview);

impl NativeWebview {
    /// Whether the app draws the page from the snapshots it is sent and
    /// passes it the pointer and the keyboard. Otherwise the system shows the
    /// page in a native view above the window and delivers input to it.
    pub const DRAWN_BY_APP: bool = backend::DRAWN_BY_APP;

    /// Loads `source` with `script` run at the start of each of its documents.
    pub fn new(
        window: &Window,
        source: Source,
        script: &str,
        background: Rgba,
        transparent_background: bool,
    ) -> Result<(Self, Receiver<Event>), String> {
        backend::NativeWebview::new(window, source, script, background, transparent_background)
            .map(|(view, events)| (Self(view), events))
    }

    /// Which keys the app takes back from the page: Escape while `modal`, and
    /// `shortcuts` where the system gives the page its keys first.
    pub fn set_shortcuts(&self, modal: bool, shortcuts: Vec<Keystroke>) {
        self.0.set_shortcuts(modal, shortcuts);
    }

    /// Whether the app shows this page focused. Keys reach the page only
    /// while it is.
    pub fn set_focused(&self, focused: bool) {
        self.0.set_focused(focused);
    }

    /// The resize grips drawn over the page, in window coordinates. Call
    /// after [`Self::occlude`].
    pub fn set_grips(&self, grips: &[(Bounds<Pixels>, CursorStyle)]) {
        self.0.set_grips(grips);
    }

    /// Keeps the pointer from the page, for a drag that crosses it.
    pub fn set_mouse_passthrough(&self, enabled: bool) {
        self.0.set_mouse_passthrough(enabled);
    }

    /// Counts the documents the page has shown. A request or snapshot from an
    /// earlier one is void.
    pub fn generation(&self) -> u64 {
        self.0.generation()
    }

    pub fn install_script(&self, script: &str) {
        self.0.install_script(script);
    }

    pub fn evaluate(&self, script: &str) {
        self.0.evaluate(script);
    }

    /// Answers the request `id` of document `generation` with `json`.
    pub fn reply(&self, id: u64, generation: u64, json: &str) {
        self.0.reply(id, generation, json);
    }

    /// Finishes the asset load an [`Event::Asset`] reported.
    pub fn complete_asset(&self, id: u64, result: AssetResult) {
        self.0.complete_asset(id, result);
    }

    /// Where the page is in the window this frame, and the display's scale.
    pub fn sync(
        &self,
        bounds: Bounds<Pixels>,
        clip: Bounds<Pixels>,
        background: Rgba,
        radius: f64,
        scale: f32,
    ) {
        self.0.sync(bounds, clip, background, radius, scale);
    }

    /// App content drawn above the page, in window coordinates.
    pub fn occlude(&self, occlusions: &[Bounds<Pixels>]) {
        self.0.occlude(occlusions);
    }

    /// A hidden page stays loaded and stops drawing.
    pub fn set_visible(&self, visible: bool) {
        self.0.set_visible(visible);
    }

    pub fn focus(&self) {
        self.0.focus();
    }

    pub fn blur(&self) {
        self.0.blur();
    }

    /// Asks for an [`Event::Snapshot`] of the page as it is now.
    pub fn snapshot(&self) {
        self.0.snapshot();
    }

    /// Whether the snapshot `id` of document `generation` is still the one to
    /// show. A backend that paces its snapshots sends the next one after this.
    pub fn accept_snapshot(&self, id: u64, generation: u64) -> bool {
        self.0.accept_snapshot(id, generation)
    }

    /// Passes the pointer to a page the app draws.
    pub fn pointer(&self, event: Pointer) {
        self.0.pointer(event);
    }

    /// Passes a key to a page the app draws. `clipboard` is read when the key
    /// pastes.
    pub fn key(
        &self,
        keystroke: &Keystroke,
        pressed: bool,
        clipboard: impl FnOnce() -> Option<String>,
    ) {
        self.0.key(keystroke, pressed, clipboard);
    }

    /// Answers an [`Event::Menu`]: the option chosen, or `None` to close it.
    pub fn choose(&self, option: Option<usize>) {
        self.0.choose(option);
    }
}
