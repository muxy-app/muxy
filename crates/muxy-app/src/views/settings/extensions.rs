use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window,
    div, px,
};
use muxy_ui::{
    controls::{self, Style},
    text_input::{InputEvent, InputStyle, TextInput},
    theme::{Metrics, Theme},
};
use serde_json::{Value, json};

use crate::{extensions::marketplace, model::AppModel};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Installed,
    Marketplace,
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
    busy: bool,
    mutating: bool,
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
            busy: false,
            mutating: false,
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
        if self.mutating {
            return;
        }
        self.revision += 1;
        let revision = self.revision;
        if reset {
            self.page = 1;
            self.items.clear();
        }
        let page = self.page;
        let query = self.query.clone();
        self.busy = true;
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
                view.busy = false;
                match result {
                    Ok(result) => {
                        view.has_next = result["links"]["next"].is_string();
                        if let Some(items) = result["data"].as_array() {
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
        self.revision += 1;
        let revision = self.revision;
        self.busy = true;
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = crate::extensions::io::run(move || marketplace::detail(&name)).await;
            let _ = view.update(cx, |view, cx| {
                if view.revision != revision {
                    return;
                }
                view.busy = false;
                match result {
                    Ok(details) => view.selected = Some(details),
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load_unpacked(&mut self, cx: &mut Context<Self>) {
        if self.folder.is_some() || self.mutating {
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
                self.mutating = true;
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
                view.mutating = false;
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
        if self.busy || self.mutating {
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
        self.mutating = true;
        self.error = None;
        cx.spawn(async move |view, cx| {
            let result = crate::extensions::io::run(move || {
                let stage = marketplace::download(&details, &directory)?;
                marketplace::install(&stage, &directory, &name)
            })
            .await;
            let _ = view.update(cx, |view, cx| {
                view.mutating = false;
                match result {
                    Ok(()) => {
                        let _ = view
                            .model
                            .update(cx, AppModel::refresh_installed_extensions);
                        view.tab = Tab::Installed;
                        view.selected = None;
                        view.query.clear();
                        view.search.update(cx, |search, cx| search.set_text("", cx));
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn enable(&mut self, name: &str, enabled: bool, cx: &mut Context<Self>) {
        self.change_extension(name, Some(enabled), cx);
    }

    fn remove(&mut self, name: &str, cx: &mut Context<Self>) {
        self.change_extension(name, None, cx);
    }

    fn change_extension(&mut self, name: &str, enabled: Option<bool>, cx: &mut Context<Self>) {
        if self.busy || self.mutating {
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
        self.mutating = true;
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
                view.mutating = false;
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
        if self.mutating {
            return;
        }
        let task = self
            .model
            .update(cx, |model, cx| model.reset_extension_permissions(owner, cx));
        self.mutating = true;
        cx.spawn(async move |view, cx| {
            let result = match task {
                Ok(task) => task.await,
                Err(error) => Err(error.to_string()),
            };
            let _ = view.update(cx, |view, cx| {
                view.mutating = false;
                view.error = result.err();
                cx.notify();
            });
        })
        .detach();
    }

    fn button(
        &self,
        id: impl Into<SharedString>,
        label: &str,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &Context<Self>,
    ) -> AnyElement {
        controls::button(
            Style {
                theme: &self.theme,
                metrics: &self.metrics,
            },
            &id.into(),
            label,
            !self.mutating,
            cx.listener(move |view, _, window, cx| {
                if !view.mutating {
                    action(view, window, cx);
                }
            }),
        )
        .into_any_element()
    }
}

impl Render for ExtensionsView {
    #[allow(
        clippy::too_many_lines,
        reason = "Settings toolbar and content share one render layout"
    )]
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
        let mut body = div().flex().flex_col().gap(px(16.0)).w_full();
        body = if let Some(details) = &self.selected {
            self.details_body(details, &installed, body, cx)
        } else if self.tab == Tab::Installed {
            self.installed_body(&installed, body, cx)
        } else {
            self.marketplace_body(body, cx)
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .gap(px(20.0))
            .py(px(24.0))
            .child(div().text_size(px(26.0)).child("Extensions"))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .gap(px(6.0))
                            .child(self.button(
                                "installed-tab",
                                if self.tab == Tab::Installed {
                                    "Installed ✓"
                                } else {
                                    "Installed"
                                },
                                |view, _, cx| {
                                    view.tab = Tab::Installed;
                                    view.selected = None;
                                    view.revision += 1;
                                    view.busy = false;
                                    cx.notify();
                                },
                                cx,
                            ))
                            .child(self.button(
                                "marketplace-tab",
                                if self.tab == Tab::Marketplace {
                                    "Marketplace ✓"
                                } else {
                                    "Marketplace"
                                },
                                |view, _, cx| {
                                    view.tab = Tab::Marketplace;
                                    view.selected = None;
                                    view.fetch(true, cx);
                                },
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(6.0))
                            .child(self.button(
                                "extension-reload",
                                "Reload",
                                |view, _, cx| {
                                    let _ = view.model.update(cx, AppModel::reload_extensions);
                                    cx.notify();
                                },
                                cx,
                            ))
                            .child(self.button(
                                "extension-load",
                                "Load Unpacked…",
                                |view, _, cx| view.load_unpacked(cx),
                                cx,
                            )),
                    ),
            )
            .when(self.selected.is_none(), |body| {
                body.child(controls::search_field(
                    Style {
                        theme: &self.theme,
                        metrics: &self.metrics,
                    },
                    "extension-search",
                    &self.search,
                ))
            })
            .when_some(self.error.clone(), |body, error| {
                body.child(div().text_color(self.theme.danger).child(error))
            })
            .children(
                errors
                    .into_iter()
                    .map(|error| div().text_color(self.theme.danger).child(error)),
            )
            .when(self.busy || self.mutating, |body| {
                body.child(div().text_color(self.theme.fg_muted).child("Loading…"))
            })
            .child(
                div()
                    .id("extension-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
            .into_any_element()
    }
}

impl ExtensionsView {
    fn details_body(
        &self,
        details: &Value,
        installed: &[(muxy_app_core::extensions::Extension, bool, bool)],
        mut body: gpui::Div,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let name = details["name"].as_str().unwrap_or("").to_owned();
        let local = installed
            .iter()
            .find(|(extension, _, _)| extension.name == name);
        body = body
            .child(self.button(
                "extension-back",
                "Back",
                |view, _, cx| {
                    view.selected = None;
                    cx.notify();
                },
                cx,
            ))
            .child(div().text_size(px(24.0)).child(name.clone()))
            .child(
                div()
                    .text_color(self.theme.fg_muted)
                    .child(details["description"].as_str().unwrap_or("").to_owned()),
            )
            .child(div().text_size(px(14.0)).child("Requested permissions"));
        for permission in details["permissions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            body = body.child(
                div()
                    .text_color(self.theme.fg_muted)
                    .child(permission.to_owned()),
            );
        }
        body=body.child(div().text_size(px(12.0)).text_color(self.theme.fg_muted).child("Extensions can access the capabilities listed above. Commands and file or Git changes ask for permission when used."));
        if let Some((_, enabled, unpacked)) = local {
            let enabled = *enabled;
            let remove = name.clone();
            let reset = name.clone();
            body = body
                .child(self.button(
                    "extension-enable",
                    if enabled { "Disable" } else { "Enable" },
                    move |view, _, cx| view.enable(&name, !enabled, cx),
                    cx,
                ))
                .child(self.button(
                    "extension-reset-permissions",
                    "Reset remembered permissions",
                    move |view, _, cx| {
                        view.reset_permissions(&reset, cx);
                    },
                    cx,
                ))
                .child(self.button(
                    "extension-remove",
                    if *unpacked {
                        "Unload folder"
                    } else {
                        "Uninstall"
                    },
                    move |view, _, cx| view.remove(&remove, cx),
                    cx,
                ));
        } else if !self.busy {
            body = body.child(self.button(
                "extension-install",
                "Install",
                |view, _, cx| view.install(cx),
                cx,
            ));
        }
        body
    }
    fn installed_body(
        &self,
        installed: &[(muxy_app_core::extensions::Extension, bool, bool)],
        mut body: gpui::Div,
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
            body = body.child(div().py(px(32.0)).child(if installed.is_empty() {
                "No extensions installed. Load a folder or browse the marketplace."
            } else {
                "No matching extensions."
            }));
        }
        for (extension, enabled, unpacked) in filtered {
            let details = json!({"name":extension.name,"description":extension.manifest.description,"permissions":extension.manifest.permissions});
            body = body.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .py(px(16.0))
                    .border_b_1()
                    .border_color(self.theme.border)
                    .child(
                        div()
                            .flex_1()
                            .child(
                                div().child(format!("{}  {}", extension.name, extension.version)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(self.theme.fg_muted)
                                    .child(extension.manifest.description.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(self.theme.fg_muted)
                                    .child(format!(
                                        "{}{}",
                                        if *enabled { "Enabled" } else { "Disabled" },
                                        if *unpacked { " · Unpacked" } else { "" }
                                    )),
                            ),
                    )
                    .child(self.button(
                        format!("manage-{}", extension.name),
                        "Manage",
                        move |view, _, cx| {
                            view.selected = Some(details.clone());
                            cx.notify();
                        },
                        cx,
                    )),
            );
        }
        body
    }
    fn marketplace_body(&self, mut body: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        let mut cards = div().flex().flex_wrap().gap(px(12.0));
        for item in &self.items {
            let name = item["name"].as_str().unwrap_or("").to_owned();
            let selected = name.clone();
            cards = cards.child(
                div()
                    .flex_1()
                    .min_w(px(220.0))
                    .p(px(18.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(self.theme.border)
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(name)
                            .child(self.button(
                                format!("details-{selected}"),
                                "View",
                                move |view, _, cx| view.details(selected.clone(), cx),
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .mt(px(8.0))
                            .text_color(self.theme.fg_muted)
                            .child(item["description"].as_str().unwrap_or("").to_owned()),
                    ),
            );
        }
        body = body.child(cards);
        if !self.busy && self.items.is_empty() {
            body = body.child(div().child("No marketplace extensions found."));
        }
        if self.has_next && !self.busy {
            body = body.child(self.button(
                "extension-more",
                "Load more",
                |view, _, cx| {
                    view.page += 1;
                    view.fetch(false, cx);
                },
                cx,
            ));
        }
        body
    }
}
