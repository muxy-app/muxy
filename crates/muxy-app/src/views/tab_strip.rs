use muxy_core::shortcuts::ShortcutId;
pub(super) mod drag;

pub(crate) use drag::{TabDragState, TabDrop};

use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px, relative,
};
use muxy_app_core::{Tab, TabGroup, TabId, TabSide};

use super::titlebar;
use crate::model::AppModel;
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::theme::Theme;
use muxy_ui::tr;

pub(crate) fn tab_strip(
    model: &AppModel,
    sidebar_width: f32,
    window: &Window,
    actions: (gpui::Div, f32),
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let strip = titlebar::background("tab-strip")
        .relative()
        .flex()
        .items_center()
        .h(px(32.0))
        .flex_none();
    let (actions, actions_width) = actions;
    if model.state.current_project().status() == muxy_app_core::ProjectStatus::Missing {
        return strip.justify_end().child(actions).into_any_element();
    }
    let leading = (titlebar::navigation_width(model, window) - sidebar_width).max(0.0);
    let new_tab_width = f32::from(model.metrics.control_medium() + model.metrics.spacing2() * 2.0);
    let available = (f32::from(titlebar::content_size(window).width)
        - sidebar_width
        - leading
        - actions_width
        - new_tab_width)
        .max(0.0);
    let count = u16::try_from(model.state.current_project().tabs.len()).unwrap_or(u16::MAX);
    let ideal_width = available / f32::from(count.max(1));
    let width = ideal_width.clamp(44.0, 200.0);
    let tabs: Vec<_> = model.state.current_project().tabs.iter().collect();
    let mut cells = tab_cells(&tabs, width, true, model, cx);
    let new_button = new_tab_button(None, model, cx);
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
        .child(actions)
        .into_any_element()
}

