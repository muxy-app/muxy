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
    ThemePicker,
}

const WIDE_FOOTER_CONTROLS: [FooterControl; 2] =
    [FooterControl::ToggleSidebar, FooterControl::ThemePicker];
const NARROW_FOOTER_CONTROLS: [FooterControl; 2] =
    [FooterControl::ThemePicker, FooterControl::ToggleSidebar];

fn footer_controls(wide: bool) -> &'static [FooterControl] {
    if wide {
        &WIDE_FOOTER_CONTROLS
    } else {
        &NARROW_FOOTER_CONTROLS
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
        FooterControl::ThemePicker => IconButton::new(
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
        ))
        .into_any_element(),
    }
}

#[cfg(test)]
fn footer_control_ids(wide: bool) -> Vec<&'static str> {
    footer_controls(wide)
        .iter()
        .map(|control| match control {
            FooterControl::ToggleSidebar => "toggle-sidebar",
            FooterControl::ThemePicker => "theme-picker",
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_sidebar_footer_contract_excludes_notifications_and_extensions() {
        assert_eq!(
            footer_control_ids(true),
            vec!["toggle-sidebar", "theme-picker"]
        );
        assert_eq!(
            footer_control_ids(false),
            vec!["theme-picker", "toggle-sidebar"]
        );
        for id in ["notifications", "extensions"] {
            assert!(!footer_control_ids(true).contains(&id));
            assert!(!footer_control_ids(false).contains(&id));
        }
    }
}
