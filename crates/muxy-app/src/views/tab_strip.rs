use muxy_core::shortcuts::ShortcutId;
pub(super) mod drag;

pub(crate) use drag::TabDragState;

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};
use muxy_app_core::{Tab, TabId};

use super::titlebar;
use crate::model::AppModel;
use muxy_ui::components::{IconButton, IconGlyph};
use muxy_ui::icon::Icon;
use muxy_ui::theme::Theme;

pub(crate) fn tab_strip(
    model: &AppModel,
    sidebar_width: f32,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let strip = titlebar::background("tab-strip")
        .relative()
        .flex()
        .items_center()
        .h(px(32.0))
        .flex_none();
    let settings = settings_button(model, cx);
    if model.state.current_project().status() == muxy_app_core::ProjectStatus::Missing {
        return strip.justify_end().child(settings).into_any_element();
    }
    let theme = &model.theme;
    let zoom_tab = model
        .state
        .current_project()
        .tabs
        .iter()
        .find(|tab| Some(tab.id) == model.active_tab())
        .filter(|tab| tab.zoomed.is_some() || tab.panes.len() > 1);
    let control_width = f32::from(model.metrics.control_medium() + model.metrics.spacing2());
    let zoom_width = zoom_tab.map_or(0.0, |_| control_width);
    let existing_count = model.existing_terminal_count();
    let existing_width = if existing_count == 0 {
        0.0
    } else {
        control_width
    };
    let leading = (titlebar::navigation_width(model) - sidebar_width).max(0.0);
    let available = (f32::from(window.viewport_size().width)
        - sidebar_width
        - leading
        - 28.0
        - zoom_width
        - existing_width
        - control_width
        - model.extension_toolbar_width())
    .max(0.0);
    let count = u16::try_from(model.state.current_project().tabs.len()).unwrap_or(u16::MAX);
    let ideal_width = available / f32::from(count.max(1));
    let width = ideal_width.clamp(44.0, 200.0);
    let targets = drag::TabBounds::default();
    let mut cells = div().flex().flex_none().h_full();
    for (index, tab) in model.state.current_project().tabs.iter().enumerate() {
        cells = cells.child(tab_cell(tab, index, width, model, cx));
    }
    let mut cells = drag::measure_tabs(cells, targets.clone(), model);
    let tooltip = "New Tab (⌘T)";
    let new_button = div()
        .debug_selector(|| "new-tab-button".into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .pl(px(4.0))
        .w(px(28.0))
        .h_full()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            IconButton::new(
                "new-tab",
                Icon::Plus,
                px(13.0),
                px(24.0),
                theme.fg_muted,
                theme.fg,
            )
            .tooltip(tooltip, theme.raised(), theme.fg, theme.border, theme.bg)
            .on_click(cx.listener(|model, _, _, cx| {
                cx.stop_propagation();
                model.new_tab(cx);
            })),
        );
    let pinned_button = if ideal_width < 44.0 {
        Some(new_button)
    } else {
        cells = cells.child(new_button);
        None
    };
    strip
        .pl(px(leading))
        .child(
            div()
                .id("tabs-scroll")
                .debug_selector(|| "tabs-scroll".into())
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .h_full()
                .overflow_x_scroll()
                .child(cells),
        )
        .children(pinned_button)
        .children(
            (existing_count > 0).then(|| existing_terminals_button(existing_count, model, cx)),
        )
        .children(zoom_tab.map(|tab| zoom_control(tab.zoomed.is_some(), model, cx)))
        .child(model.extension_toolbar(cx))
        .child(settings)
        .child(drag::track_pointer(targets, cx))
        .into_any_element()
}

