use gpui::{Context, Entity, FocusHandle, Window};
use muxy_app_core::ProjectId;
use muxy_app_core::extensions::PanelControl as Hidden;
use muxy_app_core::extensions::panels::{
    FrameSpec, MAX_PANELS, PanelChrome, PanelOptions, PanelScope, Rect, SizeSpec,
};
use muxy_ui::floating::{FloatingFrame, SizeLimits};
use muxy_ui::panel::{PanelId, PanelMode, PanelPlacement, PanelPosition, PanelResizeState};
use muxy_ui::tr;
use muxy_ui::webview::assets::Source;
use serde_json::{Value, json};

use super::panel_sessions::SavedPanel;
use super::{AppModel, Page, Request, Surface, SurfaceKind, Webview};
use crate::model::extensions::root_of;

const DEFAULT_FRAME: [f32; 4] = [80.0, 80.0, 360.0, 260.0];
const CASCADE: f32 = 24.0;
const MIN_SIZE: (f32, f32) = (24.0, 24.0);

/// How a detached panel floats, as its manifest declares it.
#[derive(Clone, Debug, Default, PartialEq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Mirrors the manifest's independent panel flags"
)]
pub(super) struct Style {
    pub frame: FrameSpec,
    pub min_size: Option<SizeSpec>,
    pub max_size: Option<SizeSpec>,
    pub chrome: PanelChrome,
    pub transparent: Option<bool>,
    pub click_through: bool,
    pub movable: bool,
    pub resizable: bool,
    pub scope: PanelScope,
}

impl Style {
    /// Whether a page opened with `options` draws nothing behind it: as
    /// asked, or else whenever Muxy draws no frame around it.
    fn transparent_with(&self, options: &PanelOptions) -> bool {
        options
            .transparent
            .or(self.transparent)
            .unwrap_or(options.chrome.unwrap_or(self.chrome) == PanelChrome::None)
    }
}

#[derive(Clone)]
pub(super) struct Definition {
    pub source: Source,
    pub title: Option<String>,
    pub position: PanelPosition,
    pub mode: PanelMode,
    /// Whether the manifest opens it detached.
    pub detached: bool,
    pub default_data: Value,
    /// Whether the user's pin choice overrides the declared mode.
    pub allows_mode_selection: bool,
    /// Whether the user's pop-out choice overrides the declared mode.
    pub allows_detach_selection: bool,
    pub style: Style,
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent panel state flags"
)]
pub(crate) struct Panel {
    pub surface: Surface,
    pub owner: String,
    pub kind: String,
    /// Tells apart several open copies of one panel.
    pub instance: Option<String>,
    pub title: Option<String>,
    /// Where the panel docks, and kept while it floats detached.
    pub placement: PanelPlacement,
    pub resize: PanelResizeState,
    pub controls: [FocusHandle; 5],
    pub header_controls: Vec<FocusHandle>,
    /// Whether it floats freely beside other panels instead of docking.
    pub detached: bool,
    /// Whether the page lets what is behind it show through. Fixed when the
    /// page is made.
    pub transparent: bool,
    /// Where it floats while detached, before fitting the window.
    pub frame: FloatingFrame,
    pub limits: SizeLimits,
    pub chrome: PanelChrome,
    /// Whether the pointer passes through the panel to what is under it,
    /// except over its hit regions.
    pub click_through: bool,
    pub movable: bool,
    pub resizable: bool,
    /// The root project it shows in; `None` shows it in every project.
    pub scope: Option<ProjectId>,
    /// The header controls the page hid at run time, over the manifest's.
    pub hidden_controls: Option<Vec<Hidden>>,
    /// Where the page takes the pointer, from its top left corner.
    pub hit_regions: Option<Vec<Rect>>,
    /// Where dragging the page moves the panel, from its top left corner.
    pub drag_regions: Vec<Rect>,
    /// The state the page last heard about, and the document that heard it.
    reported: Option<(u64, Value)>,
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

