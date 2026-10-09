use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Bounds, Context, FontWeight, InteractiveElement, IntoElement, MouseMoveEvent,
    ParentElement, Pixels, Point, SharedString, Size, StatefulInteractiveElement, Styled, Window,
    div, point, px,
};
use muxy_ui::components::SymbolGlyph;
use muxy_ui::popover;
use muxy_ui::theme::Metrics;

use super::{
    CloseSubmenu, Command, ConfirmHighlighted, DismissMenu, HighlightNext, HighlightPrevious, Item,
    Level, Menu, OpenSubmenu,
};
use crate::model::AppModel;
use crate::views::overlays::clamp;

const CHECKMARK: f32 = 12.0;
const SWATCH: f32 = 10.0;
const CHEVRON: f32 = 10.0;

pub(crate) fn render(
    menu: &Menu,
    model: &AppModel,
    window: &Window,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let viewport = window.viewport_size();
    let mut layer = div()
        .absolute()
        .top_0()
        .left_0()
        .key_context("Menu")
        .track_focus(&model.overlay_focus)
        .on_action(cx.listener(|model, _: &DismissMenu, _, cx| model.dismiss_menu_level(cx)))
        .on_action(cx.listener(|model, _: &HighlightPrevious, _, cx| {
            model.update_menu(cx, |menu| menu.move_highlight(false));
        }))
        .on_action(cx.listener(|model, _: &HighlightNext, _, cx| {
            model.update_menu(cx, |menu| menu.move_highlight(true));
        }))
        .on_action(
            cx.listener(|model, _: &ConfirmHighlighted, window, cx| model.confirm_menu(window, cx)),
        )
        .on_action(cx.listener(|model, _: &OpenSubmenu, _, cx| {
            model.update_menu(cx, Menu::open_submenu);
        }))
        .on_action(cx.listener(|model, _: &CloseSubmenu, _, cx| {
            model.update_menu(cx, Menu::close_submenu);
        }));
    let mut panels: Vec<Bounds<Pixels>> = Vec::new();
    let mut items = menu.items.as_slice();
    let mut opener: Option<Pixels> = None;
    for (level, state) in menu.levels.iter().enumerate() {
        let shortcuts: Vec<_> = items
            .iter()
            .map(|item| {
                item.command()
                    .and_then(|command| command.shortcut(model, cx))
            })
            .collect();
        let size = dimensions(items, &shortcuts, level == 0, model, window);
        let origin = match (panels.last(), opener) {
            (Some(parent), Some(row)) => submenu_origin(*parent, row, size, viewport, m),
            _ => clamp(menu.position, size, viewport),
        };
        let bounds = Bounds::new(origin, size);
        panels.push(bounds);
        layer = layer.child(panel(level, items, state, &shortcuts, bounds, model, cx));
        let Some((index, children)) = state.highlighted.and_then(|index| {
            items
                .get(index)
                .and_then(Item::submenu_items)
                .map(|children| (index, children))
        }) else {
            break;
        };
        opener = Some(
            bounds.top()
                + px(1.0)
                + m.scaled(popover::PADDING)
                + row_offset(items, index, m)
                + state.scroll.offset().y,
        );
        items = children;
    }
    for bounds in &panels {
        layer = layer.child(
            div()
                .absolute()
                .left(bounds.origin.x)
                .top(bounds.origin.y)
                .w(bounds.size.width)
                .h(bounds.size.height)
                .child(model.webview_occlusion()),
        );
    }
    *menu.panels.borrow_mut() = panels;
    layer.into_any_element()
}

