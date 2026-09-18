use gpui::{
    AnyElement, AppContext, Context, Entity, Focusable, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Subscription,
    WeakEntity, Window, div,
};

use muxy_ui::command_palette::{CommandPalette, CommandPaletteEvent};

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
}

impl SettingsOverlay {
    pub(crate) fn source(&self) -> &PickerRequest {
        match self {
            Self::Themes { source, .. } | Self::Fonts { source, .. } => source,
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
        let events =
            cx.subscribe_in(
                &view,
                window,
                |root: &mut Self, _, event, window, cx| match event {
                    SettingsEvent::Change(change) => {
                        let _ = root
                            .model
                            .update(cx, |model, cx| model.change_preference(change.clone(), cx));
                    }
                    SettingsEvent::Picker(kind, anchor) => {
                        root.open_picker(
                            PickerRequest {
                                kind: *kind,
                                anchor: anchor.clone(),
                            },
                            window,
                            cx,
                        );
                    }
                    SettingsEvent::ServerControl { restart } => {
                        let _ = root.model.update(cx, |model, cx| {
                            model.confirm_server_control(*restart, window.window_handle(), cx);
                        });
                    }
                    SettingsEvent::ReadServer => {
                        let _ = root.model.update(cx, AppModel::read_server_settings);
                    }
                    SettingsEvent::Connect => {
                        let _ = root.model.update(cx, AppModel::connect);
                    }
                    SettingsEvent::OpenConfiguration(filename) => {
                        root.open_configuration(filename, cx);
                    }
                    SettingsEvent::ReloadConfiguration => {
                        if let Ok(error) = root.model.update(cx, |model, cx| {
                            model.reload_configuration(cx);
                            model.configuration_error.clone()
                        }) {
                            root.view.update(cx, |view, cx| {
                                view.set_error("configuration", error.as_deref(), cx);
                            });
                        }
                    }
                },
            );
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
                let picker = cx.new(|cx| CommandPalette::new(registry, theme, metrics, cx));
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
        }
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self.model.update(cx, |model, cx| {
            model.flush_preferences(cx);
            model.settings_window = None;
            cx.notify();
        });
        window.remove_window();
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
            SettingsOverlay::Fonts { picker, source } => {
                pickers::dropdown(picker.clone().into_any_element(), source.clone(), cx)
            }
        };
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
                    root.open_configuration(root.view.read(cx).configuration_file(), cx);
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
