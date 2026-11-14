use crate::webview::Snapshot;
pub use polybrowser;
use polybrowser::{BrowserEvent, Engine, Profile};
#[cfg(not(windows))]
pub const ENGINE: Engine = Engine::WebKit;
#[cfg(windows)]
pub const ENGINE: Engine = Engine::WebView2;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod unsupported;
#[cfg(target_os = "linux")]
pub use crate::webview::linux::browser::{
    NativeBrowser, clear_site_data, close_profile, import_cookies, installed_browsers,
};
#[cfg(target_os = "macos")]
pub use macos::{NativeBrowser, installed_browsers};
#[cfg(not(target_os = "linux"))]
pub use polybrowser::{clear_site_data, import_cookies};
#[cfg(windows)]
pub use unsupported::{NativeBrowser, close_profile, installed_browsers};

#[derive(Debug)]
pub enum Event {
    Page {
        navigation: u64,
        event: BrowserEvent,
    },
    Surface(crate::webview::Event),
    ContextMenu(gpui::Point<gpui::Pixels>),
    Escape,
    Shortcut(gpui::Keystroke),
    /// The user clicked into the page.
    Clicked,
    Snapshot {
        id: u64,
        image: Snapshot,
    },
}

/// What a new page starts with.
#[derive(Debug)]
pub struct Options<'a> {
    pub profile: &'a Profile,
    pub url: Option<&'a str>,
    /// Whether the engine may load an address; others are cancelled.
    pub allows: fn(&str) -> bool,
}
