mod panels;
mod results;

use std::collections::HashMap;

use gpui::{Context, Entity, Subscription, Window};
use muxy_app_core::{
    PaneContent, PaneId, Tab,
    webview::{ModalOptions, ModalResult, WebviewDescriptor},
};
use muxy_ui::webview::assets::Source;
use serde_json::{Value, json};

use super::AppModel;
use crate::views::{
    overlays::Overlay,
    webview::{Page, Request, SurfaceEvent, SurfaceKind, Webview},
};

struct Registered {
    source: Source,
    title: String,
}

pub(crate) struct Surface {
    pub view: Entity<Webview>,
    kind: String,
    _subscription: Subscription,
}

pub(crate) struct Modal {
    pub id: String,
    pub surface: Surface,
    pub options: ModalOptions,
    result: ModalResult,
    completion: async_channel::Sender<Value>,
    closing: bool,
    opener: gpui::WeakEntity<Webview>,
}

impl Drop for Modal {
    fn drop(&mut self) {
        if let Some(result) = self.result.complete(Value::Null) {
            let _ = self.completion.try_send(result);
        }
    }
}

type Occlusion = (Option<gpui::EntityId>, gpui::Bounds<gpui::Pixels>);

#[derive(Default)]
pub(crate) struct Webviews {
    registered: HashMap<(String, String), Registered>,
    pub panes: HashMap<PaneId, Surface>,
    pub modal: Option<Modal>,
    pub panels: std::collections::BTreeMap<muxy_ui::panel::PanelId, panels::Panel>,
    registered_panels: HashMap<(String, String), panels::Definition>,
    pub occlusions: std::rc::Rc<std::cell::RefCell<Vec<Occlusion>>>,
    results: results::Results,
    sequence: u64,
    close_sequence: u64,
    initialized: bool,
    restore_focus: Option<gpui::WeakEntity<Webview>>,
}

impl AppModel {
    pub(crate) fn sync_webviews(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.initialize_webview_demo(cx);
        if !matches!(self.overlay, Some(Overlay::Webview)) && self.webviews.modal.is_some() {
            self.webviews.modal = None;
        }
        let descriptors = self.ensure_webview_panes(window, cx);
        self.prune_webview_results(cx);
        let visible = self.visible_panes();
        if self.overlay.is_none()
            && let Some(view) = self
                .webviews
                .restore_focus
                .take()
                .and_then(|view| view.upgrade())
        {
            let live = self
                .webviews
                .panes
                .iter()
                .any(|(id, surface)| visible.contains(id) && surface.view == view)
                || self.webviews.panels.iter().any(|(id, panel)| {
                    self.panels.placement(id).is_some() && panel.surface.view == view
                });
            if live {
                view.read(cx).focus.focus(window);
                self.focus_requested = false;
            }
        }
        self.webviews.occlusions.borrow_mut().clear();
        let composer_dialog = (self.composer.view.is_some() || self.composer.closing.is_some())
            && self.settings.composer.presentation
                == muxy_app_core::settings::ComposerPresentation::Floating;
        let blocked = composer_dialog
            || self.overlay.is_some()
            || self.close_prompt.is_some()
            || self.sidebar_resize.is_some()
            || self.split_resize.active()
            || self
                .webviews
                .panels
                .values()
                .any(|panel| panel.resize.is_active())
            || self
                .composer
                .view
                .as_ref()
                .is_some_and(|view| view.read(cx).sizing(window).resize_state().is_active());
        let shortcuts = surface_shortcuts(&self.settings.keymap);
        for (id, surface) in &self.webviews.panes {
            surface
                .view
                .read(cx)
                .native
                .set_shortcuts(false, shortcuts.clone());
            let Some(descriptor) = descriptors.get(id) else {
                continue;
            };
            surface.view.update(cx, |view, cx| {
                view.update_content(descriptor.data.clone(), &self.theme, self.metrics, cx);
                view.present(
                    visible.contains(id),
                    blocked,
                    window.is_window_active() && view.focus.is_focused(window),
                    0.0,
                    cx,
                );
            });
        }
        self.sync_webview_panels(blocked, window, cx);
        if let Some(modal) = &self.webviews.modal {
            modal
                .surface
                .view
                .read(cx)
                .native
                .set_shortcuts(true, Vec::new());
            modal.surface.view.update(cx, |view, cx| {
                view.refresh_theme(&self.theme, self.metrics, cx);
                view.present(
                    true,
                    self.close_prompt.is_some(),
                    window.is_window_active() && view.focus.is_focused(window),
                    f64::from(f32::from(self.metrics.radius_xl())),
                    cx,
                );
            });
        }
    }

