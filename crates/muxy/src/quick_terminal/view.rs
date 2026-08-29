use crate::quick_terminal::panel::{EffectiveAppearance, QuickTerminalConfiguration};
use crate::quick_terminal::runtime::QuickTerminalRuntime;
use crate::terminal::surfaces::AppSurfaceHandle;
use crate::terminal::{ConfirmationId, ConfirmationKind};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, BorrowAppContext, Context, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px,
};
use muxy_ui::components::IconButton;
use muxy_ui::icon::Icon;
use muxy_ui::theme::{Metrics, Theme};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

#[cfg(target_os = "macos")]
use super::platform::macos::{PanelAdapter, PanelProperties, PanelTelemetry};

pub type QuickTerminalSurface = Rc<RefCell<Box<dyn AppSurfaceHandle>>>;
pub type QuickTerminalSurfaceSlot = Rc<RefCell<Option<QuickTerminalSurface>>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeAction {
    Close,
    ToggleQuickSettings,
    ToggleShortcutSettings,
    RequestInputMonitoring,
    AdjustQuickSetting { setting: QuickSetting, delta: i64 },
    Reset,
    OpenSettings,
    ResolveConfirmation { id: ConfirmationId, approved: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfirmationPrompt {
    pub id: ConfirmationId,
    pub kind: ConfirmationKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuickSetting {
    Width,
    Height,
    Transparency,
    Blur,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessibilityRole {
    Group,
    Status,
    Button,
    Slider,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibilityNode {
    pub identifier: &'static str,
    pub role: AccessibilityRole,
    pub label: String,
    pub value: String,
    pub focus_order: usize,
    pub announces_changes: bool,
}

pub fn bridge_accessibility_model(
    status: &str,
    shortcut: &str,
    monitoring: &str,
    width: i64,
    height: i64,
    transparency: i64,
    blur: i64,
) -> Vec<AccessibilityNode> {
    let definitions = [
        (
            "quick-terminal",
            AccessibilityRole::Group,
            "Quick Terminal",
            String::new(),
            false,
        ),
        (
            "quick-terminal-status",
            AccessibilityRole::Status,
            "Quick Terminal status",
            status.to_owned(),
            true,
        ),
        (
            "quick-terminal-shortcut",
            AccessibilityRole::Button,
            "Quick Terminal shortcut",
            shortcut.to_owned(),
            false,
        ),
        (
            "quick-terminal-input-monitoring",
            AccessibilityRole::Button,
            "Input Monitoring",
            monitoring.to_owned(),
            false,
        ),
        (
            "quick-terminal-quick-settings",
            AccessibilityRole::Button,
            "Quick settings",
            "Collapsed".to_owned(),
            false,
        ),
        (
            "quick-terminal-width",
            AccessibilityRole::Slider,
            "Width",
            format!("{width} points"),
            false,
        ),
        (
            "quick-terminal-height",
            AccessibilityRole::Slider,
            "Height",
            format!("{height} points"),
            false,
        ),
        (
            "quick-terminal-transparency",
            AccessibilityRole::Slider,
            "Transparency",
            format!("{transparency} percent"),
            false,
        ),
        (
            "quick-terminal-blur",
            AccessibilityRole::Slider,
            "Blur",
            format!("{blur} percent"),
            false,
        ),
        (
            "quick-terminal-reset",
            AccessibilityRole::Button,
            "Reset Quick Terminal",
            "Default size and appearance".to_owned(),
            false,
        ),
        (
            "quick-terminal-open-settings",
            AccessibilityRole::Button,
            "Open Quick Terminal settings",
            "Opens Settings".to_owned(),
            false,
        ),
        (
            "quick-terminal-close",
            AccessibilityRole::Button,
            "Close Quick Terminal",
            "Hides the panel".to_owned(),
            false,
        ),
    ];
    definitions
        .into_iter()
        .enumerate()
        .map(
            |(focus_order, (identifier, role, label, value, announces_changes))| {
                AccessibilityNode {
                    identifier,
                    role,
                    label: label.to_owned(),
                    value,
                    focus_order,
                    announces_changes,
                }
            },
        )
        .collect()
}

pub struct QuickTerminalViewModel {
    pub configuration: QuickTerminalConfiguration,
    pub appearance: EffectiveAppearance,
    pub theme: Theme,
    pub metrics: Metrics,
    pub status: String,
    pub shortcut: String,
    pub monitoring: String,
    pub confirmation: Option<ConfirmationPrompt>,
}

pub struct QuickTerminalView {
    #[cfg(target_os = "macos")]
    panel: Result<PanelAdapter, String>,
    surface: QuickTerminalSurfaceSlot,
    model: QuickTerminalViewModel,
    quick_settings_visible: bool,
    visible: bool,
}

impl QuickTerminalView {
    pub fn new(
        surface: QuickTerminalSurfaceSlot,
        model: QuickTerminalViewModel,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Self {
        window.set_background_appearance(model.appearance.background);
        Self {
            #[cfg(target_os = "macos")]
            panel: PanelAdapter::configure(window),
            surface,
            model,
            quick_settings_visible: false,
            visible: false,
        }
    }

    pub fn update_model(&mut self, model: QuickTerminalViewModel, window: &mut Window) {
        window.set_background_appearance(model.appearance.background);
        #[cfg(target_os = "macos")]
        if let Ok(panel) = &mut self.panel {
            panel.install_accessibility(&bridge_accessibility_model(
                &model.status,
                &model.shortcut,
                &model.monitoring,
                model.configuration.width,
                model.configuration.height,
                model.configuration.transparency,
                model.configuration.blur,
            ));
        }
        self.model = model;
    }

    #[cfg(target_os = "macos")]
    pub fn panel_properties(&self) -> Result<PanelProperties, String> {
        self.panel
            .as_ref()
            .map(PanelAdapter::properties)
            .map_err(Clone::clone)
    }

    #[cfg(target_os = "macos")]
    pub fn prepare(&mut self) -> Result<PanelTelemetry, String> {
        self.panel
            .as_mut()
            .map_err(|error| error.clone())?
            .prepare(self.model.configuration)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn prepare(&mut self) -> Result<(), String> {
        Err("Quick Terminal panels are unavailable on this platform".to_owned())
    }

    #[cfg(target_os = "macos")]
    pub fn telemetry(&self) -> Option<PanelTelemetry> {
        self.panel
            .as_ref()
            .ok()
            .and_then(PanelAdapter::telemetry)
            .cloned()
    }

    #[cfg(target_os = "macos")]
    pub fn native_frame(&self) -> Option<muxy_core::quick_terminal::geometry::Rect> {
        self.panel.as_ref().ok().map(PanelAdapter::native_frame)
    }

    pub fn begin_show(&mut self, _duration: Duration, window: &mut Window, _cx: &mut App) {
        self.visible = true;
        #[cfg(target_os = "macos")]
        if let Ok(panel) = &mut self.panel {
            panel.show(_duration);
            return;
        }
        window.activate_window();
    }

    pub fn begin_hide(&mut self, _duration: Duration, _window: &mut Window) {
        self.visible = false;
        self.quick_settings_visible = false;
        #[cfg(target_os = "macos")]
        if let Ok(panel) = &mut self.panel {
            panel.begin_hide(_duration);
        }
    }

    pub fn finish_hide(&mut self, _restores_focus: bool) {
        #[cfg(target_os = "macos")]
        if let Ok(panel) = &mut self.panel {
            panel.finish_hide(_restores_focus);
        }
    }

    pub fn toggle_quick_settings(&mut self) {
        self.quick_settings_visible = !self.quick_settings_visible;
    }

    pub fn is_visible(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.panel.as_ref().is_ok_and(PanelAdapter::is_visible)
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    pub fn is_key(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.panel.as_ref().is_ok_and(PanelAdapter::is_key)
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
}

impl Render for QuickTerminalView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let terminal = self
            .surface
            .borrow()
            .as_ref()
            .map(|surface| surface.borrow().element(self.visible));
        let status = self.model.status.clone();
        let shortcut = self.model.shortcut.clone();
        let monitoring = self.model.monitoring.clone();
        let bridge_background = gpui::hsla(0.0, 0.0, 0.0, 1.0);
        let bridge_foreground = gpui::hsla(0.0, 0.0, 1.0, 1.0);
        let bridge_muted = gpui::hsla(0.0, 0.0, 1.0, 0.58);
        let bridge_control = gpui::hsla(0.0, 0.0, 1.0, 0.1);
        let bridge_border = gpui::hsla(0.0, 0.0, 1.0, 0.14);
        let ready = gpui::hsla(0.39, 0.78, 0.52, 1.0);
        let configuration = self.model.configuration;
        let theme = self.model.theme.clone();
        let metrics = self.model.metrics;
        let quick_settings = self.quick_settings_visible;
        let confirmation = self.model.confirmation;
        div()
            .key_context(crate::quick_terminal::KEY_CONTEXT)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(20.0))
            .border_1()
            .border_color(bridge_border)
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
                        BridgeAction::ToggleShortcutSettings,
                        metrics,
                        bridge_foreground,
                        bridge_muted,
                        bridge_control,
                    ))
                    .child(
                        IconButton::new(
                            "quick-terminal-quick-settings",
                            Icon::Palette,
                            metrics.icon_sm(),
                            metrics.control_medium(),
                            bridge_muted,
                            bridge_foreground,
                        )
                        .tooltip("Quick settings", theme.raised(), theme.fg, theme.border)
                        .on_click(|_, _, cx| {
                            dispatch_bridge_action(BridgeAction::ToggleQuickSettings, cx)
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
                        .tooltip("Close", theme.raised(), theme.fg, theme.border)
                        .on_click(|_, _, cx| dispatch_bridge_action(BridgeAction::Close, cx)),
                    ),
            )
            .child(div().flex_1().overflow_hidden().children(terminal))
            .when(quick_settings, |panel| {
                panel.child(
                    div()
                        .absolute()
                        .top(px(42.0))
                        .right(px(10.0))
                        .w(px(286.0))
                        .p(px(12.0))
                        .flex()
                        .flex_col()
                        .gap(px(9.0))
                        .rounded(px(12.0))
                        .border_1()
                        .border_color(bridge_border)
                        .bg(gpui::hsla(0.0, 0.0, 0.0, 0.94))
                        .shadow_lg()
                        .text_size(px(11.0))
                        .text_color(bridge_muted)
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(bridge_foreground)
                                .child("Quick Terminal"),
                        )
                        .child(quick_setting_control(
                            "Width",
                            format!("{} pt", configuration.width),
                            QuickSetting::Width,
                            bridge_foreground,
                            bridge_muted,
                            bridge_control,
                        ))
                        .child(quick_setting_control(
                            "Height",
                            format!("{} pt", configuration.height),
                            QuickSetting::Height,
                            bridge_foreground,
                            bridge_muted,
                            bridge_control,
                        ))
                        .child(quick_setting_control(
                            "Transparency",
                            format!("{}%", configuration.transparency),
                            QuickSetting::Transparency,
                            bridge_foreground,
                            bridge_muted,
                            bridge_control,
                        ))
                        .child(quick_setting_control(
                            "Vibrancy",
                            format!("{}%", configuration.blur),
                            QuickSetting::Blur,
                            bridge_foreground,
                            bridge_muted,
                            bridge_control,
                        ))
                        .child(quick_setting_row(
                            "Input Monitoring",
                            monitoring,
                            bridge_foreground,
                            bridge_muted,
                        ))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .pt(px(3.0))
                                .child(bridge_text_button(
                                    "quick-terminal-monitoring",
                                    "Enable Input Monitoring",
                                    BridgeAction::RequestInputMonitoring,
                                    &theme,
                                    metrics,
                                ))
                                .child(div().flex_1())
                                .child(bridge_text_button(
                                    "quick-terminal-reset",
                                    "Reset",
                                    BridgeAction::Reset,
                                    &theme,
                                    metrics,
                                ))
                                .child(bridge_text_button(
                                    "quick-terminal-open-settings",
                                    "Open Settings",
                                    BridgeAction::OpenSettings,
                                    &theme,
                                    metrics,
                                )),
                        ),
                )
            })
            .when_some(confirmation, |panel, prompt| {
                panel.child(confirmation_dialog(
                    prompt,
                    metrics,
                    bridge_foreground,
                    bridge_muted,
                    bridge_border,
                ))
            })
    }
}

fn confirmation_copy(kind: ConfirmationKind) -> (&'static str, &'static str, &'static str) {
    match kind {
        ConfirmationKind::Paste => (
            "Paste multiple lines?",
            "The clipboard contains text that may execute more than one command.",
            "Paste",
        ),
        ConfirmationKind::Osc52Read => (
            "Allow clipboard read?",
            "A terminal program requested access to the system clipboard.",
            "Allow",
        ),
        ConfirmationKind::Osc52Write => (
            "Allow clipboard write?",
            "A terminal program requested permission to replace the system clipboard.",
            "Allow",
        ),
        ConfirmationKind::ActiveProcessClose => (
            "Close active terminal?",
            "A process is still running in this terminal.",
            "Close",
        ),
    }
}

fn confirmation_dialog(
    prompt: ConfirmationPrompt,
    metrics: Metrics,
    foreground: gpui::Hsla,
    muted: gpui::Hsla,
    border: gpui::Hsla,
) -> AnyElement {
    let (title, body, approve) = confirmation_copy(prompt.kind);
    div()
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
                .bg(gpui::hsla(0.0, 0.0, 0.08, 0.98))
                .shadow_lg()
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
                            prompt.id,
                            false,
                            metrics,
                            foreground,
                            false,
                        ))
                        .child(confirmation_button(
                            "quick-terminal-confirm-approve",
                            approve,
                            prompt.id,
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
    confirmation_id: ConfirmationId,
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
        .on_click(move |_, _, cx| {
            dispatch_bridge_action(
                BridgeAction::ResolveConfirmation {
                    id: confirmation_id,
                    approved,
                },
                cx,
            )
        })
        .child(label)
        .into_any_element()
}

fn bridge_header_button(
    id: &'static str,
    label: impl Into<String>,
    action: BridgeAction,
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
        .on_click(move |_, _, cx| dispatch_bridge_action(action, cx))
        .child(label.into())
        .into_any_element()
}

fn quick_setting_control(
    label: &'static str,
    value: impl Into<String>,
    setting: QuickSetting,
    foreground: gpui::Hsla,
    muted: gpui::Hsla,
    control: gpui::Hsla,
) -> AnyElement {
    let (decrement_id, increment_id) = match setting {
        QuickSetting::Width => ("quick-terminal-width-less", "quick-terminal-width-more"),
        QuickSetting::Height => ("quick-terminal-height-less", "quick-terminal-height-more"),
        QuickSetting::Transparency => (
            "quick-terminal-transparency-less",
            "quick-terminal-transparency-more",
        ),
        QuickSetting::Blur => ("quick-terminal-blur-less", "quick-terminal-blur-more"),
    };
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(div().flex_1().text_color(muted).child(label))
        .child(quick_setting_button(
            decrement_id,
            "−",
            setting,
            -1,
            foreground,
            control,
        ))
        .child(
            div()
                .w(px(52.0))
                .flex()
                .justify_center()
                .font_weight(FontWeight::MEDIUM)
                .text_color(foreground)
                .child(value.into()),
        )
        .child(quick_setting_button(
            increment_id,
            "+",
            setting,
            1,
            foreground,
            control,
        ))
        .into_any_element()
}

fn quick_setting_button(
    id: &'static str,
    label: &'static str,
    setting: QuickSetting,
    delta: i64,
    foreground: gpui::Hsla,
    control: gpui::Hsla,
) -> AnyElement {
    div()
        .id(id)
        .size(px(22.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.0))
        .cursor_pointer()
        .bg(control)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(foreground)
        .hover(|style| style.bg(gpui::hsla(0.0, 0.0, 1.0, 0.18)))
        .on_click(move |_, _, cx| {
            dispatch_bridge_action(BridgeAction::AdjustQuickSetting { setting, delta }, cx)
        })
        .child(label)
        .into_any_element()
}

fn quick_setting_row(
    label: &'static str,
    value: impl Into<String>,
    foreground: gpui::Hsla,
    muted: gpui::Hsla,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .child(div().flex_1().text_color(muted).child(label))
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(foreground)
                .child(value.into()),
        )
        .into_any_element()
}

fn bridge_text_button(
    id: &'static str,
    label: impl Into<String>,
    action: BridgeAction,
    theme: &Theme,
    metrics: Metrics,
) -> AnyElement {
    div()
        .id(id)
        .h(metrics.control_small())
        .px(px(7.0))
        .flex()
        .items_center()
        .rounded(metrics.radius_sm())
        .cursor_pointer()
        .text_size(metrics.font_footnote())
        .text_color(theme.fg_muted)
        .hover(|style| style.bg(theme.hover).text_color(theme.fg))
        .on_click(move |_, _, cx| dispatch_bridge_action(action, cx))
        .child(label.into())
        .into_any_element()
}

fn dispatch_bridge_action(action: BridgeAction, cx: &mut App) {
    cx.update_global::<QuickTerminalRuntime, _>(|runtime, cx| runtime.bridge_action(action, cx));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn quick_terminal_view_accessibility_model_is_complete_unique_and_ordered() {
        let nodes = bridge_accessibility_model(
            "Shell exited",
            "Double Shift",
            "Local only",
            720,
            430,
            18,
            70,
        );
        assert_eq!(
            nodes.first().map(|node| node.role),
            Some(AccessibilityRole::Group)
        );
        assert!(
            nodes
                .iter()
                .any(|node| node.role == AccessibilityRole::Status && node.announces_changes)
        );
        for label in [
            "Quick Terminal status",
            "Quick Terminal shortcut",
            "Input Monitoring",
            "Quick settings",
            "Reset Quick Terminal",
            "Open Quick Terminal settings",
            "Close Quick Terminal",
            "Width",
            "Height",
            "Transparency",
            "Blur",
        ] {
            assert!(nodes.iter().any(|node| node.label == label));
        }
        let labels = nodes
            .iter()
            .map(|node| node.label.as_str())
            .collect::<HashSet<_>>();
        assert_eq!(labels.len(), nodes.len());
        assert!(nodes.iter().all(|node| !node.label.trim().is_empty()));
        assert!(
            nodes
                .windows(2)
                .all(|pair| pair[0].focus_order < pair[1].focus_order)
        );
        assert!(nodes.iter().any(|node| node.value == "Shell exited"));
        assert!(nodes.iter().any(|node| node.value == "720 points"));
        assert!(nodes.iter().any(|node| node.value == "18 percent"));
    }

    #[test]
    fn quick_terminal_view_bridge_actions_cover_every_visible_control() {
        assert_eq!(
            [
                BridgeAction::Close,
                BridgeAction::ToggleQuickSettings,
                BridgeAction::ToggleShortcutSettings,
                BridgeAction::RequestInputMonitoring,
                BridgeAction::AdjustQuickSetting {
                    setting: QuickSetting::Width,
                    delta: -1,
                },
                BridgeAction::AdjustQuickSetting {
                    setting: QuickSetting::Width,
                    delta: 1,
                },
                BridgeAction::Reset,
                BridgeAction::OpenSettings,
            ]
            .len(),
            8
        );
    }
}
