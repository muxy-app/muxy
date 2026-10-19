//! Browser panes: their live views, the profiles they browse with, and
//! everything that opens, closes, or drives them.

pub(crate) mod automation;
mod profiles;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{Context, Entity, Pixels, Point, Subscription, Window};
use muxy_app_core::browser::BrowserPane;
use muxy_app_core::{PaneContent, PaneId, ProjectId, Tab};
use muxy_core::capability::Feature;
use muxy_ui::browser::polybrowser::ProfileManager;
use muxy_ui::tr;

use super::AppModel;
use crate::views::browser::{BrowserView, BrowserViewEvent, Menu, Setup};

/// What the browser shortcuts do to the focused browser pane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BrowserAction {
    FocusAddress,
    Reload,
    Back,
    Forward,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    CopyUrl,
    Inspect,
}

const ADDRESS_SAVE_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

pub(crate) struct Surface {
    pub view: Entity<BrowserView>,
    _subscription: Subscription,
}

#[derive(Default)]
pub(crate) struct Browsers {
    pub views: HashMap<PaneId, Surface>,
    profiles: Option<ProfileManager>,
    retiring: HashSet<String>,
    favicons: HashMap<String, Option<Arc<gpui::RenderImage>>>,
    /// Panes opened from the UI, whose address field takes focus.
    focus_address: HashSet<PaneId>,
    /// Panes a page opened, which may close themselves.
    opened_by_page: HashSet<PaneId>,
    /// Saves addresses once pages stop changing them.
    save: Option<gpui::Task<()>>,
    /// Panes whose view could not be created, with why.
    failures: HashMap<PaneId, String>,
    shortcuts: Option<Rc<Vec<gpui::Keystroke>>>,
    /// Profiles as last listed, by ID and name, for Settings.
    profile_list: Vec<(String, String)>,
    /// Other browsers that can be imported from, found when Settings opens.
    pub(crate) import_sources: Vec<muxy_ui::browser::polybrowser::ExternalProfile>,
    pub(crate) socket: Option<crate::app_socket::AppSocket>,
}

/// Where and how to open a browser tab.
#[derive(Clone, Debug)]
pub(crate) struct OpenBrowser {
    pub url: Option<String>,
    /// A profile ID or name; none means the default profile, which the tab
    /// keeps even if another becomes the default later.
    pub profile: Option<String>,
    pub project: ProjectId,
    /// Splits the project's shown pane instead of opening a tab.
    pub split: bool,
    /// Shows the new browser. Without it, the user's view stays where it is.
    pub select: bool,
    pub focus_address: bool,
}

impl AppModel {
    pub(crate) fn browser_enabled(&self) -> bool {
        self.settings.browser.enabled && self.supports(Feature::BrowserPanes)
    }

    /// Every browser pane, with its project and tab.
    pub(crate) fn browser_panes(&self) -> Vec<(ProjectId, &Tab, PaneId, &BrowserPane)> {
        self.state
            .projects()
            .iter()
            .flat_map(|project| project.tabs.iter().map(move |tab| (project.id, tab)))
            .flat_map(|(project, tab)| {
                tab.panes
                    .iter()
                    .filter_map(move |pane| match &pane.content {
                        PaneContent::Browser(browser) => Some((project, tab, pane.id, browser)),
                        _ => None,
                    })
            })
            .collect()
    }

    pub(crate) fn browser_view(&self, pane: PaneId) -> Option<&Entity<BrowserView>> {
        self.browsers.views.get(&pane).map(|surface| &surface.view)
    }

    pub(crate) fn active_browser(&self) -> Option<Entity<BrowserView>> {
        self.active_pane()
            .and_then(|pane| self.browser_view(pane))
            .cloned()
    }

    /// Why a browser pane shows no page, if it can't.
    pub(crate) fn browser_unavailable(&self, pane: PaneId) -> Option<String> {
        if !self.browser_enabled() {
            return Some(tr!("Built-in browser is disabled").into());
        }
        self.browsers.failures.get(&pane).cloned()
    }