pub(super) fn existing_terminals_button(
    count: usize,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let project = model.state.current_project().id;
    let tooltip = if count == 1 {
        "1 Existing Terminal".to_owned()
    } else {
        format!("{count} Existing Terminals")
    };
    let tooltip = model
        .settings
        .keymap
        .chord(ShortcutId::ExistingTerminals)
        .map_or_else(|| tooltip.clone(), |chord| format!("{tooltip} ({chord})"));
    div()
        .debug_selector(|| "existing-terminals-button".into())
        .flex()
        .flex_none()
        .items_center()
        .h_full()
        .pr(model.metrics.spacing2())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            IconButton::new(
                "existing-terminals",
                Icon::TerminalStack,
                model.metrics.scaled(14.0),
                model.metrics.control_medium(),
                theme.fg_muted,
                theme.fg,
            )
            .tooltip(tooltip, theme.raised(), theme.fg, theme.border, theme.bg)
            .on_click(cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                model.open_session_picker(project, window, cx);
            })),
        )
        .into_any_element()
}

pub(super) fn settings_button(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let theme = &model.theme;
    let tooltip = model
        .settings
        .keymap
        .chord(ShortcutId::OpenSettings)
        .map_or_else(
            || "Settings".to_owned(),
            |chord| format!("Settings ({chord})"),
        );
    div()
        .debug_selector(|| "settings-button".into())
        .flex()
        .flex_none()
        .items_center()
        .h_full()
        .pr(model.metrics.spacing2())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            IconButton::new(
                "open-settings",
                Icon::Settings,
                model.metrics.scaled(13.0),
                model.metrics.control_medium(),
                theme.fg_muted,
                theme.fg,
            )
            .tooltip(tooltip, theme.raised(), theme.fg, theme.border, theme.bg)
            .on_click(cx.listener(|model, _, window, cx| {
                cx.stop_propagation();
                model.open_settings(window, cx);
            })),
        )
        .into_any_element()
}

pub(super) fn zoom_control(
    zoomed: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let label = if zoomed {
        "Restore Pane"
    } else {
        "Maximize Pane"
    };
    let tooltip = model
        .settings
        .keymap
        .chord(ShortcutId::ToggleZoomPane)
        .map_or_else(|| label.to_owned(), |chord| format!("{label} ({chord})"));
    div()
        .debug_selector(move || {
            if zoomed {
                "restore-pane"
            } else {
                "maximize-pane"
            }
            .into()
        })
        .flex()
        .flex_none()
        .items_center()
        .h_full()
        .pr(model.metrics.spacing2())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            IconButton::new(
                "toggle-zoom-pane",
                if zoomed {
                    Icon::Restore
                } else {
                    Icon::Maximize
                },
                model.metrics.scaled(13.0),
                model.metrics.control_medium(),
                theme.fg_muted,
                theme.fg,
            )
            .tooltip(tooltip, theme.raised(), theme.fg, theme.border, theme.bg)
            .on_click(cx.listener(|model, _, window, cx| {
                cx.stop_propagation();
                model.toggle_zoom_pane(cx);
                model.focus_active(window, cx);
            })),
        )
        .into_any_element()
}

fn close_control(
    id: TabId,
    active: bool,
    shows_title: bool,
    group: SharedString,
    theme: &Theme,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    div()
        .id(SharedString::from(format!("close-{id}")))
        .debug_selector(|| "close-tab-button".into())
        .absolute()
        .top_0()
        .flex()
        .items_center()
        .justify_center()
        .h_full()
        .when(shows_title, |button| button.right(px(10.0)).w(px(14.0)))
        .when(!shows_title, |button| {
            button.left(relative(0.5)).ml(px(-7.0)).w(px(14.0))
        })
        .opacity(if active { 1.0 } else { 0.0 })
        .group_hover(group, |style| style.opacity(1.0))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |model, _, window, cx| {
            cx.stop_propagation();
            model.close_tab(id, cx);
            model.focus_active(window, cx);
        }))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(14.0))
                .rounded(px(4.0))
                .hover(|style| style.bg(theme.hover))
                .child(IconGlyph::new(Icon::X, px(10.0), theme.fg_muted)),
        )
        .into_any_element()
}

