pub(crate) mod input;
pub(crate) mod modal;
pub(crate) mod panel;
pub(crate) mod popover;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Task, Window, canvas,
    div, img, prelude::FluentBuilder, px,
};
use muxy_app_core::webview::{ACKNOWLEDGEMENT_TIMEOUT, CloseRequests};
use muxy_ui::theme::{Metrics, Theme};
use muxy_ui::tr;
use muxy_ui::webview::{Event, NativeWebview, OptionMenu, assets::Source};
use serde_json::{Value, json};

#[derive(Clone)]
pub(crate) struct Request {
    pub id: u64,
    pub generation: u64,
    pub body: Value,
}

pub(crate) enum SurfaceEvent {
    Request(Request),
    /// The page started showing another document.
    Navigated,
    Escape,
    Shortcut(gpui::Keystroke),
}

pub(crate) enum PageIcon {
    Symbol(String),
    Svg(Arc<gpui::Image>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SurfaceKind {
    Tab,
    Panel,
    Modal,
    Popover,
    Sidebar,
}

impl SurfaceKind {
    fn name(self) -> &'static str {
        match self {
            Self::Tab => "tab",
            Self::Panel => "panel",
            Self::Modal => "modalWebview",
            Self::Popover => "popover",
            Self::Sidebar => "sidebar",
        }
    }
}

pub(crate) struct Page {
    pub source: Source,
    pub instance: String,
    pub data: Value,
    pub kind: SurfaceKind,
}

#[derive(Default)]
struct Presentation {
    focused: bool,
    visible: bool,
    blocked: bool,
}

impl Presentation {
    fn native_visible(&self, failed: bool) -> bool {
        self.visible && !self.blocked && !failed
    }

    /// A page the app draws is its snapshots. One the system shows keeps a
    /// snapshot under it for when it is hidden, except where the page lets
    /// what is behind it show through.
    fn show_snapshot(&self, modal: bool, failed: bool) -> bool {
        NativeWebview::DRAWN_BY_APP || !modal || !self.native_visible(failed)
    }

    fn update_focus(&mut self, visible: bool, blocked: bool, focused: bool) -> Option<bool> {
        let focused = focused && visible && !blocked;
        if focused == self.focused {
            return None;
        }
        self.focused = focused;
        Some(focused)
    }
}

pub(crate) struct Webview {
    pub native: Rc<NativeWebview>,
    pub focus: FocusHandle,
    pub source: Source,
    pub instance: String,
    pub kind: SurfaceKind,
    pub title: Option<String>,
    pub icon: Option<PageIcon>,
    pub icon_revision: u64,
    data: Value,
    theme: Value,
    background: gpui::Rgba,
    presentation: Presentation,
    snapshot: Option<Arc<gpui::RenderImage>>,
    error: Option<String>,
    radius: f64,
    palette: Theme,
    metrics: Metrics,
    cursor: gpui::CursorStyle,
    pointer: Rc<input::PointerState>,
    /// The `<select>` a page the app draws has open.
    menu: Option<OptionMenu>,
    closes: CloseRequests,
    close_results: HashMap<u64, async_channel::Sender<bool>>,
    _events: Task<()>,
}

impl EventEmitter<SurfaceEvent> for Webview {}

impl Webview {
    pub(crate) fn create(
        page: Page,
        theme: &Theme,
        metrics: Metrics,
        window: &Window,
        cx: &mut gpui::App,
    ) -> Result<Entity<Self>, String> {
        let Page {
            source,
            instance,
            data,
            kind,
        } = page;
        let theme_values = theme_snapshot(theme, metrics);
        let theme = theme.clone();
        let script = bridge_script(&source.owner, &instance, kind, &data, &theme_values);
        let (native, events) = NativeWebview::new(
            window,
            source.clone(),
            &script,
            theme.bg.into(),
            matches!(kind, SurfaceKind::Modal | SurfaceKind::Popover),
        )?;
        Ok(cx.new(move |cx| {
            cx.on_release(|view: &mut Self, cx| view.clear_snapshot(cx))
                .detach();
            let events = cx.spawn(async move |view: gpui::WeakEntity<Self>, cx| {
                while let Ok(event) = events.recv().await {
                    if view.update(cx, |view, cx| view.handle(event, cx)).is_err() {
                        break;
                    }
                }
            });
            Self {
                native: Rc::new(native),
                focus: cx.focus_handle(),
                source,
                instance,
                kind,
                title: None,
                icon: None,
                icon_revision: 0,
                data,
                theme: theme_values,
                background: theme.bg.into(),
                presentation: Presentation::default(),
                snapshot: None,
                error: None,
                radius: 0.0,
                palette: theme.clone(),
                metrics,
                cursor: gpui::CursorStyle::Arrow,
                pointer: Rc::default(),
                menu: None,
                closes: CloseRequests::default(),
                close_results: HashMap::new(),
                _events: events,
            }
        }))
    }