    /// Creates the views of shown browser panes, drops those of closed ones,
    /// and shows each as its pane is.
    pub(super) fn sync_browsers(
        &mut self,
        blocked: bool,
        resizing: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.browser_enabled() {
            self.browsers.views.clear();
            return;
        }
        let panes: HashSet<_> = self
            .browser_panes()
            .into_iter()
            .map(|(_, _, pane, _)| pane)
            .collect();
        self.browsers.views.retain(|pane, _| panes.contains(pane));
        self.browsers
            .failures
            .retain(|pane, _| panes.contains(pane));
        self.browsers
            .focus_address
            .retain(|pane| panes.contains(pane));
        self.browsers
            .opened_by_page
            .retain(|pane| panes.contains(pane));
        let visible = self.visible_panes();
        for pane in &visible {
            if self
                .state
                .browser(*pane)
                .and_then(|browser| browser.profile.as_ref())
                .is_some_and(|profile| self.browsers.retiring.contains(profile))
            {
                continue;
            }
            if panes.contains(pane)
                && !self.browsers.views.contains_key(pane)
                && !self.browsers.failures.contains_key(pane)
                && let Err(error) = self.ensure_browser_view(*pane, window, cx)
            {
                self.browsers.failures.insert(*pane, error);
            }
        }
        let shortcuts = self.browser_shortcuts();
        let drag = self.layout_drag_pending();
        let theme = self.theme.clone();
        let metrics = self.metrics;
        let engine = self.settings.browser.search_engine;
        for (id, surface) in &self.browsers.views {
            let command_drag = window.modifiers().platform && self.pane_drag_available(*id);
            surface.view.update(cx, |view, cx| {
                view.native.set_shortcuts(drag, shortcuts.to_vec());
                view.native.set_mouse_passthrough(resizing || command_drag);
                view.engine = engine;
                view.refresh_theme(&theme, metrics, cx);
                view.present(
                    visible.contains(id),
                    blocked,
                    window.is_window_active() && view.focus.contains_focused(window, cx),
                    window,
                    cx,
                );
            });
        }
    }

    /// The pane's view, created hidden if it has none yet, as automation
    /// needs for tabs never shown.
    pub(crate) fn ensure_browser_view(
        &mut self,
        pane: PaneId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Entity<BrowserView>, String> {
        if let Some(view) = self.browser_view(pane) {
            return Ok(view.clone());
        }
        let browser = self
            .state
            .browser(pane)
            .cloned()
            .ok_or_else(|| format!("browser tab not found {pane}"))?;
        let profile = self.pane_profile(browser.profile.as_deref())?;
        let view = BrowserView::create(
            Setup {
                profile,
                url: browser.url.as_deref(),
                engine: self.settings.browser.search_engine,
                focus_address: self.browsers.focus_address.remove(&pane),
            },
            &self.theme,
            self.metrics,
            window,
            cx,
        )?;
        let subscription = cx.subscribe_in(&view, window, move |model, _, event, window, cx| {
            model.browser_event(pane, event, window, cx);
        });
        self.browsers.views.insert(
            pane,
            Surface {
                view: view.clone(),
                _subscription: subscription,
            },
        );
        Ok(view)
    }

    fn browser_event(
        &mut self,
        pane: PaneId,
        event: &BrowserViewEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            BrowserViewEvent::Shortcut(keystroke) => {
                super::webviews::dispatch_shortcut(keystroke.clone(), window, cx);
            }
            BrowserViewEvent::Escape => {
                self.cancel_layout_drag(cx);
            }
            BrowserViewEvent::Navigated(url) => {
                if self
                    .state
                    .set_browser_url(pane, url.clone())
                    .unwrap_or(false)
                    && self.browsers.save.is_none()
                {
                    self.browsers.save = Some(cx.spawn(async move |model, cx| {
                        cx.background_executor().timer(ADDRESS_SAVE_DELAY).await;
                        let _ = model.update(cx, |model, cx| {
                            model.browsers.save = None;
                            model.save(cx);
                        });
                    }));
                }
            }
            BrowserViewEvent::Focus | BrowserViewEvent::PageClicked => {
                if self.overlay.is_none() && self.close_prompt.is_none() {
                    self.focus_pane(pane, cx);
                    if let Some(view) = self.browser_view(pane) {
                        let clicked_into_page = matches!(event, BrowserViewEvent::PageClicked);
                        let toolbar_field_clicked =
                            view.read(cx).focus.contains_focused(window, cx);
                        if clicked_into_page || !toolbar_field_clicked {
                            view.read(cx).focus.focus(window);
                        }
                    }
                    self.focus_requested = false;
                }
            }
            BrowserViewEvent::OpenTab(url) => {
                let Some(project) = self.state.pane_project(pane) else {
                    return;
                };
                let profile = self
                    .state
                    .browser(pane)
                    .and_then(|browser| browser.profile.clone());
                let request = OpenBrowser {
                    url: Some(url.clone()),
                    profile,
                    project,
                    split: false,
                    select: true,
                    focus_address: false,
                };
                match self.open_browser(request, cx) {
                    Ok(opened) => {
                        self.browsers.opened_by_page.insert(opened);
                    }
                    Err(error) => self.fail(error, cx),
                }
            }
            BrowserViewEvent::Close => {
                if self.browsers.opened_by_page.contains(&pane) {
                    self.close_pane(pane, cx);
                }
            }
            BrowserViewEvent::External(url) => self.open_externally(url.clone(), None, cx),
            BrowserViewEvent::Favicon(url) => self.load_favicon(pane, url.clone(), cx),
            BrowserViewEvent::Menu(menu, position) => {
                self.open_browser_menu(pane, *menu, *position, window, cx);
            }
        }
    }