/// The strip above a group's panes: its tabs and a button for a new tab in
/// the group.
pub(crate) fn group_strip(
    group: &TabGroup,
    width: f32,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let project = model.state.current_project();
    let tabs: Vec<_> = group
        .tabs()
        .iter()
        .filter_map(|id| project.tabs.iter().find(|tab| tab.id == *id))
        .collect();
    let focused = model
        .active_tab()
        .is_some_and(|tab| group.tabs().contains(&tab));
    let count = u16::try_from(tabs.len()).unwrap_or(u16::MAX).max(1);
    let control_width = f32::from(model.metrics.control_medium() + model.metrics.spacing2() * 2.0);
    let cell_width = ((width - control_width) / f32::from(count)).clamp(44.0, 200.0);
    let cells = tab_cells(&tabs, cell_width, focused, model, cx).child(new_tab_button(
        group.tabs().last().copied(),
        model,
        cx,
    ));
    let shown = group.selected();
    let strips = model.tab_drag.strips.clone();
    div()
        .debug_selector(move || format!("group-strip-{shown}"))
        .relative()
        .flex()
        .items_center()
        .h(px(32.0))
        .flex_none()
        .bg(model.theme.bg)
        .child(
            div()
                .id(SharedString::from(format!("group-tabs-{}", group.id())))
                .flex()
                .flex_1()
                .min_w(px(0.0))
                .h_full()
                .overflow_x_scroll()
                .child(cells),
        )
        .child(
            gpui::canvas(
                move |bounds, _, _| strips.borrow_mut().push((shown, bounds)),
                |_, (), _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .into_any_element()
}

/// Tab cells for `tabs`, measured for dragging. `shortcuts` shows ⌘1–9 hints.
fn tab_cells(
    tabs: &[&Tab],
    width: f32,
    shortcuts: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> gpui::Div {
    let mut cells = div().flex().flex_none().items_center().h_full();
    for (index, tab) in tabs.iter().enumerate() {
        cells = cells.child(tab_cell(tab, index, width, shortcuts, model, cx));
    }
    drag::measure_tabs(
        cells,
        tabs.iter().map(|tab| tab.id).collect(),
        model.tab_drag.cells.clone(),
    )
}

/// Opens a tab after `anchor` in its group, or at the end of the focused one.
pub(super) fn new_tab_button(
    anchor: Option<TabId>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> gpui::Stateful<gpui::Div> {
    let id = anchor.map_or_else(
        || SharedString::from("new-tab"),
        |anchor| SharedString::from(format!("new-tab-{anchor}")),
    );
    titlebar::icon_button(id, Icon::Plus, tr!("New Tab (⌘T)"), model)
        .ml(model.metrics.spacing2())
        .debug_selector(move || {
            if anchor.is_some() {
                "group-new-tab-button"
            } else {
                "new-tab-button"
            }
            .into()
        })
        .on_click(cx.listener(move |model, _, _, cx| {
            cx.stop_propagation();
            match anchor {
                Some(anchor) => model.new_tab_adjacent(anchor, TabSide::Right, cx),
                None => model.new_tab(cx),
            }
        }))
}

pub(super) fn existing_terminals_button(
    count: usize,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let project = model.state.current_project().id;
    let tooltip = if count == 1 {
        tr!("1 Existing Terminal").to_string()
    } else {
        tr!("%lld Existing Terminals", count).to_string()
    };
    let tooltip = model
        .settings
        .keymap
        .chord(ShortcutId::ExistingTerminals)
        .map_or_else(|| tooltip.clone(), |chord| format!("{tooltip} ({chord})"));
    titlebar::icon_button("existing-terminals", Icon::TerminalStack, tooltip, model)
        .debug_selector(|| "existing-terminals-button".into())
        .on_click(cx.listener(move |model, _, window, cx| {
            cx.stop_propagation();
            model.open_session_picker(project, window, cx);
        }))
        .into_any_element()
}

/// Opens a browser tab in the current project.
pub(super) fn browser_button(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let tooltip = model
        .settings
        .keymap
        .chord(ShortcutId::NewBrowserTab)
        .map_or_else(
            || tr!("Open Browser Tab").to_string(),
            |chord| format!("{} ({chord})", tr!("Open Browser Tab")),
        );
    titlebar::icon_button("new-browser-tab", Icon::Globe, tooltip, model)
        .debug_selector(|| "browser-button".into())
        .on_click(cx.listener(|model, _, _, cx| {
            cx.stop_propagation();
            model.new_browser_tab(cx);
        }))
        .into_any_element()
}

pub(super) fn settings_button(model: &AppModel, cx: &mut Context<AppModel>) -> AnyElement {
    let tooltip = model
        .settings
        .keymap
        .chord(ShortcutId::OpenSettings)
        .map_or_else(
            || tr!("Settings").to_string(),
            |chord| format!("{} ({chord})", tr!("Settings")),
        );
    titlebar::icon_button("open-settings", Icon::Settings, tooltip, model)
        .debug_selector(|| "settings-button".into())
        .on_click(cx.listener(|model, _, window, cx| {
            cx.stop_propagation();
            model.open_settings(window, cx);
        }))
        .into_any_element()
}

pub(super) fn zoom_control(
    zoomed: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let label = if zoomed {
        tr!("Restore Pane")
    } else {
        tr!("Maximize Pane")
    };
    let tooltip = model
        .settings
        .keymap
        .chord(ShortcutId::ToggleZoomPane)
        .map_or_else(|| label.to_string(), |chord| format!("{label} ({chord})"));
    titlebar::icon_button(
        "toggle-zoom-pane",
        if zoomed {
            Icon::Restore
        } else {
            Icon::Maximize
        },
        tooltip,
        model,
    )
    .debug_selector(move || {
        if zoomed {
            "restore-pane"
        } else {
            "maximize-pane"
        }
        .into()
    })
    .on_click(cx.listener(|model, _, window, cx| {
        cx.stop_propagation();
        model.toggle_zoom_pane(cx);
        model.focus_active(window, cx);
    }))
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

#[allow(
    clippy::too_many_lines,
    reason = "Declarative tab cell and interaction layout"
)]
fn tab_cell(
    tab: &Tab,
    index: usize,
    width: f32,
    shortcuts: bool,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let theme = &model.theme;
    let pane = tab.displayed_pane(model.state.shown_pane(tab));
    let active = model.visible_tabs().contains(&tab.id);
    let grouped = model.state.current_project().groups().is_some();
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
        .debug_selector(move || {
            if grouped {
                format!("group-tab-{id}")
            } else {
                format!("tab-cell-{index}")
            }
        })
        .when(!model.tab_drag.is_active(), |cell| {
            cell.group(group.clone())
        })
        // On Windows the strip behind is the window's caption, where a press
        // would never start a drag.
        .when(cfg!(windows), InteractiveElement::occlude)
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
                model.begin_tab_drag(id, event.position, drag::Source::Strip, cx);
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
            ShortcutId::TABS
                .get(index)
                .filter(|_| shortcuts)
                .and_then(|id| model.shortcut_hints.label(*id, &model.settings.keymap))
                .map_or_else(
                    || {
                        super::tab_activity::glyph(
                            tab,
                            model,
                            px(14.0),
                            model
                                .webview_glyph(tab, px(14.0), foreground, cx)
                                .unwrap_or_else(|| {
                                    tab_glyph(tab.pinned, settings, bell, foreground, theme)
                                }),
                        )
                    },
                    |label| {
                        div()
                            .debug_selector(move || format!("tab-shortcut-{index}"))
                            .child(super::shortcut_hints::badge(label, px(14.0), model))
                            .into_any_element()
                    },
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