    pub(crate) fn reply(&self, request: &Request, result: Result<Value, String>) {
        let response = match result {
            Ok(value) => {
                json!({ "ok": true, "value": value, "requestID": request.body["requestID"] })
            }
            Err(error) => {
                json!({ "ok": false, "error": error, "requestID": request.body["requestID"] })
            }
        };
        self.native
            .reply(request.id, request.generation, &response.to_string());
    }

    fn handle(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Asset { id, result } => self.native.complete_asset(id, result),
            Event::Message {
                id,
                generation,
                json,
            } => {
                if generation != self.native.generation() {
                    return;
                }
                let body: Value = serde_json::from_str(&json).unwrap_or(Value::Null);
                let request = Request {
                    id,
                    generation,
                    body,
                };
                match request.body["verb"].as_str() {
                    Some("lifecycle.ackBeforeClose") => {
                        if let Some(id) = request.body["args"]["callID"]
                            .as_str()
                            .and_then(|id| id.parse().ok())
                        {
                            self.closes.acknowledge(id);
                        }
                        self.reply(&request, Ok(Value::Null));
                    }
                    Some("lifecycle.resolveBeforeClose") => {
                        if let Some(id) = request.body["args"]["callID"]
                            .as_str()
                            .and_then(|id| id.parse().ok())
                            && self.closes.resolve(id)
                        {
                            self.resolve_close(
                                id,
                                request.body["args"]["prevent"].as_bool().unwrap_or(false),
                            );
                        }
                        self.reply(&request, Ok(Value::Null));
                    }
                    Some(_)
                        if request.body["requestID"].is_string()
                            && request.body["args"].is_object() =>
                    {
                        cx.emit(SurfaceEvent::Request(request));
                    }
                    _ => self.reply(&request, Err("invalid webview message".into())),
                }
            }
            Event::Navigated => {
                self.fail_closes();
                self.error = None;
                self.clear_snapshot(cx);
                cx.emit(SurfaceEvent::Navigated);
            }
            Event::Loaded => {
                self.push_state();
                if self.presentation.native_visible(false) {
                    self.native.snapshot();
                }
            }
            Event::Failed(error) => {
                self.fail_closes();
                self.error = Some(error);
                self.native.set_visible(false);
            }
            Event::Escape => cx.emit(SurfaceEvent::Escape),
            Event::Shortcut(key) => cx.emit(SurfaceEvent::Shortcut(key)),
            Event::Cursor(cursor) => self.cursor = cursor,
            Event::Copied(text) => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                return;
            }
            Event::Menu(menu) => self.menu = menu,
            Event::Snapshot {
                id,
                generation,
                image,
            } => {
                // Asked even when the snapshot will not be shown: a backend
                // that paces its snapshots sends the next one after this.
                let current = self.native.accept_snapshot(id, generation);
                if current
                    && self.error.is_none()
                    && let Some(image) = render_snapshot(image)
                {
                    self.clear_snapshot(cx);
                    self.snapshot = Some(Arc::new(image));
                    cx.notify();
                }
                return;
            }
        }
        cx.notify();
    }

    fn clear_snapshot(&mut self, cx: &mut gpui::App) {
        if let Some(image) = self.snapshot.take() {
            cx.defer(move |cx| cx.drop_image(image, None));
        }
    }

    fn push_state(&self) {
        self.native.evaluate(&format!("window.__muxyApplyData?.({});window.__muxyApplyTheme?.({});window.__muxyApplyFocus?.({});", self.data, self.theme, self.presentation.focused));
        if self.presentation.focused && !self.presentation.blocked {
            self.native.focus();
        }
    }

    pub(crate) fn refresh_theme(
        &mut self,
        theme: &Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) {
        self.update_content(self.data.clone(), theme, metrics, cx);
    }

    /// The page's current `muxy.data`.
    pub(crate) fn data(&self) -> &Value {
        &self.data
    }

    pub(crate) fn update_content(
        &mut self,
        data: Value,
        theme: &Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) {
        let values = theme_snapshot(theme, metrics);
        self.background = theme.bg.into();
        let data_changed = self.data != data;
        let theme_changed = self.theme != values;
        if data_changed || theme_changed {
            self.data = data;
            self.theme = values;
            self.palette = theme.clone();
            self.metrics = metrics;
            if data_changed {
                self.native
                    .evaluate(&format!("window.__muxyApplyData?.({});", self.data));
            }
            if theme_changed {
                self.native
                    .evaluate(&format!("window.__muxyApplyTheme?.({});", self.theme));
            }
            self.native.install_script(&bridge_script(
                &self.source.owner,
                &self.instance,
                self.kind,
                &self.data,
                &self.theme,
            ));
            cx.notify();
        }
    }

    pub(crate) fn present(
        &mut self,
        visible: bool,
        blocked: bool,
        focused: bool,
        radius: f64,
        cx: &mut Context<Self>,
    ) {
        let focus_change = self.presentation.update_focus(visible, blocked, focused);
        self.native.set_focused(self.presentation.focused);
        if focus_change.is_some()
            || self.presentation.visible != visible
            || self.presentation.blocked != blocked
        {
            crate::diagnostics::event(
                "webview.present",
                format_args!(
                    "instance={} kind={:?} visible={visible} blocked={blocked} focused={focused} failed={}",
                    self.instance,
                    self.kind,
                    self.error.is_some()
                ),
            );
        }
        if focus_change == Some(false) || !visible || blocked {
            self.choose(None, cx);
        }
        if let Some(focused) = focus_change {
            if !focused {
                self.native.blur();
            }
            self.native
                .evaluate(&format!("window.__muxyApplyFocus?.({focused});"));
        }
        if self.presentation.visible != visible || self.presentation.blocked != blocked {
            let failed = self.error.is_some();
            let was_shown = self.presentation.native_visible(failed);
            let shown = visible && !blocked && !failed;
            let appeared = visible && !self.presentation.visible;
            if was_shown && !shown && visible {
                self.native.snapshot();
            }
            if !visible {
                self.clear_snapshot(cx);
            }
            self.presentation.visible = visible;
            self.presentation.blocked = blocked;
            self.native.set_visible(shown);
            if shown && appeared {
                self.native.snapshot();
            }
            cx.notify();
        }
        self.radius = radius;
        if focus_change == Some(true) {
            self.native.focus();
        }
    }

    pub(crate) fn before_close(&mut self, cx: &mut Context<Self>) -> async_channel::Receiver<bool> {
        let (sender, receiver) = async_channel::bounded(1);
        let id = self.closes.begin(Instant::now());
        crate::diagnostics::event(
            "webview.close",
            format_args!("instance={} id={id} phase=begin", self.instance),
        );
        self.close_results.insert(id, sender);
        let kind = self.kind.name();
        self.native.evaluate(&format!(
            "window.__muxyBeforeClose?.({id},{},{});",
            json!(kind),
            json!(self.instance)
        ));
        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(ACKNOWLEDGEMENT_TIMEOUT)
                .await;
            let _ = view.update(cx, |view, _| {
                for id in view.closes.expire(Instant::now()) {
                    view.resolve_close(id, false);
                }
            });
        })
        .detach();
        receiver
    }

    /// Passes a key to a page the app draws.
    fn key(&mut self, keystroke: &gpui::Keystroke, pressed: bool, cx: &mut Context<Self>) {
        self.native.key(keystroke, pressed, || {
            cx.read_from_clipboard().and_then(|item| item.text())
        });
        cx.stop_propagation();
    }

    /// Answers the page's open `<select>`, with `None` to close it unchanged.
    fn choose(&mut self, option: Option<usize>, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            self.native.choose(option);
            cx.notify();
        }
    }

    fn option_menu(&self, menu: &OptionMenu, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = menu.options.iter().enumerate().map(|(index, option)| {
            if option.heading {
                return muxy_ui::popover::header(&self.palette, self.metrics)
                    .child(option.label.clone())
                    .into_any_element();
            }
            muxy_ui::popover::row(
                &self.palette,
                self.metrics,
                ("page-option", index),
                option.enabled,
                option.selected,
            )
            .child(option.label.clone())
            .when(option.enabled, |row| {
                row.on_click(cx.listener(move |view, _, _, cx| view.choose(Some(index), cx)))
            })
            .into_any_element()
        });
        div()
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.choose(None, cx);
                    cx.stop_propagation();
                }),
            )
            .child(muxy_ui::popover::contained_dropdown(
                menu.anchor,
                px(240.0),
                muxy_ui::popover::surface(&self.palette, self.metrics)
                    .id("page-options")
                    .overflow_y_scroll()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(rows),
            ))
    }

    fn resolve_close(&mut self, id: u64, prevent: bool) {
        crate::diagnostics::event(
            "webview.close",
            format_args!(
                "instance={} id={id} phase=resolved prevent={prevent}",
                self.instance
            ),
        );
        if let Some(sender) = self.close_results.remove(&id) {
            let _ = sender.try_send(prevent);
        }
    }

    fn fail_closes(&mut self) {
        for id in self.closes.drain() {
            self.resolve_close(id, false);
        }
    }
}