    /// Applies the parts of `options` that shape how the panel floats.
    /// `root` is the project a project-scoped panel belongs to; the frame is
    /// fitted to `area`.
    fn apply_style(&mut self, options: &PanelOptions, root: ProjectId, area: gpui::Size<f32>) {
        let size = |size: SizeSpec| gpui::Size {
            width: size.width,
            height: size.height,
        };
        if options.min_size.is_some() || options.max_size.is_some() {
            let min = options.min_size.map_or(self.limits.min, size);
            let max = options.max_size.map_or(self.limits.max, size);
            self.limits = SizeLimits::new(min, max);
        }
        if let Some(frame) = options.frame {
            self.frame = FloatingFrame::from_array(frame.over(self.frame.to_array()));
        }
        self.frame = self.frame.clamped(area, self.limits);
        self.chrome = options.chrome.unwrap_or(self.chrome);
        self.click_through = options.click_through.unwrap_or(self.click_through);
        self.movable = options.movable.unwrap_or(self.movable);
        self.resizable = options.resizable.unwrap_or(self.resizable);
        if let Some(scope) = options.scope {
            self.scope = (scope == PanelScope::Project).then_some(root);
        }
        if let Some(title) = &options.title {
            self.title = title.clone().into_option();
        }
        if let Some(hidden) = &options.hidden_controls {
            self.hidden_controls = Some(hidden.clone());
        }
        if let Some(regions) = &options.hit_regions {
            self.hit_regions = regions.clone().into_option();
        }
        if let Some(regions) = &options.drag_regions {
            self.drag_regions.clone_from(regions);
        }
    }

    /// Whether the pointer reaches the page anywhere, or only over its hit
    /// regions.
    pub(crate) fn pointer_regions(&self) -> Option<&[Rect]> {
        if !self.detached {
            return None;
        }
        match &self.hit_regions {
            Some(regions) => Some(regions),
            None if self.click_through => Some(&[]),
            None => None,
        }
    }

    pub(crate) fn mode_name(&self) -> &'static str {
        if self.detached {
            "detached"
        } else if self.placement.mode == PanelMode::Pinned {
            "pinned"
        } else {
            "floating"
        }
    }
}

/// One size for every extension panel, as on main: a right panel's width and a
/// bottom panel's height.
#[derive(Clone, Copy)]
pub(crate) struct PanelSize {
    width: f32,
    height: f32,
}

impl Default for PanelSize {
    fn default() -> Self {
        Self {
            width: 360.0,
            height: 260.0,
        }
    }
}

impl PanelSize {
    pub(crate) fn get(self, position: PanelPosition) -> f32 {
        match position {
            PanelPosition::Right => self.width,
            PanelPosition::Bottom => self.height,
        }
    }

    pub(crate) fn set(&mut self, position: PanelPosition, dimension: f32) {
        match position {
            PanelPosition::Right => self.width = dimension,
            PanelPosition::Bottom => self.height = dimension,
        }
    }
}

fn limits_of(min: Option<SizeSpec>, max: Option<SizeSpec>) -> SizeLimits {
    let min = min.map_or(
        gpui::Size {
            width: MIN_SIZE.0,
            height: MIN_SIZE.1,
        },
        |size| gpui::Size {
            width: size.width,
            height: size.height,
        },
    );
    let max = max.map_or(
        gpui::Size {
            width: f32::INFINITY,
            height: f32::INFINITY,
        },
        |size| gpui::Size {
            width: size.width,
            height: size.height,
        },
    );
    SizeLimits::new(min, max)
}