fn panel(
    level: usize,
    items: &[Item],
    state: &Level,
    shortcuts: &[Option<String>],
    bounds: Bounds<Pixels>,
    model: &AppModel,
    cx: &mut Context<AppModel>,
) -> AnyElement {
    let m = model.metrics;
    let theme = &model.theme;
    let name = if level == 0 {
        "context-menu".to_owned()
    } else {
        format!("context-submenu-{level}")
    };
    let columns = Columns::of(items);
    let mut panel = popover::surface(theme, m)
        .id(SharedString::from(name.clone()))
        .debug_selector(move || name)
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_move(cx.listener(|model, event: &MouseMoveEvent, _, _| {
            model.track_menu_pointer(event.position);
        }))
        .absolute()
        .left(bounds.origin.x)
        .top(bounds.origin.y)
        .w(bounds.size.width)
        .h(bounds.size.height)
        .overflow_y_scroll()
        .track_scroll(&state.scroll);
    for (index, item) in items.iter().enumerate() {
        if item.separator_before {
            panel = panel.child(
                div()
                    .flex()
                    .flex_none()
                    .flex_col()
                    .child(popover::divider(theme, m)),
            );
        }
        panel = panel.child(item.render(
            level,
            index,
            state.highlighted == Some(index),
            shortcuts[index].clone(),
            columns,
            model,
            cx,
        ));
    }
    panel.into_any_element()
}

impl Item {
    #[allow(clippy::too_many_arguments, reason = "One menu row and its placement")]
    fn render(
        &self,
        level: usize,
        index: usize,
        highlighted: bool,
        shortcut: Option<String>,
        columns: Columns,
        model: &AppModel,
        cx: &Context<AppModel>,
    ) -> AnyElement {
        let m = model.metrics;
        let theme = &model.theme;
        let enabled = !self.disabled;
        let name = if level == 0 {
            format!("menu-item-{index}")
        } else {
            format!("menu-item-{level}-{index}")
        };
        popover::menu_row(
            theme,
            m,
            SharedString::from(name.clone()),
            enabled,
            highlighted,
        )
        .debug_selector(move || name)
        .font_weight(FontWeight::NORMAL)
        .when(columns.checkmarks, |row| {
            row.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(m.scaled(CHECKMARK))
                    .when(self.checked == Some(true), |mark| {
                        mark.child(self.part("check").flex().child(SymbolGlyph::new(
                            "checkmark",
                            m.font_caption(),
                            theme.fg,
                        )))
                    }),
            )
        })
        .when(columns.swatches, |row| {
            row.child(
                self.part("swatch")
                    .flex_none()
                    .size(m.scaled(SWATCH))
                    .rounded_full()
                    .when_some(self.swatch, Styled::bg),
            )
        })
        .child(
            self.part("label")
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .child(self.label.clone()),
        )
        .when_some(shortcut, |row, shortcut| {
            row.child(
                self.part("shortcut")
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(m.font_footnote())
                    .text_color(if enabled {
                        theme.fg_muted
                    } else {
                        theme.fg_dim
                    })
                    .child(shortcut),
            )
        })
        .when(self.submenu_items().is_some(), |row| {
            row.child(self.chevron(highlighted, model))
        })
        .on_mouse_move(cx.listener(move |model, event: &MouseMoveEvent, _, cx| {
            model.hover_menu_row(level, index, event.position, cx);
        }))
        .on_hover(cx.listener(move |model, hovered: &bool, _, cx| {
            if !*hovered {
                model.update_menu(cx, |menu| menu.leave(level, index));
            }
        }))
        .when(enabled, |row| {
            row.on_click(cx.listener(move |model, _, window, cx| {
                model.click_menu_row(level, index, window, cx);
            }))
        })
        .into_any_element()
    }

    /// A piece of the row, found in tests as `menu-{part}-{label}`.
    fn part(&self, part: &'static str) -> gpui::Div {
        let label = self.label.clone();
        div().debug_selector(move || format!("menu-{part}-{label}"))
    }

    fn chevron(&self, highlighted: bool, model: &AppModel) -> gpui::Div {
        let theme = &model.theme;
        let color = match (self.disabled, highlighted) {
            (true, _) => theme.fg_dim,
            (false, true) => theme.fg,
            (false, false) => theme.fg_muted,
        };
        self.part("chevron")
            .flex()
            .flex_none()
            .items_center()
            .justify_end()
            .w(model.metrics.scaled(CHEVRON))
            .child(SymbolGlyph::new("chevron.right", model.metrics.font_xs(), color).bold())
    }
}

