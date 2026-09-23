//! Extension popovers: one open at a time app-wide, anchored to the topbar or
//! status-bar item that opened it.

use gpui::{Context, Window};
use muxy_app_core::extensions::Side;
use muxy_ui::popover::PopoverAnchor;
use muxy_ui::webview::assets::Source;
use serde_json::{Value, json};

use super::{AppModel, Surface, SurfaceKind, Webview};
use crate::views::overlays::Overlay;

const MIN_SIZE: f64 = 80.0;
const MAX_WIDTH: f64 = 600.0;
const MAX_HEIGHT: f64 = 720.0;

pub(crate) struct Popover {
    pub owner: String,
    pub id: String,
    pub anchor_key: String,
    pub anchor: PopoverAnchor,
    pub above: bool,
    pub width: f64,
    pub height: f64,
    pub surface: Surface,
    closing: bool,
}

impl AppModel {
    fn popover_event(&self, name: &str, owner: &str, id: &str, cx: &gpui::App) {
        self.emit_extension_event(
            None,
            name,
            &json!({"extensionID": owner, "popoverID": id}),
            cx,
        );
    }

    /// Clicking an item opens its popover, or closes it when already open there.
    pub(crate) fn toggle_extension_popover(
        &mut self,
        owner: &str,
        popover: &str,
        anchor_key: &str,
        above: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .webviews
            .popover
            .as_ref()
            .is_some_and(|open| open.anchor_key == anchor_key)
        {
            self.close_extension_popover(cx);
            return;
        }
        self.close_extension_popover(cx);
        let Some((directory, declared)) =
            self.extensions
                .registry
                .enabled(owner)
                .and_then(|extension| {
                    extension
                        .manifest
                        .popover(popover)
                        .map(|declared| (extension.directory.clone(), declared.clone()))
                })
        else {
            return;
        };
        self.webviews.sequence += 1;
        let surface = self.create_webview(
            Source {
                owner: owner.into(),
                directory,
                entry: declared.entry.clone(),
            },
            format!("popover:{owner}:{popover}:{}", self.webviews.sequence),
            declared.default_data.clone(),
            SurfaceKind::Popover,
            window,
            cx,
        );
        let surface = match surface {
            Ok(surface) => surface,
            Err(error) => {
                self.fail(error, cx);
                return;
            }
        };
        surface.view.read(cx).focus.focus(window);
        self.webviews.popover = Some(Popover {
            owner: owner.into(),
            id: popover.into(),
            anchor_key: anchor_key.into(),
            anchor: self.extensions.items.anchor(anchor_key),
            above,
            width: declared.width,
            height: declared.height,
            surface,
            closing: false,
        });
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::Popover);
        self.popover_event("popover.opened", owner, popover, cx);
        cx.notify();
    }

    /// Closes without asking the page, like main's outside clicks and toggles.
    pub(crate) fn close_extension_popover(&mut self, cx: &mut Context<Self>) {
        let Some(popover) = self.webviews.popover.take() else {
            return;
        };
        if matches!(self.overlay, Some(Overlay::Popover)) {
            self.overlay = None;
            self.focus_requested = true;
        }
        self.popover_event("popover.closed", &popover.owner, &popover.id, cx);
        cx.notify();
    }

    /// `muxy.popover.close()`: the page's `onBeforeClose` may keep it open.
    pub(crate) fn request_close_popover(&mut self, owner: &str, cx: &mut Context<Self>) {
        let Some(popover) = self
            .webviews
            .popover
            .as_mut()
            .filter(|popover| popover.owner == owner && !popover.closing)
        else {
            return;
        };
        popover.closing = true;
        let entity = popover.surface.view.entity_id();
        let result = popover.surface.view.update(cx, Webview::before_close);
        cx.spawn(async move |model, cx| {
            let prevent = result.recv().await.unwrap_or(false);
            let _ = model.update(cx, |model, cx| {
                if let Some(popover) = model
                    .webviews
                    .popover
                    .as_mut()
                    .filter(|popover| popover.surface.view.entity_id() == entity)
                {
                    popover.closing = false;
                    if !prevent {
                        model.close_extension_popover(cx);
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn resize_popover(
        &mut self,
        owner: &str,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let number = |key: &str| {
            args[key]
                .as_f64()
                .ok_or_else(|| format!("missing argument '{key}'"))
        };
        let (width, height) = (number("width")?, number("height")?);
        if width.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
            || height.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
        {
            return Err("popover.resize requires positive width and height".into());
        }
        if let Some(popover) = self
            .webviews
            .popover
            .as_mut()
            .filter(|popover| popover.owner == owner)
        {
            popover.width = width.clamp(MIN_SIZE, MAX_WIDTH);
            popover.height = height.clamp(MIN_SIZE, MAX_HEIGHT);
            cx.notify();
        }
        Ok(Value::Null)
    }

    /// A popover closes when the item it is anchored to is hidden.
    pub(crate) fn close_hidden_popover(&mut self, cx: &mut Context<Self>) {
        let Some(anchor) = self
            .webviews
            .popover
            .as_ref()
            .map(|popover| popover.anchor_key.clone())
        else {
            return;
        };
        let shown = self
            .topbar_items()
            .iter()
            .any(|item| format!("top:{}:{}", item.owner, item.id) == anchor)
            || [Side::Left, Side::Right].into_iter().any(|side| {
                self.status_items(side)
                    .iter()
                    .any(|item| format!("status:{}:{}", item.owner, item.id) == anchor)
            });
        if !shown {
            self.close_extension_popover(cx);
        }
    }
}
