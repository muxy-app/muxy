//! A browser pane: the toolbar, the find bar, and the page in a native view.

mod pages;
mod toolbar;

use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Pixels, Point, Render, Styled, Subscription, Task, Window, canvas,
    div, img, px,
};
use muxy_app_core::browser::{self, SearchEngine};
use muxy_ui::browser::polybrowser::{
    BrowserEvent, FindOptions, History, HistoryOrder, HistoryQuery, Profile,
};
use muxy_ui::browser::{Event, NativeBrowser, Options};
use muxy_ui::l10n::tr_key;
use muxy_ui::text_input::{InputEvent, InputStyle, SEARCH_CONTEXT, TextInput};
use muxy_ui::theme::{Metrics, Theme};

const SUGGESTIONS: u32 = 8;
/// How soon after a click into the page a link to another app may open it.
const EXTERNAL_LINK_CLICK: std::time::Duration = std::time::Duration::from_secs(2);

pub(crate) enum BrowserViewEvent {
    Shortcut(gpui::Keystroke),
    Escape,
    /// The page's address changed; the pane saves it.
    Navigated(Option<String>),
    /// The page asked for a new window.
    OpenTab(String),
    /// The page called `window.close()`.
    Close,
    /// A link the system opens, such as `mailto:`.
    External(String),
    /// The user clicked the pane around the page.
    Focus,
    /// The user clicked into the page itself.
    PageClicked,
    Favicon(String),
    Menu(Menu, Point<Pixels>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Menu {
    External,
    Profile,
}

/// What the page last reported.
#[derive(Clone, Debug, Default)]
pub(crate) struct Page {
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub progress: f64,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub error: Option<LoadError>,
    /// How many loads have started, so a wait can tell one began.
    pub loads: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct LoadError {
    pub url: String,
    pub message: String,
}

#[derive(Clone, Debug)]
struct Suggestion {
    url: String,
    title: Option<String>,
}

#[derive(Default)]
struct Suggestions {
    entries: Vec<Suggestion>,
    selected: Option<usize>,
    open: bool,
}

struct FindBar {
    input: Entity<TextInput>,
    found: Option<bool>,
    _subscription: Subscription,
}

#[derive(Default)]
struct Presentation {
    focused: bool,
    visible: bool,
    blocked: bool,
}

pub(crate) struct BrowserView {
    pub(crate) native: Rc<NativeBrowser>,
    pub(crate) focus: FocusHandle,
    pub(crate) profile: Profile,
    pub(crate) page: Page,
    pub(crate) favicon: Option<Arc<gpui::RenderImage>>,
    pub(crate) zoom: f64,
    pub(crate) engine: SearchEngine,
    address: Entity<TextInput>,
    /// The address field's width when last drawn, for its suggestions.
    address_width: Rc<std::cell::Cell<Pixels>>,
    history: Option<History>,
    suggestions: Suggestions,
    find: Option<FindBar>,
    presentation: Presentation,
    snapshot: Option<Arc<gpui::RenderImage>>,
    theme: Theme,
    metrics: Metrics,
    /// The address field takes focus once the pane is shown focused.
    focus_address: bool,
    /// The address field has focus, so page addresses don't replace its text.
    editing: bool,
    _subscriptions: Vec<Subscription>,
    _events: Task<()>,
}

impl EventEmitter<BrowserViewEvent> for BrowserView {}

pub(crate) struct Setup<'a> {
    pub profile: Profile,
    pub url: Option<&'a str>,
    pub engine: SearchEngine,
    pub focus_address: bool,
}

impl BrowserView {
    pub(crate) fn create(
        setup: Setup<'_>,
        theme: &Theme,
        metrics: Metrics,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> Result<Entity<Self>, String> {
        let (native, events) = NativeBrowser::new(
            window,
            &Options {
                profile: &setup.profile,
                url: setup.url,
                allows: browser::engine_loads,
            },
        )?;
        let history = setup.profile.history().ok();
        let url = setup.url.unwrap_or_default().to_owned();
        Ok(cx.new(|cx| {
            cx.on_release(|view: &mut Self, cx| view.clear_snapshot(cx))
                .detach();
            let address = cx.new(|cx| {
                TextInput::new(InputStyle::compact(theme, &metrics), cx)
                    .with_placeholder_key(tr_key!("Search or enter address"))
                    .with_text(display_address(&url))
            });
            let mut subscriptions = vec![cx.subscribe_in(&address, window, Self::address_event)];
            subscriptions.push(cx.on_focus(
                &address.focus_handle(cx),
                window,
                |view: &mut Self, _, cx| view.address_focused(cx),
            ));
            subscriptions.push(cx.on_blur(
                &address.focus_handle(cx),
                window,
                |view: &mut Self, _, cx| view.address_blurred(cx),
            ));
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
                profile: setup.profile,
                page: Page {
                    url,
                    loading: setup.url.is_some(),
                    ..Page::default()
                },
                favicon: None,
                zoom: 1.0,
                engine: setup.engine,
                address,
                address_width: Rc::default(),
                history,
                suggestions: Suggestions::default(),
                find: None,
                presentation: Presentation::default(),
                snapshot: None,
                theme: theme.clone(),
                metrics,
                focus_address: setup.focus_address,
                editing: false,
                _subscriptions: subscriptions,
                _events: events,
            }
        }))
    }

    /// The title a tab shows for this page.
    pub(crate) fn title(&self) -> Option<&str> {
        browser::display_title(&self.page.title, Some(&self.page.url))
    }

    pub(crate) fn is_blank(&self) -> bool {
        browser::is_blank(&self.page.url) && !self.page.loading
    }

    fn handle(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Page(event) => self.page_event(event, cx),
            Event::Escape => cx.emit(BrowserViewEvent::Escape),
            Event::Shortcut(keystroke) => cx.emit(BrowserViewEvent::Shortcut(keystroke)),
            Event::Clicked => cx.emit(BrowserViewEvent::PageClicked),
            Event::Snapshot { id, image } => {
                if self.native.is_current_snapshot(id)
                    && let Some(image) = render_snapshot(image)
                {
                    self.clear_snapshot(cx);
                    self.snapshot = Some(Arc::new(image));
                }
            }
        }
        cx.notify();
    }

    fn page_event(&mut self, event: BrowserEvent, cx: &mut Context<Self>) {
        match event {
            BrowserEvent::LoadStarted { .. } => {
                self.page.loads += 1;
                self.page.loading = true;
                self.page.error = None;
                self.page.title.clear();
            }
            BrowserEvent::UrlChanged { url } => {
                self.page.url.clone_from(&url);
                if !self.editing {
                    self.address
                        .update(cx, |input, cx| input.set_text(display_address(&url), cx));
                }
                cx.emit(BrowserViewEvent::Navigated(Some(url)));
            }
            BrowserEvent::TitleChanged { title } => self.page.title = title,
            BrowserEvent::LoadFinished { .. } => {
                self.page.loading = false;
                self.page.progress = 1.0;
                if self.presentation.visible && !self.presentation.blocked {
                    self.native.snapshot();
                }
            }
            BrowserEvent::LoadFailed { url, error } => {
                self.page.loading = false;
                self.page.error = Some(LoadError {
                    url,
                    message: error,
                });
            }
            BrowserEvent::ProgressChanged { progress } => {
                self.page.progress = progress;
                let request_in_flight = progress > 0.0 && progress < 1.0;
                if request_in_flight {
                    self.page.loading = true;
                }
            }
            BrowserEvent::NavigationStateChanged {
                can_go_back,
                can_go_forward,
            } => {
                self.page.can_go_back = can_go_back;
                self.page.can_go_forward = can_go_forward;
            }
            BrowserEvent::FaviconChanged { url } => cx.emit(BrowserViewEvent::Favicon(url)),
            BrowserEvent::NewWindowRequested { url } => {
                if browser::engine_loads(&url) {
                    cx.emit(BrowserViewEvent::OpenTab(url));
                }
            }
            BrowserEvent::CloseRequested => cx.emit(BrowserViewEvent::Close),
            BrowserEvent::ExternalLinkRequested { url }
                if self.native.clicked_within(EXTERNAL_LINK_CLICK) =>
            {
                cx.emit(BrowserViewEvent::External(url));
            }
            _ => {}
        }
    }

    /// Loads what was typed in the address bar: an address or a search.
    pub(crate) fn submit(&mut self, input: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(url) = browser::resolve_input(input, self.engine) else {
            return;
        };
        self.load(&url, cx);
        self.suggestions.open = false;
        self.focus.focus(window);
        self.native.focus();
    }

    pub(crate) fn load(&mut self, url: &str, cx: &mut Context<Self>) {
        self.page.error = None;
        if let Err(error) = self.native.browser().load_url(url) {
            self.page.error = Some(LoadError {
                url: url.to_owned(),
                message: error.to_string(),
            });
        } else {
            self.page.loading = true;
            self.address
                .update(cx, |input, cx| input.set_text(display_address(url), cx));
        }
        cx.notify();
    }

    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        if let Some(error) = self.page.error.take() {
            self.load(&error.url, cx);
            return;
        }
        let _ = self.native.browser().reload();
        cx.notify();
    }

