use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Subscription,
    WeakEntity, Window, div,
};

use muxy_ui::command_palette::{CommandPalette, CommandPaletteEvent};
use muxy_ui::picker::{Picker, PickerConfig, PickerEvent, PickerItem, PickerRow};

use super::languages::{LanguageEvent, LanguagePicker};
use super::{Change, PickerRequest, SettingsEvent, SettingsView, pickers};
use crate::model::AppModel;
use crate::views::{
    command_palette::Handler,
    font_picker::{FontEvent, FontPicker},
    theme_picker,
    titlebar::BeginWindowMove,
    workspace,
};

pub(crate) enum SettingsOverlay {
    Themes {
        picker: Entity<CommandPalette<Handler>>,
        source: PickerRequest,
    },
    Fonts {
        picker: Entity<FontPicker>,
        source: PickerRequest,
    },
    Languages {
        picker: Entity<LanguagePicker>,
        source: PickerRequest,
    },
    Providers {
        picker: Entity<Picker>,
        source: PickerRequest,
    },
}

impl SettingsOverlay {
    pub(crate) fn source(&self) -> &PickerRequest {
        match self {
            Self::Themes { source, .. }
            | Self::Fonts { source, .. }
            | Self::Languages { source, .. }
            | Self::Providers { source, .. } => source,
        }
    }
}

