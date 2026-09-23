use gpui::{Context, Entity, FocusHandle, Window};
use muxy_ui::panel::{PanelId, PanelMode, PanelPlacement, PanelPosition, PanelResizeState};
use muxy_ui::webview::assets::Source;
use serde_json::{Value, json};

use super::{AppModel, Request, Surface, SurfaceKind, Webview};

#[derive(Clone)]
pub(super) struct Definition {
    pub source: Source,
    pub title: Option<String>,
    pub position: PanelPosition,
    pub mode: PanelMode,
    pub default_data: Value,
    /// Whether the user's pin choice overrides the declared mode.
    pub allows_mode_selection: bool,
}

pub(crate) struct Panel {
    pub surface: Surface,
    pub owner: String,
    pub kind: String,
    pub title: Option<String>,
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
                    title: Some("Webview Panel".into()),
                    position,
                    mode: PanelMode::Floating,
                    default_data: Value::Null,
                    allows_mode_selection: true,
                },
            );
        }
    }

    pub(super) fn register_extension_panel(
        &mut self,
        extension: &muxy_app_core::extensions::Extension,
        panel: &muxy_app_core::extensions::Panel,
    ) {
        use muxy_app_core::extensions::{PanelMode as Mode, PanelPosition as Position};
        self.webviews.registered_panels.insert(
            (extension.name.clone(), panel.id.clone()),
            Definition {
                source: Source {
                    owner: extension.name.clone(),
                    directory: extension.directory.clone(),
                    entry: panel.entry.clone(),
                },
                title: panel.title.clone(),
                position: match panel.position {
                    Position::Right => PanelPosition::Right,
                    Position::Bottom => PanelPosition::Bottom,
                },
                mode: match panel.mode {
                    Mode::Floating => PanelMode::Floating,
                    Mode::Pinned => PanelMode::Pinned,
                },
                default_data: panel.default_data.clone(),
                allows_mode_selection: panel.allows_mode_selection(),
            },
        );
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

    /// `panels.open|toggle|close`. Toggling closes without asking the page;
    /// closing asks it first. Closing an unknown panel is not an error.
    pub(in crate::model) fn panel_operation(
        &mut self,
        owner: &str,
        verb: &str,
        args: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let owner = owner.to_owned();
        let kind = args["panelID"].as_str().filter(|kind| !kind.is_empty());
        if verb == "panels.close" {
            if let Some(kind) = kind {
                self.dismiss_webview_panel(&panel_id(&owner, kind), cx);
            }
            return Ok(Value::Null);
        }
        let kind = kind.ok_or("missing argument 'panel'")?;
        let definition = self
            .webviews
            .registered_panels
            .get(&(owner.clone(), kind.into()))
            .cloned()
            .ok_or_else(|| format!("unknown panel '{kind}'"))?;
        let id = panel_id(&owner, kind);
        if verb == "panels.toggle" && self.panels.placement(&id).is_some() {
            self.remove_webview_panel(&id, cx);
            return Ok(Value::Null);
        }
        let data = Some(&args["data"]).filter(|data| !data.is_null());
        if let Some(panel) = self.webviews.panels.get(&id) {
            panel.surface.view.update(cx, |view, cx| {
                if let Some(data) = data {
                    view.update_content(data.clone(), &self.theme, self.metrics, cx);
                }
                if self.overlay.is_none() {
                    view.focus.focus(window);
                }
            });
            let placement = panel.placement.clone();
            self.place_panel(placement, cx);
            self.focus_requested = false;
            self.panel_event("panel.opened", &owner, kind, cx);
            return Ok(Value::Null);
        }
        let surface = self.create_webview(
            definition.source,
            kind.into(),
            data.cloned().unwrap_or(definition.default_data),
            SurfaceKind::Panel,
            window,
            cx,
        )?;
        let mode = if definition.allows_mode_selection {
            self.webview_panel_mode(&owner, kind, definition.mode)
        } else {
            definition.mode
        };
        let placement = PanelPlacement::new(id.clone(), definition.position, mode);
        self.place_panel(placement.clone(), cx);
        if self.overlay.is_none() {
            surface.view.read(cx).focus.focus(window);
        }
        let header_buttons = self
            .extensions
            .registry
            .enabled(&owner)
            .and_then(|extension| extension.manifest.panel(kind))
            .map_or(0, |panel| panel.header_buttons.len());
        self.webviews.panels.insert(
            id,
            Panel {
                surface,
                owner: owner.clone(),
                kind: kind.into(),
                title: definition.title,
                placement,
                width: 360.0,
                height: 260.0,
                resize: PanelResizeState::default(),
                controls: std::array::from_fn(|_| cx.focus_handle()),
                header_controls: (0..header_buttons).map(|_| cx.focus_handle()).collect(),
                closing: false,
                focused: false,
            },
        );
        self.focus_requested = false;
        self.panel_event("panel.opened", &owner, kind, cx);
        cx.notify();
        Ok(Value::Null)
    }

    fn panel_event(&self, event: &str, owner: &str, kind: &str, cx: &gpui::App) {
        self.emit_extension_event(
            None,
            event,
            &json!({"extensionID": owner, "panelID": kind}),
            cx,
        );
    }

    fn webview_panel_mode(&self, owner: &str, kind: &str, default: PanelMode) -> PanelMode {
        if self
            .settings
            .panel_pinned(owner, kind, default == PanelMode::Pinned)
        {
            PanelMode::Pinned
        } else {
            PanelMode::Floating
        }
    }

    pub(crate) fn place_panel(&mut self, placement: PanelPlacement, cx: &mut Context<Self>) {
        if let Some(displacement) = self.panels.place(placement) {
            let id = displacement.displaced.id;
            if id.as_str() == crate::views::composer::PANEL {
                self.dismiss_composer(false, cx);
            } else if let Some(panel) = self.webviews.panels.get(&id) {
                panel.resize.end();
                self.panel_event("panel.closed", &panel.owner, &panel.kind, cx);
            }
        }
        cx.notify();
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
            if let Err(error) = self.settings.set_panel_pinned(
                &panel.owner,
                &panel.kind,
                panel.placement.mode != PanelMode::Pinned,
                &self.path.with_file_name("settings.toml"),
            ) {
                self.fail(format!("Could not save panel pin: {error}"), cx);
                return;
            }
            panel.placement.mode = panel.placement.mode.toggled();
        } else {
            panel.placement.position = panel.placement.position.moved();
        }
        panel.resize.end();
        let placement = panel.placement.clone();
        self.place_panel(placement, cx);
        cx.notify();
    }

    /// Closes a panel once its page allows it (`onBeforeClose`).
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
            if self.panels.remove(id).is_some() {
                self.panel_event("panel.closed", &panel.owner, &panel.kind, cx);
            }
            self.focus_requested |= panel.focused;
            cx.notify();
        }
    }

    pub(super) fn sync_webview_panels(
        &mut self,
        blocked: bool,
        resizing: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let shortcuts = self.webview_shortcuts();
        for (id, panel) in &mut self.webviews.panels {
            let visible = self.panels.placement(id).is_some();
            panel.focused = panel.is_focused(window, cx);
            panel.surface.view.update(cx, |view, cx| {
                view.native.set_shortcuts(false, shortcuts.to_vec());
                view.native.set_mouse_passthrough(resizing);
                view.native.set_mouse_passthrough_left(
                    if panel.placement.position == PanelPosition::Right {
                        self.metrics.resize_handle_hit_area() - gpui::px(1.0)
                    } else {
                        gpui::px(0.0)
                    },
                );
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
