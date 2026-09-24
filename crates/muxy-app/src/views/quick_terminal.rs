use crate::model::AppModel;
use crate::views::terminal::pane::TerminalPane;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, Context, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window,
    div, px,
};
use muxy_ui::components::IconButton;
use muxy_ui::icon::Icon;
use muxy_ui::popover;
use muxy_ui::quick_terminal::panel::{EffectiveAppearance, QuickTerminalConfiguration};
use muxy_ui::theme::{Metrics, Theme};
use std::time::Duration;

#[cfg(target_os = "macos")]
use muxy_ui::quick_terminal::platform::macos::{PanelAdapter, PanelTelemetry};

use muxy_ui::quick_terminal::view::bridge_accessibility_model;
#[derive(Clone, Copy)]
pub(crate) enum BridgeAction {
    Close,
    OpenSettings,
}

pub(crate) struct QuickTerminalViewModel {
    pub configuration: QuickTerminalConfiguration,
    pub appearance: EffectiveAppearance,
    pub theme: Theme,
    pub metrics: Metrics,
    pub status: String,
    pub shortcut: String,
}

pub(crate) struct QuickTerminalView {
    #[cfg(target_os = "macos")]
    panel: Result<PanelAdapter, String>,
    owner: WeakEntity<AppModel>,
    terminal: Option<Entity<TerminalPane>>,
    model: QuickTerminalViewModel,
    visible: bool,
    pub(crate) confirm_close: bool,
    menu: Option<(gpui::Point<gpui::Pixels>, usize)>,
    menu_focus: gpui::FocusHandle,
    confirmation_focus: gpui::FocusHandle,
    _activation_subscription: Subscription,
    _bounds_subscription: Subscription,
}

fn hide_for_activation_change(visible: bool, active: bool) -> bool {
    visible && !active
}

fn set_window_appearance(window: &mut Window, appearance: EffectiveAppearance) {
    window.set_background_appearance(appearance.background);
}

impl QuickTerminalView {
    pub(crate) fn new(
        owner: WeakEntity<AppModel>,
        model: QuickTerminalViewModel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        set_window_appearance(window, model.appearance);
        let activation_subscription = cx.observe_window_activation(window, |view, window, cx| {
            if hide_for_activation_change(view.visible, window.is_window_active()) {
                view.visible = false;
                let owner = view.owner.clone();
                cx.defer(move |cx| {
                    let _ = owner.update(cx, |model, cx| model.hide_quick_terminal(false, cx));
                });
            }
        });
        #[cfg(target_os = "macos")]
        let panel = PanelAdapter::configure(window);
        let bounds_subscription = cx.observe_window_bounds(window, |view, _, _| {
            #[cfg(target_os = "macos")]
            if let Ok(panel) = &view.panel {
                panel.sync_bounds(view.visible);
            }
        });
        Self {
            #[cfg(target_os = "macos")]
            panel,
            owner,
            terminal: None,
            model,
            visible: false,
            confirm_close: false,
            menu: None,
            menu_focus: cx.focus_handle(),
            confirmation_focus: cx.focus_handle(),
            _activation_subscription: activation_subscription,
            _bounds_subscription: bounds_subscription,
        }
    }

