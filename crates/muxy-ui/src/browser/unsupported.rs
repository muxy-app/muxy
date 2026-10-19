//! Browser panes on platforms without a web engine backend yet. No page can
//! be made, so a browser never exists.

use std::path::PathBuf;
use std::time::Duration;

use async_channel::Receiver;
use gpui::{Bounds, CursorStyle, Keystroke, Pixels, Window};
use polybrowser::{Cookie, FindOptions, Image, Profile, Result};

use super::{Event, Options};
use crate::webview::Pointer;

#[derive(Debug)]
pub enum NativeBrowser {}

#[allow(
    clippy::unused_self,
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    reason = "Matches the browser backends' API"
)]
impl NativeBrowser {
    pub const DRAWN_BY_APP: bool = false;

    pub fn new(
        _window: &Window,
        _options: &Options<'_>,
    ) -> std::result::Result<(Self, Receiver<Event>), String> {
        Err(muxy_core::capability::Feature::BrowserPanes
            .unsupported_reason()
            .into_owned())
    }

    pub fn browser(&self) -> &Self {
        self
    }
    pub fn set_shortcuts(&self, _modal: bool, _shortcuts: Vec<Keystroke>) {
        match *self {}
    }
    pub fn set_focused(&self, _focused: bool) {
        match *self {}
    }
    pub fn set_grips(&self, _grips: &[(Bounds<Pixels>, CursorStyle)]) {
        match *self {}
    }
    pub fn set_mouse_passthrough(&self, _enabled: bool) {
        match *self {}
    }
    pub fn sync(&self, _bounds: Bounds<Pixels>, _clip: Bounds<Pixels>, _radius: f64, _scale: f32) {
        match *self {}
    }
    pub fn occlude(&self, _occlusions: &[Bounds<Pixels>]) {
        match *self {}
    }
    pub fn occlude_also(&self, _bounds: Bounds<Pixels>) {
        match *self {}
    }
    pub fn clicked_within(&self, _elapsed: Duration) -> bool {
        match *self {}
    }
    pub fn is_visible(&self) -> bool {
        match *self {}
    }
    pub fn set_visible(&self, _visible: bool) {
        match *self {}
    }
    pub fn focus(&self) {
        match *self {}
    }
    pub fn blur(&self) {
        match *self {}
    }
    pub fn snapshot(&self) -> u64 {
        match *self {}
    }
    pub fn is_current_snapshot(&self, _id: u64) -> bool {
        match *self {}
    }
    pub fn accept_frame(&self, _id: u64, _generation: u64) -> bool {
        match *self {}
    }
    pub fn pointer(&self, _event: Pointer) {
        match *self {}
    }
    pub fn key(
        &self,
        _key: &Keystroke,
        _pressed: bool,
        _clipboard: impl FnOnce() -> Option<String>,
    ) {
        match *self {}
    }
    pub fn choose(&self, _option: Option<usize>) {
        match *self {}
    }
    pub fn load_url(&self, _url: &str) -> Result<()> {
        match *self {}
    }
    pub fn is_current_navigation(&self, _navigation: u64) -> bool {
        match *self {}
    }
    pub fn reload(&self) -> Result<()> {
        match *self {}
    }
    pub fn stop(&self) -> Result<()> {
        match *self {}
    }
    pub fn go_back(&self) -> Result<()> {
        match *self {}
    }
    pub fn go_forward(&self) -> Result<()> {
        match *self {}
    }
    pub fn set_zoom(&self, _zoom: f64) -> Result<()> {
        match *self {}
    }
    pub fn run_script(
        &self,
        _body: &str,
        _done: impl FnOnce(Result<String>) + Send + 'static,
    ) -> Result<()> {
        match *self {}
    }
    pub fn capture(&self, _done: impl FnOnce(Result<Image>) + Send + 'static) -> Result<()> {
        match *self {}
    }
    pub fn find(
        &self,
        _query: &str,
        _options: FindOptions,
        _done: impl FnOnce(Result<bool>) + Send + 'static,
    ) -> Result<()> {
        match *self {}
    }
    pub fn clear_find(&self) -> Result<()> {
        match *self {}
    }
    pub fn is_devtools_open(&self) -> bool {
        match *self {}
    }
    pub fn open_devtools(&self) -> Result<()> {
        match *self {}
    }
    pub fn close_devtools(&self) -> Result<()> {
        match *self {}
    }
    pub fn cookies(
        &self,
        _url: Option<&str>,
        _done: impl FnOnce(Result<Vec<Cookie>>) + Send + 'static,
    ) -> Result<()> {
        match *self {}
    }
    pub fn set_cookie(
        &self,
        _cookie: Cookie,
        _done: impl FnOnce(Result<()>) + Send + 'static,
    ) -> Result<()> {
        match *self {}
    }
    pub fn delete_cookies(
        &self,
        _name: &str,
        _domain: Option<&str>,
        _done: impl FnOnce(Result<usize>) + Send + 'static,
    ) -> Result<()> {
        match *self {}
    }
    pub fn clear_cookies(&self, _done: impl FnOnce(Result<()>) + Send + 'static) -> Result<()> {
        match *self {}
    }
}

pub fn installed_browsers() -> Vec<(String, PathBuf)> {
    Vec::new()
}

/// No engine holds the profile open.
pub fn close_profile(_profile: &Profile, done: impl FnOnce(Result<()>) + Send + 'static) {
    done(Ok(()));
}
