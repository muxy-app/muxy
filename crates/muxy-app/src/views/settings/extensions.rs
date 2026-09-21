use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window,
    div,
};
use muxy_ui::{
    components::{ButtonInteraction, SymbolGlyph},
    controls::{self, Choice, Style},
    form,
    text_input::{InputEvent, InputStyle, TextInput},
    theme::{Metrics, Theme},
};
use serde_json::{Value, json};

use crate::{extensions::marketplace, model::AppModel};
use muxy_app_core::extensions::Extension;

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
    Enable,
    Disable,
    Remove,
    LoadUnpacked,
    Reload,
    ResetPermissions,
}

impl Mutation {
    fn button(self) -> &'static str {
        match self {
            Self::Install => "extension-install",
            Self::Enable | Self::Disable => "extension-enable",
            Self::Remove => "extension-remove",
            Self::LoadUnpacked => "extension-load",
            Self::Reload => "extension-reload",
            Self::ResetPermissions => "extension-reset-permissions",
        }
    }

    fn label(self, idle: &str) -> &str {
        match self {
            Self::Install => "Installing…",
            Self::Enable => "Enabling…",
            Self::Disable => "Disabling…",
            Self::Remove if idle == "Unload folder" => "Unloading…",
            Self::Remove => "Uninstalling…",
            Self::LoadUnpacked => "Loading…",
            Self::Reload => "Reloading…",
            Self::ResetPermissions => "Resetting…",
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
    items: Vec<Value>,
    page: u32,
    has_next: bool,
    query: String,
    revision: u64,
    loading: Option<Loading>,
    mutation: Option<Mutation>,
    completed: Option<Mutation>,
    error: Option<String>,
    selected: Option<Value>,
    folder: Option<muxy_ui::dialog::FolderPicker>,
    _subscriptions: Vec<Subscription>,
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
                .with_placeholder("Search extensions…")
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
            subscriptions.push(cx.observe(&model, |_, _, cx| cx.notify()));
        }
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
            revision: 0,
            loading: None,
            mutation: None,
            completed: None,
            error: None,
            selected: None,
            folder: None,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn sync_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        self.theme = theme;
        self.search.update(cx, |input, cx| {
            input.set_style(InputStyle::field(&self.theme, &self.metrics), cx);
        });
        cx.notify();
    }

    fn fetch(&mut self, reset: bool, cx: &mut Context<Self>) {
        if self.mutation.is_some() {
            return;
        }
        self.revision += 1;
        let revision = self.revision;
        if reset {
            self.page = 1;
            self.items.clear();
            self.has_next = false;
        }
        let page = if reset { 1 } else { self.page + 1 };
        let query = self.query.clone();
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
            let result = crate::extensions::io::run(move || marketplace::list(&query, page)).await;
            let _ = view.update(cx, |view, cx| {
                if view.revision != revision {
                    return;
                }
                view.loading = None;
                match result {
                    Ok(result) => {
                        view.has_next = result["links"]["next"].is_string();
                        if let Some(items) = result["data"].as_array() {
                            view.page = page;
                            view.items.extend(items.clone());
                        } else {
                            view.error = Some("Invalid marketplace response".into());
                        }
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
        if self.loading.is_some() || self.mutation.is_some() {
            return;
        }
        self.revision += 1;
        let revision = self.revision;
        self.loading = Some(Loading::Details(name.clone()));
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = crate::extensions::io::run(move || marketplace::detail(&name)).await;
            let _ = view.update(cx, |view, cx| {
                if view.revision != revision {
                    return;
                }
                view.loading = None;
                match result {
                    Ok(details) => view.selected = Some(details),
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn load_unpacked(&mut self, cx: &mut Context<Self>) {
        if self.folder.is_some() || self.mutation.is_some() {
            return;
        }
        let (sender, receiver) = async_channel::bounded(1);
        match muxy_ui::dialog::choose_folder(
            "Choose an extension folder containing package.json or a built dist folder",
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
            let path = receiver.recv().await.ok().flatten();
            let task = view.update(cx, |view, cx| {
                view.folder = None;
                path.map(|path| {
                    view.model
                        .update(cx, |model, cx| model.load_unpacked_extension(path, cx))
                })
            });
            let result = match task {
                Ok(Some(Ok(task))) => task.await,
                Ok(None) => Ok(()),
                Ok(Some(Err(error))) | Err(error) => Err(error.to_string()),
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
        if self.loading.is_some() || self.mutation.is_some() {
            return;
        }
        let Some(details) = self.selected.clone() else {
            return;
        };
        let Some(name) = details["name"].as_str().map(str::to_owned) else {
            return;
        };
        let Some(model) = self.model.upgrade() else {
            return;
        };
        let directory = model.read(cx).extensions.registry.directory();
        self.mutation = Some(Mutation::Install);
        self.error = None;
        cx.spawn(async move |view, cx| {
            let installed_name = name.clone();
            let result = crate::extensions::io::run(move || {
                let stage = marketplace::download(&details, &directory)?;
                marketplace::install(&stage, &directory, &name)
            })
            .await;
            let result = match result {
                Ok(()) => match model.update(cx, AppModel::refresh_installed_extensions_task) {
                    Ok(task) => task.await,
                    Err(error) => Err(error.to_string()),
                },
                Err(error) => Err(error),
            };
            let _ = view.update(cx, |view, cx| {
                view.finish_install(&installed_name, result, cx);
            });
        })
        .detach();
    }

    fn finish_install(&mut self, name: &str, result: Result<(), String>, cx: &mut Context<Self>) {
        self.mutation = None;
        match result {
            Ok(()) => {
                let details = self.model.upgrade().and_then(|model| {
                    model
                        .read(cx)
                        .extensions
                        .registry
                        .extensions
                        .get(name)
                        .map(installed_details)
                });
                if let Some(details) = details {
                    self.tab = Tab::Installed;
                    self.query.clear();
                    self.search.update(cx, |search, cx| search.set_text("", cx));
                    self.selected = Some(details);
                    self.error = None;
                } else {
                    self.error = Some("The installed extension could not be loaded. Reload extensions to try again.".into());
                }
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }

    fn enable(&mut self, name: &str, enabled: bool, cx: &mut Context<Self>) {
        self.change_extension(name, Some(enabled), cx);
    }

    fn remove(&mut self, name: &str, cx: &mut Context<Self>) {
        self.change_extension(name, None, cx);
    }

    fn change_extension(&mut self, name: &str, enabled: Option<bool>, cx: &mut Context<Self>) {
        if self.loading.is_some() || self.mutation.is_some() {
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
                Err("The extension kept an open view. Save or close it before continuing.".into())
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
        if self.mutation.is_some() {
            return;
        }
        let task = self
            .model
            .update(cx, |model, cx| model.reset_extension_permissions(owner, cx));
        self.mutation = Some(Mutation::ResetPermissions);
        self.completed = None;
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = match task {
                Ok(task) => task.await,
                Err(error) => Err(error.to_string()),
            };
            let _ = view.update(cx, |view, cx| {
                view.mutation = None;
                view.completed = result.is_ok().then_some(Mutation::ResetPermissions);
                view.error = result.err();
                cx.notify();
            });
        })
        .detach();
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        if self.mutation.is_some() {
            return;
        }
        let task = self.model.update(cx, AppModel::reload_extensions);
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

    fn button_label<'a>(&self, id: &str, idle: &'a str) -> &'a str {
        if let Some(mutation) = self.mutation
            && mutation.button() == id
        {
            return mutation.label(idle);
        }
        match self.completed {
            Some(Mutation::Reload) if id == "extension-reload" => return "Reloaded",
            Some(Mutation::ResetPermissions) if id == "extension-reset-permissions" => {
                return "Permissions reset";
            }
            _ => {}
        }
        match &self.loading {
            Some(Loading::More) if id == "extension-more" => "Loading…",
            Some(Loading::Details(name)) if id.strip_prefix("details-") == Some(name) => "Loading…",
            _ => idle,
        }
    }

    fn button(
        &self,
        id: impl Into<SharedString>,
        label: &str,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let id = id.into();
        controls::button(
            Style {
                theme: &self.theme,
                metrics: &self.metrics,
            },
            &id,
            self.button_label(&id, label),
            self.mutation.is_none() && self.loading.is_none(),
            cx.listener(move |view, _, window, cx| {
                if view.mutation.is_none() && view.loading.is_none() {
                    view.completed = None;
                    action(view, window, cx);
                    cx.notify();
                }
            }),
        )
    }
}

impl Render for ExtensionsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(model) = self.model.upgrade() else {
            return div().into_any_element();
        };
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
        let body = if let Some(details) = &self.selected {
            self.details_body(details, &installed, cx)
        } else if self.tab == Tab::Installed {
            self.installed_body(&installed, cx)
        } else {
            self.marketplace_body(cx)
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_w_0()
            .min_h_0()
            .text_size(self.metrics.font_body())
            .child(self.header(cx))
            .child(
                div()
                    .id(if self.selected.is_some() {
                        "extension-detail-scroll"
                    } else {
                        "extension-list-scroll"
                    })
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(self.metrics.spacing7())
                            .py(self.metrics.spacing7())
                            .when(self.selected.is_none(), |body| {
                                body.child(controls::search_field(
                                    self.style(),
                                    "extension-search",
                                    &self.search,
                                ))
                            })
                            .when_some(self.error.as_ref(), |body, error| {
                                body.child(form::note(self.style(), error, true))
                            })
                            .children(
                                errors
                                    .iter()
                                    .map(|error| form::note(self.style(), error, true)),
                            )
                            .child(body),
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

    fn switch_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if self.mutation.is_some() {
            return;
        }
        self.tab = tab;
        self.selected = None;
        self.error = None;
        self.completed = None;
        self.revision += 1;
        self.loading = None;
        if tab == Tab::Marketplace {
            self.fetch(true, cx);
        }
        cx.notify();
    }

    fn header(&self, cx: &Context<Self>) -> gpui::Div {
        let header = div()
            .flex()
            .flex_none()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(self.metrics.spacing6())
            .py(self.metrics.spacing7())
            .border_b_1()
            .border_color(self.theme.border);
        if self.selected.is_some() {
            return header.child(
                self.button(
                    "extension-back",
                    "Extensions",
                    |view, _, cx| {
                        view.selected = None;
                        view.error = None;
                        cx.notify();
                    },
                    cx,
                )
                .flex_row_reverse()
                .gap(self.metrics.spacing3())
                .child(SymbolGlyph::new(
                    "chevron.left",
                    self.metrics.icon_sm(),
                    self.theme.fg_muted,
                )),
            );
        }
        header
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
                                    .child("Extensions"),
                            ),
                    )
                    .child(self.tab_control(cx)),
            )
            .when(self.tab == Tab::Installed, |header| {
                header.child(self.header_actions(cx))
            })
    }

    fn tab_control(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let choices = [
            Choice::new("installed", "Installed"),
            Choice::new("marketplace", "Browse"),
        ]
        .map(|mut choice| {
            choice.enabled = self.mutation.is_none();
            choice
        });
        controls::segmented(
            self.style(),
            "extensions",
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

    fn header_actions(&self, cx: &Context<Self>) -> gpui::Div {
        div()
            .flex()
            .flex_wrap()
            .gap(self.metrics.spacing4())
            .child(self.button(
                "extension-load",
                "Load Unpacked…",
                |view, _, cx| view.load_unpacked(cx),
                cx,
            ))
            .child(self.button(
                "extension-reload",
                "Reload",
                |view, _, cx| view.reload(cx),
                cx,
            ))
    }

    fn card(&self) -> gpui::Div {
        div()
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
        self.card()
            .p(self.metrics.spacing7())
            .flex()
            .flex_col()
            .gap(self.metrics.spacing6())
            .child(
                div()
                    .text_size(self.metrics.font_emphasis())
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.to_owned()),
            )
            .child(content)
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
                    .child(title.to_owned()),
            )
            .child(
                div()
                    .text_color(self.theme.fg_muted)
                    .text_center()
                    .child(description.to_owned()),
            )
    }

    fn details_body(
        &self,
        details: &Value,
        installed: &[(Extension, bool, bool)],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let name = details["name"].as_str().unwrap_or("").to_owned();
        let local = installed
            .iter()
            .find(|(extension, _, _)| extension.name == name);
        let details = local.map_or_else(
            || details.clone(),
            |(extension, _, _)| installed_details(extension),
        );
        let hero = self.detail_header(&name, &details, local, cx);
        let mut body = div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing7())
            .min_w_0()
            .child(hero)
            .child(self.section("Requested permissions", self.permissions_body(&details)));
        if let Some((_, _, unpacked)) = local {
            let reset = name.clone();
            body = body.child(
                self.section(
                    "Manage extension",
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(self.metrics.spacing4())
                        .child(self.button(
                            "extension-reset-permissions",
                            "Reset remembered permissions",
                            move |view, _, cx| view.reset_permissions(&reset, cx),
                            cx,
                        ))
                        .child(
                            self.button(
                                "extension-remove",
                                if *unpacked {
                                    "Unload folder"
                                } else {
                                    "Uninstall"
                                },
                                move |view, _, cx| view.remove(&name, cx),
                                cx,
                            )
                            .text_color(self.theme.danger),
                        ),
                ),
            );
        }
        body
    }

    fn detail_header(
        &self,
        name: &str,
        details: &Value,
        local: Option<&(Extension, bool, bool)>,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let mut heading = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(self.metrics.spacing4())
            .child(
                div()
                    .text_size(self.metrics.font_display())
                    .font_weight(FontWeight::SEMIBOLD)
                    .min_w_0()
                    .max_w_full()
                    .truncate()
                    .child(name.to_owned()),
            )
            .when_some(details["version"].as_str(), |heading, version| {
                heading.child(
                    div()
                        .text_color(self.theme.fg_muted)
                        .child(format!("v{version}")),
                )
            });
        if let Some((_, enabled, unpacked)) = local {
            heading =
                heading.child(self.badge(if *enabled { "Enabled" } else { "Disabled" }, *enabled));
            if *unpacked {
                heading = heading.child(self.badge("Unpacked", false));
            }
        }
        let action = if let Some((_, enabled, _)) = local {
            let enabled = *enabled;
            let owner = name.to_owned();
            self.button(
                "extension-enable",
                if enabled { "Disable" } else { "Enable" },
                move |view, _, cx| view.enable(&owner, !enabled, cx),
                cx,
            )
        } else {
            self.button(
                "extension-install",
                "Install",
                |view, _, cx| view.install(cx),
                cx,
            )
        };
        self.card()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing6())
            .p(self.metrics.spacing7())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(self.metrics.spacing6())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .min_w_0()
                            .gap(self.metrics.spacing6())
                            .child(SymbolGlyph::new(
                                "puzzlepiece.extension.fill",
                                self.metrics.icon_xl(),
                                self.theme.fg_muted,
                            ))
                            .child(heading),
                    )
                    .child(action),
            )
            .child(
                div()
                    .text_color(self.theme.fg_muted)
                    .child(details["description"].as_str().unwrap_or("").to_owned()),
            )
            .when(local.is_some_and(|(_, enabled, _)| !enabled), |hero| {
                hero.child(
                    div().text_color(self.theme.fg_muted).child(
                        "Review the permissions below, then enable this extension to use it.",
                    ),
                )
            })
    }

    fn permissions_body(&self, details: &Value) -> gpui::Div {
        let permissions: Vec<_> = details["permissions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let mut permission_tags = div().flex().flex_wrap().gap(self.metrics.spacing4());
        for permission in &permissions {
            permission_tags = permission_tags.child(self.badge((*permission).to_owned(), false));
        }
        div()
            .flex()
            .flex_col()
            .gap(self.metrics.spacing6())
            .child(permission_tags)
            .when(permissions.is_empty(), |body| {
                body.child(
                    div()
                        .text_color(self.theme.fg_muted)
                        .child("No permissions requested."),
                )
            })
            .child(
                div()
                    .text_size(self.metrics.font_footnote())
                    .text_color(self.theme.fg_muted)
                    .child("Commands and file or Git changes ask for permission when used."),
            )
    }

    fn installed_body(
        &self,
        installed: &[(Extension, bool, bool)],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let query = self.query.to_lowercase();
        let filtered: Vec<_> = installed
            .iter()
            .filter(|(extension, _, _)| {
                format!("{} {}", extension.name, extension.manifest.description)
                    .to_lowercase()
                    .contains(&query)
            })
            .collect();
        if filtered.is_empty() {
            return if installed.is_empty() {
                self.empty_state(
                    "No extensions installed",
                    "Browse extensions to add tools to Muxy, or load an unpacked extension folder.",
                )
                .child(self.button(
                    "extension-browse",
                    "Browse extensions",
                    |view, _, cx| view.switch_tab(Tab::Marketplace, cx),
                    cx,
                ))
            } else {
                self.empty_state(
                    "No matching extensions",
                    "Try a different name or clear your search.",
                )
            };
        }
        let mut rows = self.card().overflow_hidden();
        for (index, (extension, enabled, unpacked)) in filtered.into_iter().enumerate() {
            rows = rows.child(self.installed_row(extension, *enabled, *unpacked, index > 0, cx));
        }
        rows
    }

    fn installed_row(
        &self,
        extension: &Extension,
        enabled: bool,
        unpacked: bool,
        separator: bool,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let details = installed_details(extension);
        let mut title = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(self.metrics.spacing4())
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .min_w_0()
                    .max_w_full()
                    .truncate()
                    .child(extension.name.clone()),
            )
            .child(
                div()
                    .text_size(self.metrics.font_footnote())
                    .text_color(self.theme.fg_muted)
                    .child(format!("v{}", extension.version)),
            )
            .child(self.badge(if enabled { "Enabled" } else { "Disabled" }, enabled));
        if unpacked {
            title = title.child(self.badge("Unpacked", false));
        }

        div()
            .id(SharedString::from(format!(
                "extension-row-{}",
                extension.name
            )))
            .flex()
            .items_center()
            .gap(self.metrics.spacing6())
            .p(self.metrics.spacing7())
            .when(separator, |row| {
                row.border_t_1().border_color(self.theme.border)
            })
            .cursor_pointer()
            .hover(|row| row.bg(self.theme.hover))
            .focus(|row| row.bg(self.theme.accent_soft))
            .button_interaction(cx.listener(move |view, _, _, cx| {
                if view.mutation.is_none() {
                    view.selected = Some(details.clone());
                    view.error = None;
                    view.completed = None;
                    cx.notify();
                }
            }))
            .child(SymbolGlyph::new(
                "puzzlepiece.extension.fill",
                self.metrics.icon_lg(),
                self.theme.fg_muted,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(self.metrics.spacing2())
                    .child(title)
                    .child(
                        div()
                            .text_size(self.metrics.font_footnote())
                            .text_color(self.theme.fg_muted)
                            .line_clamp(2)
                            .child(extension.manifest.description.clone()),
                    ),
            )
            .child(SymbolGlyph::new(
                "chevron.right",
                self.metrics.icon_sm(),
                self.theme.fg_muted,
            ))
    }

    fn marketplace_body(&self, cx: &mut Context<Self>) -> gpui::Div {
        let mut body = div().flex().flex_col().gap(self.metrics.spacing6());
        if self.loading == Some(Loading::Search) {
            body = body.child(
                div()
                    .text_color(self.theme.fg_muted)
                    .child("Loading extensions…"),
            );
        }
        for item in &self.items {
            let name = item["name"].as_str().unwrap_or("").to_owned();
            let selected = name.clone();
            body = body.child(
                self.card()
                    .p(self.metrics.spacing7())
                    .flex()
                    .items_center()
                    .gap(self.metrics.spacing6())
                    .child(SymbolGlyph::new(
                        "puzzlepiece.extension",
                        self.metrics.icon_xl(),
                        self.theme.fg_muted,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(self.metrics.spacing2())
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                            .child(
                                div()
                                    .text_size(self.metrics.font_footnote())
                                    .text_color(self.theme.fg_muted)
                                    .line_clamp(2)
                                    .child(item["description"].as_str().unwrap_or("").to_owned()),
                            ),
                    )
                    .child(self.button(
                        format!("details-{selected}"),
                        "View",
                        move |view, _, cx| view.details(selected.clone(), cx),
                        cx,
                    )),
            );
        }
        if self.loading.is_none() && self.items.is_empty() && self.error.is_none() {
            body = body.child(self.empty_state(
                "No extensions found",
                "Try a different search to find extensions.",
            ));
        }
        if self.has_next {
            body = body.child(div().flex().justify_center().child(self.button(
                "extension-more",
                "Load more",
                |view, _, cx| {
                    view.fetch(false, cx);
                },
                cx,
            )));
        }
        body
    }
}

fn installed_details(extension: &Extension) -> Value {
    json!({
        "name": extension.name,
        "version": extension.version,
        "description": extension.manifest.description,
        "permissions": extension.manifest.permissions,
    })
}
