use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Pixels, Render, ScrollHandle, SharedString, StatefulInteractiveElement, Styled,
    Subscription, WeakEntity, Window, canvas, div, point, px,
};
use muxy_ui::{
    accessibility,
    components::SymbolGlyph,
    controls::{self, Choice, Style},
    form,
    text_input::{InputEvent, InputStyle, TextInput},
    theme::{Metrics, Theme},
};

use crate::{
    extensions::{
        authoring::AgentTask,
        marketplace::{self, ExtensionDetails, MarketplaceItem, Sort},
        publish::PublishFailure,
    },
    model::{
        AppModel,
        extensions::{ActiveInstall, FinishedInstall},
    },
};
use muxy_app_core::extensions::{Consent, Extension, Rule};
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::tr;

mod cards;
mod configuration;
mod detail;
mod pictures;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Installed,
    Marketplace,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mutation {
    Install,
    Update,
    UpdateAll,
    Enable,
    Disable,
    Remove,
    LoadUnpacked,
    Reload,
    Publish,
    ResetPermissions,
    RemoveRule,
    Trust,
    Untrust,
}

impl From<&ActiveInstall> for Mutation {
    fn from(job: &ActiveInstall) -> Self {
        match job {
            ActiveInstall::Install { .. } => Self::Install,
            ActiveInstall::Update { .. } => Self::Update,
            ActiveInstall::UpdateAll => Self::UpdateAll,
        }
    }
}

impl Mutation {
    fn button(self) -> &'static str {
        match self {
            Self::Install => "extension-install",
            Self::Update => "extension-update",
            Self::UpdateAll => "extension-update-all",
            Self::Enable | Self::Disable => "extension-enable",
            Self::Remove => "extension-remove",
            Self::LoadUnpacked => "extension-load",
            Self::Reload => "extension-reload",
            Self::Publish => "extension-publish",
            Self::ResetPermissions => "extension-reset-permissions",
            Self::RemoveRule => "extension-rule-remove",
            Self::Trust => "extension-trust",
            Self::Untrust => "extension-untrust",
        }
    }

    /// The English key of the busy label shown in place of `idle`.
    fn label(self, idle: &str) -> &'static str {
        match self {
            Self::Install => tr_key!("Installing…"),
            Self::Update | Self::UpdateAll => tr_key!("Updating…"),
            Self::Enable => tr_key!("Enabling…"),
            Self::Disable => tr_key!("Disabling…"),
            Self::Remove if idle == "Unload folder" => tr_key!("Unloading…"),
            Self::Remove => tr_key!("Uninstalling…"),
            Self::LoadUnpacked => tr_key!("Loading…"),
            Self::Reload => tr_key!("Reloading…"),
            Self::Publish => tr_key!("Publishing…"),
            Self::ResetPermissions => tr_key!("Clearing…"),
            Self::RemoveRule => tr_key!("Removing…"),
            Self::Trust | Self::Untrust => tr_key!("Saving…"),
        }
    }
}

#[derive(Debug, PartialEq)]
enum Loading {
    Search,
    More,
    Details(String),
}

pub(crate) struct ExtensionsView {
    model: WeakEntity<AppModel>,
    theme: Theme,
    metrics: Metrics,
    tab: Tab,
    search: Entity<TextInput>,
    items: Vec<MarketplaceItem>,
    page: u32,
    has_next: bool,
    query: String,
    /// The marketplace category Browse is limited to, if any.
    category: Option<&'static str>,
    sort: Sort,
    revision: u64,
    loading: Option<Loading>,
    mutation: Option<Mutation>,
    completed: Option<Mutation>,
    error: Option<String>,
    /// The last marketplace job's result, until the next action clears it.
    install_status: Option<String>,
    /// The extension of the last publish, and its pull request.
    published: Option<(String, String)>,
    /// Whether this view started the running job, and so opens what it installs.
    started_install: bool,
    selected: Option<ExtensionDetails>,
    /// Which screenshot the open extension's gallery shows.
    screenshot: usize,
    pictures: pictures::Pictures,
    list_scroll: ScrollHandle,
    detail_scroll: ScrollHandle,
    /// Where each section of the open extension's page sits among the
    /// scrolled children, for the header's links.
    sections: Vec<(detail::Section, usize)>,
    /// The section a header link last scrolled to, shown as current until
    /// the page is scrolled by hand.
    jumped: Option<detail::Section>,
    /// The scrolled area's width when it was last laid out, which sets how
    /// many grid columns fit and whether the gallery has a column beside it.
    width: Rc<Cell<Pixels>>,
    folder: Option<muxy_ui::dialog::FolderPicker>,
    /// Text fields for string and number extension settings, by extension and key.
    setting_inputs: std::collections::HashMap<(String, String), Entity<TextInput>>,
    subscriptions: Vec<Subscription>,
}