pub(crate) struct SettingsWindow {
    model: WeakEntity<AppModel>,
    pub(crate) view: Entity<SettingsView>,
    pub(crate) overlay: Option<SettingsOverlay>,
    overlay_subscription: Option<Subscription>,
    #[cfg(target_os = "macos")]
    window_drag: Option<muxy_ui::window_drag::WindowDrag>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsWindow {
    pub(crate) fn new(
        model: WeakEntity<AppModel>,
        view: Entity<SettingsView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        view.read(cx).focus.focus(window);
        let events = cx.subscribe_in(&view, window, |root: &mut Self, _, event, window, cx| {
            root.handle_event(event, window, cx);
        });
        let appearance = cx.observe(&view, |root: &mut Self, _, cx| {
            let view = root.view.read(cx);
            let theme = view.theme.clone();
            let metrics = view.metrics;
            if let Some(overlay) = &root.overlay {
                match overlay {
                    SettingsOverlay::Themes { picker, source } => {
                        let active = if source.kind == super::PickerKind::Theme(true) {
                            view.snapshot.settings.appearance.dark_theme.clone()
                        } else {
                            view.snapshot.settings.appearance.light_theme.clone()
                        };
                        picker.update(cx, |picker, cx| {
                            picker.set_appearance(theme, metrics, cx);
                            picker.set_current(&active, cx);
                        });
                    }
                    SettingsOverlay::Fonts { picker, .. } => {
                        picker.update(cx, |picker, cx| picker.set_appearance(theme, metrics, cx));
                    }
                    SettingsOverlay::Languages { picker, .. } => {
                        picker.update(cx, |picker, cx| picker.set_appearance(theme, metrics, cx));
                    }
                    SettingsOverlay::Providers { .. } => (),
                }
            }
            cx.notify();
        });
        let focus_lost = cx.on_focus_lost(window, |root, window, cx| root.focus(window, cx));
        let mut subscriptions = vec![events, appearance, focus_lost];
        if let Some(model) = model.upgrade() {
            subscriptions.push(cx.observe(&model, |root: &mut Self, model, cx| {
                let error = model.read(cx).error.clone();
                root.view.update(cx, |view, cx| {
                    if view.errors.get("application") != error.as_ref() {
                        view.set_error("application", error.as_deref(), cx);
                    }
                });
            }));
        }
        Self {
            model,
            view,
            overlay: None,
            overlay_subscription: None,
            #[cfg(target_os = "macos")]
            window_drag: muxy_ui::window_drag::WindowDrag::new(&window.window_title()),
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn focus(&self, window: &mut Window, cx: &Context<Self>) {
        match &self.overlay {
            Some(SettingsOverlay::Themes { picker, .. }) => picker.focus_handle(cx).focus(window),
            Some(SettingsOverlay::Fonts { picker, .. }) => picker.focus_handle(cx).focus(window),
            Some(SettingsOverlay::Languages { picker, .. }) => {
                picker.focus_handle(cx).focus(window);
            }
            Some(SettingsOverlay::Providers { picker, .. }) => {
                picker.focus_handle(cx).focus(window);
            }
            None => self.view.read(cx).focus.focus(window),
        }
    }

    pub(crate) fn dismiss_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = None;
        self.overlay_subscription = None;
        self.focus(window, cx);
        cx.notify();
    }

    fn open_picker(&mut self, request: PickerRequest, window: &mut Window, cx: &mut Context<Self>) {
        if request.anchor.get().is_none() {
            return;
        }
        let _ = self.model.update(cx, AppModel::flush_preferences);
        let theme = self.view.read(cx).theme.clone();
        let metrics = self.view.read(cx).metrics;
        match request.kind {
            super::PickerKind::FontFamily => {
                let active = self
                    .view
                    .read(cx)
                    .snapshot
                    .terminal
                    .font_families
                    .first()
                    .cloned()
                    .unwrap_or_default();
                let picker = cx.new(|cx| FontPicker::new(active, theme, metrics, cx));
                self.overlay_subscription =
                    Some(
                        cx.subscribe_in(&picker, window, |root, _, event, window, cx| {
                            if let FontEvent::Selected(name) = event {
                                let _ = root.model.update(cx, |model, cx| {
                                    model.change_preference(
                                        Change::Field("font-family", name.clone()),
                                        cx,
                                    );
                                });
                            }
                            root.dismiss_overlay(window, cx);
                        }),
                    );
                self.overlay = Some(SettingsOverlay::Fonts {
                    picker,
                    source: request,
                });
            }
            super::PickerKind::Theme(dark) => {
                let Ok(registry) = self
                    .model
                    .update(cx, |model, cx| model.command_registry(dark, cx))
                else {
                    return;
                };
                let picker = cx.new(|cx| CommandPalette::dropdown(registry, theme, metrics, cx));
                picker.update(cx, |picker, cx| {
                    picker.open_root_page(theme_picker::PAGE_ID, cx);
                });
                self.overlay_subscription = Some(cx.subscribe_in(
                    &picker,
                    window,
                    move |root, picker, event, window, cx| {
                        let keep_open = matches!(event, CommandPaletteEvent::Applied(_));
                        if !keep_open {
                            root.dismiss_overlay(window, cx);
                        }
                        if let CommandPaletteEvent::Selected(handler)
                        | CommandPaletteEvent::Applied(handler) = event
                            && let Some(model) = root.model.upgrade()
                        {
                            let workspace = model.read(cx).window;
                            let handler = handler.clone();
                            let picker = picker.clone();
                            cx.defer(move |cx| {
                                let _ = workspace.update(cx, |_, window, cx| {
                                    model.update(cx, |model, cx| {
                                        handler(model, window, cx);
                                        if keep_open {
                                            let active =
                                                model.themes.active_name(&model.appearance, dark);
                                            picker.update(cx, |picker, cx| {
                                                picker.set_current(&active, cx);
                                            });
                                        }
                                    });
                                });
                            });
                        }
                    },
                ));
                self.overlay = Some(SettingsOverlay::Themes {
                    picker,
                    source: request,
                });
            }
            super::PickerKind::Language => self.open_language_picker(request, window, cx),
            super::PickerKind::AiProvider(action) => {
                self.open_provider_picker(action, request, window, cx);
            }
            super::PickerKind::ExtensionSidebar => self.open_sidebar_picker(request, window, cx),
        }
        cx.notify();
    }

    /// Chooses between the built-in sidebar and enabled extension sidebars.
    fn open_sidebar_picker(
        &mut self,
        request: PickerRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = self.view.read(cx);
        let sidebars = view.snapshot.sidebars.clone();
        let (theme, metrics) = (view.theme.clone(), view.metrics);
        let picker = cx.new(|cx| {
            Picker::new(
                PickerConfig::popover("extension-sidebar", "Search sidebars…"),
                theme,
                metrics,
                cx,
            )
        });
        let items = std::iter::once(PickerItem::Row(PickerRow::new("", "Built-in")))
            .chain(
                sidebars
                    .into_iter()
                    .map(|(id, label)| PickerItem::Row(PickerRow::new(id, label))),
            )
            .collect();
        picker.update(cx, |picker, cx| picker.set_items(items, cx));
        self.overlay_subscription = Some(cx.subscribe_in(
            &picker,
            window,
            move |root, _, event, window, cx| match event {
                PickerEvent::Confirmed(selection) => {
                    let owner = selection.id.to_string();
                    let _ = root.model.update(cx, |model, cx| {
                        model.change_preference(Change::ExtensionSidebar(owner), cx);
                    });
                    root.dismiss_overlay(window, cx);
                }
                PickerEvent::Dismissed => root.dismiss_overlay(window, cx),
                _ => (),
            },
        ));
        picker.focus_handle(cx).focus(window);
        self.overlay = Some(SettingsOverlay::Providers {
            picker,
            source: request,
        });
    }

    fn open_language_picker(
        &mut self,
        request: PickerRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = self.view.read(cx);
        let active = view.snapshot.settings.composer.language.clone();
        let theme = view.theme.clone();
        let metrics = view.metrics;
        let picker = cx.new(|cx| LanguagePicker::new(active, theme, metrics, cx));
        self.overlay_subscription =
            Some(
                cx.subscribe_in(&picker, window, |root, _, event, window, cx| {
                    if let LanguageEvent::Selected(language) = event {
                        let _ = root.model.update(cx, |model, cx| {
                            model.change_preference(
                                Change::Field("composer-language", language.clone()),
                                cx,
                            );
                        });
                    }
                    root.dismiss_overlay(window, cx);
                }),
            );
        self.overlay = Some(SettingsOverlay::Languages {
            picker,
            source: request,
        });
    }

    fn open_provider_picker(
        &mut self,
        action: crate::repository_actions::Action,
        request: PickerRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = self.view.read(cx);
        let installed = view.snapshot.ai_installed.clone();
        let (theme, metrics) = (view.theme.clone(), view.metrics);
        let picker = cx.new(|cx| {
            Picker::new(
                PickerConfig::popover("ai-provider", "Search providers…"),
                theme,
                metrics,
                cx,
            )
        });
        let items = std::iter::once(PickerItem::Row(PickerRow::new("", "Auto")))
            .chain(crate::ai::PROVIDERS.iter().map(|provider| {
                let title = if installed.contains(&provider.id) {
                    provider.name.to_owned()
                } else {
                    format!("{} · Not installed", provider.name)
                };
                PickerItem::Row(PickerRow::new(provider.id, title))
            }))
            .collect();
        picker.update(cx, |picker, cx| picker.set_items(items, cx));
        self.overlay_subscription = Some(cx.subscribe_in(
            &picker,
            window,
            move |root, _, event, window, cx| match event {
                PickerEvent::Confirmed(selection) => {
                    let id = selection.id.to_string();
                    let _ = root
                        .model
                        .update(cx, |model, cx| model.set_ai_provider(action, &id, cx));
                    root.dismiss_overlay(window, cx);
                }
                PickerEvent::Dismissed => root.dismiss_overlay(window, cx),
                _ => (),
            },
        ));
        picker.focus_handle(cx).focus(window);
        self.overlay = Some(SettingsOverlay::Providers {
            picker,
            source: request,
        });
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.model.update(cx, |model, cx| {
            model.flush_preferences(cx);
            model.settings_window = None;
            cx.notify();
        });
        window.remove_window();
    }

    fn handle_event(&mut self, event: &SettingsEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            SettingsEvent::Change(change) => {
                let _ = self
                    .model
                    .update(cx, |model, cx| model.change_preference(change.clone(), cx));
            }
            SettingsEvent::Picker(kind, anchor) => {
                self.open_picker(
                    PickerRequest {
                        kind: *kind,
                        anchor: anchor.clone(),
                    },
                    window,
                    cx,
                );
            }
            SettingsEvent::ServerControl { restart } => {
                let _ = self.model.update(cx, |model, cx| {
                    model.confirm_server_control(*restart, window.window_handle(), cx);
                });
            }
            SettingsEvent::ReadServer => {
                let _ = self.model.update(cx, AppModel::read_server_settings);
            }
            SettingsEvent::ReadMobile => {
                let _ = self.model.update(cx, AppModel::read_remote_access);
            }
            SettingsEvent::PairPhone => {
                let _ = self.model.update(cx, AppModel::pair_phone);
            }
            SettingsEvent::CancelPairing => {
                let _ = self.model.update(cx, AppModel::cancel_pairing);
            }
            SettingsEvent::RevokeDevice(device) => {
                let _ = self.model.update(cx, |model, cx| {
                    model.confirm_revoke(*device, window.window_handle(), cx);
                });
            }
            SettingsEvent::Connect => {
                let _ = self.model.update(cx, AppModel::connect);
            }
            SettingsEvent::OpenConfiguration(filename) => {
                self.open_configuration(filename, cx);
            }
            SettingsEvent::ReloadConfiguration => {
                if let Ok(error) = self.model.update(cx, |model, cx| {
                    model.reload_configuration(cx);
                    model.configuration_error.clone()
                }) {
                    self.view.update(cx, |view, cx| {
                        view.set_error("configuration", error.as_deref(), cx);
                    });
                }
            }
        }
    }

    fn open_configuration(&self, filename: &'static str, cx: &mut Context<Self>) {
        let Ok(path) = self
            .model
            .update(cx, |model, _| model.configuration_path(filename))
        else {
            return;
        };
        cx.spawn(async move |root, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&path)?;
                    let status = std::process::Command::new("/usr/bin/open")
                        .args(["-a", "TextEdit"])
                        .arg(path)
                        .status()?;
                    if status.success() {
                        Ok(())
                    } else {
                        Err(std::io::Error::other(format!(
                            "TextEdit exited with {status}"
                        )))
                    }
                })
                .await;
            let _ = root.update(cx, |root, cx| {
                root.view.update(cx, |view, cx| {
                    view.set_error(
                        "configuration",
                        result
                            .err()
                            .map(|error| format!("Could not open {filename}: {error}"))
                            .as_deref(),
                        cx,
                    );
                });
            });
        })
        .detach();
    }

    fn overlay(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(overlay) = &self.overlay else {
            return div().into_any_element();
        };
        let picker = match overlay {
            SettingsOverlay::Themes { picker, .. } => picker.clone().into_any_element(),
            SettingsOverlay::Fonts { picker, .. } => picker.clone().into_any_element(),
            SettingsOverlay::Languages { picker, .. } => picker.clone().into_any_element(),
            SettingsOverlay::Providers { picker, .. } => picker.clone().into_any_element(),
        };
        let picker = pickers::dropdown(picker, overlay.source().clone(), cx);
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                div()
                    .id("settings-picker-backdrop")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|root, _, window, cx| {
                        root.dismiss_overlay(window, cx);
                        cx.stop_propagation();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|root, _, window, cx| {
                            root.dismiss_overlay(window, cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
            .child(picker)
            .into_any_element()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-window")
            .key_context("SettingsWindow")
            .relative()
            .size_full()
            .overflow_hidden()
            .on_action(
                cx.listener(|root, _: &super::CloseSettings, window, cx| root.close(window, cx)),
            )
            .on_action(
                cx.listener(|root, _: &workspace::OpenSettings, window, cx| root.focus(window, cx)),
            )
            .on_action(cx.listener(|root, _: &workspace::Quit, _, cx| {
                let _ = root.model.update(cx, AppModel::quit);
            }))
            .on_action(cx.listener(|root, _: &workspace::CheckForUpdates, _, cx| {
                let _ = root
                    .model
                    .update(cx, |model, cx| model.check_for_updates(true, cx));
            }))
            .on_action(
                cx.listener(|root, _: &workspace::EndAllSessionsAndQuit, _, cx| {
                    let _ = root.model.update(cx, AppModel::end_all_and_quit);
                }),
            )
            .on_action(
                cx.listener(|_, _: &workspace::Minimize, window, _| window.minimize_window()),
            )
            .on_action(cx.listener(|_, _: &workspace::Zoom, window, _| window.zoom_window()))
            .on_action(
                cx.listener(|_, _: &workspace::ToggleFullScreen, window, _| {
                    window.toggle_fullscreen();
                }),
            )
            .on_action(
                cx.listener(|root, _: &workspace::OpenConfiguration, _, cx| {
                    if let Some(file) = root.view.read(cx).configuration_file() {
                        root.open_configuration(file, cx);
                    }
                }),
            )
            .on_action(cx.listener(|root, _: &BeginWindowMove, _window, _| {
                #[cfg(target_os = "macos")]
                if let Some(drag) = &root.window_drag {
                    drag.begin();
                }
                #[cfg(not(target_os = "macos"))]
                _window.start_window_move();
            }))
            .child(self.view.clone())
            .child(self.overlay(cx))
    }
}
