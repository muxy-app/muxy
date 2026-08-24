use crate::command::Command;
use crate::state::AppState;
use crate::views::window::MainWindow;
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement,
    Pixels, Point, SharedString, StatefulInteractiveElement, Styled, actions, div, px,
};
use muxy_ui::components::IconGlyph;
use muxy_ui::icon::Icon;

const MIN_WIDTH: f32 = 180.0;
const ROW_HEIGHT: f32 = 22.0;
const SUBMENU_INDEX_BASE: usize = 1000;
pub const KEY_CONTEXT: &str = "Menu";

actions!(
    menu,
    [
        DismissMenu,
        HighlightPrevious,
        HighlightNext,
        ConfirmHighlighted
    ]
);

pub fn key_bindings() -> Vec<gpui::KeyBinding> {
    let context = Some(KEY_CONTEXT);
    vec![
        gpui::KeyBinding::new("escape", DismissMenu, context),
        gpui::KeyBinding::new("up", HighlightPrevious, context),
        gpui::KeyBinding::new("down", HighlightNext, context),
        gpui::KeyBinding::new("enter", ConfirmHighlighted, context),
    ]
}

#[derive(Debug, Clone)]
pub enum Item {
    Separator,
    Action {
        label: SharedString,
        command: Command,
        disabled: bool,
        destructive: bool,
        checked: bool,
    },
    Submenu {
        label: SharedString,
        items: Vec<Item>,
        disabled: bool,
    },
}

impl Item {
    pub fn action(label: impl Into<SharedString>, command: Command) -> Self {
        Self::Action {
            label: label.into(),
            command,
            disabled: false,
            destructive: false,
            checked: false,
        }
    }

    pub fn label(label: impl Into<SharedString>) -> Self {
        Self::Action {
            label: label.into(),
            command: Command::DismissOverlay,
            disabled: true,
            destructive: false,
            checked: false,
        }
    }

    pub fn submenu(label: impl Into<SharedString>, items: Vec<Item>) -> Self {
        Self::Submenu {
            label: label.into(),
            items,
            disabled: false,
        }
    }

    pub fn disabled(mut self, value: bool) -> Self {
        match &mut self {
            Self::Action { disabled, .. } | Self::Submenu { disabled, .. } => *disabled = value,
            Self::Separator => {}
        }
        self
    }

    pub fn destructive(mut self) -> Self {
        if let Self::Action { destructive, .. } = &mut self {
            *destructive = true;
        }
        self
    }

    pub fn checked(mut self, value: bool) -> Self {
        if let Self::Action { checked, .. } = &mut self {
            *checked = value;
        }
        self
    }
}

#[derive(Debug, Clone)]
pub struct Menu {
    pub items: Vec<Item>,
    pub position: Point<Pixels>,
    pub open_submenu: Option<usize>,
    pub highlighted: Option<usize>,
}

impl Menu {
    pub fn selectable(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| match item {
                Item::Separator => false,
                Item::Action { disabled, .. } | Item::Submenu { disabled, .. } => !disabled,
            })
            .map(|(index, _)| index)
            .collect()
    }

    pub fn move_highlight(&mut self, delta: i32) {
        let selectable = self.selectable();
        if selectable.is_empty() {
            return;
        }
        let current = self
            .highlighted
            .and_then(|highlighted| selectable.iter().position(|index| *index == highlighted));
        let next = match current {
            None => {
                if delta > 0 {
                    0
                } else {
                    selectable.len() - 1
                }
            }
            Some(position) => {
                (position as i32 + delta).rem_euclid(selectable.len() as i32) as usize
            }
        };
        self.highlighted = Some(selectable[next]);
    }

    pub fn highlighted_command(&self) -> Option<Command> {
        match self.items.get(self.highlighted?) {
            Some(Item::Action {
                command, disabled, ..
            }) if !disabled => Some(command.clone()),
            _ => None,
        }
    }
}

impl Menu {
    pub fn new(items: Vec<Item>, position: Point<Pixels>) -> Self {
        Self {
            items,
            position,
            open_submenu: None,
            highlighted: None,
        }
    }
}

pub fn render(
    menu: &Menu,
    state: &AppState,
    focus: &gpui::FocusHandle,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    let mut panel = div()
        .key_context(KEY_CONTEXT)
        .track_focus(focus)
        .on_action(
            cx.listener(|window: &mut MainWindow, _: &DismissMenu, _, cx| {
                window.dismiss_overlay(cx);
            }),
        )
        .on_action(
            cx.listener(|window: &mut MainWindow, _: &HighlightPrevious, _, cx| {
                window.move_menu_highlight(-1, cx);
            }),
        )
        .on_action(
            cx.listener(|window: &mut MainWindow, _: &HighlightNext, _, cx| {
                window.move_menu_highlight(1, cx);
            }),
        )
        .on_action(cx.listener(
            |window: &mut MainWindow, _: &ConfirmHighlighted, view, cx| {
                window.confirm_menu_highlight(view, cx);
            },
        ))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .occlude()
        .absolute()
        .left(menu.position.x)
        .top(menu.position.y)
        .flex()
        .flex_col()
        .min_w(metrics.scaled(MIN_WIDTH))
        .py(metrics.spacing2())
        .rounded(metrics.radius_lg())
        .bg(theme.raised())
        .border_1()
        .border_color(theme.border)
        .shadow_lg();

    for (index, item) in menu.items.iter().enumerate() {
        panel = panel.child(row(index, item, menu, state, cx));
    }

    if let Some(index) = menu.open_submenu
        && let Some(Item::Submenu { items, .. }) = menu.items.get(index)
    {
        panel = panel.child(flyout(index, items, menu, state, cx));
    }

    panel.into_any_element()
}