    pub(crate) fn update_model(
        &mut self,
        model: QuickTerminalViewModel,
        terminal: Option<Entity<TerminalPane>>,
        window: &mut Window,
    ) {
        set_window_appearance(window, model.appearance);
        #[cfg(target_os = "macos")]
        if let Ok(panel) = &mut self.panel {
            panel
                .install_accessibility(&bridge_accessibility_model(&model.status, &model.shortcut));
        }
        self.model = model;
        self.terminal = terminal;
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn prepare(&mut self, window: &mut Window) -> Result<PanelTelemetry, String> {
        self.panel
            .as_mut()
            .map_err(|error| error.clone())?
            .prepare(self.model.configuration, window)
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn prepare(&mut self, _window: &mut Window) -> Result<(), String> {
        Err("Quick Terminal panels are unavailable on this platform".to_owned())
    }

    fn update_owner(
        &self,
        action: fn(&mut AppModel, &mut Context<AppModel>),
        cx: &mut Context<Self>,
    ) {
        let owner = self.owner.clone();
        cx.defer(move |cx| {
            let _ = owner.update(cx, action);
        });
    }

    pub(crate) fn show_close_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_close = true;
        self.menu = None;
        self.confirmation_focus.focus(window);
        if let Some(terminal) = &self.terminal {
            terminal.update(cx, |pane, cx| pane.set_focused(false, cx));
        }
        cx.notify();
    }

    pub(crate) fn open_menu(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.menu = Some((position, 0));
        self.menu_focus.focus(window);
        cx.notify();
    }

    fn menu_command(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let position = self.menu.take().map(|(position, _)| position);
        if let Some(terminal) = &self.terminal {
            terminal.update(cx, |pane, cx| match index {
                0 => pane.copy_selection(cx),
                1 => pane.paste_clipboard(cx),
                2 => pane.select_all(cx),
                3 => pane.select_command_output(position, cx),
                _ => {}
            });
        }
        self.focus_terminal(window, cx);
        cx.notify();
    }

    pub(crate) fn focus_terminal(&self, window: &mut Window, cx: &mut App) {
        if let Some(terminal) = &self.terminal {
            terminal.update(cx, |pane, cx| {
                pane.set_focused(true, cx);
                pane.focus.focus(window);
            });
        }
    }

    pub(crate) fn begin_show(&mut self, duration: Duration, window: &mut Window, _cx: &mut App) {
        self.visible = true;
        #[cfg(target_os = "macos")]
        if let Ok(panel) = &mut self.panel {
            panel.show(duration);
            return;
        }
        window.activate_window();
    }

    pub(crate) fn begin_hide(&mut self, duration: Duration, _window: &mut Window) {
        self.visible = false;
        self.confirm_close = false;
        self.menu = None;
        #[cfg(target_os = "macos")]
        if let Ok(panel) = &mut self.panel {
            panel.begin_hide(duration);
        }
    }

    pub(crate) fn finish_hide(&mut self, restores_focus: bool) {
        #[cfg(target_os = "macos")]
        if let Ok(panel) = &mut self.panel {
            panel.finish_hide(restores_focus);
        }
    }
}

impl Render for QuickTerminalView {
    #[allow(
        clippy::too_many_lines,
        reason = "Declarative panel layout and terminal action wiring"
    )]
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let terminal = self.terminal.clone();
        if let Some(terminal) = &terminal {
            terminal.update(cx, |pane, _| {
                pane.native_visible = self.visible && !self.confirm_close && self.menu.is_none();
                #[cfg(target_os = "macos")]
                if let Some(scroll) = &pane.native_scroll {
                    scroll.set_visible(pane.native_visible && pane.grid.is_some());
                }
            });
        }
        let menu = self.render_menu(window, cx);
        let owner_settings = self.owner.clone();
        let owner_close = self.owner.clone();
        let owner_shortcut = self.owner.clone();
        let status = self.model.status.clone();
        let shortcut = self.model.shortcut.clone();
        let theme = self.model.theme.clone();
        let mut bridge_background = theme.bg;
        bridge_background.a = f32::from(self.model.appearance.tint_alpha_percent) / 100.0;
        let bridge_foreground = theme.fg;
        let bridge_muted = theme.fg_alpha(0.58);
        let bridge_control = theme.fg_alpha(0.1);
        let ready = gpui::hsla(0.39, 0.78, 0.52, 1.0);
        let metrics = self.model.metrics;
        div()
            .key_context(if self.confirm_close {
                "QuickTerminalConfirmation"
            } else {
                "QuickTerminal"
            })
            .on_action(
                cx.listener(|view, _: &crate::views::workspace::CloseTab, _, cx| {
                    view.update_owner(AppModel::request_close_quick_terminal, cx);
                }),
            )
            .on_action(
                cx.listener(|view, _: &crate::views::workspace::ClosePane, _, cx| {
                    view.update_owner(AppModel::request_close_quick_terminal, cx);
                }),
            )
            .on_action(
                cx.listener(|view, _: &crate::views::workspace::Find, window, cx| {
                    let theme = view.model.theme.clone();
                    let metrics = view.model.metrics;
                    if let Some(terminal) = &view.terminal {
                        terminal.update(cx, |pane, cx| {
                            pane.open_find(&theme, metrics, window, cx);
                        });
                    }
                }),
            )
            .on_action(
                cx.listener(|view, _: &crate::views::workspace::FindNext, _, cx| {
                    if let Some(terminal) = &view.terminal {
                        terminal.update(cx, |pane, cx| {
                            pane.step_find(false, cx);
                        });
                    }
                }),
            )
            .on_action(
                cx.listener(|view, _: &crate::views::workspace::FindPrevious, _, cx| {
                    if let Some(terminal) = &view.terminal {
                        terminal.update(cx, |pane, cx| {
                            pane.step_find(true, cx);
                        });
                    }
                }),
            )
            .on_action(
                cx.listener(|view, _: &crate::views::workspace::PreviousPrompt, _, cx| {
                    if let Some(terminal) = &view.terminal {
                        terminal.update(cx, |pane, cx| {
                            pane.jump_prompt(true, cx);
                        });
                    }
                }),
            )
            .on_action(
                cx.listener(|view, _: &crate::views::workspace::NextPrompt, _, cx| {
                    if let Some(terminal) = &view.terminal {
                        terminal.update(cx, |pane, cx| {
                            pane.jump_prompt(false, cx);
                        });
                    }
                }),
            )
            .on_action(cx.listener(
                |view, _: &crate::views::workspace::SelectCommandOutput, _, cx| {
                    if let Some(terminal) = &view.terminal {
                        terminal.update(cx, |pane, cx| {
                            pane.select_command_output(None, cx);
                        });
                    }
                },
            ))
            .on_action(cx.listener(
                |view, _: &crate::views::workspace::IncreaseFontSize, _, cx| {
                    if let Some(terminal) = &view.terminal {
                        terminal.update(cx, |pane, cx| {
                            pane.terminal.zoom(1.0);
                            cx.notify();
                        });
                    }
                },
            ))
            .on_action(cx.listener(
                |view, _: &crate::views::workspace::DecreaseFontSize, _, cx| {
                    if let Some(terminal) = &view.terminal {
                        terminal.update(cx, |pane, cx| {
                            pane.terminal.zoom(-1.0);
                            cx.notify();
                        });
                    }
                },
            ))
            .on_action(
                cx.listener(|view, _: &crate::views::workspace::OpenSettings, _, cx| {
                    view.update_owner(AppModel::open_quick_terminal_settings, cx);
                }),
            )
            .on_action(
                cx.listener(|view, _: &crate::views::workspace::Quit, _, cx| {
                    view.update_owner(AppModel::quit, cx);
                }),
            )
            .on_action(cx.listener(
                |view, _: &crate::views::workspace::EndAllSessionsAndQuit, _, cx| {
                    view.update_owner(AppModel::end_all_and_quit, cx);
                },
            ))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(20.0))
            .child(
                div()
                    .h(px(34.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .bg(bridge_background)
                    .child(div().size(px(6.0)).rounded_full().bg(ready))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(bridge_foreground)
                            .child("Quick Terminal"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_size(px(11.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(bridge_muted)
                            .child(status),
                    )
                    .child(bridge_header_button(
                        "quick-terminal-shortcut",
                        shortcut,
                        owner_shortcut,
                        metrics,
                        bridge_foreground,
                        bridge_muted,
                        bridge_control,
                    ))
                    .child(
                        IconButton::new(
                            "quick-terminal-settings",
                            Icon::Settings,
                            metrics.icon_sm(),
                            metrics.control_medium(),
                            bridge_muted,
                            bridge_foreground,
                        )
                        .tooltip("Settings", theme.raised(), theme.fg, theme.border, theme.bg)
                        .on_click(move |_, _, cx| {
                            dispatch_bridge_action(&owner_settings, BridgeAction::OpenSettings, cx);
                        }),
                    )
                    .child(
                        IconButton::new(
                            "quick-terminal-close",
                            Icon::X,
                            metrics.icon_sm(),
                            metrics.control_medium(),
                            bridge_muted,
                            bridge_foreground,
                        )
                        .tooltip("Close", theme.raised(), theme.fg, theme.border, theme.bg)
                        .on_click(move |_, _, cx| {
                            dispatch_bridge_action(&owner_close, BridgeAction::Close, cx);
                        }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .children(terminal),
            )
            .children(menu)
            .when(self.confirm_close, |panel| {
                panel.child(confirmation_dialog(
                    &self.owner,
                    &self.confirmation_focus,
                    metrics,
                    theme.fg,
                    theme.fg_muted,
                    theme.border_solid(),
                ))
            })
    }
}

fn bridge_header_button(
    id: &'static str,
    label: impl Into<String>,
    owner: WeakEntity<AppModel>,
    metrics: Metrics,
    foreground: gpui::Hsla,
    muted: gpui::Hsla,
    background: gpui::Hsla,
) -> AnyElement {
    div()
        .id(id)
        .h(metrics.control_small())
        .px(px(8.0))
        .flex()
        .items_center()
        .rounded(px(5.0))
        .cursor_pointer()
        .bg(background)
        .text_size(px(10.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(muted)
        .hover(move |style| {
            style
                .bg(gpui::hsla(0.0, 0.0, 1.0, 0.16))
                .text_color(foreground)
        })
        .on_click(move |_, _, cx| dispatch_bridge_action(&owner, BridgeAction::OpenSettings, cx))
        .child(label.into())
        .into_any_element()
}

fn dispatch_bridge_action(owner: &WeakEntity<AppModel>, action: BridgeAction, cx: &mut App) {
    let owner = owner.clone();
    cx.defer(move |cx| {
        let _ = owner.update(cx, |model, cx| match action {
            BridgeAction::Close => model.hide_quick_terminal(true, cx),
            BridgeAction::OpenSettings => model.open_quick_terminal_settings(cx),
        });
    });
}
fn confirmation_dialog(
    owner: &WeakEntity<AppModel>,
    focus: &gpui::FocusHandle,
    metrics: Metrics,
    foreground: gpui::Hsla,
    muted: gpui::Hsla,
    border: gpui::Hsla,
) -> AnyElement {
    let background = gpui::hsla(0.0, 0.0, 0.08, 0.98);
    let (title, body, approve) = (
        "Close active terminal?",
        "A process is still running in this terminal.",
        "Close",
    );
    let cancel_owner = owner.clone();
    let confirm_owner = owner.clone();
    div()
        .track_focus(focus)
        .on_action(move |_: &muxy_ui::picker::Dismiss, _, cx| {
            resolve_confirmation(&cancel_owner, false, cx);
        })
        .on_action(move |_: &muxy_ui::picker::Confirm, _, cx| {
            resolve_confirmation(&confirm_owner, true, cx);
        })
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.34))
        .child(
            div()
                .w(px(420.0))
                .max_w(px(420.0))
                .p(px(22.0))
                .flex()
                .flex_col()
                .gap(px(14.0))
                .rounded(px(14.0))
                .border_1()
                .border_color(border)
                .bg(background)
                .shadow(muxy_ui::theme::Elevation::Modal.shadow(background))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(foreground)
                        .child(title),
                )
                .child(div().text_size(px(12.0)).text_color(muted).child(body))
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.0))
                        .child(confirmation_button(
                            "quick-terminal-confirm-cancel",
                            "Cancel",
                            owner.clone(),
                            false,
                            metrics,
                            foreground,
                            false,
                        ))
                        .child(confirmation_button(
                            "quick-terminal-confirm-approve",
                            approve,
                            owner.clone(),
                            true,
                            metrics,
                            foreground,
                            true,
                        )),
                ),
        )
        .into_any_element()
}

