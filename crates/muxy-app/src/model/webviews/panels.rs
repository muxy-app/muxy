use gpui::{Context, Entity, FocusHandle, Window};
use muxy_ui::panel::{PanelId, PanelMode, PanelPlacement, PanelPosition, PanelResizeState};
use muxy_ui::webview::assets::Source;
use serde_json::Value;

use super::{AppModel, Request, Surface, SurfaceKind, Webview};

#[derive(Clone)]
pub(super) struct Definition {
    pub source: Source,
    pub title: String,
    pub position: PanelPosition,
    pub mode: PanelMode,
}

pub(crate) struct Panel {
    pub surface: Surface,
    pub title: String,
    pub placement: PanelPlacement,
    pub width: f32,
    pub height: f32,
    pub resize: PanelResizeState,
    pub controls: [FocusHandle; 4],
    pub header_controls: Vec<FocusHandle>,
    closing: bool,
    focused: bool,
}

impl Panel {
    fn is_focused(&self, window: &Window, cx: &gpui::App) -> bool {
        self.surface.view.read(cx).focus.is_focused(window)
            || self
                .controls
                .iter()
                .chain(&self.header_controls)
                .any(|focus| focus.is_focused(window))
    }
}

impl AppModel {
    pub(super) fn register_demo_panels(&mut self) {
        for (id, position) in [
            ("right", PanelPosition::Right),
            ("bottom", PanelPosition::Bottom),
        ] {
            self.webviews.registered_panels.insert(
                ("foundation".into(), id.into()),
                Definition {
                    source: Source {
                        owner: "foundation".into(),
                        directory: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("fixtures/webview"),
                        entry: "panel.html".into(),
                    },
                    title: "Webview Panel".into(),
                    position,
                    mode: PanelMode::Floating,
                },
            );
        }
    }

    pub(super) fn panel_request(
        &mut self,
        view: &Entity<Webview>,
        request: &Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let args = &request.body["args"];
        let owner = view.read(cx).source.owner.clone();
        self.panel_operation(
            &owner,
            request.body["verb"].as_str().unwrap_or(""),
            args,
            window,
            cx,
        )
    }

    pub(in crate::model) fn panel_operation(
        &mut self,
        owner: &str,
        verb: &str,
        args: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let owner = owner.to_owned();
        let kind = args["panelID"].as_str().ok_or("panelID is required")?;
        let definition = self
            .webviews
            .registered_panels
            .get(&(owner.clone(), kind.into()))
            .cloned()
            .ok_or("panel is not registered for this owner")?;
        let id = panel_id(&owner, kind);
        let open = self.panels.placement(&id).is_some();
        if verb == "panels.close" || (verb == "panels.toggle" && open) {
            if verb == "panels.toggle" {
                self.focus_requested |= self
                    .webviews
                    .panels
                    .get(&id)
                    .is_some_and(|panel| panel.is_focused(window, cx));
                self.panels.remove(&id);
                cx.notify();
            } else {
                self.dismiss_webview_panel(&id, cx);
            }
            return Ok(Value::Null);
        }
        if let Some(panel) = self.webviews.panels.get(&id) {
            panel.surface.view.update(cx, |view, cx| {
                if let Some(data) = args.get("data") {
                    view.update_content(data.clone(), &self.theme, self.metrics, cx);
                }
                if self.overlay.is_none() {
                    view.focus.focus(window);
                }
            });
            let placement = panel.placement.clone();
            self.place_panel(placement, cx);
            self.focus_requested = false;
            if !open {
                self.emit_extension_event(
                    Some(&owner),
                    "panel.opened",
                    serde_json::json!({"panelID":kind,"extensionID":owner}),
                    cx,
                );
            }
            return Ok(Value::Null);
        }
        let surface = self.create_webview(
            definition.source,
            kind.into(),
            args["data"].clone(),
            SurfaceKind::Panel,
            window,
            cx,
        )?;
        let placement = PanelPlacement::new(id.clone(), definition.position, definition.mode);
        self.place_panel(placement.clone(), cx);
        if self.overlay.is_none() {
            surface.view.read(cx).focus.focus(window);
        }
        self.webviews.panels.insert(
            id,
            Panel {
                surface,
                title: definition.title,
                placement,
                width: 360.0,
                height: 260.0,
                resize: PanelResizeState::default(),
                controls: std::array::from_fn(|_| cx.focus_handle()),
                header_controls: (0..self
                    .extensions
                    .registry
                    .enabled(&owner)
                    .and_then(|extension| {
                        extension
                            .manifest
                            .panels
                            .iter()
                            .find(|panel| panel.surface.id == kind)
                    })
                    .map_or(0, |panel| panel.header_buttons.len()))
                    .map(|_| cx.focus_handle())
                    .collect(),
                closing: false,
                focused: false,
            },
        );
        self.focus_requested = false;
        self.emit_extension_event(
            Some(&owner),
            "panel.opened",
            serde_json::json!({"panelID":kind,"extensionID":owner}),
            cx,
        );
        cx.notify();
        Ok(Value::Null)
    }