fn tab_cell(
    tab: &Tab,
    index: usize,
    width: f32,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let pane = tab.displayed_pane(model.state.window().active_pane);
    let active = model.active_tab() == Some(tab.id);
    let bell = tab.panes.iter().any(|pane| {
        model
            .terminal(&pane.id)
            .is_some_and(|pane| pane.view.read(cx).bell_flashing)
    });
    let title = model.webview_title(tab, cx).to_owned();
    let color = tab
        .color
        .as_ref()
        .and_then(|color| muxy_ui::theme::parse_hex(color.as_str()))
        .map(gpui::Hsla::from);
    let settings = pane.is_some_and(|pane| pane.content == muxy_app_core::PaneContent::Settings);
    let shows_title = width >= 80.0;
    let id = tab.id;
    let group = SharedString::from(format!("tab-{id}"));
    let show_close = active && shows_title;
    let close = close_control(id, show_close, shows_title, group.clone(), theme, cx);
    let foreground = if active { theme.fg } else { theme.fg_muted };
    div()
        .id(group.clone())
        .debug_selector(move || format!("tab-cell-{index}"))
        .when(!model.tab_drag.is_active(), |cell| {
            cell.group(group.clone())
        })
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .w(px(width))
        .h_full()
        .overflow_hidden()
        .border_r(px(1.0))
        .border_color(theme.border)
        .cursor_pointer()
        .text_size(px(12.0))
        .text_color(foreground)
        .when(active, |tab| tab.bg(theme.surface))
        .when_some(color, |cell, color| {
            cell.bg(color.opacity(if active { 0.18 } else { 0.04 }))
                .when(!model.tab_drag.is_active(), |cell| {
                    cell.hover(|style| style.bg(color.opacity(if active { 0.18 } else { 0.08 })))
                })
        })
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                cx.stop_propagation();
                model.open_tab_menu(id, event.position, window, cx);
            }),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |model, event: &gpui::MouseDownEvent, window, cx| {
                cx.stop_propagation();
                model
                    .tab_drag
                    .begin(model.state.current_project().id, id, event.position);
                model.select_tab(id, cx);
                model.focus_active(window, cx);
            }),
        )
        .on_click(cx.listener(move |model, _, window, cx| {
            cx.stop_propagation();
            model.select_tab(id, cx);
            model.focus_active(window, cx);
        }))
        .on_mouse_down(
            MouseButton::Middle,
            cx.listener(move |model, _, window, cx| {
                cx.stop_propagation();
                model.close_tab(id, cx);
                model.focus_active(window, cx);
            }),
        )
        .child(tab_label(
            &title,
            shows_title,
            active,
            group,
            tab.pinned,
            super::tab_activity::glyph(
                tab,
                model,
                px(14.0),
                model
                    .webview_glyph(tab, px(14.0), foreground, cx)
                    .unwrap_or_else(|| tab_glyph(tab.pinned, settings, bell, foreground, theme)),
            ),
        ))
        .when(!tab.pinned, |cell| cell.child(close))
        .into_any_element()
}

fn tab_glyph(
    pinned: bool,
    settings: bool,
    bell: bool,
    foreground: gpui::Hsla,
    theme: &Theme,
) -> AnyElement {
    let (icon, selector) = if bell {
        (Icon::Bell, "tab-bell")
    } else if pinned {
        (Icon::Pin, "tab-pin")
    } else if settings {
        (Icon::Settings, "tab-settings")
    } else {
        (Icon::Terminal, "tab-terminal")
    };
    div()
        .debug_selector(move || selector.into())
        .child(IconGlyph::new(
            icon,
            px(14.0),
            if bell { theme.accent } else { foreground },
        ))
        .into_any_element()
}

fn tab_label(
    title: &str,
    shows_title: bool,
    active: bool,
    group: SharedString,
    pinned: bool,
    glyph: AnyElement,
) -> impl IntoElement {
    div()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .items_center()
        .gap(px(6.0))
        .h_full()
        .when(shows_title, |row| row.pl(px(12.0)).pr(px(28.0)))
        .when(!shows_title, Styled::justify_center)
        .child(
            div()
                .flex()
                .flex_none()
                .when(!shows_title && !pinned, |icon| {
                    icon.group_hover(group, |style| style.opacity(0.0))
                })
                .child(glyph),
        )
        .when(shows_title, |row| {
            row.child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .font_weight(if active {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .truncate()
                    .child(title.to_owned()),
            )
        })
}
