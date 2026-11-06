use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, Context, CursorStyle, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Role, SharedString, StatefulInteractiveElement, Styled, Window, div, px, svg,
};
use muxy_app_core::{Direction, Pane, PaneContent, PaneId, Tab};
use muxy_core::shortcuts::ShortcutId;
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;
use muxy_ui::tr;

use crate::model::AppModel;

/// What a header button does: something to the pane itself, or a command that
/// acts on the active pane once the pane is made active.
#[derive(Clone, Copy)]
enum Perform {
    Pane(fn(&mut AppModel, PaneId, &mut Context<AppModel>)),
    Active(fn(&mut AppModel, &mut Context<AppModel>)),
}

impl Perform {
    fn run(self, model: &mut AppModel, pane: PaneId, cx: &mut Context<AppModel>) {
        match self {
            Self::Pane(run) => run(model, pane, cx),
            Self::Active(run) => {
                model.focus_pane(pane, cx);
                if model.active_pane() == Some(pane) {
                    run(model, cx);
                }
            }
        }
    }
}

struct Action {
    name: &'static str,
    icon: Icon,
    label: SharedString,
    shortcut: ShortcutId,
    perform: Perform,
}

/// The background a pane's header and dimming take, and the text on it.
pub(super) fn colors(pane: PaneId, model: &AppModel, cx: &App) -> (Hsla, Hsla) {
    model.terminal(&pane).map_or_else(
        || (model.theme.bg, model.theme.fg),
        |terminal| terminal.view.read(cx).header_colors(),
    )
}

/// The bar above a pane: what it shows, its title and, while the pane is
/// active or under the pointer, its actions.
pub(super) fn header(
    pane: &Pane,
    tab: &Tab,
    group: SharedString,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    let id = pane.id;
    let m = model.metrics;
    let (background, foreground) = colors(id, model, cx);
    let active = model.active_pane() == Some(id);
    let bell = model
        .terminal(&id)
        .is_some_and(|terminal| terminal.view.read(cx).bell_flashing);
    let zoomed = tab.zoomed == Some(id);
    let actions = actions(pane, tab, model);
    let count = u16::try_from(actions.len()).unwrap_or(u16::MAX);
    let shown = m.spacing4()
        + m.spacing4()
        + m.control_small() * f32::from(count)
        + m.spacing2() * f32::from(count.saturating_sub(1));
    div()
        .id(SharedString::from(format!("pane-header-{id}")))
        .debug_selector(move || format!("pane-header-{id}"))
        .flex()
        .flex_none()
        .items_center()
        .gap(m.spacing3())
        .relative()
        .min_w(px(0.0))
        .overflow_hidden()
        .h(m.title_bar_height())
        .pl(m.spacing6())
        .pr(if active { shown } else { m.spacing4() })
        .group_hover(group.clone(), move |style| style.pr(shown))
        .bg(background)
        .when(zoomed, |header| {
            header.rounded_t((m.radius_lg() - px(1.0)).max(px(0.0)))
        })
        .text_size(m.font_emphasis())
        .text_color(foreground)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |model, _, window, cx| {
                model.focus_pane(id, cx);
                model.focus_active(window, cx);
            }),
        )
        .child(handle(pane, tab, bell, foreground, model, cx))
        .child(
            div()
                .debug_selector(move || format!("pane-header-actions-{id}"))
                .absolute()
                .top(((m.title_bar_height() - m.control_small()) / 2.0).max(px(0.0)))
                .left(m.spacing6())
                .right(m.spacing4())
                .h(m.control_small())
                .overflow_hidden()
                .flex()
                .flex_row_reverse()
                .flex_wrap()
                .gap(m.spacing2())
                .opacity(if active { 1.0 } else { 0.0 })
                .group_hover(group, |style| style.opacity(1.0))
                .children(
                    actions
                        .into_iter()
                        .rev()
                        .map(|action| button(id, action, foreground, model, cx)),
                ),
        )
        .into_any_element()
}

/// The pane's icon and title. While the pane can be moved, they turn into a
/// handle under the pointer: the icon becomes a move icon and dragging moves the
/// pane, as dragging with Command held does.
fn handle(
    pane: &Pane,
    tab: &Tab,
    bell: bool,
    foreground: Hsla,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    let id = pane.id;
    let m = model.metrics;
    let movable = tab.zoomed.is_none() && tab.panes.len() > 1;
    let group = SharedString::from(format!("pane-handle-{id}"));
    let size = m.scaled(13.0);
    let hover = foreground.opacity(0.08);
    div()
        .id(group.clone())
        .debug_selector(move || format!("pane-handle-{id}"))
        .group(group.clone())
        .flex()
        .flex_shrink()
        .items_center()
        .gap(m.spacing3())
        .min_w(px(0.0))
        .overflow_hidden()
        .h(m.control_medium())
        .ml(-m.spacing3())
        .px(m.spacing3())
        .rounded(m.radius_md())
        .when(movable, |handle| {
            handle
                .cursor(CursorStyle::OpenHand)
                .hover(move |style| style.bg(hover))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |model, event, window, cx| {
                        model.begin_pane_handle_drag(id, event, window, cx);
                    }),
                )
        })
        .child(
            div()
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(size)
                .child(
                    div()
                        .flex()
                        .when(movable, |glyph| {
                            glyph.group_hover(group.clone(), |style| style.opacity(0.0))
                        })
                        .child(glyph(pane, bell, foreground, model, cx)),
                )
                .when(movable, |glyph| {
                    glyph.child(
                        svg()
                            .debug_selector(move || format!("pane-move-{id}"))
                            .absolute()
                            .inset_0()
                            .path(Icon::Move.path())
                            .size(size)
                            .text_color(foreground)
                            .opacity(0.0)
                            .group_hover(group, |style| style.opacity(1.0)),
                    )
                }),
        )
        .child(
            div()
                .min_w(px(0.0))
                .truncate()
                .child(title(pane, model, cx)),
        )
        .into_any_element()
}

