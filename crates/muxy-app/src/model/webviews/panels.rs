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
    closing: bool,
    focused: bool,
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
        let kind = args["panelID"].as_str().ok_or("panelID is required")?;
        let definition = self
            .webviews
            .registered_panels
            .get(&(owner.clone(), kind.into()))
            .cloned()
            .ok_or("panel is not registered for this owner")?;
        let id = panel_id(&owner, kind);
        let open = self.panels.placement(&id).is_some();
        let verb = request.body["verb"].as_str().unwrap_or("");
        if verb == "panels.close" || (verb == "panels.toggle" && open) {
            self.dismiss_webview_panel(&id, cx);
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
                closing: false,
                focused: false,
            },
        );
        self.focus_requested = false;
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
        let shortcuts = super::surface_shortcuts(&self.settings.keymap);
        for (id, panel) in &mut self.webviews.panels {
            let visible = self.panels.placement(id).is_some();
            panel.focused = panel.surface.view.read(cx).focus.is_focused(window)
                || panel.controls.iter().any(|focus| focus.is_focused(window));
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