    fn ensure_webview_panes(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> HashMap<PaneId, WebviewDescriptor> {
        let descriptors: HashMap<_, _> = self
            .state
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .filter_map(|pane| {
                if let PaneContent::Webview(descriptor) = &pane.content {
                    Some((pane.id, descriptor.clone()))
                } else {
                    None
                }
            })
            .collect();
        self.webviews.panes.retain(|id, surface| {
            descriptors.get(id).is_some_and(|descriptor| {
                descriptor.owner == surface.view.read(cx).source.owner
                    && descriptor.kind == surface.kind
            })
        });
        let visible = self.visible_panes();
        for (id, descriptor) in &descriptors {
            if !self.webviews.panes.contains_key(id) && visible.contains(id) {
                let Some(registered) = self
                    .webviews
                    .registered
                    .get(&(descriptor.owner.clone(), descriptor.kind.clone()))
                else {
                    continue;
                };
                match self.create_webview(
                    registered.source.clone(),
                    id.to_string(),
                    descriptor.data.clone(),
                    SurfaceKind::Tab,
                    window,
                    cx,
                ) {
                    Ok(mut surface) => {
                        surface.kind.clone_from(&descriptor.kind);
                        self.webviews.panes.insert(*id, surface);
                    }
                    Err(error) => {
                        self.error = Some(error);
                    }
                }
            }
        }
        descriptors
    }

    fn prune_webview_results(&mut self, cx: &gpui::App) {
        let live_documents = self
            .webviews
            .panes
            .values()
            .map(|surface| &surface.view)
            .chain(
                self.webviews
                    .panels
                    .values()
                    .map(|panel| &panel.surface.view),
            )
            .chain(self.webviews.modal.iter().map(|modal| &modal.surface.view))
            .map(|view| (view.entity_id(), view.read(cx).native.generation()));
        self.webviews.results.retain(live_documents);
    }

    fn initialize_webview_demo(&mut self, cx: &mut Context<Self>) {
        if !self.webviews.initialized {
            self.webviews.initialized = true;
            if cfg!(debug_assertions)
                && std::env::var("MUXY_WEBVIEW_DEMO").is_ok_and(|value| value == "1")
                && std::env::var_os("MUXY_DIR").is_some()
            {
                self.webviews.registered.insert(
                    ("foundation".into(), "demo".into()),
                    Registered {
                        source: Source {
                            owner: "foundation".into(),
                            directory: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                                .join("fixtures/webview"),
                            entry: "index.html".into(),
                        },
                        title: "Webview Foundation".into(),
                    },
                );
                self.register_demo_panels();
                let descriptor = WebviewDescriptor {
                    owner: "foundation".into(),
                    kind: "demo".into(),
                    data: json!({"message":"Foundation harness"}),
                };
                let restored = self.state.projects().iter().flat_map(|project| &project.tabs)
                    .flat_map(|tab| &tab.panes).any(|pane| matches!(&pane.content,
                        PaneContent::Webview(saved) if saved.owner == descriptor.owner && saved.kind == descriptor.kind));
                if !restored {
                    let _ = self.open_webview_tab(descriptor, true, cx);
                }
            }
        }
    }

    fn create_webview(
        &mut self,
        source: Source,
        id: String,
        data: Value,
        kind: SurfaceKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Surface, String> {
        let view = Webview::create(
            Page {
                source,
                instance: id,
                data,
                kind,
            },
            &self.theme,
            self.metrics,
            window,
            cx,
        )?;
        let subscription =
            cx.subscribe_in(
                &view,
                window,
                |model, view, event, window, cx| match event {
                    SurfaceEvent::Request(request) => {
                        model.webview_request(view, request, window, cx);
                    }
                    SurfaceEvent::Escape if view.read(cx).kind == SurfaceKind::Modal => {
                        model.dismiss_webview_modal(cx);
                    }
                    SurfaceEvent::Escape => {}
                    SurfaceEvent::Shortcut(key) => {
                        window.dispatch_keystroke(key.clone(), cx);
                    }
                },
            );
        Ok(Surface {
            view,
            kind: String::new(),
            _subscription: subscription,
        })
    }

    pub(crate) fn open_webview_tab(
        &mut self,
        descriptor: WebviewDescriptor,
        singleton: bool,
        cx: &mut Context<Self>,
    ) -> Result<PaneId, String> {
        let registered = self
            .webviews
            .registered
            .get(&(descriptor.owner.clone(), descriptor.kind.clone()))
            .ok_or("webview type is not registered")?;
        registered
            .source
            .entry_url()
            .map_err(|error| error.to_string())?;
        let previous = self.state.clone();
        let (_, pane) = self
            .state
            .open_webview(
                self.state.current_project().id,
                descriptor,
                &registered.title,
                singleton,
            )
            .map_err(|error| error.to_string())?;
        if !self.save(cx) {
            self.state = previous;
            return Err("could not save webview tab".into());
        }
        self.sync_visible(cx);
        self.focus_requested = true;
        cx.notify();
        Ok(pane)
    }

    fn webview_request(
        &mut self,
        view: &Entity<Webview>,
        request: &Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if view.read(cx).native.generation() != request.generation {
            return;
        }
        let result = self.dispatch_webview(view, request, window, cx);
        if let Some(result) = result {
            view.read(cx).reply(request, result);
        }
        cx.notify();
    }

    fn dispatch_webview(
        &mut self,
        view: &Entity<Webview>,
        request: &Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Result<Value, String>> {
        let args = &request.body["args"];
        let pane = self
            .webviews
            .panes
            .iter()
            .find_map(|(id, surface)| (surface.view == *view).then_some(*id));
        let is_modal = self
            .webviews
            .modal
            .as_ref()
            .is_some_and(|modal| modal.surface.view == *view);
        let panel = self
            .webviews
            .panels
            .iter()
            .find_map(|(id, panel)| (panel.surface.view == *view).then_some(id.clone()));
        if pane.is_none() && !is_modal && panel.is_none() {
            return Some(Err("surface is no longer registered".into()));
        }
        let result = match request.body["verb"].as_str().unwrap_or("") {
            "surface.focus" => {
                self.focus_webview(view, pane, is_modal, window, cx);
                Ok(Value::Null)
            }
            "tabs.setTitle" | "tabs.setIcon" => {
                return Self::webview_metadata(view, request, pane, cx);
            }
            "tabs.open" => self.open_page_tab(view, args, cx),
            "panels.open" | "panels.toggle" | "panels.close" => {
                self.panel_request(view, request, window, cx)
            }
            "modal.openWebview" => self
                .open_webview_modal(view, args, window, cx)
                .map(|id| json!({"requestID":id})),
            "modal.awaitWebview" => {
                let id = args["requestID"].as_str().unwrap_or("");
                match self
                    .webviews
                    .results
                    .take(id, view.entity_id(), request.generation)
                {
                    Ok(result) => {
                        let view = view.downgrade();
                        let request = request.clone();
                        cx.spawn(async move |_, cx| {
                            let result = result.recv().await.unwrap_or(Value::Null);
                            let _ = view.update(cx, |view, _| view.reply(&request, Ok(result)));
                        })
                        .detach();
                        return None;
                    }
                    Err(error) => Err(error),
                }
            }
            "modal.submitWebview" if is_modal => {
                view.read(cx).reply(request, Ok(Value::Null));
                self.complete_webview_modal(args["result"].clone(), cx);
                return None;
            }
            "modal.closeWebview" => {
                let owner = &view.read(cx).source.owner;
                if self
                    .webviews
                    .modal
                    .as_ref()
                    .is_some_and(|modal| &modal.surface.view.read(cx).source.owner == owner)
                {
                    self.dismiss_webview_modal(cx);
                }
                Ok(Value::Null)
            }
            "lifecycle.closeSelf" => {
                view.read(cx).reply(request, Ok(Value::Null));
                if is_modal {
                    self.complete_webview_modal(Value::Null, cx);
                } else if let Some(panel) = panel {
                    self.remove_webview_panel(&panel, cx);
                } else if let Some(pane) = pane {
                    let previous = self.state.clone();
                    if self.state.close_pane(pane).is_err() || !self.save(cx) {
                        self.state = previous;
                    }
                    self.sync_visible(cx);
                    self.focus_requested = true;
                }
                return None;
            }
            _ => {
                Err("API unavailable: extension runtime and authorization are not installed".into())
            }
        };
        Some(result)
    }

    fn focus_webview(
        &mut self,
        view: &Entity<Webview>,
        pane: Option<PaneId>,
        is_modal: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if (self.overlay.is_some() && !is_modal)
            || self.close_prompt.is_some()
            || pane.is_some_and(|pane| !self.visible_panes().contains(&pane))
            || (view.read(cx).kind == SurfaceKind::Panel
                && !self.webviews.panels.iter().any(|(id, panel)| {
                    panel.surface.view == *view && self.panels.placement(id).is_some()
                }))
        {
            return;
        }
        if let Some(pane) = pane {
            self.focus_pane(pane, cx);
        }
        view.read(cx).focus.focus(window);
        self.focus_requested = false;
        if !is_modal {
            let outside: Vec<_> = self
                .webviews
                .panels
                .iter()
                .filter_map(|(id, panel)| {
                    (self.panels.placement(id).is_some()
                        && panel.placement.mode == muxy_ui::panel::PanelMode::Floating
                        && panel.surface.view != *view)
                        .then_some(id.clone())
                })
                .collect();
            for id in outside {
                self.dismiss_webview_panel(&id, cx);
            }
        }
    }

    fn open_page_tab(
        &mut self,
        view: &Entity<Webview>,
        args: &Value,
        cx: &mut Context<Self>,
    ) -> Result<Value, String> {
        let target = &args["extension"];
        let owner = view.read(cx).source.owner.clone();
        if args["kind"] != "extensionWebView" || target["id"].as_str() != Some(&owner) {
            Err("only registered webviews belonging to this owner are available before extension authorization".into())
        } else {
            let descriptor = WebviewDescriptor {
                owner,
                kind: target["tabType"].as_str().unwrap_or("").into(),
                data: target["data"].clone(),
            };
            self.open_webview_tab(
                descriptor,
                target["singleton"].as_bool().unwrap_or(false),
                cx,
            )
            .map(|id| json!(id.to_string()))
        }
    }

    fn open_webview_modal(
        &mut self,
        opener: &Entity<Webview>,
        args: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        self.webviews.results.check_capacity()?;
        let mut source = opener.read(cx).source.clone();
        source.entry = args["entry"].as_str().ok_or("entry is required")?.into();
        source.entry_url().map_err(|error| error.to_string())?;
        self.webviews.sequence += 1;
        let id = format!("webview:{}:{}", source.owner, self.webviews.sequence);
        let options = ModalOptions {
            width: args["width"].as_f64().unwrap_or(480.0),
            height: args["height"].as_f64().unwrap_or(320.0),
            dismiss_on_outside_click: args["dismissOnOutsideClick"].as_bool().unwrap_or(true),
        }
        .normalized();
        let surface = self.create_webview(
            source,
            id.clone(),
            args["data"].clone(),
            SurfaceKind::Modal,
            window,
            cx,
        )?;
        let completion = self.webviews.results.reserve(
            id.clone(),
            opener.entity_id(),
            opener.read(cx).native.generation(),
        )?;
        surface.view.read(cx).focus.focus(window);
        self.webviews.modal = Some(Modal {
            id: id.clone(),
            surface,
            options,
            completion,
            result: ModalResult::default(),
            closing: false,
            opener: opener.downgrade(),
        });
        self.overlay_subscription = None;
        self.overlay = Some(Overlay::Webview);
        cx.notify();
        Ok(id)
    }

    pub(crate) fn complete_webview_modal(&mut self, value: Value, cx: &mut Context<Self>) {
        if let Some(mut modal) = self.webviews.modal.take() {
            self.webviews.restore_focus = Some(modal.opener.clone());
            if let Some(result) = modal.result.complete(value) {
                let _ = modal.completion.try_send(result);
            }
            if matches!(self.overlay, Some(Overlay::Webview)) {
                self.overlay = None;
            }
            self.focus_requested = true;
            cx.notify();
        }
    }

    pub(crate) fn dismiss_webview_modal(&mut self, cx: &mut Context<Self>) {
        let Some(modal) = self.webviews.modal.as_mut().filter(|modal| !modal.closing) else {
            return;
        };
        modal.closing = true;
        let id = modal.id.clone();
        let result = modal.surface.view.update(cx, Webview::before_close);
        cx.spawn(async move |model, cx| {
            let prevent = result.recv().await.unwrap_or(false);
            let _ = model.update(cx, |model, cx| {
                if let Some(modal) = model.webviews.modal.as_mut().filter(|modal| modal.id == id) {
                    modal.closing = false;
                    if !prevent {
                        model.complete_webview_modal(Value::Null, cx);
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn check_webview_closes(&mut self, cx: &mut Context<Self>) {
        let Some(request) = &self.close_request else {
            return;
        };
        let replies: Vec<_> = request
            .panes
            .iter()
            .filter_map(|id| self.webviews.panes.get(id))
            .map(|surface| surface.view.update(cx, Webview::before_close))
            .collect();
        if replies.is_empty() {
            self.check_next_close(cx);
            return;
        }
        self.webviews.close_sequence += 1;
        let sequence = self.webviews.close_sequence;
        let panes = request.panes.clone();
        cx.spawn(async move |model, cx| {
            let mut prevent = false;
            for reply in replies {
                prevent |= reply.recv().await.unwrap_or(false);
            }
            let _ = model.update(cx, |model, cx| {
                if model.webviews.close_sequence == sequence
                    && model
                        .close_request
                        .as_ref()
                        .is_some_and(|request| request.panes == panes)
                {
                    if prevent {
                        model.close_request = None;
                    } else {
                        model.check_next_close(cx);
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn webview_title<'a>(&'a self, tab: &'a Tab, cx: &'a gpui::App) -> &'a str {
        if tab.custom_title.is_none()
            && let Some(pane) = tab.displayed_pane(self.state.window().active_pane)
            && let Some(surface) = self.webviews.panes.get(&pane.id)
            && let Some(title) = &surface.view.read(cx).title
        {
            return title;
        }
        tab.title(self.state.window().active_pane)
    }
}

impl AppModel {
    pub(crate) fn blur_webviews(&self, cx: &gpui::App) {
        for view in self
            .webviews
            .panes
            .values()
            .map(|surface| &surface.view)
            .chain(
                self.webviews
                    .panels
                    .values()
                    .map(|panel| &panel.surface.view),
            )
        {
            view.read(cx).native.blur();
        }
    }

    pub(crate) fn webview_occlusion(&self) -> gpui::AnyElement {
        self.webview_occlusion_except(None)
    }

    pub(crate) fn webview_occlusion_except(
        &self,
        except: Option<gpui::EntityId>,
    ) -> gpui::AnyElement {
        use gpui::{IntoElement, Styled};
        let occlusions = self.webviews.occlusions.clone();
        gpui::canvas(
            move |bounds, _, _| occlusions.borrow_mut().push((except, bounds)),
            |_, (), _, _| (),
        )
        .absolute()
        .size_full()
        .into_any_element()
    }

    pub(crate) fn apply_webview_occlusions(&self, cx: &gpui::App) -> gpui::AnyElement {
        use gpui::{IntoElement, Styled};
        let views: Vec<_> = self
            .webviews
            .panes
            .values()
            .map(|surface| &surface.view)
            .chain(
                self.webviews
                    .panels
                    .values()
                    .map(|panel| &panel.surface.view),
            )
            .map(|view| (view.entity_id(), view.read(cx).native.clone()))
            .collect();
        let occlusions = self.webviews.occlusions.clone();
        gpui::canvas(
            move |_, _, _| {
                for (id, view) in &views {
                    let regions = occlusions_for(*id, &occlusions.borrow());
                    view.occlude(&regions);
                }
            },
            |_, (), _, _| (),
        )
        .absolute()
        .size_full()
        .into_any_element()
    }
}

fn surface_shortcuts(keymap: &muxy_app_core::settings::Keymap) -> Vec<gpui::Keystroke> {
    use muxy_core::shortcuts::{ALL, ShortcutSettings};
    ALL.iter()
        .flat_map(|shortcut| {
            shortcut
                .contexts
                .iter()
                .filter(|context| context.is_none() || **context == Some("WorkspaceTabs"))
                .flat_map(|context| keymap.keys(shortcut.id, *context))
        })
        .filter_map(|key| gpui::Keystroke::parse(&key).ok())
        .collect()
}

impl AppModel {
    fn webview_metadata(
        view: &Entity<Webview>,
        request: &Request,
        pane: Option<PaneId>,
        cx: &mut Context<Self>,
    ) -> Option<Result<Value, String>> {
        if pane.is_none() {
            return Some(Err("only a tab may set its title or icon".into()));
        }
        let args = &request.body["args"];
        let revision = view.update(cx, |view, _| {
            if request.body["verb"] == "tabs.setIcon" {
                view.icon_revision += 1;
            }
            view.icon_revision
        });
        if request.body["verb"] == "tabs.setTitle" {
            let title = args["title"]
                .as_str()
                .unwrap_or("")
                .trim()
                .chars()
                .take(512)
                .collect::<String>();
            view.update(cx, |view, _| {
                view.title = (!title.is_empty()).then_some(title);
            });
        } else if args["icon"].is_null() {
            view.update(cx, |view, _| view.icon = None);
        } else if let Some(symbol) = args["icon"]
            .as_str()
            .or_else(|| args["icon"]["symbol"].as_str())
        {
            let icon = crate::views::webview::PageIcon::Symbol(symbol.chars().take(200).collect());
            view.update(cx, |view, _| view.icon = Some(icon));
        } else if let Some(svg) = args["icon"]["svg"].as_str() {
            let root = view.read(cx).source.directory.clone();
            let svg = svg.to_owned();
            let load = cx.background_executor().spawn(async move {
                use std::io::Read;
                let path = muxy_ui::webview::assets::resolve(&root, &svg)
                    .map_err(|error| error.to_string())?;
                let mut bytes = Vec::new();
                std::fs::File::open(path)
                    .and_then(|file| file.take(256 * 1024 + 1).read_to_end(&mut bytes))
                    .map_err(|error| error.to_string())?;
                if bytes.len() > 256 * 1024 {
                    return Err("icon exceeds 256 KiB".into());
                }
                Ok(bytes)
            });
            let view = view.downgrade();
            let request = request.clone();
            cx.spawn(async move |model, cx| {
                let result: Result<Vec<u8>, String> = load.await;
                let _ = view.update(cx, |view, _| {
                    if view.native.generation() != request.generation {
                        return;
                    }
                    let result = result.map(|bytes| {
                        if view.icon_revision != revision {
                            return Value::Null;
                        }
                        view.icon =
                            Some(crate::views::webview::PageIcon::Svg(std::sync::Arc::new(
                                gpui::Image::from_bytes(gpui::ImageFormat::Svg, bytes),
                            )));
                        Value::Null
                    });
                    view.reply(&request, result);
                });
                let _ = model.update(cx, |_, cx| cx.notify());
            })
            .detach();
            return None;
        } else {
            return Some(Err("expected a symbol name, SVG resource, or null".into()));
        }
        Some(Ok(Value::Null))
    }

    pub(crate) fn webview_glyph(
        &self,
        tab: &Tab,
        size: gpui::Pixels,
        foreground: gpui::Hsla,
        cx: &gpui::App,
    ) -> Option<gpui::AnyElement> {
        use gpui::{IntoElement, Styled};
        let pane = tab.displayed_pane(self.state.window().active_pane)?;
        if !matches!(pane.content, PaneContent::Webview(_)) || tab.pinned {
            return None;
        }
        let icon = self
            .webviews
            .panes
            .get(&pane.id)
            .and_then(|surface| surface.view.read(cx).icon.as_ref());
        Some(
            if let Some(crate::views::webview::PageIcon::Svg(image)) = icon {
                gpui::img(image.clone()).size(size).into_any_element()
            } else {
                let symbol = if let Some(crate::views::webview::PageIcon::Symbol(symbol)) = icon {
                    symbol.clone()
                } else {
                    "puzzlepiece.extension".into()
                };
                muxy_ui::components::SymbolGlyph::new(symbol, size, foreground).into_any_element()
            },
        )
    }
}

fn occlusions_for(id: gpui::EntityId, occlusions: &[Occlusion]) -> Vec<gpui::Bounds<gpui::Pixels>> {
    let start = occlusions
        .iter()
        .position(|(owner, _)| *owner == Some(id))
        .map_or(0, |index| index + 1);
    occlusions[start..]
        .iter()
        .map(|(_, bounds)| *bounds)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AppContext;

    #[gpui::test]
    fn overlapping_panels_only_occlude_surfaces_below_them(cx: &mut gpui::TestAppContext) {
        let tab = cx.new(|_| ()).entity_id();
        let first = cx.new(|_| ()).entity_id();
        let second = cx.new(|_| ()).entity_id();
        let right = gpui::Bounds::new(
            gpui::point(gpui::px(70.0), gpui::px(0.0)),
            gpui::size(gpui::px(30.0), gpui::px(100.0)),
        );
        let bottom = gpui::Bounds::new(
            gpui::point(gpui::px(0.0), gpui::px(70.0)),
            gpui::size(gpui::px(100.0), gpui::px(30.0)),
        );
        let voice = gpui::Bounds::new(
            gpui::point(gpui::px(20.0), gpui::px(20.0)),
            gpui::size(gpui::px(20.0), gpui::px(20.0)),
        );
        let occlusions = [(Some(first), right), (Some(second), bottom), (None, voice)];
        assert_eq!(occlusions_for(tab, &occlusions), vec![right, bottom, voice]);
        assert_eq!(occlusions_for(first, &occlusions), vec![bottom, voice]);
        assert_eq!(occlusions_for(second, &occlusions), vec![voice]);
    }
}
