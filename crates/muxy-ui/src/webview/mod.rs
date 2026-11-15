pub mod assets;
#[cfg(target_os = "macos")]
pub(crate) mod host;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod rules;

use std::path::Path;
use std::rc::Rc;

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
    Scrolled {
        position: Point<Pixels>,
        delta: Point<Pixels>,
        modifiers: Modifiers,
    },
    Left,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MenuOption {
    pub label: String,
    pub enabled: bool,
    pub selected: bool,
    pub heading: bool,
}

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
    Cursor(CursorStyle),
    Copied(String),
    Menu(Option<OptionMenu>),
}

pub fn probe() -> Capability {
    backend::probe()
}

pub fn set_data_directory(directory: &Path) {
    backend::set_data_directory(directory);
}

#[derive(Debug)]
pub struct NativeWebview(backend::NativeWebview);

impl NativeWebview {
    pub const DRAWN_BY_APP: bool = backend::DRAWN_BY_APP;

    pub fn new(
        window: &Window,
        source: Source,
        script: &str,
        background: Rgba,
    ) -> Result<(Self, Receiver<Event>), String> {
        backend::NativeWebview::new(window, source, script, background)
            .map(|(view, events)| (Self(view), events))
    }

    pub fn set_shortcuts(&self, modal: bool, shortcuts: Rc<Vec<Keystroke>>) {
        self.0.set_shortcuts(modal, shortcuts);
    }

    pub fn set_focused(&self, focused: bool) {
        self.0.set_focused(focused);
    }

    pub fn take_click(&self) -> bool {
        self.0.take_click()
    }

    pub fn set_grips(&self, grips: &[(Bounds<Pixels>, CursorStyle)]) {
        self.0.set_grips(grips);
    }

    pub fn set_mouse_passthrough(&self, enabled: bool) {
        self.0.set_mouse_passthrough(enabled);
    }

    pub fn set_hit_regions(&self, regions: Option<&[Bounds<Pixels>]>) {
        self.0.set_hit_regions(regions);
    }

    pub fn move_to_window(&self, window: &Window) -> Result<(), String> {
        self.0.move_to_window(window)
    }

    pub fn order_front(views: &[&Self]) {
        let views: Vec<_> = views.iter().map(|view| &view.0).collect();
        backend::NativeWebview::order_front(&views);
    }

    pub fn generation(&self) -> u64 {
        self.0.generation()
    }

    pub fn install_script(&self, script: &str) {
        self.0.install_script(script);
    }

    pub fn evaluate(&self, script: &str) {
        self.0.evaluate(script);
    }

    pub fn reply(&self, id: u64, generation: u64, json: &str) {
        self.0.reply(id, generation, json);
    }

    pub fn complete_asset(&self, id: u64, result: AssetResult) {
        self.0.complete_asset(id, result);
    }

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

    pub fn occlude(&self, occlusions: &[Bounds<Pixels>]) {
        self.0.occlude(occlusions);
    }

    pub fn occlude_also(&self, bounds: Bounds<Pixels>) {
        self.0.occlude_also(bounds);
    }

    pub fn set_visible(&self, visible: bool) {
        self.0.set_visible(visible);
    }

    pub fn focus(&self) {
        self.0.focus();
    }

    pub fn blur(&self) {
        self.0.blur();
    }

    pub fn snapshot(&self) {
        self.0.snapshot();
    }

    pub fn accept_snapshot(&self, id: u64, generation: u64) -> bool {
        self.0.accept_snapshot(id, generation)
    }

    pub fn pointer(&self, event: Pointer) {
        self.0.pointer(event);
    }

    pub fn key(
        &self,
        keystroke: &Keystroke,
        pressed: bool,
        clipboard: impl FnOnce() -> Option<String>,
    ) {
        self.0.key(keystroke, pressed, clipboard);
    }

    pub fn choose(&self, option: Option<usize>) {
        self.0.choose(option);
    }
}