fn rect_json(frame: FloatingFrame) -> Value {
    json!({"x": frame.x, "y": frame.y, "width": frame.width, "height": frame.height})
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
                    detached: false,
                    default_data: Value::Null,
                    allows_mode_selection: true,
                    allows_detach_selection: true,
                    style: Style {
                        movable: true,
                        resizable: true,
                        ..Style::default()
                    },
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
                    Mode::Floating | Mode::Detached => PanelMode::Floating,
                    Mode::Pinned => PanelMode::Pinned,
                },
                detached: panel.mode == Mode::Detached,
                default_data: panel.default_data.clone(),
                allows_mode_selection: panel.allows_mode_selection(),
                allows_detach_selection: panel.allows_detach_selection(),
                style: Style {
                    frame: panel.frame.unwrap_or_default(),
                    min_size: panel.min_size,
                    max_size: panel.max_size,
                    chrome: panel.chrome,
                    transparent: panel.transparent,
                    click_through: panel.click_through,
                    movable: panel.movable,
                    resizable: panel.resizable,
                    scope: panel.scope,
                },
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

    /// `panel.update|get|raise`: the calling panel acting on itself.
    pub(super) fn own_panel_request(
        &mut self,
        id: &PanelId,
        request: &Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let args = &request.body["args"];
        self.measure_panel_area(window);
        match request.body["verb"].as_str().unwrap_or("") {
            "panel.update" => {
                let options = PanelOptions::parse(&args["options"])?;
                self.update_and_announce_panel(id, &options, window, cx)?;
                Ok(self.panel_state(id).unwrap_or(Value::Null))
            }
            "panel.raise" => {
                self.raise_floating(id, cx);
                Ok(Value::Null)
            }
            _ => Ok(self.panel_state(id).unwrap_or(Value::Null)),
        }
    }

    /// `panels.open|toggle|close|update|list`. Toggling closes without asking
    /// the page; closing asks it first. Closing an unknown panel is not an
    /// error.
    pub(in crate::model) fn panel_operation(
        &mut self,
        owner: &str,
        verb: &str,
        args: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        self.open_restored_panel(window, cx);
        self.measure_panel_area(window);
        let owner = owner.to_owned();
        if verb == "panels.list" {
            return Ok(Value::Array(
                self.open_panels(&owner)
                    .filter_map(|id| self.panel_state(id))
                    .collect(),
            ));
        }
        let kind = args["panelID"].as_str().filter(|kind| !kind.is_empty());
        let options = PanelOptions::parse(&args["options"])?;
        let instance = match &args["instance"] {
            Value::Null => options.instance.clone(),
            Value::String(name) if muxy_app_core::extensions::panels::valid_instance(name) => {
                Some(name.clone())
            }
            _ => return Err("instance must be 1 to 64 letters, digits, '.', '_', or '-'".into()),
        };
        if verb == "panels.close" {
            if let Some(kind) = kind {
                self.dismiss_webview_panel(&panel_id(&owner, kind, instance.as_deref()), cx);
            }
            return Ok(Value::Null);
        }
        let kind = kind.ok_or("missing argument 'panel'")?;
        let id = panel_id(&owner, kind, instance.as_deref());
        if verb == "panels.update" {
            if !self.webviews.panels.contains_key(&id) {
                return Err(format!("panel '{kind}' is not open"));
            }
            self.update_and_announce_panel(&id, &options, window, cx)?;
            return Ok(self.panel_state(&id).unwrap_or(Value::Null));
        }
        let definition = self
            .webviews
            .registered_panels
            .get(&(owner.clone(), kind.into()))
            .cloned()
            .ok_or_else(|| format!("unknown panel '{kind}'"))?;
        if verb == "panels.toggle" && self.webview_panel_shown(&id) {
            self.remove_webview_panel(&id, cx);
            return Ok(Value::Null);
        }
        let data = Some(&args["data"]).filter(|data| !data.is_null());
        if self.webviews.panels.contains_key(&id) {
            self.update_webview_panel(&id, &options, window, cx)?;
            let root = root_of(self.state.current_project());
            if let Some(panel) = self.webviews.panels.get_mut(&id)
                && panel.scope.is_some()
            {
                panel.scope = Some(root);
            }
            let Some(panel) = self.webviews.panels.get(&id) else {
                return Ok(Value::Null);
            };
            panel.surface.view.update(cx, |view, cx| {
                if let Some(data) = data {
                    view.update_content(data.clone(), &self.theme, self.metrics, cx);
                }
                if self.overlay.is_none() {
                    view.focus.focus(window);
                }
            });
            if panel.detached {
                self.raise_floating(&id, cx);
            } else {
                let placement = panel.placement.clone();
                self.place_panel(placement, cx);
            }
            self.focus_owner.settle();
            self.panel_event("panel.opened", &owner, kind, instance.as_deref(), cx);
            return Ok(self.panel_state(&id).unwrap_or(Value::Null));
        }
        let data = data
            .cloned()
            .unwrap_or_else(|| definition.default_data.clone());
        self.create_panel(
            &owner,
            kind,
            instance.as_deref(),
            definition,
            data,
            &options,
            window,
            cx,
        )?;
        if self.overlay.is_none()
            && let Some(panel) = self.webviews.panels.get(&id)
        {
            panel.surface.view.read(cx).focus.focus(window);
        }
        self.focus_owner.settle();
        self.panel_event("panel.opened", &owner, kind, instance.as_deref(), cx);
        cx.notify();
        Ok(self.panel_state(&id).unwrap_or(Value::Null))
    }

    /// Creates a panel's page and shows it, docked or detached.
    #[allow(clippy::too_many_arguments, reason = "One panel's whole opening")]
    fn create_panel(
        &mut self,
        owner: &str,
        kind: &str,
        instance: Option<&str>,
        definition: Definition,
        data: Value,
        options: &PanelOptions,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let layout = if instance.is_none() {
            self.settings.panel_layout(owner, kind)
        } else {
            muxy_app_core::settings::PanelLayout::default()
        };
        let detached = match options.mode {
            Some(mode) => mode == muxy_app_core::extensions::PanelMode::Detached,
            None => layout
                .detached
                .filter(|_| definition.allows_detach_selection)
                .unwrap_or(definition.detached),
        };
        let id = panel_id(owner, kind, instance);
        if self.webviews.panels.contains_key(&id) {
            return Err(format!("panel '{kind}' is already open"));
        }
        if self.panel_count(owner) >= MAX_PANELS {
            return Err(format!(
                "an extension keeps at most {MAX_PANELS} panels open"
            ));
        }
        let transparent = definition.style.transparent_with(options);
        let surface = self.create_page_surface(
            Page {
                source: definition.source.clone(),
                instance: kind.into(),
                data,
                kind: SurfaceKind::Panel,
                panel_instance: instance.map(Into::into),
                transparent,
            },
            window,
            cx,
        )?;
        let mode = self.docked_mode(owner, kind, &definition);
        let placement = PanelPlacement::new(id.clone(), definition.position, mode);
        let header_buttons = self
            .extensions
            .registry
            .enabled(owner)
            .and_then(|extension| extension.manifest.panel(kind))
            .map_or(0, |panel| panel.header_buttons.len());
        let base = layout
            .frame
            .unwrap_or_else(|| self.first_frame(owner, &definition.style));
        let style = &definition.style;
        let root = root_of(self.state.current_project());
        self.webviews.panels.insert(
            id.clone(),
            Panel {
                surface,
                owner: owner.into(),
                kind: kind.into(),
                instance: instance.map(Into::into),
                title: definition.title,
                placement,
                resize: PanelResizeState::default(),
                controls: std::array::from_fn(|_| cx.focus_handle()),
                header_controls: (0..header_buttons).map(|_| cx.focus_handle()).collect(),
                detached: false,
                transparent,
                frame: FloatingFrame::from_array(base),
                limits: limits_of(style.min_size, style.max_size),
                chrome: style.chrome,
                click_through: style.click_through,
                movable: style.movable,
                resizable: style.resizable,
                scope: (style.scope == PanelScope::Project).then_some(root),
                hidden_controls: None,
                hit_regions: None,
                drag_regions: Vec::new(),
                reported: None,
                closing: false,
                focused: false,
            },
        );
        let options = PanelOptions {
            transparent: None,
            mode: options.mode.or(Some(if detached {
                muxy_app_core::extensions::PanelMode::Detached
            } else if mode == PanelMode::Pinned {
                muxy_app_core::extensions::PanelMode::Pinned
            } else {
                muxy_app_core::extensions::PanelMode::Floating
            })),
            ..options.clone()
        };
        if let Err(error) = self.update_webview_panel(&id, &options, window, cx) {
            self.webviews.panels.remove(&id);
            self.panels.remove(&id);
            return Err(error);
        }
        if let Some(panel) = self.webviews.panels.get(&id)
            && !panel.detached
        {
            let placement = panel.placement.clone();
            self.place_panel(placement, cx);
        }
        Ok(())
    }

    fn panel_count(&self, owner: &str) -> usize {
        self.open_panels(owner).count()
    }

    /// An extension's open panels: docked, or detached even while another
    /// project hides them. A docked panel another one replaced is closed.
    fn open_panels<'a>(&'a self, owner: &'a str) -> impl Iterator<Item = &'a PanelId> + 'a {
        self.webviews
            .panels
            .iter()
            .filter(move |(id, panel)| {
                panel.owner == owner && (panel.detached || self.panels.placement(id).is_some())
            })
            .map(|(id, _)| id)
    }

    fn measure_panel_area(&mut self, window: &Window) {
        let viewport = window.viewport_size();
        self.webviews.area = gpui::Size {
            width: f32::from(viewport.width),
            height: f32::from(viewport.height),
        };
    }

    /// [`Self::update_webview_panel`] for a request from an extension, which
    /// hears `panel.opened` when the change shows a panel that was closed,
    /// such as one a docked panel had replaced.
    fn update_and_announce_panel(
        &mut self,
        id: &PanelId,
        options: &PanelOptions,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let shown = self.webview_panel_shown(id);
        self.update_webview_panel(id, options, window, cx)?;
        if !shown
            && self.webview_panel_shown(id)
            && let Some(panel) = self.webviews.panels.get(id)
        {
            self.panel_event(
                "panel.opened",
                &panel.owner,
                &panel.kind,
                panel.instance.as_deref(),
                cx,
            );
        }
        Ok(())
    }

    /// Where a detached panel opens before anything moved it: the manifest's
    /// frame over a default that steps down and right for each panel the
    /// extension already floats.
    fn first_frame(&self, owner: &str, style: &Style) -> [f32; 4] {
        #[allow(
            clippy::cast_precision_loss,
            reason = "At most a few dozen detached panels"
        )]
        let cascade = self.detached_count(owner) as f32 * CASCADE;
        style.frame.over([
            DEFAULT_FRAME[0] + cascade,
            DEFAULT_FRAME[1] + cascade,
            DEFAULT_FRAME[2],
            DEFAULT_FRAME[3],
        ])
    }

    fn detached_count(&self, owner: &str) -> usize {
        self.webviews
            .panels
            .values()
            .filter(|panel| panel.owner == owner && panel.detached)
            .count()
    }

    /// Applies `options` to an open panel, moving it between docked and
    /// detached as asked.
    pub(in crate::model) fn update_webview_panel(
        &mut self,
        id: &PanelId,
        options: &PanelOptions,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        use muxy_app_core::extensions::{PanelMode as Mode, PanelPosition as Position};
        let root = root_of(self.state.current_project());
        let Some(panel) = self.webviews.panels.get(id) else {
            return Err("panel is not open".into());
        };
        if options
            .transparent
            .is_some_and(|transparent| transparent != panel.transparent)
        {
            return Err("transparent can only be chosen when the panel opens".into());
        }
        let area = self.floating_area_for(id, window);
        let Some(panel) = self.webviews.panels.get_mut(id) else {
            return Err("panel is not open".into());
        };
        let was_detached = panel.detached;
        let mut placement_changed = false;
        if let Some(mode) = options.mode {
            panel.detached = mode == Mode::Detached;
            let docked = match mode {
                Mode::Pinned => Some(PanelMode::Pinned),
                Mode::Floating => Some(PanelMode::Floating),
                Mode::Detached => None,
            };
            if let Some(docked) = docked
                && panel.placement.mode != docked
            {
                panel.placement.mode = docked;
                placement_changed = true;
            }
        }
        if let Some(position) = options.position {
            let position = match position {
                Position::Right => PanelPosition::Right,
                Position::Bottom => PanelPosition::Bottom,
            };
            placement_changed |= panel.placement.position != position;
            panel.placement.position = position;
        }
        panel.apply_style(options, root, area);
        let detached = panel.detached;
        let placement = panel.placement.clone();
        panel.resize.end();
        if detached && !was_detached {
            self.panels.remove(id);
            self.floating.stack.raise(id);
        } else if !detached && (was_detached || placement_changed) {
            self.place_panel(placement, cx);
        }
        cx.notify();
        Ok(())
    }

    /// What a page can learn about one of its extension's panels.
    pub(crate) fn panel_state(&self, id: &PanelId) -> Option<Value> {
        let panel = self.webviews.panels.get(id)?;
        let area = self.webviews.area;
        let frame = panel.frame.clamped(area, panel.limits);
        Some(json!({
            "panelID": panel.kind,
            "instance": panel.instance,
            "mode": panel.mode_name(),
            "position": match panel.placement.position {
                PanelPosition::Right => "right",
                PanelPosition::Bottom => "bottom",
            },
            "visible": self.webview_panel_shown(id),
            "frame": rect_json(frame),
            "bounds": {"width": area.width, "height": area.height},
            "scope": if panel.scope.is_some() { "project" } else { "global" },
            "chrome": match panel.chrome {
                PanelChrome::Standard => "standard",
                PanelChrome::None => "none",
            },
            "transparent": panel.transparent,
            "clickThrough": panel.click_through,
            "movable": panel.movable,
            "resizable": panel.resizable,
            "title": panel.title,
            "focused": panel.focused,
        }))
    }

    /// Whether the panel shows now: docked, or detached in this project.
    pub(crate) fn webview_panel_shown(&self, id: &PanelId) -> bool {
        self.panels.placement(id).is_some()
            || self.webviews.panels.get(id).is_some_and(|panel| {
                panel.detached
                    && panel
                        .scope
                        .is_none_or(|scope| scope == root_of(self.state.current_project()))
            })
    }

    /// The detached extension panels shown now.
    pub(crate) fn detached_webview_panels(&self) -> Vec<PanelId> {
        self.webviews
            .panels
            .iter()
            .filter(|(id, panel)| panel.detached && self.webview_panel_shown(id))
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub(crate) fn detached_webview_geometry(
        &self,
        id: &PanelId,
        window: &Window,
    ) -> Option<(FloatingFrame, SizeLimits)> {
        let panel = self
            .webviews
            .panels
            .get(id)
            .filter(|panel| panel.detached)?;
        Some((
            panel
                .frame
                .clamped(self.floating_area_for(id, window), panel.limits),
            panel.limits,
        ))
    }

    pub(in crate::model) fn set_detached_webview_frame(
        &mut self,
        id: &PanelId,
        frame: FloatingFrame,
    ) {
        if let Some(panel) = self.webviews.panels.get_mut(id) {
            panel.frame = frame;
        }
    }

    /// Remembers where the user left a panel that has no instance name.
    /// Named instances are the extension's to remember.
    pub(in crate::model) fn save_detached_webview_frame(
        &mut self,
        id: &PanelId,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self
            .webviews
            .panels
            .get(id)
            .filter(|panel| panel.instance.is_none())
        else {
            return;
        };
        let (owner, kind) = (panel.owner.clone(), panel.kind.clone());
        let mut layout = self.settings.panel_layout(&owner, &kind);
        layout.frame = Some(panel.frame.to_array());
        self.save_panel_layout(&owner, &kind, layout, cx);
    }

    fn save_panel_layout(
        &mut self,
        owner: &str,
        kind: &str,
        layout: muxy_app_core::settings::PanelLayout,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.settings.set_panel_layout(
            owner,
            kind,
            layout,
            &self.path.with_file_name("settings.toml"),
        ) {
            self.fail(
                tr!("Could not save the panel layout: %@", error.to_string()).to_string(),
                cx,
            );
        }
    }

    /// The header's pop-out control: floats a docked panel freely, or docks
    /// a detached one, and remembers the choice.
    pub(crate) fn toggle_webview_panel_detached(
        &mut self,
        id: &PanelId,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        use muxy_app_core::extensions::PanelMode as Mode;
        let Some(panel) = self.webviews.panels.get(id) else {
            return;
        };
        let detached = !panel.detached;
        let (owner, kind, unnamed) = (
            panel.owner.clone(),
            panel.kind.clone(),
            panel.instance.is_none(),
        );
        let mode = if detached {
            Mode::Detached
        } else if panel.placement.mode == PanelMode::Pinned {
            Mode::Pinned
        } else {
            Mode::Floating
        };
        let options = PanelOptions {
            mode: Some(mode),
            ..PanelOptions::default()
        };
        if let Err(error) = self.update_webview_panel(id, &options, window, cx) {
            self.fail(error, cx);
            return;
        }
        if unnamed {
            let mut layout = self.settings.panel_layout(&owner, &kind);
            layout.detached = Some(detached);
            self.save_panel_layout(&owner, &kind, layout, cx);
        }
    }

    /// Extension panels belong to the root project they were opened in, as on
    /// main. Switching projects closes the open docked panel without asking
    /// its page, and announces the new project's panel before
    /// `project.switched`; its page is created once the update that switched
    /// is done, by [`Self::open_restored_panel`]. Worktree switches keep
    /// panels. Detached panels stay open: global ones keep showing, and ones
    /// scoped to a project hide until it returns.
    pub(in crate::model) fn switch_panel_session(&mut self, cx: &mut Context<Self>) {
        let root = root_of(self.state.current_project());
        let Some(previous) = self.webviews.panel_sessions.enter(root) else {
            return;
        };
        let open = self
            .webviews
            .panels
            .iter()
            .find(|(id, _)| self.panels.placement(id).is_some())
            .map(|(_, panel)| SavedPanel {
                owner: panel.owner.clone(),
                kind: panel.kind.clone(),
                instance: panel.instance.clone(),
                position: panel.placement.position,
                data: panel.surface.view.read(cx).data().clone(),
            });
        let pending = self.webviews.panel_sessions.pending.take();
        if let Some(panel) = &pending {
            self.panel_event(
                "panel.closed",
                &panel.owner,
                &panel.kind,
                panel.instance.as_deref(),
                cx,
            );
        }
        if let Some(panel) = open.or(pending) {
            self.webviews.panel_sessions.save(previous, panel);
        }
        let docked: Vec<_> = self
            .webviews
            .panels
            .iter()
            .filter(|(_, panel)| !panel.detached)
            .map(|(id, _)| id.clone())
            .collect();
        for id in docked {
            self.remove_webview_panel(&id, cx);
        }
        let state = &self.state;
        self.webviews
            .panel_sessions
            .retain_projects(|project| state.project(project).is_some());
        if self.builtin_panel_open() {
            return;
        }
        if let Some(panel) = self.webviews.panel_sessions.take(root)
            && self
                .webviews
                .registered_panels
                .contains_key(&(panel.owner.clone(), panel.kind.clone()))
            && !self.webviews.panels.contains_key(&panel_id(
                &panel.owner,
                &panel.kind,
                panel.instance.as_deref(),
            ))
        {
            self.panel_event(
                "panel.opened",
                &panel.owner,
                &panel.kind,
                panel.instance.as_deref(),
                cx,
            );
            self.webviews.panel_sessions.pending = Some(panel);
            cx.notify();
        }
    }

    /// Built-in panels are global, so a project's panel never displaces
    /// them; that panel reopens on a later visit instead.
    fn builtin_panel_open(&self) -> bool {
        [
            crate::views::composer::PANEL,
            crate::model::agents_panel::PANEL,
            crate::model::agent_sessions::PANEL,
        ]
        .into_iter()
        .any(|id| self.panels.placement(&PanelId::new(id)).is_some())
    }

    /// Creates the page of a panel reopened by a project switch. Panel
    /// operations call it first, so they act on the reopened panel.
    pub(in crate::model) fn open_restored_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.webviews.panel_sessions.pending.take() else {
            return;
        };
        cx.notify();
        let definition = self
            .webviews
            .registered_panels
            .get(&(panel.owner.clone(), panel.kind.clone()))
            .cloned();
        let instance = panel.instance.clone();
        let Some(definition) = definition else {
            self.panel_event(
                "panel.closed",
                &panel.owner,
                &panel.kind,
                instance.as_deref(),
                cx,
            );
            return;
        };
        if self.builtin_panel_open() {
            self.panel_event(
                "panel.closed",
                &panel.owner,
                &panel.kind,
                instance.as_deref(),
                cx,
            );
            self.webviews.panel_sessions.keep(panel);
            return;
        }
        let definition = Definition {
            position: panel.position,
            ..definition
        };
        let (owner, kind) = (panel.owner, panel.kind);
        let docked = if self.docked_mode(&owner, &kind, &definition) == PanelMode::Pinned {
            muxy_app_core::extensions::PanelMode::Pinned
        } else {
            muxy_app_core::extensions::PanelMode::Floating
        };
        let options = PanelOptions {
            mode: Some(docked),
            ..PanelOptions::default()
        };
        if let Err(error) = self.create_panel(
            &owner,
            &kind,
            instance.as_deref(),
            definition,
            panel.data,
            &options,
            window,
            cx,
        ) {
            self.extension_log(
                &owner,
                format!("[muxy] could not reopen panel {kind}: {error}"),
            );
            self.panel_event("panel.closed", &owner, &kind, instance.as_deref(), cx);
        }
    }

    /// Forgets the remembered panels of stopped extensions, closing one that
    /// was about to reopen.
    pub(super) fn forget_extension_panels(
        &mut self,
        keep: impl Fn(&str) -> bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(panel) = self
            .webviews
            .panel_sessions
            .pending
            .take_if(|panel| !keep(&panel.owner))
        {
            self.panel_event(
                "panel.closed",
                &panel.owner,
                &panel.kind,
                panel.instance.as_deref(),
                cx,
            );
        }
        self.webviews.panel_sessions.retain_owners(keep);
    }

    fn panel_event(
        &self,
        event: &str,
        owner: &str,
        kind: &str,
        instance: Option<&str>,
        cx: &gpui::App,
    ) {
        let mut payload = json!({"extensionID": owner, "panelID": kind});
        if let Some(instance) = instance {
            payload["instance"] = json!(instance);
        }
        self.emit_extension_event(None, event, &payload, cx);
    }

    /// How the panel docks: the user's pin choice where the manifest allows
    /// one, otherwise the manifest's mode.
    fn docked_mode(&self, owner: &str, kind: &str, definition: &Definition) -> PanelMode {
        if definition.allows_mode_selection {
            self.webview_panel_mode(owner, kind, definition.mode)
        } else {
            definition.mode
        }
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
            } else if id.as_str() == crate::model::agents_panel::PANEL {
                self.agents_panel.drag = None;
                self.agents_panel.resize.end();
                self.appearance.agents_panel_visible = false;
                self.save_appearance(cx);
            } else if id.as_str() == crate::model::agent_sessions::PANEL {
                self.appearance.agent_sessions_visible = false;
                self.sync_agent_sessions(cx);
                self.save_appearance(cx);
            } else if let Some(panel) = self.webviews.panels.get(&id) {
                panel.resize.end();
                self.panel_event(
                    "panel.closed",
                    &panel.owner,
                    &panel.kind,
                    panel.instance.as_deref(),
                    cx,
                );
                let named = panel.instance.is_some();
                if named {
                    self.remove_webview_panel(&id, cx);
                }
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
                self.fail(
                    tr!("Could not save panel pin: %@", error.to_string()).to_string(),
                    cx,
                );
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
        let Some(panel) = self.webviews.panels.remove(id) else {
            return;
        };
        let docked = self.panels.remove(id).is_some();
        self.floating.stack.remove(id);
        if self
            .floating
            .drag
            .as_ref()
            .is_some_and(|(dragged, _)| dragged == id)
        {
            self.floating.drag = None;
        }
        if docked || panel.detached {
            self.panel_event(
                "panel.closed",
                &panel.owner,
                &panel.kind,
                panel.instance.as_deref(),
                cx,
            );
        }
        if panel.focused {
            self.focus_owner.request_active_pane();
        }
        cx.notify();
    }

    pub(super) fn sync_webview_panels(
        &mut self,
        blocked: bool,
        resizing: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.measure_panel_area(window);
        let state = &self.state;
        let orphaned: Vec<_> = self
            .webviews
            .panels
            .iter()
            .filter(|(_, panel)| {
                panel
                    .scope
                    .is_some_and(|project| state.project(project).is_none())
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in orphaned {
            self.remove_webview_panel(&id, cx);
        }
        let shortcuts = self.webview_shortcuts();
        let capture_escape = self.layout_drag_pending();
        for panel in self.webviews.panels.values_mut() {
            panel.focused = panel.is_focused(window, cx);
        }
        let shown: Vec<_> = self
            .webviews
            .panels
            .keys()
            .map(|id| (id.clone(), self.webview_panel_shown(id)))
            .collect();
        let reports: Vec<_> = shown
            .iter()
            .filter_map(|(id, _)| self.panel_state(id).map(|state| (id.clone(), state)))
            .collect();
        for (id, visible) in shown {
            let Some(panel) = self.webviews.panels.get_mut(&id) else {
                continue;
            };
            let regions: Option<Vec<_>> = panel.pointer_regions().map(|regions| {
                regions
                    .iter()
                    .map(|rect| {
                        gpui::Bounds::new(
                            gpui::point(gpui::px(rect.x), gpui::px(rect.y)),
                            gpui::size(gpui::px(rect.width), gpui::px(rect.height)),
                        )
                    })
                    .collect()
            });
            let generation = panel.surface.view.read(cx).native.generation();
            let report = reports
                .iter()
                .find(|(reported, _)| *reported == id)
                .map(|(_, state)| state.clone())
                .filter(|state| {
                    panel
                        .reported
                        .as_ref()
                        .is_none_or(|(seen, last)| *seen != generation || last != state)
                });
            if let Some(state) = &report {
                panel.reported = Some((generation, state.clone()));
            }
            panel.surface.view.update(cx, |view, cx| {
                view.native.set_shortcuts(capture_escape, shortcuts.clone());
                view.native.set_mouse_passthrough(resizing);
                view.native.set_hit_regions(regions.as_deref());
                view.set_pass_through(regions.is_some(), cx);
                view.refresh_theme(&self.theme, self.metrics, cx);
                view.present(
                    visible,
                    blocked,
                    window.is_window_active() && view.focus.is_focused(window),
                    0.0,
                    cx,
                );
                if let Some(state) = report {
                    view.native
                        .evaluate(&format!("window.__muxyApplyPanel?.({state});"));
                }
            });
        }
        let natives: Vec<_> = self
            .floating_members()
            .iter()
            .filter_map(|id| self.webviews.panels.get(id))
            .map(|panel| panel.surface.view.read(cx).native.clone())
            .collect();
        let natives: Vec<_> = natives.iter().map(AsRef::as_ref).collect();
        muxy_ui::webview::NativeWebview::order_front(&natives);
    }
}

/// Extension names and instance names never hold `:`, so a named copy's
/// key can't match another panel's, whatever its panel ID holds.
pub(super) fn panel_id(owner: &str, kind: &str, instance: Option<&str>) -> PanelId {
    match instance {
        Some(instance) => PanelId::new(format!("webview-instance:{owner}:{instance}:{kind}")),
        None => PanelId::new(format!("webview:{owner}:{kind}")),
    }
}

#[cfg(test)]
mod tests {
    use super::panel_id;

    #[test]
    fn a_named_copy_never_shares_a_key_with_another_panel() {
        assert_ne!(
            panel_id("notes", "note#one", None),
            panel_id("notes", "note", Some("one"))
        );
        assert_ne!(
            panel_id("notes", "a:b", Some("c")),
            panel_id("notes", "b", Some("a:c"))
        );
        assert_ne!(
            panel_id("notes", "note", Some("one")),
            panel_id("notes", "note", Some("two"))
        );
    }
}
