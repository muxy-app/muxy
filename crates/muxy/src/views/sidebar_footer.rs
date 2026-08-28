use crate::state::AppState;
use crate::views::app::AppLayout;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use muxy_ui::components::IconButton;
use muxy_ui::icon::Icon;

use crate::views::window::MainWindow;

pub fn sidebar_footer(
    state: &AppState,
    layout: AppLayout,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let glyph = metrics.scaled(13.0);
    let box_size = metrics.control_medium();
    let controls = footer_controls(layout.wide_sidebar);

    if layout.wide_sidebar {
        return div()
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(metrics.spacing2())
            .px(metrics.spacing5())
            .pb(metrics.spacing4())
            .child(footer_control(controls[0], state, glyph, box_size, cx))
            .child(div().flex_grow())
            .child(footer_control(controls[1], state, glyph, box_size, cx))
            .child(footer_control(controls[2], state, glyph, box_size, cx))
            .into_any_element();
    }

    let mut footer = div()
        .flex()
        .flex_col()
        .flex_none()
        .items_center()
        .gap(metrics.spacing2())
        .pb(metrics.spacing4());
    for control in controls {
        footer = footer.child(footer_control(*control, state, glyph, box_size, cx));
    }
    footer.into_any_element()
}

#[derive(Clone, Copy)]
enum FooterControl {
    ToggleSidebar,
    Notifications,
    ThemePicker,
}

const WIDE_FOOTER_CONTROLS: [FooterControl; 3] = [
    FooterControl::ToggleSidebar,
    FooterControl::Notifications,
    FooterControl::ThemePicker,
];
const NARROW_FOOTER_CONTROLS: [FooterControl; 3] = [
    FooterControl::Notifications,
    FooterControl::ThemePicker,
    FooterControl::ToggleSidebar,
];

fn footer_controls(wide: bool) -> &'static [FooterControl] {
    if wide {
        &WIDE_FOOTER_CONTROLS
    } else {
        &NARROW_FOOTER_CONTROLS
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NotificationControlModel {
    icon: Icon,
    accessibility_label: String,
    opens_panel: bool,
}

fn notification_control_model(unread: usize) -> NotificationControlModel {
    NotificationControlModel {
        icon: if unread == 0 {
            Icon::Bell
        } else {
            Icon::BellDot
        },
        accessibility_label: match unread {
            0 => "Notifications, no unread notifications".to_owned(),
            1 => "Notifications, 1 unread notification".to_owned(),
            count => format!("Notifications, {count} unread notifications"),
        },
        opens_panel: true,
    }
}

fn footer_control(
    control: FooterControl,
    state: &AppState,
    glyph: gpui::Pixels,
    box_size: gpui::Pixels,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let theme = &state.theme;
    match control {
        FooterControl::ToggleSidebar => IconButton::new(
            "toggle-sidebar",
            Icon::PanelLeft,
            glyph,
            box_size,
            theme.fg_muted,
            theme.fg,
        )
        .on_click(cx.listener(|window: &mut MainWindow, _, _, cx| {
            window.toggle_sidebar(cx);
        }))
        .into_any_element(),
        FooterControl::Notifications => {
            let view = cx.weak_entity();
            let model = notification_control_model(state.notification_store.unread_total());
            div()
                .flex()
                .flex_none()
                .child(
                    IconButton::new(
                        "notifications",
                        model.icon,
                        glyph,
                        box_size,
                        theme.fg_muted,
                        theme.fg,
                    )
                    .tooltip(
                        model.accessibility_label,
                        theme.raised(),
                        theme.fg,
                        theme.border,
                    )
                    .on_click(cx.listener(
                        |window: &mut MainWindow, _, _, cx| {
                            window.toggle_notifications(cx);
                        },
                    )),
                )
                .on_children_prepainted(move |bounds, _, cx| {
                    let Some(bounds) = bounds.first().copied() else {
                        return;
                    };
                    let _ = view.update(cx, |window, _| {
                        window.record_notification_anchor(bounds);
                    });
                })
                .into_any_element()
        }
        FooterControl::ThemePicker => {
            let view = cx.weak_entity();
            div()
                .flex()
                .flex_none()
                .child(
                    IconButton::new(
                        "theme-picker",
                        Icon::Palette,
                        glyph,
                        box_size,
                        theme.fg_muted,
                        theme.fg,
                    )
                    .on_click(cx.listener(
                        |window: &mut MainWindow, _, window_handle: &mut gpui::Window, cx| {
                            window.open_theme_picker(window_handle, cx);
                        },
                    )),
                )
                .on_children_prepainted(move |bounds, _, cx| {
                    let Some(bounds) = bounds.first().copied() else {
                        return;
                    };
                    let _ = view.update(cx, |window, _| {
                        window.record_theme_picker_anchor(bounds);
                    });
                })
                .into_any_element()
        }
    }
}

#[cfg(test)]
fn footer_control_ids(wide: bool) -> Vec<&'static str> {
    footer_controls(wide)
        .iter()
        .map(|control| match control {
            FooterControl::ToggleSidebar => "toggle-sidebar",
            FooterControl::Notifications => "notifications",
            FooterControl::ThemePicker => "theme-picker",
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_footer_notification_order_badge_and_intent_are_exact() {
        assert_eq!(
            footer_control_ids(true),
            vec!["toggle-sidebar", "notifications", "theme-picker"]
        );
        assert_eq!(
            footer_control_ids(false),
            vec!["notifications", "theme-picker", "toggle-sidebar"]
        );
        assert!(!footer_control_ids(true).contains(&"extensions"));
        assert!(!footer_control_ids(false).contains(&"extensions"));
        assert_eq!(
            notification_control_model(0),
            NotificationControlModel {
                icon: Icon::Bell,
                accessibility_label: "Notifications, no unread notifications".to_owned(),
                opens_panel: true,
            }
        );
        assert_eq!(notification_control_model(1).icon, Icon::BellDot);
        assert_eq!(
            notification_control_model(1).accessibility_label,
            "Notifications, 1 unread notification"
        );
        assert_eq!(
            notification_control_model(12).accessibility_label,
            "Notifications, 12 unread notifications"
        );
        let source = include_str!("sidebar_footer.rs");
        assert!(source.contains("record_notification_anchor(bounds)"));
        assert!(source.contains("toggle_notifications(cx)"));
    }
}