impl ExtensionsView {
    pub(crate) fn new(
        model: WeakEntity<AppModel>,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(InputStyle::field(&theme, &metrics), cx)
                .with_placeholder_key(tr_key!("Search extensions…"))
                .with_accessibility_id("extension-search")
        });
        let changes = cx.subscribe(&search, |view: &mut Self, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                view.query = view.search.read(cx).text().to_owned();
                view.selected = None;
                view.completed = None;
                if view.tab == Tab::Marketplace {
                    view.fetch(true, cx);
                }
                cx.notify();
            }
        });
        let mut subscriptions = vec![changes];
        if let Some(model) = model.upgrade() {
            subscriptions.push(cx.observe(&model, |view, _, cx| {
                view.take_finished_install(cx);
                cx.notify();
            }));
        }
        let view = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = view.update(cx, Self::take_finished_install);
        });
        cx.on_release(|view, cx| view.pictures.release(cx)).detach();
        Self {
            model,
            theme,
            metrics,
            tab: Tab::Installed,
            search,
            items: Vec::new(),
            page: 1,
            has_next: false,
            query: String::new(),
            category: None,
            sort: Sort::default(),
            revision: 0,
            loading: None,
            mutation: None,
            completed: None,
            error: None,
            install_status: None,
            published: None,
            started_install: false,
            selected: None,
            screenshot: 0,
            pictures: pictures::Pictures::default(),
            list_scroll: ScrollHandle::new(),
            detail_scroll: ScrollHandle::new(),
            sections: Vec::new(),
            jumped: None,
            width: Rc::new(Cell::new(px(0.0))),
            folder: None,
            setting_inputs: std::collections::HashMap::new(),
            subscriptions,
        }
    }

    pub(crate) fn sync_appearance(
        &mut self,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) {
        self.theme = theme;
        self.metrics = metrics;
        let style = InputStyle::field(&self.theme, &self.metrics);
        for input in std::iter::once(&self.search).chain(self.setting_inputs.values()) {
            input.update(cx, |input, cx| input.set_style(style, cx));
        }
        cx.notify();
    }

    fn layout_scale(&self) -> Metrics {
        super::layout_metrics(self.metrics)
    }

    fn active_install<'a>(&self, cx: &'a gpui::App) -> Option<&'a ActiveInstall> {
        let model = self.model.upgrade()?;
        model.read(cx).extensions.installs.active.as_ref()
    }

    fn status_note(&self, cx: &gpui::App) -> Option<String> {
        self.model
            .upgrade()
            .and_then(|model| model.read(cx).extensions.installs.status.clone())
            .or_else(|| self.install_status.clone())
    }

    fn mutating(&self, cx: &gpui::App) -> bool {
        self.mutation.is_some() || self.active_install(cx).is_some()
    }

    fn busy(&self, cx: &gpui::App) -> bool {
        self.mutating(cx) || self.loading.is_some()
    }

    fn fetch(&mut self, reset: bool, cx: &mut Context<Self>) {
        if self.mutating(cx) {
            return;
        }
        self.revision += 1;
        let revision = self.revision;
        if reset {
            self.page = 1;
            self.items.clear();
            self.has_next = false;
            self.pictures.forget_listed_screenshots(cx);
        }
        let page = if reset { 1 } else { self.page + 1 };
        let query = self.query.clone();
        let category = self.category;
        let sort = self.sort;
        self.loading = Some(if reset {
            Loading::Search
        } else {
            Loading::More
        });
        self.error = None;
        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(180))
                .await;
            if !view
                .update(cx, |view, _| view.revision == revision)
                .unwrap_or(false)
            {
                return;
            }
            let result = crate::extensions::io::network(move || {
                marketplace::list(&query, page, category, sort)
            })
            .await;
            let _ = view.update(cx, |view, cx| {
                if view.revision != revision {
                    return;
                }
                view.loading = None;
                match result {
                    Ok(listing) => {
                        view.has_next = listing.has_next;
                        view.page = page;
                        view.items.extend(listing.items);
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn details(&mut self, name: String, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        self.revision += 1;
        let revision = self.revision;
        self.loading = Some(Loading::Details(name.clone()));
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = crate::extensions::io::network(move || marketplace::detail(&name)).await;
            let _ = view.update(cx, |view, cx| {
                if view.revision != revision {
                    return;
                }
                view.loading = None;
                match result {
                    Ok(mut details) => {
                        // Only listings say whether an extension is official.
                        details.about.official |= view
                            .items
                            .iter()
                            .any(|item| item.name == details.name && item.about.official);
                        view.open_details(details, cx);
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn load_unpacked(&mut self, cx: &mut Context<Self>) {
        if self.folder.is_some() || self.mutating(cx) {
            return;
        }
        let (sender, receiver) = async_channel::bounded(1);
        match muxy_ui::dialog::choose_folder(
            cx,
            &tr!("Choose an extension folder containing package.json or a built dist folder"),
            std::path::Path::new("/"),
            move |path| {
                let _ = sender.try_send(path);
            },
        ) {
            Ok(folder) => {
                self.folder = Some(folder);
                self.mutation = Some(Mutation::LoadUnpacked);
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        }
        cx.spawn(async move |view, cx| {
            let answer = receiver.recv().await.unwrap_or(Ok(None));
            let task = view.update(cx, |view, cx| {
                view.folder = None;
                answer.map(|path| {
                    path.map(|path| {
                        view.model
                            .update(cx, |model, cx| model.load_unpacked_extension(path, cx))
                    })
                })
            });
            let result = match task {
                Ok(Ok(Some(Ok(task)))) => task.await,
                Ok(Ok(None)) => Ok(()),
                Ok(Err(error)) => Err(error.to_string()),
                Ok(Ok(Some(Err(error)))) | Err(error) => Err(error.to_string()),
            };
            let _ = view.update(cx, |view, cx| {
                view.mutation = None;
                match result {
                    Ok(()) => {
                        view.tab = Tab::Installed;
                        view.selected = None;
                        view.error = None;
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn install(&mut self, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        let Some(details) = self.selected.clone() else {
            return;
        };
        let started = self.model.update(cx, |model, cx| {
            model.install_marketplace_extension(details, cx)
        });
        self.install_started(started.unwrap_or(false));
    }

    fn update_all(&mut self, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        let started = self.model.update(cx, AppModel::update_all_extensions);
        self.install_started(started.unwrap_or(false));
    }

    fn install_started(&mut self, started: bool) {
        if started {
            self.started_install = true;
            self.install_status = None;
            self.completed = None;
            self.error = None;
        }
    }

    fn take_finished_install(&mut self, cx: &mut Context<Self>) {
        let finished = self
            .model
            .update(cx, |model, _| model.extensions.installs.finished.take());
        if let Ok(Some(finished)) = finished {
            self.show_finished_install(finished, cx);
            cx.notify();
        }
    }

    fn show_finished_install(&mut self, finished: FinishedInstall, cx: &mut Context<Self>) {
        let started = std::mem::take(&mut self.started_install);
        let succeeded = finished.error.is_none();
        self.error = finished.error;
        let opened = match &finished.job {
            ActiveInstall::Install { name } if started && succeeded => {
                self.open_installed(name, false, cx)
            }
            ActiveInstall::Update { name } if started && succeeded => {
                self.open_installed(name, true, cx)
            }
            _ => false,
        };
        if !opened {
            self.install_status = finished.status;
        }
    }

    fn open_installed(&mut self, name: &str, updated: bool, cx: &mut Context<Self>) -> bool {
        let details = self.model.upgrade().and_then(|model| {
            model
                .read(cx)
                .extensions
                .registry
                .extensions
                .get(name)
                .map(ExtensionDetails::from)
        });
        let Some(details) = details else {
            return false;
        };
        self.tab = Tab::Installed;
        self.query.clear();
        self.search.update(cx, |search, cx| search.set_text("", cx));
        self.open_details(details, cx);
        self.completed = updated.then_some(Mutation::Update);
        true
    }

    /// Shows `details` on its own page. The same extension, refreshed, keeps
    /// its place; another starts at the top with its first screenshot.
    fn open_details(&mut self, details: ExtensionDetails, cx: &mut Context<Self>) {
        let same = self
            .selected
            .as_ref()
            .is_some_and(|selected| selected.name == details.name);
        if !same {
            self.screenshot = 0;
            self.jumped = None;
            self.detail_scroll.set_offset(point(px(0.0), px(0.0)));
        }
        self.selected = Some(details);
        self.error = None;
        cx.notify();
    }

    fn enable(&mut self, name: &str, enabled: bool, cx: &mut Context<Self>) {
        self.change_extension(name, Some(enabled), cx);
    }

    fn remove(&mut self, name: &str, cx: &mut Context<Self>) {
        self.change_extension(name, None, cx);
    }

    fn change_extension(&mut self, name: &str, enabled: Option<bool>, cx: &mut Context<Self>) {
        if self.busy(cx) {
            return;
        }
        let name = name.to_owned();
        let checks = if enabled == Some(true) {
            Vec::new()
        } else {
            self.model
                .update(cx, |model, cx| {
                    model.extension_close_checks(Some(&name), cx)
                })
                .unwrap_or_default()
        };
        self.mutation = Some(match enabled {
            Some(true) => Mutation::Enable,
            Some(false) => Mutation::Disable,
            None => Mutation::Remove,
        });
        self.error = None;
        cx.spawn(async move |view, cx| {
            let mut prevented = false;
            for surface in checks {
                let Ok(check) = surface.update(cx, crate::views::webview::Webview::before_close)
                else {
                    prevented = true;
                    break;
                };
                if check.recv().await.unwrap_or(true) {
                    prevented = true;
                    break;
                }
            }
            let result = if prevented {
                Err(
                    tr!("The extension kept an open view. Save or close it before continuing.")
                        .to_string(),
                )
            } else {
                match view.update(cx, |view, cx| {
                    view.model.update(cx, |model, cx| match enabled {
                        Some(enabled) => model.set_extension_enabled(&name, enabled, cx),
                        None => model.remove_extension(&name, cx),
                    })
                }) {
                    Ok(Ok(task)) => task.await,
                    Ok(Err(error)) | Err(error) => Err(error.to_string()),
                }
            };
            let _ = view.update(cx, |view, cx| {
                view.mutation = None;
                match result {
                    Ok(()) => {
                        view.error = None;
                        if enabled.is_none() {
                            view.selected = None;
                        }
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn reset_permissions(&mut self, owner: &str, cx: &mut Context<Self>) {
        let owner = owner.to_owned();
        self.run(
            Mutation::ResetPermissions,
            move |model, cx| model.reset_extension_permissions(&owner, cx),
            cx,
        );
    }

    fn remove_rule(&mut self, owner: &str, rule: Rule, cx: &mut Context<Self>) {
        let owner = owner.to_owned();
        self.run(
            Mutation::RemoveRule,
            move |model, cx| model.remove_extension_rule(&owner, rule, cx),
            cx,
        );
    }

    /// Asks twice, through the button and then a warning, before allowing
    /// every gated action of the extension without a prompt.
    fn trust(&mut self, owner: &str, window: &mut Window, cx: &mut Context<Self>) {
        let owner = owner.to_owned();
        let parent = muxy_ui::dialog::Parent::new(window.window_handle(), self.style());
        cx.spawn(async move |view, cx| {
            let (title, message) = AppModel::trust_question(&owner);
            let [cancel, allow] = AppModel::trust_buttons();
            let answer =
                muxy_ui::dialog::choose(&parent, &title, &message, &[&cancel, &allow], cx).await;
            match answer {
                Ok(Some(1)) => {
                    let _ = view.update(cx, |view, cx| view.set_trusted(&owner, true, cx));
                }
                Ok(_) => {}
                Err(error) => {
                    let _ = view.update(cx, |view, cx| {
                        view.error = Some(error.to_string());
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn set_trusted(&mut self, owner: &str, trusted: bool, cx: &mut Context<Self>) {
        let owner = owner.to_owned();
        self.run(
            if trusted {
                Mutation::Trust
            } else {
                Mutation::Untrust
            },
            move |model, cx| model.set_extension_trusted(&owner, trusted, cx),
            cx,
        );
    }

    /// Runs a model change, showing progress on its button until it finishes.
    fn run(
        &mut self,
        mutation: Mutation,
        change: impl FnOnce(&mut AppModel, &mut Context<AppModel>) -> gpui::Task<Result<(), String>>,
        cx: &mut Context<Self>,
    ) {
        if self.mutating(cx) {
            return;
        }
        let task = self.model.update(cx, change);
        self.mutation = Some(mutation);
        self.completed = None;
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = match task {
                Ok(task) => task.await,
                Err(error) => Err(error.to_string()),
            };
            let _ = view.update(cx, |view, cx| {
                view.mutation = None;
                view.completed = result.is_ok().then_some(mutation);
                view.error = result.err();
                cx.notify();
            });
        })
        .detach();
    }

    /// Shows a file in Finder.
    fn reveal(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        match crate::opener::submit(move || crate::opener::reveal(&path)) {
            Ok(result) => cx
                .spawn(async move |view, cx| {
                    if let Ok(Err(error)) = result.recv().await {
                        let _ = view.update(cx, |view, cx| {
                            view.error = Some(
                                tr!("Could not show the file: %@", error.to_string()).to_string(),
                            );
                            cx.notify();
                        });
                    }
                })
                .detach(),
            Err(error) => {
                self.error =
                    Some(tr!("Could not show the file: %@", error.to_string()).to_string());
            }
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.mutating(cx) {
            return;
        }
        let task = self.model.update(cx, AppModel::reload_extensions);
        self.pictures.forget_files(cx);
        self.mutation = Some(Mutation::Reload);
        self.completed = None;
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = match task {
                Ok(task) => task.await,
                Err(error) => Err(error.to_string()),
            };
            let _ = view.update(cx, |view, cx| {
                view.mutation = None;
                view.completed = result.is_ok().then_some(Mutation::Reload);
                view.error = result.err();
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn agent_task(&self, task: AgentTask, cx: &mut Context<Self>) {
        let _ = self
            .model
            .update(cx, |model, cx| model.open_extension_agent(task, cx));
    }

    fn publish(&mut self, name: &str, cx: &mut Context<Self>) {
        if self.mutating(cx) {
            return;
        }
        let name = name.to_owned();
        let task = self
            .model
            .update(cx, |model, cx| model.publish_extension(&name, cx));
        self.mutation = Some(Mutation::Publish);
        self.published = None;
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = match task {
                Ok(task) => task.await,
                Err(error) => Err(PublishFailure::Failed(error.to_string())),
            };
            let _ = view.update(cx, |view, cx| {
                view.mutation = None;
                match result {
                    Ok(request) => {
                        view.install_status = Some(if request.updated {
                            tr!("Updated the open pull request on %@.", request.branch).to_string()
                        } else {
                            tr!("Opened a pull request from %@.", request.branch).to_string()
                        });
                        view.published = Some((name.clone(), request.url));
                    }
                    Err(PublishFailure::Declined) => {}
                    Err(PublishFailure::Failed(error)) => view.error = Some(error),
                    Err(failure) => {
                        let _ = view.model.update(cx, |model, cx| {
                            model.offer_publish_help(&name, &failure, cx);
                        });
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn button_label<'a>(&self, id: &str, idle: &'a str, cx: &gpui::App) -> &'a str {
        let mutation = self
            .mutation
            .or_else(|| self.active_install(cx).map(Mutation::from));
        if let Some(mutation) = mutation
            && mutation.button() == id
        {
            return mutation.label(idle);
        }
        match self.completed {
            Some(Mutation::Reload) if id == "extension-reload" => return tr_key!("Reloaded"),
            Some(Mutation::ResetPermissions) if id == "extension-reset-permissions" => {
                return tr_key!("Cleared");
            }
            _ => {}
        }
        match &self.loading {
            Some(Loading::Details(_)) if id == "extension-check-update" => tr_key!("Checking…"),
            Some(Loading::More) if id == "extension-more" => tr_key!("Loading…"),
            _ => idle,
        }
    }

    /// A button whose `label` is an English key, translated when shown.
    fn button(
        &self,
        id: impl Into<SharedString>,
        label: &'static str,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        self.styled_button(id, label, false, action, cx)
    }

    /// A [`Self::button`], filled with the accent when it is the page's main
    /// action.
    fn styled_button(
        &self,
        id: impl Into<SharedString>,
        label: &'static str,
        primary: bool,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let id = id.into();
        let text = translate(self.button_label(&id, label, cx));
        let kind = if primary {
            controls::Button::primary(&text)
        } else {
            controls::Button::secondary(&text)
        };
        controls::button(
            Style {
                theme: &self.theme,
                metrics: &self.metrics,
            },
            &id,
            kind.enabled(!self.busy(cx)),
            cx.listener(move |view, _, window, cx| {
                if !view.busy(cx) {
                    view.completed = None;
                    view.install_status = None;
                    action(view, window, cx);
                    cx.notify();
                }
            }),
        )
    }
}

impl Render for ExtensionsView {
    #[allow(clippy::too_many_lines, reason = "The page's one declarative layout")]
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(model) = self.model.upgrade() else {
            return div().into_any_element();
        };
        let selected = self.selected.as_ref().and_then(|details| {
            let name = details.name.as_str();
            let runtime = &model.read(cx).extensions;
            let extension = runtime.registry.extensions.get(name)?.clone();
            let stored = runtime.settings.get(name).cloned().unwrap_or_default();
            Some((extension, stored))
        });
        if let Some((extension, stored)) = selected {
            self.sync_setting_inputs(&extension, &stored, cx);
        }
        let registry = &model.read(cx).extensions.registry;
        let installed: Vec<_> = registry
            .extensions
            .values()
            .cloned()
            .map(|extension| {
                let enabled = registry.enabled(&extension.name).is_some();
                let unpacked = registry.is_unpacked(&extension.name);
                (extension, enabled, unpacked)
            })
            .collect();
        let errors = registry.errors.clone();
        let status = self.status_note(cx);
        let capabilities = &model.read(cx).capabilities;
        let mut unavailable: Vec<_> = [
            muxy_core::capability::Feature::ExtensionScripts,
            muxy_core::capability::Feature::ExtensionWebviews,
        ]
        .into_iter()
        .filter_map(|feature| capabilities.reason(feature))
        .collect();
        if !unavailable.is_empty() {
            unavailable.push(muxy_core::tr!(
                "Language packs and other resource-only extensions keep working."
            ));
        }

        let mut children: Vec<AnyElement> = Vec::new();
        if let Some(error) = &self.error {
            children.push(
                accessibility::announced(
                    form::note(self.style(), error, true).id("extension-error"),
                    error.clone(),
                    true,
                )
                .into_any_element(),
            );
        }
        if let Some(status) = &status {
            children.push(
                accessibility::announced(
                    form::note(self.style(), status, false).id("extension-status"),
                    status.clone(),
                    false,
                )
                .into_any_element(),
            );
        }
        children.extend(errors.iter().enumerate().map(|(index, error)| {
            accessibility::announced(
                form::note(self.style(), error, true).id(("extension-registry-error", index)),
                error.clone(),
                true,
            )
            .into_any_element()
        }));
        children.extend(
            unavailable
                .iter()
                .map(|reason| form::note(self.style(), reason, false).into_any_element()),
        );
        let mut sections = Vec::new();
        if let Some(details) = &self.selected {
            for (section, child) in self.detail_children(details, &installed, cx) {
                if let Some(section) = section {
                    sections.push((section, children.len()));
                }
                children.push(child);
            }
        } else if self.tab == Tab::Installed {
            children.push(self.installed_body(&installed, cx).into_any_element());
        } else {
            children.push(self.marketplace_body(cx).into_any_element());
        }
        self.sections = sections;

        let header = if let Some(details) = &self.selected {
            self.detail_bar(details, &installed, cx)
        } else {
            self.header(&installed, cx).child(self.toolbar(cx))
        };
        let width = self.width.clone();
        let view = cx.entity_id();
        let (scroll, scroll_id) = if self.selected.is_some() {
            (&self.detail_scroll, "extension-detail-scroll")
        } else {
            (&self.list_scroll, "extension-list-scroll")
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .text_size(self.metrics.font_body())
            .child(
                header
                    .flex_none()
                    .py(self.metrics.spacing7())
                    .border_b_1()
                    .border_color(self.theme.border),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                if (width.get() - bounds.size.width).abs() > px(0.5) {
                                    width.set(bounds.size.width);
                                    cx.notify(view);
                                }
                            },
                            |_, (), _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        div()
                            .id(scroll_id)
                            .track_scroll(scroll)
                            .on_scroll_wheel(cx.listener(|view, _, _, cx| {
                                if view.selected.is_some() {
                                    view.jumped = None;
                                    cx.notify();
                                }
                            }))
                            .size_full()
                            .overflow_y_scroll()
                            .flex()
                            .flex_col()
                            .gap(self.metrics.spacing7())
                            .py(self.metrics.spacing7())
                            .children(children),
                    ),
            )
            .into_any_element()
    }
}

impl ExtensionsView {
    fn style(&self) -> Style<'_> {
        Style {
            theme: &self.theme,
            metrics: &self.metrics,
        }
    }

    /// Opens Browse limited to language packs, with an empty search.
    pub(crate) fn browse_languages(&mut self, cx: &mut Context<Self>) {
        if self.mutating(cx) {
            return;
        }
        self.category = Some("localization");
        self.query.clear();
        self.search.update(cx, |search, cx| search.set_text("", cx));
        self.switch_tab(Tab::Marketplace, cx);
    }

    fn switch_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if self.mutating(cx) {
            return;
        }
        self.tab = tab;
        self.selected = None;
        self.error = None;
        self.completed = None;
        self.revision += 1;
        self.install_status = None;
        self.loading = None;
        if tab == Tab::Marketplace {
            self.fetch(true, cx);
        }
        cx.notify();
    }

    /// The page title with the Installed and Browse tabs and their actions.
    fn header(&self, installed: &[(Extension, bool, bool)], cx: &Context<Self>) -> gpui::Div {
        div().flex().flex_col().gap(self.metrics.spacing6()).child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(self.metrics.spacing6())
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(self.metrics.spacing6())
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(self.metrics.spacing4())
                                .child(SymbolGlyph::new(
                                    "puzzlepiece.extension",
                                    self.metrics.icon_md(),
                                    self.theme.fg_muted,
                                ))
                                .child(
                                    div()
                                        .text_size(self.metrics.font_title())
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(accessibility::label(
                                            "extensions-heading",
                                            tr!("Extensions"),
                                        )),
                                ),
                        )
                        .child(self.tab_control(installed.len(), cx)),
                )
                .when(self.tab == Tab::Installed, |header| {
                    header.child(self.header_actions(installed, cx))
                }),
        )
    }

    fn tab_control(&self, installed: usize, cx: &Context<Self>) -> AnyElement {
        let enabled = !self.mutating(cx);
        let choices = [
            Choice::new(
                "installed",
                if installed == 0 {
                    tr!("Installed")
                } else {
                    tr!("Installed (%@)", installed.to_string())
                },
            ),
            Choice::new("marketplace", tr!("Browse")),
        ]
        .map(|mut choice| {
            choice.enabled = enabled;
            choice
        });
        controls::segmented(
            self.style(),
            "extensions",
            &tr!("Extensions"),
            &choices,
            if self.tab == Tab::Installed {
                "installed"
            } else {
                "marketplace"
            },
            cx.listener(|view, value: &SharedString, _, cx| {
                view.switch_tab(
                    if value.as_ref() == "installed" {
                        Tab::Installed
                    } else {
                        Tab::Marketplace
                    },
                    cx,
                );
            }),
        )
    }

    fn header_actions(
        &self,
        installed: &[(Extension, bool, bool)],
        cx: &Context<Self>,
    ) -> gpui::Div {
        let updates = self
            .model
            .upgrade()
            .map_or(0, |model| model.read(cx).extension_update_count());
        let packaged = installed.iter().any(|(_, _, unpacked)| !unpacked);
        div()
            .flex()
            .flex_wrap()
            .gap(self.metrics.spacing4())
            .child(self.button(
                "extension-load",
                tr_key!("Load Unpacked…"),
                |view, _, cx| view.load_unpacked(cx),
                cx,
            ))
            .child(self.button(
                "extension-reload",
                tr_key!("Reload"),
                |view, _, cx| view.reload(cx),
                cx,
            ))
            .when(packaged, |actions| {
                let button = self.styled_button(
                    "extension-update-all",
                    tr_key!("Update all"),
                    updates > 0,
                    |view, _, cx| view.update_all(cx),
                    cx,
                );
                actions.child(if updates > 0 {
                    button
                        .aria_description(if updates == 1 {
                            tr!("1 update available")
                        } else {
                            tr!("%@ updates available", updates.to_string())
                        })
                        .child(
                            div()
                                .px(self.metrics.spacing2())
                                .rounded_full()
                                .bg(self.theme.accent_foreground)
                                .text_color(self.theme.accent)
                                .text_size(self.metrics.font_caption())
                                .child(updates.to_string()),
                        )
                } else {
                    button
                })
            })
    }

    fn card(&self) -> gpui::Div {
        div()
            .flex_none()
            .min_w_0()
            .w_full()
            .rounded(self.metrics.radius_lg())
            .bg(self.theme.surface)
            .border_1()
            .border_color(self.theme.border)
    }

    fn badge(&self, text: impl Into<SharedString>, active: bool) -> gpui::Div {
        div()
            .flex_none()
            .px(self.metrics.spacing3())
            .py(self.metrics.spacing1())
            .rounded(self.metrics.radius_sm())
            .text_size(self.metrics.font_footnote())
            .text_color(if active {
                self.theme.accent
            } else {
                self.theme.fg_muted
            })
            .bg(if active {
                self.theme.accent_soft
            } else {
                self.theme.hover
            })
            .child(text.into())
    }

    fn section(&self, title: &str, content: impl IntoElement) -> gpui::Div {
        self.section_with_actions(title, div(), content)
    }

    /// A titled card with buttons beside its title.
    fn section_with_actions(
        &self,
        title: &str,
        actions: impl IntoElement,
        content: impl IntoElement,
    ) -> gpui::Div {
        self.card()
            .p(self.metrics.spacing7())
            .flex()
            .flex_col()
            .gap(self.metrics.spacing6())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(self.metrics.spacing6())
                    .child(
                        div()
                            .text_size(self.metrics.font_emphasis())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(accessibility::label(
                                SharedString::from(format!("extension-section-{title}")),
                                title.to_owned(),
                            )),
                    )
                    .child(actions),
            )
            .child(content)
    }

    /// Remembered consent answers, each removable, as on main, and whether
    /// the extension is always allowed.
    fn rules_section(&self, owner: &str, model: &AppModel, cx: &Context<Self>) -> gpui::Div {
        let rules = model.extension_rules(owner);
        let trusted = model.extension_trusted(owner);
        let audit_log = model.extension_audit_log();
        let clear = owner.to_owned();
        let trust = owner.to_owned();
        let actions = div()
            .flex()
            .flex_wrap()
            .gap(self.metrics.spacing4())
            .child(self.button(
                "extension-audit-log",
                tr_key!("Reveal audit log"),
                move |view, _, cx| view.reveal(audit_log.clone(), cx),
                cx,
            ))
            .when(!rules.is_empty(), |actions| {
                actions.child(self.button(
                    "extension-reset-permissions",
                    tr_key!("Clear all"),
                    move |view, _, cx| view.reset_permissions(&clear, cx),
                    cx,
                ))
            })
            .when(!trusted, |actions| {
                actions.child(
                    self.button(
                        "extension-trust",
                        tr_key!("Always Allow…"),
                        move |view, window, cx| view.trust(&trust, window, cx),
                        cx,
                    )
                    .text_color(self.theme.danger),
                )
            });
        let content = if rules.is_empty() {
            div().text_color(self.theme.fg_muted).child(accessibility::label(
                "extension-no-rules",
                tr!("No saved rules. When you choose to remember an answer to a permission request, it appears here."),
            ))
        } else {
            rules.iter().enumerate().fold(
                div().flex().flex_col().gap(self.metrics.spacing3()),
                |list, (index, rule)| list.child(self.rule_row(owner, index, rule, cx)),
            )
        };
        let content = div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing6())
            .when(trusted, |column| column.child(self.trusted_row(owner, cx)))
            .child(content);
        self.section_with_actions(&tr!("Permission rules"), actions, content)
    }

    /// Says the extension skips every permission prompt, with a way to stop.
    fn trusted_row(&self, owner: &str, cx: &Context<Self>) -> gpui::Div {
        let owner = owner.to_owned();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(self.metrics.spacing6())
            .px(self.metrics.spacing5())
            .py(self.metrics.spacing3())
            .rounded(self.metrics.radius_sm())
            .border_1()
            .border_color(self.theme.danger)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(self.theme.danger)
                    .child(accessibility::label(
                        "extension-trusted",
                        tr!("Always allowed: every permission request from this extension is allowed without asking, and the rules below don't apply."),
                    )),
            )
            .child(self.button(
                "extension-untrust",
                tr_key!("Stop Always Allowing"),
                move |view, _, cx| view.set_trusted(&owner, false, cx),
                cx,
            ))
    }

    fn rule_row(&self, owner: &str, index: usize, rule: &Rule, cx: &Context<Self>) -> gpui::Div {
        let (decision, color) = match rule.consent {
            Consent::Allow => (tr!("allow"), self.theme.diff_add),
            Consent::Blocked => (tr!("blocked"), self.theme.danger),
            Consent::Deny | Consent::Ask => (tr!("deny"), self.theme.danger),
        };
        let scope = if rule.consent == Consent::Blocked {
            tr!("blocks all %@", rule.gate.kind())
        } else {
            rule.scope().into()
        };
        let (owner, removed) = (owner.to_owned(), rule.clone());
        let name = tr!("Remove %@ %@ %@", &decision, rule.gate.name(), &scope);
        div()
            .flex()
            .items_center()
            .gap(self.metrics.spacing6())
            .px(self.metrics.spacing5())
            .py(self.metrics.spacing3())
            .rounded(self.metrics.radius_sm())
            .bg(self.theme.hover)
            .child(
                div()
                    .flex_none()
                    .w(self.layout_scale().scaled(64.0))
                    .text_size(self.metrics.font_footnote())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color)
                    .child(decision),
            )
            .child(
                div()
                    .flex_none()
                    .w(self.layout_scale().scaled(130.0))
                    .truncate()
                    .child(rule.gate.name()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(".SystemUIFontMonospaced")
                    .text_color(self.theme.fg_muted)
                    .child(scope),
            )
            .child(
                self.button(
                    format!("extension-rule-remove-{index}"),
                    tr_key!("Remove"),
                    move |view, _, cx| view.remove_rule(&owner, removed.clone(), cx),
                    cx,
                )
                .aria_label(name),
            )
    }

    fn empty_state(&self, title: &str, description: &str) -> gpui::Div {
        self.card()
            .flex()
            .flex_col()
            .items_center()
            .gap(self.metrics.spacing6())
            .p(self.metrics.spacing9())
            .child(SymbolGlyph::new(
                "puzzlepiece.extension",
                self.metrics.icon_xxl(),
                self.theme.fg_muted,
            ))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(accessibility::label(
                        "extension-empty-title",
                        title.to_owned(),
                    )),
            )
            .child(
                div()
                    .text_color(self.theme.fg_muted)
                    .text_center()
                    .child(accessibility::label(
                        "extension-empty-description",
                        description.to_owned(),
                    )),
            )
    }

    /// Edits an unpacked extension, or clones one from the marketplace to edit
    /// the copy.
    fn agent_action(
        &self,
        extension: &Extension,
        unpacked: bool,
        cx: &Context<Self>,
    ) -> Option<(&'static str, AgentTask)> {
        let name = extension.name.clone();
        if unpacked {
            let folder = self
                .model
                .upgrade()?
                .read(cx)
                .extension_source(&name)
                .ok()?;
            return Some((
                tr_key!("Edit with Agent…"),
                AgentTask::Edit { name, folder },
            ));
        }
        Some((
            tr_key!("Clone and Modify…"),
            AgentTask::Clone {
                name,
                installed: extension.directory.clone(),
            },
        ))
    }

    fn agent_button(
        &self,
        id: &str,
        extension: &Extension,
        unpacked: bool,
        cx: &Context<Self>,
    ) -> Option<gpui::Stateful<gpui::Div>> {
        let (label, task) = self.agent_action(extension, unpacked, cx)?;
        Some(self.button(
            format!("{id}-{}", extension.name),
            label,
            move |view, _, cx| view.agent_task(task.clone(), cx),
            cx,
        ))
    }
}

/// Why no permission tags are shown. Unknown permissions never read as none.
fn permissions_note(permissions: Option<&BTreeSet<String>>) -> Option<SharedString> {
    match permissions {
        None => Some(tr!(
            "The marketplace did not list this extension's permissions."
        )),
        Some(permissions) if permissions.is_empty() => Some(tr!("No permissions requested.")),
        Some(_) => None,
    }
}