fn row(
    index: usize,
    item: &Item,
    menu: &Menu,
    state: &AppState,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    match item {
        Item::Separator => div()
            .h(px(1.0))
            .my(metrics.spacing2())
            .bg(theme.border)
            .into_any_element(),
        Item::Action {
            label,
            command,
            disabled,
            destructive,
            checked,
        } => {
            let color = if *disabled {
                theme.fg_dim
            } else if *destructive {
                theme.danger
            } else {
                theme.fg
            };
            let command = command.clone();
            let disabled = *disabled;

            let group = SharedString::from(format!("menu-row-{index}"));
            let highlighted = menu.highlighted == Some(index) && !disabled;
            let mut row = base_row(index, state)
                .group(group.clone())
                .when(highlighted, |element| element.bg(theme.fg_alpha(0.1)))
                .child(check_mark(*checked, state))
                .child(
                    div()
                        .flex_grow()
                        .text_size(metrics.font_emphasis())
                        .text_color(color)
                        .child(label.clone()),
                );

            if disabled {
                return row.into_any_element();
            }

            row = row
                .cursor_pointer()
                .hover(|style| style.bg(theme.fg_alpha(0.1)))
                .on_click(cx.listener(move |window: &mut MainWindow, _, view, cx| {
                    window.perform(command.clone(), view, cx);
                }));
            row.into_any_element()
        }
        Item::Submenu {
            label,
            items: _,
            disabled,
        } => {
            let color = if *disabled { theme.fg_dim } else { theme.fg };

            let highlighted = menu.highlighted == Some(index) && !*disabled;
            let mut row = base_row(index, state)
                .when(highlighted, |element| element.bg(theme.fg_alpha(0.1)))
                .child(check_mark(false, state))
                .child(
                    div()
                        .flex_grow()
                        .text_size(metrics.font_emphasis())
                        .text_color(color)
                        .child(label.clone()),
                )
                .child(IconGlyph::new(Icon::ChevronRight, metrics.font_xs(), color));

            if *disabled {
                return row.into_any_element();
            }

            row = row
                .cursor_pointer()
                .hover(|style| style.bg(theme.fg_alpha(0.1)))
                .on_hover(
                    cx.listener(move |window: &mut MainWindow, hovered: &bool, _, cx| {
                        if *hovered {
                            window.open_submenu(Some(index));
                            cx.notify();
                        }
                    }),
                );

            row.into_any_element()
        }
    }
}

fn flyout(
    index: usize,
    items: &[Item],
    menu: &Menu,
    state: &AppState,
    cx: &mut Context<MainWindow>,
) -> AnyElement {
    let metrics = &state.metrics;
    let theme = &state.theme;

    let offset = menu
        .items
        .iter()
        .take(index)
        .fold(metrics.spacing2(), |total, item| {
            total + item_height(item, state)
        });

    let mut panel = div()
        .occlude()
        .absolute()
        .left_full()
        .top(offset - metrics.spacing2())
        .flex()
        .flex_col()
        .min_w(metrics.scaled(MIN_WIDTH))
        .py(metrics.spacing2())
        .rounded(metrics.radius_lg())
        .bg(theme.raised())
        .border_1()
        .border_color(theme.border)
        .shadow_lg();

    for (child_index, child) in items.iter().enumerate() {
        panel = panel.child(row(
            SUBMENU_INDEX_BASE + index * 100 + child_index,
            child,
            menu,
            state,
            cx,
        ));
    }

    panel.into_any_element()
}

pub fn item_height(item: &Item, state: &AppState) -> Pixels {
    match item {
        Item::Separator => px(1.0) + state.metrics.spacing2() * 2.0,
        _ => state.metrics.scaled(ROW_HEIGHT),
    }
}

fn base_row(index: usize, state: &AppState) -> gpui::Stateful<gpui::Div> {
    let metrics = &state.metrics;
    div()
        .id(ElementId::Name(SharedString::from(format!(
            "menu-item-{index}"
        ))))
        .flex()
        .flex_row()
        .items_center()
        .gap(metrics.spacing2())
        .h(metrics.scaled(ROW_HEIGHT))
        .px(metrics.spacing3())
        .mx(metrics.spacing2())
        .rounded(metrics.radius_sm())
        .font_weight(FontWeight::NORMAL)
}

fn check_mark(checked: bool, state: &AppState) -> AnyElement {
    let metrics = &state.metrics;
    let mut mark = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(metrics.scaled(12.0));
    if checked {
        mark = mark.child(muxy_ui::components::SymbolGlyph::new(
            "checkmark",
            metrics.font_caption(),
            state.theme.fg,
        ));
    }
    mark.into_any_element()
}