    pub(crate) fn stop(&mut self, cx: &mut Context<Self>) {
        let _ = self.native.browser().stop();
        self.page.loading = false;
        cx.notify();
    }

    pub(crate) fn go_back(&mut self, cx: &mut Context<Self>) {
        self.page.error = None;
        let _ = self.native.browser().go_back();
        cx.notify();
    }

    pub(crate) fn go_forward(&mut self, cx: &mut Context<Self>) {
        self.page.error = None;
        let _ = self.native.browser().go_forward();
        cx.notify();
    }

    pub(crate) fn set_zoom(&mut self, zoom: f64, cx: &mut Context<Self>) {
        self.zoom = zoom.clamp(browser::MIN_ZOOM, browser::MAX_ZOOM);
        let _ = self.native.browser().set_zoom(self.zoom);
        cx.notify();
    }

    pub(crate) fn toggle_inspector(&mut self) -> Result<(), String> {
        let browser = self.native.browser();
        if browser.is_devtools_open() {
            browser.close_devtools()
        } else {
            browser.open_devtools()
        }
        .map_err(|error| error.to_string())
    }

    pub(crate) fn focus_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_address = false;
        self.native.blur();
        self.address.focus_handle(cx).focus(window);
        cx.notify();
    }

    fn address_focused(&mut self, cx: &mut Context<Self>) {
        self.editing = true;
        self.address.update(cx, TextInput::select_all_text);
        self.update_suggestions("", cx);
        self.suggestions.open = true;
        cx.notify();
    }

    fn address_blurred(&mut self, cx: &mut Context<Self>) {
        self.editing = false;
        self.suggestions.open = false;
        let url = display_address(&self.page.url);
        self.address.update(cx, |input, cx| input.set_text(url, cx));
        cx.notify();
    }

    fn address_event(
        &mut self,
        input: &Entity<TextInput>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Changed => {
                let query = input.read(cx).text().to_owned();
                self.update_suggestions(&query, cx);
                self.suggestions.open = true;
            }
            InputEvent::Submitted => {
                let chosen = self
                    .suggestions
                    .selected
                    .filter(|_| self.suggestions.open)
                    .and_then(|index| self.suggestions.entries.get(index))
                    .map(|suggestion| suggestion.url.clone());
                let text = chosen.unwrap_or_else(|| input.read(cx).text().to_owned());
                self.submit(&text, window, cx);
            }
            InputEvent::Cancelled => {
                if self.suggestions.open {
                    self.suggestions.open = false;
                } else {
                    self.address_blurred(cx);
                    self.focus.focus(window);
                    self.native.focus();
                }
            }
        }
        cx.notify();
    }

    fn update_suggestions(&mut self, query: &str, cx: &mut Context<Self>) {
        let query = query.trim();
        let entries = self
            .history
            .as_ref()
            .and_then(|history| {
                let mut request = HistoryQuery::new().limit(SUGGESTIONS);
                if !query.is_empty() {
                    request = request.text(query).order(HistoryOrder::MostVisited);
                }
                history.entries(&request).ok()
            })
            .unwrap_or_default();
        self.suggestions.entries = entries
            .into_iter()
            .map(|entry| Suggestion {
                url: entry.url,
                title: entry.title.filter(|title| !title.is_empty()),
            })
            .collect();
        self.suggestions.selected = None;
        cx.notify();
    }

    /// Moves the highlighted suggestion; past either end, none is.
    fn move_suggestion(&mut self, down: bool, cx: &mut Context<Self>) -> bool {
        if !self.suggestions.open || self.suggestions.entries.is_empty() {
            return false;
        }
        let count = self.suggestions.entries.len();
        self.suggestions.selected = match (self.suggestions.selected, down) {
            (None, true) => Some(0),
            (None, false) => Some(count - 1),
            (Some(index), true) => (index + 1 < count).then_some(index + 1),
            (Some(index), false) => index.checked_sub(1),
        };
        cx.notify();
        true
    }

    pub(crate) fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(find) = &self.find {
            find.input.update(cx, TextInput::select_all_text);
            find.input.focus_handle(cx).focus(window);
            return;
        }
        let (theme, metrics) = (&self.theme, self.metrics);
        let input = cx.new(|cx| {
            TextInput::new(InputStyle::compact(theme, &metrics), cx)
                .with_placeholder_key(tr_key!("Find on page"))
                .with_key_context(SEARCH_CONTEXT)
        });
        let subscription =
            cx.subscribe_in(
                &input,
                window,
                |view, input, event, window, cx| match event {
                    InputEvent::Changed => {
                        if let Some(find) = &mut view.find {
                            find.found = None;
                        }
                        let query = input.read(cx).text().to_owned();
                        view.find_text(&query, false, cx);
                    }
                    InputEvent::Submitted => view.step_find(false, cx),
                    InputEvent::Cancelled => view.close_find(window, cx),
                },
            );
        input.focus_handle(cx).focus(window);
        self.native.blur();
        self.find = Some(FindBar {
            input,
            found: None,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(crate) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.take().is_some() {
            let _ = self.native.browser().clear_find();
            self.focus_address = false;
            self.focus.focus(window);
            self.native.focus();
            cx.notify();
        }
    }

    pub(crate) fn step_find(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let Some(find) = &self.find else {
            return;
        };
        let query = find.input.read(cx).text().to_owned();
        self.find_text(&query, backwards, cx);
    }

    fn find_text(&mut self, query: &str, backwards: bool, cx: &mut Context<Self>) {
        if query.is_empty() {
            let _ = self.native.browser().clear_find();
            return;
        }
        let options = FindOptions {
            backwards,
            ..FindOptions::default()
        };
        let (sender, found) = async_channel::bounded(1);
        let asked = query.to_owned();
        if self
            .native
            .browser()
            .find(query, options, move |result| {
                let _ = sender.try_send(result.unwrap_or(false));
            })
            .is_err()
        {
            return;
        }
        cx.spawn(async move |view, cx| {
            if let Ok(found) = found.recv().await {
                let _ = view.update(cx, |view, cx| {
                    if let Some(find) = &mut view.find
                        && find.input.read(cx).text() == asked
                    {
                        find.found = Some(found);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    pub(crate) fn set_favicon(
        &mut self,
        image: Option<Arc<gpui::RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        self.favicon = image;
        cx.notify();
    }

    pub(crate) fn refresh_theme(
        &mut self,
        theme: &Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) {
        self.theme = theme.clone();
        self.metrics = metrics;
        for input in std::iter::once(&self.address).chain(self.find.iter().map(|find| &find.input))
        {
            input.update(cx, |input, cx| {
                input.set_style(InputStyle::compact(theme, &metrics), cx);
            });
        }
    }

    /// Shows or hides the page as the pane is shown, covered, or focused.
    pub(crate) fn present(
        &mut self,
        visible: bool,
        blocked: bool,
        focused: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focused = focused && visible && !blocked;
        let focus_changed = self.presentation.focused != focused;
        self.presentation.focused = focused;
        self.native
            .set_focused(focused && !self.inputs_focused(window, cx));
        if focus_changed && !focused {
            self.native.blur();
        }
        if self.presentation.visible != visible || self.presentation.blocked != blocked {
            let was_shown =
                self.shows_page() && self.presentation.visible && !self.presentation.blocked;
            if was_shown && visible && blocked {
                self.native.snapshot();
            }
            if !visible {
                self.clear_snapshot(cx);
            }
            self.presentation.visible = visible;
            self.presentation.blocked = blocked;
            self.native
                .set_visible(visible && !blocked && self.shows_page());
            cx.notify();
        }
        if focus_changed && focused {
            if self.focus_address || self.is_blank() {
                self.focus_address(window, cx);
            } else if !self.inputs_focused(window, cx) {
                self.native.focus();
            }
        }
    }

    fn inputs_focused(&self, window: &Window, cx: &gpui::App) -> bool {
        self.address.focus_handle(cx).is_focused(window)
            || self
                .find
                .as_ref()
                .is_some_and(|find| find.input.focus_handle(cx).is_focused(window))
    }

    /// Whether the native page is what the content area shows.
    fn shows_page(&self) -> bool {
        self.page.error.is_none() && !self.is_blank()
    }

    fn clear_snapshot(&mut self, cx: &mut gpui::App) {
        if let Some(image) = self.snapshot.take() {
            cx.defer(move |cx| cx.drop_image(image, None));
        }
    }
}

impl Focusable for BrowserView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

/// What the address bar shows for `url`: nothing for the start page.
fn display_address(url: &str) -> String {
    if browser::is_blank(url) {
        String::new()
    } else {
        url.to_owned()
    }
}

fn render_snapshot(snapshot: muxy_ui::webview::Snapshot) -> Option<gpui::RenderImage> {
    let pixels = image::RgbaImage::from_raw(snapshot.width, snapshot.height, snapshot.bgra)?;
    Some(gpui::RenderImage::new([image::Frame::new(pixels)]))
}

impl Render for BrowserView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let native = self.native.clone();
        let shown = self.presentation.visible && !self.presentation.blocked && self.shows_page();
        let theme = self.theme.clone();
        let metrics = self.metrics;
        let content = div()
            .relative()
            .flex_1()
            .min_h(px(0.0))
            .overflow_hidden()
            .bg(theme.bg)
            .children(
                self.snapshot
                    .clone()
                    .filter(|_| self.presentation.blocked && self.shows_page())
                    .map(|image| img(image).absolute().size_full()),
            )
            .when(self.is_blank() && self.page.error.is_none(), |content| {
                content.child(pages::start(self.engine, &theme, metrics, cx))
            })
            .children(
                self.page
                    .error
                    .clone()
                    .map(|error| pages::error(&error, &theme, metrics, cx)),
            )
            .child(
                canvas(
                    move |bounds, window, _| {
                        native.sync(bounds, window.content_mask().bounds, 0.0);
                        native.set_visible(shown);
                    },
                    |_, (), _, _| (),
                )
                .absolute()
                .size_full(),
            );
        div()
            .track_focus(&self.focus)
            .key_context("Browser")
            .flex()
            .flex_col()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .bg(theme.bg)
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.emit(BrowserViewEvent::Focus)),
            )
            .child(toolbar::render(self, window, cx))
            .children(toolbar::find_bar(self, cx))
            .child(content)
    }
}

/// What a browser pane shows when it has no page: why, and where to change
/// that.
pub(crate) fn placeholder(
    reason: &str,
    disabled: bool,
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
        .child(muxy_ui::components::IconGlyph::new(
            muxy_ui::icon::Icon::Globe,
            metrics.icon_xxl(),
            theme.fg_muted,
        ))
        .child(
            div()
                .text_size(metrics.font_headline())
                .child(reason.to_owned()),
        )
        .when(disabled, |placeholder| {
            placeholder.child(
                div()
                    .text_color(theme.fg_muted)
                    .text_size(metrics.font_body())
                    .child(muxy_ui::tr!("Enable it in Settings → Browser.")),
            )
        })
        .into_any_element()
}