/// Leading columns a panel reserves on every row, so labels line up.
#[derive(Clone, Copy)]
struct Columns {
    checkmarks: bool,
    swatches: bool,
}

impl Columns {
    fn of(items: &[Item]) -> Self {
        Self {
            checkmarks: items.iter().any(|item| item.checked.is_some()),
            swatches: items.iter().any(|item| item.swatch.is_some()),
        }
    }
}

fn dimensions(
    items: &[Item],
    shortcuts: &[Option<String>],
    root: bool,
    model: &AppModel,
    window: &Window,
) -> Size<Pixels> {
    let m = model.metrics;
    let tab_menu = root
        && items
            .iter()
            .any(|item| matches!(item.command(), Some(Command::Tab(..))));
    let mut width = m.scaled(if tab_menu { 230.0 } else { 180.0 });
    let mut style = window.text_style();
    style.font_weight = FontWeight::NORMAL;
    let measure = |text: &str, font_size| {
        window
            .text_system()
            .shape_line(
                text.to_owned().into(),
                font_size,
                &[style.to_run(text.len())],
                None,
            )
            .width
    };
    let columns = Columns::of(items);
    let mut leading = px(0.0);
    if columns.checkmarks {
        leading += m.scaled(CHECKMARK + popover::ROW_PADDING);
    }
    if columns.swatches {
        leading += m.scaled(SWATCH + popover::ROW_PADDING);
    }
    for (item, shortcut) in items.iter().zip(shortcuts) {
        let mut row_width = measure(&item.label, m.font_body())
            + leading
            + m.scaled(popover::ROW_PADDING) * 2.0
            + m.scaled(popover::PADDING) * 2.0
            + px(2.0);
        if let Some(shortcut) = shortcut {
            row_width += m.scaled(popover::ROW_PADDING) + measure(shortcut, m.font_footnote());
        }
        if item.submenu_items().is_some() {
            row_width += m.scaled(CHEVRON + popover::ROW_PADDING);
        }
        width = width.max(row_width);
    }
    let viewport = window.viewport_size();
    gpui::size(
        width.min((viewport.width - px(16.0)).max(px(0.0))),
        height(items, m).min((viewport.height - px(16.0)).max(px(0.0))),
    )
}

fn count(value: usize) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

fn separator_height(m: Metrics) -> Pixels {
    px(1.0) + m.spacing2() * 2.0
}

pub(super) fn height(items: &[Item], m: Metrics) -> Pixels {
    let rows = count(items.len());
    let separators = count(items.iter().filter(|item| item.separator_before).count());
    m.scaled(popover::PADDING) * 2.0
        + m.scaled(popover::ROW_HEIGHT) * rows
        + separator_height(m) * separators
        + m.scaled(popover::ROW_GAP) * (rows + separators - 1.0).max(0.0)
        + px(2.0)
}

/// How far row `index` sits below a panel's first row.
fn row_offset(items: &[Item], index: usize, m: Metrics) -> Pixels {
    let separators = count(
        items
            .iter()
            .take(index + 1)
            .filter(|item| item.separator_before)
            .count(),
    );
    (m.scaled(popover::ROW_HEIGHT) + m.scaled(popover::ROW_GAP)) * count(index)
        + (separator_height(m) + m.scaled(popover::ROW_GAP)) * separators
}

/// Places a submenu beside its parent with its first row level with the row
/// that opened it, flipping to the left when the right side has no room.
pub(super) fn submenu_origin(
    parent: Bounds<Pixels>,
    row_top: Pixels,
    size: Size<Pixels>,
    viewport: Size<Pixels>,
    m: Metrics,
) -> Point<Pixels> {
    let overlap = m.scaled(popover::PADDING);
    let right = parent.right() - overlap;
    let x = if right + size.width <= viewport.width - px(8.0) {
        right
    } else {
        parent.left() + overlap - size.width
    };
    clamp(
        point(x, row_top - m.scaled(popover::PADDING) - px(1.0)),
        size,
        viewport,
    )
}
