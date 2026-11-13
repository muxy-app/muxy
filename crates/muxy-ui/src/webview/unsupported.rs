use std::path::Path;
use std::rc::Rc;

use async_channel::Receiver;
use gpui::{Bounds, CursorStyle, Keystroke, Pixels, Rgba, Window};
use muxy_core::capability::{Capability, Feature};

use super::{AssetResult, Event, Pointer, assets::Source};

pub const CAPABILITY: Capability = Capability::Unsupported;
pub(super) const DRAWN_BY_APP: bool = false;

pub(super) fn probe() -> Capability {
    CAPABILITY
}

pub(super) fn set_data_directory(_directory: &Path) {}

#[derive(Debug)]
pub(super) enum NativeWebview {}

impl NativeWebview {
    pub(super) fn new(
        _window: &Window,
        _source: Source,
        _script: &str,
        _background: Rgba,
    ) -> Result<(Self, Receiver<Event>), String> {
        Err(Feature::ExtensionWebviews.unsupported_reason().into_owned())
    }
    pub(super) fn set_shortcuts(&self, _modal: bool, _shortcuts: Rc<Vec<Keystroke>>) {
        match *self {}
    }
    pub(super) fn set_focused(&self, _focused: bool) {
        match *self {}
    }
    pub(super) fn set_grips(&self, _grips: &[(Bounds<Pixels>, CursorStyle)]) {
        match *self {}
    }
    pub(super) fn set_mouse_passthrough(&self, _enabled: bool) {
        match *self {}
    }
    pub(super) fn set_hit_regions(&self, _regions: Option<&[Bounds<Pixels>]>) {
        match *self {}
    }
    pub(super) fn move_to_window(&self, _window: &Window) -> Result<(), String> {
        match *self {}
    }
    pub(super) fn order_front(_views: &[&Self]) {}
    pub(super) fn occlude_also(&self, _bounds: Bounds<Pixels>) {
        match *self {}
    }
    pub(super) fn generation(&self) -> u64 {
        match *self {}
    }
    pub(super) fn install_script(&self, _script: &str) {
        match *self {}
    }
    pub(super) fn evaluate(&self, _script: &str) {
        match *self {}
    }
    pub(super) fn reply(&self, _id: u64, _generation: u64, _json: &str) {
        match *self {}
    }
    pub(super) fn complete_asset(&self, _id: u64, _result: AssetResult) {
        match *self {}
    }
    pub(super) fn sync(
        &self,
        _bounds: Bounds<Pixels>,
        _clip: Bounds<Pixels>,
        _background: Rgba,
        _radius: f64,
        _scale: f32,
    ) {
        match *self {}
    }
    pub(super) fn occlude(&self, _occlusions: &[Bounds<Pixels>]) {
        match *self {}
    }
    pub(super) fn set_visible(&self, _visible: bool) {
        match *self {}
    }
    pub(super) fn focus(&self) {
        match *self {}
    }
    pub(super) fn blur(&self) {
        match *self {}
    }
    pub(super) fn snapshot(&self) {
        match *self {}
    }
    pub(super) fn accept_snapshot(&self, _id: u64, _generation: u64) -> bool {
        match *self {}
    }
    pub(super) fn pointer(&self, _event: Pointer) {
        match *self {}
    }
    pub(super) fn key(
        &self,
        _keystroke: &Keystroke,
        _pressed: bool,
        _clipboard: impl FnOnce() -> Option<String>,
    ) {
        match *self {}
    }
    pub(super) fn choose(&self, _option: Option<usize>) {
        match *self {}
    }
}