    pub(crate) fn place_panel(&mut self, placement: PanelPlacement, cx: &mut Context<Self>) {
        if let Some(displacement) = self.panels.place(placement) {
            let id = displacement.displaced.id;
            if id.as_str() == crate::views::composer::PANEL {
                self.dismiss_composer(false, cx);
            }
        }
    }

    pub(crate) fn move_webview_panel(
        &mut self,
        id: &PanelId,
        toggle_mode: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.webviews.panels.get_mut(id) else {
            return;
        };
        if toggle_mode {
            panel.placement.mode = panel.placement.mode.toggled();
        } else {
            panel.placement.position = panel.placement.position.moved();
        }
        panel.resize.end();
        let placement = panel.placement.clone();
        self.place_panel(placement, cx);
        cx.notify();
    }

    pub(crate) fn dismiss_webview_panel(&mut self, id: &PanelId, cx: &mut Context<Self>) {
        let Some(panel) = self
            .webviews
            .panels
            .get_mut(id)
            .filter(|panel| !panel.closing)
        else {
            return;
        };
        panel.closing = true;
        let entity = panel.surface.view.entity_id();
        let result = panel.surface.view.update(cx, Webview::before_close);
        let id = id.clone();
        cx.spawn(async move |model, cx| {
            let prevent = result.recv().await.unwrap_or(false);
            let _ = model.update(cx, |model, cx| {
                if let Some(panel) = model
                    .webviews
                    .panels
                    .get_mut(&id)
                    .filter(|panel| panel.surface.view.entity_id() == entity)
                {
                    panel.closing = false;
                    if !prevent {
                        model.remove_webview_panel(&id, cx);
                    }
                }
            });
        })
        .detach();
    }

    pub(super) fn remove_webview_panel(&mut self, id: &PanelId, cx: &mut Context<Self>) {
        if let Some(panel) = self.webviews.panels.remove(id) {
            self.panels.remove(id);
            self.focus_requested |= panel.focused;
            cx.notify();
        }
    }

    pub(super) fn sync_webview_panels(
        &mut self,
        blocked: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let mut shortcuts = super::surface_shortcuts(&self.settings.keymap);
        shortcuts.extend(self.extension_keystrokes());
        for (id, panel) in &mut self.webviews.panels {
            let visible = self.panels.placement(id).is_some();
            panel.focused = panel.is_focused(window, cx);
            panel.surface.view.update(cx, |view, cx| {
                view.native.set_shortcuts(false, shortcuts.clone());
                view.refresh_theme(&self.theme, self.metrics, cx);
                view.present(
                    visible,
                    blocked,
                    window.is_window_active() && view.focus.is_focused(window),
                    0.0,
                    cx,
                );
            });
        }
    }
}

fn panel_id(owner: &str, kind: &str) -> PanelId {
    PanelId::new(format!("webview:{owner}:{kind}"))
}