fn render_snapshot(snapshot: muxy_ui::webview::Snapshot) -> Option<gpui::RenderImage> {
    let pixels = image::RgbaImage::from_raw(snapshot.width, snapshot.height, snapshot.bgra)?;
    Some(gpui::RenderImage::new([image::Frame::new(pixels)]))
}

impl Drop for Webview {
    fn drop(&mut self) {
        self.fail_closes();
    }
}

impl Render for Webview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let native = self.native.clone();
        let background = self.background;
        let radius = self.radius;
        let failed = self.error.is_some();
        let visible = self.presentation.native_visible(failed);
        let drawn = NativeWebview::DRAWN_BY_APP;
        let pointer = self.pointer.clone();
        let cursor = self.cursor;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "A corner radius is a few points"
        )]
        let corners = px(radius as f32);
        div()
            .track_focus(&self.focus)
            .when(drawn, |page| {
                page.on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, _, cx| {
                    view.key(&event.keystroke, true, cx);
                }))
                .on_key_up(cx.listener(
                    |view, event: &gpui::KeyUpEvent, _, cx| {
                        view.key(&event.keystroke, false, cx);
                    },
                ))
            })
            .relative()
            .flex()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .bg(background)
            .rounded(corners)
            .overflow_hidden()
            .children(
                self.snapshot
                    .clone()
                    .filter(|_| {
                        self.presentation.show_snapshot(
                            matches!(self.kind, SurfaceKind::Modal | SurfaceKind::Popover),
                            failed,
                        )
                    })
                    .map(|image| img(image).absolute().size_full().rounded(corners)),
            )
            .children(
                self.error
                    .clone()
                    .map(|error| div().p(px(20.0)).child(error)),
            )
            .child(
                canvas(
                    move |bounds, window, _| {
                        native.sync(
                            bounds,
                            window.content_mask().bounds,
                            background,
                            radius,
                            window.scale_factor(),
                        );
                        native.set_visible(visible);
                        (drawn && visible).then(|| {
                            let hitbox = window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal);
                            (native, hitbox)
                        })
                    },
                    move |bounds, page, window, _| {
                        if let Some((native, hitbox)) = page {
                            pointer.forward(
                                move |event| native.pointer(event),
                                bounds,
                                hitbox,
                                cursor,
                                window,
                            );
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .children(
                self.menu
                    .as_ref()
                    .map(|menu| self.option_menu(menu, cx).into_any_element()),
            )
    }
}

fn bridge_script(owner: &str, id: &str, kind: SurfaceKind, data: &Value, theme: &Value) -> String {
    format!(
        "{}({}, {});",
        include_str!("bridge.js"),
        json!({ "owner": owner, "id": id, "surface": kind.name(), "data": data, "theme": theme }),
        include_str!("../../extensions/bridge.js")
    )
}

fn theme_snapshot(theme: &Theme, metrics: Metrics) -> Value {
    let color = |color: gpui::Hsla| -> String {
        let rgba: gpui::Rgba = color.into();
        let hex = u32::from(rgba);
        if rgba.a >= 0.999 {
            format!("#{:06x}", hex >> 8)
        } else {
            format!("#{hex:08x}")
        }
    };
    json!({
        "background": color(theme.bg), "foreground": color(theme.fg), "foregroundMuted": color(theme.fg_muted),
        "surface": color(theme.surface), "surfaceSolid": color(theme.raised()), "border": color(theme.border),
        "hover": color(theme.hover), "accent": color(theme.accent), "accentForeground": color(theme.accent_foreground),
        "accentSoft": color(theme.accent_soft), "diffAdd": color(theme.diff_add), "diffRemove": color(theme.diff_remove),
        "diffHunk": color(theme.diff_hunk), "colorScheme": if gpui::Rgba::from(theme.bg).r + gpui::Rgba::from(theme.bg).g + gpui::Rgba::from(theme.bg).b < 1.5 { "dark" } else { "light" },
        "topbarHeight": format!("{}px", f32::from(metrics.title_bar_height()).round())
    })
}

pub(crate) fn placeholder(
    owner: &str,
    kind: &str,
    theme: &Theme,
    metrics: Metrics,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .child(
            muxy_ui::components::SymbolGlyph::new("puzzlepiece.extension", px(32.0), theme.fg)
                .regular(),
        )
        .child(
            div()
                .text_size(metrics.font_headline())
                .child(tr!("Extension %@ is not loaded", owner)),
        )
        .child(
            div()
                .text_color(theme.fg_muted)
                .text_size(metrics.font_body())
                .child(tr!("Tab type: %@", kind)),
        )
        .into_any_element()
}