fn confirmation_button(
    id: &'static str,
    label: &'static str,
    owner: WeakEntity<AppModel>,
    approved: bool,
    metrics: Metrics,
    foreground: gpui::Hsla,
    primary: bool,
) -> AnyElement {
    div()
        .id(id)
        .h(metrics.control_medium())
        .px(px(14.0))
        .flex()
        .items_center()
        .rounded(px(6.0))
        .cursor_pointer()
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .bg(if primary {
            gpui::hsla(0.58, 0.72, 0.58, 1.0)
        } else {
            gpui::hsla(0.0, 0.0, 1.0, 0.1)
        })
        .text_color(if primary {
            foreground
        } else {
            gpui::hsla(0.0, 0.0, 1.0, 0.64)
        })
        .hover(move |style| style.text_color(foreground))
        .on_click(move |_, _, cx| resolve_confirmation(&owner, approved, cx))
        .child(label)
        .into_any_element()
}

impl QuickTerminalView {
    fn render_menu(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        use crate::views::menu::{
            ConfirmHighlighted, DismissMenu, HighlightNext, HighlightPrevious,
        };
        let (position, highlighted) = self.menu?;
        let theme = &self.model.theme;
        let m = self.model.metrics;
        let width = m
            .scaled(220.0)
            .min((window.viewport_size().width - px(16.0)).max(px(0.0)));
        let height = m
            .scaled(popover::PADDING * 2.0 + popover::ROW_HEIGHT * 4.0 + popover::ROW_GAP * 3.0)
            + px(2.0);
        let origin = crate::views::overlays::clamp(
            position,
            gpui::size(width, height),
            window.viewport_size(),
        );
        let mut menu = popover::surface(theme, m)
            .id("quick-context-menu")
            .debug_selector(|| "quick-context-menu".into())
            .key_context("Menu")
            .track_focus(&self.menu_focus)
            .absolute()
            .left(origin.x)
            .top(origin.y)
            .w(width)
            .max_h((window.viewport_size().height - px(16.0)).max(px(0.0)))
            .overflow_y_scroll()
            .on_action(cx.listener(|view, _: &DismissMenu, window, cx| {
                view.menu = None;
                view.focus_terminal(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &HighlightNext, _, cx| {
                if let Some((_, index)) = &mut view.menu {
                    *index = (*index + 1) % 4;
                }
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &HighlightPrevious, _, cx| {
                if let Some((_, index)) = &mut view.menu {
                    *index = (*index + 3) % 4;
                }
                cx.notify();
            }))
            .on_action(cx.listener(|view, _: &ConfirmHighlighted, window, cx| {
                if let Some((_, index)) = view.menu {
                    view.menu_command(index, window, cx);
                }
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|view, _, window, cx| {
                view.menu = None;
                view.focus_terminal(window, cx);
                cx.notify();
            }));
        for (index, label) in ["Copy", "Paste", "Select All", "Select Command Output"]
            .into_iter()
            .enumerate()
        {
            menu = menu.child(
                popover::row(theme, m, ("quick-menu", index), true, highlighted == index)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.menu_command(index, window, cx);
                    }))
                    .child(label),
            );
        }
        Some(menu.into_any_element())
    }
}

fn resolve_confirmation(owner: &WeakEntity<AppModel>, approved: bool, cx: &mut App) {
    let owner = owner.clone();
    cx.defer(move |cx| {
        let _ = owner.update(cx, |model, cx| {
            if approved {
                model.close_quick_terminal(cx);
            } else {
                model.quick.closing = None;
                if let Some(panel) = model.quick.panel {
                    let _ = panel.update(cx, |view, window, cx| {
                        view.confirm_close = false;
                        view.focus_terminal(window, cx);
                        cx.notify();
                    });
                }
            }
        });
    });
}