    /// Answers `muxy browser` on the profile's app socket.
    pub(crate) fn start_app_socket(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let path = muxy_app_core::browser::api::socket_path(
            self.path
                .parent()
                .unwrap_or_else(|| std::path::Path::new(".")),
        );
        let (socket, requests) = match crate::app_socket::listen(&path) {
            Ok(listening) => listening,
            Err(error) => {
                crate::diagnostics::event("browser.socket", format_args!("error={error}"));
                return;
            }
        };
        self.browsers.socket = Some(socket);
        cx.spawn_in(window, async move |model, cx| {
            while let Ok(incoming) = requests.recv().await {
                let verb = incoming.request.verb.clone();
                if !verb.starts_with("browser.") {
                    incoming.reply(Err(format!("unknown command {verb}")));
                    continue;
                }
                let args = incoming.request.args.clone();
                let Ok(reply) = model.update_in(cx, |model, window, cx| {
                    model.browser_call(&verb, &args, window, cx)
                }) else {
                    incoming.reply(Err("Muxy is closing".into()));
                    break;
                };
                cx.foreground_executor()
                    .spawn(async move { incoming.reply(reply.await) })
                    .detach();
            }
        })
        .detach();
    }

    pub(crate) fn browser_action(
        &mut self,
        action: BrowserAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use muxy_app_core::browser::{zoom_in, zoom_out};
        let Some(view) = self.active_browser().filter(|_| self.overlay.is_none()) else {
            return;
        };
        let result = view.update(cx, |view, cx| {
            match action {
                BrowserAction::FocusAddress => view.focus_address(window, cx),
                BrowserAction::Reload => view.reload(cx),
                BrowserAction::Back => view.go_back(cx),
                BrowserAction::Forward => view.go_forward(cx),
                BrowserAction::ZoomIn => view.set_zoom(zoom_in(view.zoom), cx),
                BrowserAction::ZoomOut => view.set_zoom(zoom_out(view.zoom), cx),
                BrowserAction::ZoomReset => view.set_zoom(1.0, cx),
                BrowserAction::CopyUrl => {
                    if !muxy_app_core::browser::is_blank(&view.page.url) {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                            view.page.url.clone(),
                        ));
                    }
                }
                BrowserAction::Inspect => return view.toggle_inspector(),
            }
            Ok(())
        });
        if let Err(error) = result {
            self.fail(error, cx);
        }
    }

    /// Opens a browser tab on the home page in the current project, with the
    /// address field ready.
    pub(crate) fn new_browser_tab(&mut self, cx: &mut Context<Self>) {
        if !self.require(Feature::BrowserPanes, cx)
            || !self.browser_enabled()
            || self.state.current_project().status() == muxy_app_core::ProjectStatus::Missing
        {
            return;
        }
        let request = OpenBrowser {
            url: self.settings.browser.home_url(),
            profile: None,
            project: self.state.current_project().id,
            split: false,
            select: true,
            focus_address: true,
        };
        if let Err(error) = self.open_browser(request, cx) {
            self.fail(error, cx);
        }
    }

    /// Opens `url` in a new browser tab of the current project.
    pub(crate) fn open_url_in_browser(&mut self, url: String, cx: &mut Context<Self>) {
        let request = OpenBrowser {
            url: Some(url),
            profile: None,
            project: self.state.current_project().id,
            split: false,
            select: true,
            focus_address: false,
        };
        if let Err(error) = self.open_browser(request, cx) {
            self.fail(error, cx);
        }
    }

    pub(crate) fn open_browser(
        &mut self,
        request: OpenBrowser,
        cx: &mut Context<Self>,
    ) -> Result<PaneId, String> {
        let OpenBrowser {
            url,
            profile,
            project,
            split,
            select,
            focus_address,
        } = request;
        self.capabilities.check(Feature::BrowserPanes)?;
        if !self.browser_enabled() {
            return Err("the built-in browser is disabled".into());
        }
        let profile = self.browser_profile(profile.as_deref())?.id().to_string();
        let browser = BrowserPane::new(url, Some(profile));
        let previous = self.state.clone();
        let anchor = split.then(|| self.shown_pane_of(project)).flatten();
        let opened = match anchor {
            Some(anchor) => self.state.split_browser(anchor, browser, select),
            None => self
                .state
                .open_browser(project, browser, select)
                .map(|(_, pane)| pane),
        };
        let pane = opened.map_err(|error| error.to_string())?;
        if !self.save(cx) {
            self.state = previous;
            return Err(tr!("Could not save tabs").into());
        }
        if focus_address {
            self.browsers.focus_address.insert(pane);
        }
        if select {
            self.dismiss_overlay(cx);
            self.focus_requested = true;
        }
        self.changed(cx);
        Ok(pane)
    }

    /// The pane a project shows in its selected tab.
    fn shown_pane_of(&self, project: ProjectId) -> Option<PaneId> {
        if project == self.state.current_project().id {
            return self.active_pane();
        }
        let selected = self.state.window().selected_tab.get(&project)?;
        let tab = self
            .state
            .project(project)?
            .tabs
            .iter()
            .find(|tab| tab.id == *selected)?;
        self.state
            .shown_pane(tab)
            .or_else(|| tab.layout.leaves().first().copied())
    }

    /// Hands `url` to the system, or to the browser app at `app`.
    pub(crate) fn open_externally(
        &mut self,
        url: String,
        app: Option<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let result = crate::opener::submit(move || crate::opener::open_url(&url, app.as_deref()));
        match result {
            Ok(result) => cx
                .spawn(async move |model, cx| {
                    if let Ok(Err(error)) = result.recv().await {
                        let _ = model.update(cx, |model, cx| {
                            model.fail(
                                tr!("Could not open link: %@", error.to_string()).to_string(),
                                cx,
                            );
                        });
                    }
                })
                .detach(),
            Err(error) => self.fail(
                tr!("Could not open link: %@", error.to_string()).to_string(),
                cx,
            ),
        }
    }

    fn load_favicon(&mut self, pane: PaneId, url: String, cx: &mut Context<Self>) {
        if let Some(image) = self.browsers.favicons.get(&url) {
            let image = image.clone();
            if let Some(view) = self.browser_view(pane) {
                view.update(cx, |view, cx| view.set_favicon(image, cx));
            }
            return;
        }
        let fetch = url.clone();
        let task = cx
            .background_executor()
            .spawn(async move { crate::favicon::fetch(&fetch) });
        cx.spawn(async move |model, cx| {
            let image = task.await.map(Arc::new);
            let _ = model.update(cx, |model, cx| {
                model.browsers.favicons.insert(url, image.clone());
                if let Some(view) = model.browser_view(pane) {
                    view.update(cx, |view, cx| view.set_favicon(image, cx));
                }
            });
        })
        .detach();
    }

    fn open_browser_menu(
        &mut self,
        pane: PaneId,
        menu: Menu,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::views::menu::{Command, Item};
        let items = match menu {
            Menu::Page => vec![Item::action(
                tr!("Web Inspector"),
                Command::BrowserInspect(pane),
            )],
            Menu::External => {
                let url = self
                    .browser_view(pane)
                    .map(|view| view.read(cx).page.url.clone())
                    .unwrap_or_default();
                let mut items = vec![Item::action(
                    tr!("Open in Default Browser"),
                    Command::OpenInBrowser(pane, None),
                )];
                for (index, app) in crate::opener::web_browsers().iter().enumerate() {
                    let item =
                        Item::action(app.name.clone(), Command::OpenInBrowser(pane, Some(index)));
                    items.push(if index == 0 { item.separated() } else { item });
                }
                if muxy_app_core::browser::is_blank(&url) {
                    items = items.into_iter().map(Item::disabled).collect();
                }
                items
            }
            Menu::Profile => {
                self.refresh_browser_profiles();
                let current = self
                    .browser_view(pane)
                    .map(|view| view.read(cx).profile.id().to_string());
                let mut items: Vec<_> = self
                    .settings_browser_profiles()
                    .into_iter()
                    .enumerate()
                    .map(|(index, profile)| {
                        let checked = current.as_deref() == Some(profile.id.as_str());
                        Item::action(profile.name, Command::BrowserProfile(pane, index))
                            .checked_if(checked)
                    })
                    .collect();
                items.push(
                    Item::action(tr!("Manage Profiles…"), Command::ManageBrowserProfiles)
                        .separated(),
                );
                items
            }
        };
        self.open_menu(items, position, window, cx);
    }

    /// Opens the page of `pane` in another browser: the default one, or the
    /// installed browser at `index`.
    pub(crate) fn open_browser_page_in(
        &mut self,
        pane: PaneId,
        index: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let Some(url) = self
            .browser_view(pane)
            .map(|view| view.read(cx).page.url.clone())
            .filter(|url| !muxy_app_core::browser::is_blank(url))
        else {
            return;
        };
        let app = index.and_then(|index| {
            crate::opener::web_browsers()
                .get(index)
                .map(|app| app.path.clone())
        });
        self.open_externally(url, app, cx);
    }

    /// Browses `pane` with another profile: its page loads again there.
    pub(crate) fn switch_browser_profile(
        &mut self,
        pane: PaneId,
        profile: String,
        cx: &mut Context<Self>,
    ) {
        if self.browsers.retiring.contains(&profile) {
            return;
        }
        let url = self
            .browser_view(pane)
            .map(|view| view.read(cx).page.url.clone());
        if let Some(url) = url {
            let _ = self.state.set_browser_url(pane, Some(url));
        }
        match self.state.set_browser_profile(pane, Some(profile)) {
            Ok(true) => {
                self.browsers.views.remove(&pane);
                self.save(cx);
                self.focus_requested = true;
                cx.notify();
            }
            Ok(false) => {}
            Err(error) => self.fail(error.to_string(), cx),
        }
    }

    /// Keystrokes a focused page hands back to the app: Muxy's own, the
    /// browser's, and find.
    fn browser_shortcuts(&mut self) -> Rc<Vec<gpui::Keystroke>> {
        use muxy_core::shortcuts::{ALL, ShortcutSettings};
        if let Some(shortcuts) = &self.browsers.shortcuts {
            return shortcuts.clone();
        }
        let keymap = &self.settings.keymap;
        let mut shortcuts: Vec<_> = ALL
            .iter()
            .flat_map(|shortcut| {
                shortcut
                    .contexts
                    .iter()
                    .filter(|context| matches!(context, None | Some("WorkspaceTabs" | "Browser")))
                    .flat_map(|context| keymap.keys(shortcut.id, *context))
            })
            .filter_map(|key| gpui::Keystroke::parse(&key).ok())
            .collect();
        shortcuts.extend(self.extension_keystrokes());
        let shortcuts = Rc::new(shortcuts);
        self.browsers.shortcuts = Some(shortcuts.clone());
        shortcuts
    }

    pub(super) fn invalidate_browser_shortcuts(&mut self) {
        self.browsers.shortcuts = None;
    }

    pub(crate) fn browser_title<'a>(&'a self, pane: PaneId, cx: &'a gpui::App) -> Option<&'a str> {
        self.browser_view(pane)
            .and_then(|view| view.read(cx).title())
            .or_else(|| {
                let browser = self.state.browser(pane)?;
                muxy_app_core::browser::display_title("", browser.url.as_deref())
            })
    }

    pub(crate) fn browser_glyph(
        &self,
        pane: PaneId,
        size: Pixels,
        foreground: gpui::Hsla,
        cx: &gpui::App,
    ) -> gpui::AnyElement {
        use gpui::{IntoElement, Styled};
        match self
            .browser_view(pane)
            .and_then(|view| view.read(cx).favicon.clone())
        {
            Some(favicon) => gpui::img(favicon).size(size).into_any_element(),
            None => {
                muxy_ui::components::IconGlyph::new(muxy_ui::icon::Icon::Globe, size, foreground)
                    .into_any_element()
            }
        }
    }
}