/// A terminal names its shell rather than its folder, which the project
/// already shows; other panes show their usual title.
fn title(pane: &Pane, model: &AppModel, cx: &App) -> String {
    model
        .terminal(&pane.id)
        .and_then(|terminal| terminal.view.read(cx).header_title())
        .unwrap_or_else(|| model.pane_title(pane, cx).to_owned())
}

fn glyph(pane: &Pane, bell: bool, foreground: Hsla, model: &AppModel, cx: &App) -> AnyElement {
    let size = model.metrics.scaled(13.0);
    if let Some(page) = model.page_glyph(pane, size, foreground, cx) {
        return page;
    }
    let (icon, color) = match pane.content {
        PaneContent::Settings => (Icon::Settings, foreground),
        PaneContent::Servers => (Icon::Network, foreground),
        PaneContent::Browser(_) => (Icon::Globe, foreground),
        PaneContent::Webview(_) => (Icon::Puzzle, foreground),
        PaneContent::Terminal { .. } if bell => (Icon::Bell, model.theme.accent),
        PaneContent::Terminal { .. } if pane.proxy => (Icon::Network, model.theme.accent),
        PaneContent::Terminal { .. } => (Icon::Terminal, foreground),
    };
    super::super::tab_activity::pane_glyph(
        pane,
        model,
        size,
        IconGlyph::new(icon, size, color).into_any_element(),
    )
}

fn actions(pane: &Pane, tab: &Tab, model: &AppModel) -> Vec<Action> {
    let mut actions = vec![
        Action {
            name: "split-right",
            icon: Icon::Columns,
            label: tr!("Split Right"),
            shortcut: ShortcutId::SplitRight,
            perform: Perform::Active(|model, cx| model.split_pane(Direction::Right, cx)),
        },
        Action {
            name: "split-down",
            icon: Icon::Rows,
            label: tr!("Split Down"),
            shortcut: ShortcutId::SplitDown,
            perform: Perform::Active(|model, cx| model.split_pane(Direction::Down, cx)),
        },
    ];
    if model.can_detach_terminal(pane.id) {
        actions.push(Action {
            name: "detach",
            icon: Icon::Overlap,
            label: tr!("Detach Terminal"),
            shortcut: ShortcutId::DetachTerminal,
            perform: Perform::Pane(AppModel::detach_terminal),
        });
    }
    if tab.zoomed == Some(pane.id) {
        actions.push(Action {
            name: "restore",
            icon: Icon::Restore,
            label: tr!("Restore Pane"),
            shortcut: ShortcutId::ToggleZoomPane,
            perform: Perform::Active(AppModel::toggle_zoom_pane),
        });
    } else if tab.layout.leaves().len() > 1 {
        actions.push(Action {
            name: "maximize",
            icon: Icon::Maximize,
            label: tr!("Maximize Pane"),
            shortcut: ShortcutId::ToggleZoomPane,
            perform: Perform::Active(AppModel::toggle_zoom_pane),
        });
    }
    if !tab.pinned || tab.panes.len() > 1 {
        actions.push(Action {
            name: "close",
            icon: Icon::X,
            label: tr!("Close Pane"),
            shortcut: ShortcutId::ClosePane,
            perform: Perform::Pane(AppModel::close_pane),
        });
    }
    actions
}

fn button(
    id: PaneId,
    action: Action,
    foreground: Hsla,
    model: &AppModel,
    cx: &Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let tooltip = model.settings.keymap.chord(action.shortcut).map_or_else(
        || action.label.to_string(),
        |chord| format!("{} ({chord})", action.label),
    );
    let name = action.name;
    let perform = action.perform;
    let hover = foreground.opacity(0.12);
    super::super::titlebar::with_tooltip(
        div()
            .id(SharedString::from(format!("pane-{name}-{id}")))
            .debug_selector(move || format!("pane-{name}-{id}"))
            .role(Role::Button)
            .aria_label(action.label)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(m.control_small())
            .rounded(m.radius_sm())
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |model, _, window: &mut Window, cx| {
                cx.stop_propagation();
                perform.run(model, id, cx);
                model.focus_active(window, cx);
            }))
            .child(
                svg()
                    .path(action.icon.path())
                    .size(m.icon_md())
                    .text_color(foreground.opacity(0.55)),
            ),
        tooltip,
        model,
    )
    .into_any_element()
}
