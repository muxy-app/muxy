use crate::components::{ButtonInteraction, SymbolGlyph};
use crate::picker::Picker;
use crate::text_input::TextInput;
use crate::theme::{Metrics, Theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    AccessibleAction, AnyElement, App, Bounds, ClickEvent, Corner, Entity, Focusable, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels, Point,
    Role, SharedString, StatefulInteractiveElement, Styled, Toggled, Window, anchored, canvas,
    deferred, div, point, px,
};
use std::cell::Cell;
use std::rc::Rc;

pub const CONTROL_WIDTH: f32 = 210.0;
pub const SLIDER_WIDTH: f32 = 220.0;

#[derive(Clone, Copy, Debug)]
pub struct Style<'a> {
    pub theme: &'a Theme,
    pub metrics: &'a Metrics,
}

#[derive(Clone, Debug)]
pub struct Choice {
    pub value: String,
    pub label: String,
    pub enabled: bool,
}

impl Choice {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            enabled: true,
        }
    }
}

pub fn toggle(
    style: Style,
    id: &str,
    name: &str,
    value: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics } = style;
    let knob = div()
        .size(metrics.scaled(12.0))
        .flex_none()
        .rounded_full()
        .bg(if value {
            theme.accent_foreground
        } else {
            theme.fg_muted
        });

    div()
        .id(SharedString::from(format!("settings-toggle-{id}")))
        .role(Role::Switch)
        .accessibility_id(format!("settings-toggle-{id}"))
        .aria_label(name.to_owned())
        .aria_toggled(toggled(value))
        .debug_selector({
            let selector = format!("settings-toggle-{id}");
            move || selector.clone()
        })
        .flex()
        .flex_row()
        .items_center()
        .flex_none()
        .w(metrics.scaled(28.0))
        .h(metrics.scaled(16.0))
        .px(metrics.scaled(2.0))
        .rounded_full()
        .cursor_pointer()
        .bg(if value { theme.accent } else { theme.surface })
        .border_1()
        .border_color(if value { theme.accent } else { theme.border })
        .when(value, Styled::justify_end)
        .child(knob)
        .focus(|focus| focus.border_color(theme.accent))
        .button_interaction(on_click)
        .into_any_element()
}

fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}

/// A box ticked when `checked`. `mixed` shows a dash instead, for a box that
/// stands for several where only some are ticked.
pub fn checkbox(
    style: Style,
    id: &str,
    name: &str,
    checked: bool,
    mixed: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics } = style;
    let on = checked || mixed;
    div()
        .id(SharedString::from(format!("checkbox-{id}")))
        .role(Role::CheckBox)
        .accessibility_id(format!("checkbox-{id}"))
        .aria_label(name.to_owned())
        .aria_toggled(if mixed && !checked {
            Toggled::Mixed
        } else {
            toggled(checked)
        })
        .debug_selector({
            let selector = format!("checkbox-{id}");
            move || selector.clone()
        })
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(metrics.scaled(16.0))
        .rounded(metrics.radius_sm())
        .cursor_pointer()
        .bg(if on { theme.accent } else { theme.surface })
        .border_1()
        .border_color(if on {
            theme.accent
        } else {
            theme.border_solid()
        })
        .when(checked, |checkbox| {
            checkbox.child(crate::components::IconGlyph::new(
                crate::icon::Icon::Check,
                metrics.scaled(12.0),
                theme.accent_foreground,
            ))
        })
        .when(mixed && !checked, |checkbox| {
            checkbox.child(
                div()
                    .w(metrics.scaled(8.0))
                    .h(metrics.scaled(2.0))
                    .rounded_full()
                    .bg(theme.accent_foreground),
            )
        })
        .focus(|focus| focus.border_color(theme.accent))
        // Like a macOS checkbox, a click ticks it without focusing it; Tab
        // still does.
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .button_interaction(on_click)
        .into_any_element()
}

/// How a button reads: the main action, an ordinary one, or one that destroys.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ButtonKind {
    Primary,
    #[default]
    Secondary,
    Danger,
}

/// What a [`button`] shows and whether it can be pressed.
#[derive(Clone, Copy, Debug)]
pub struct Button<'a> {
    kind: ButtonKind,
    label: &'a str,
    name: Option<&'a str>,
    icon: Option<&'static str>,
    enabled: bool,
}

impl<'a> Button<'a> {
    pub fn primary(label: &'a str) -> Self {
        Self::new(ButtonKind::Primary, label)
    }

    pub fn secondary(label: &'a str) -> Self {
        Self::new(ButtonKind::Secondary, label)
    }

    pub fn danger(label: &'a str) -> Self {
        Self::new(ButtonKind::Danger, label)
    }

    fn new(kind: ButtonKind, label: &'a str) -> Self {
        Self {
            kind,
            label,
            name: None,
            icon: None,
            enabled: true,
        }
    }

    /// What accessibility clients read when it isn't the label, such as for a
    /// button that only shows an icon.
    #[must_use]
    pub fn named(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    /// An SF Symbol shown before the label.
    #[must_use]
    pub fn icon(mut self, symbol: &'static str) -> Self {
        self.icon = Some(symbol);
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// A button that the pointer and the keyboard (Tab, then Space or Return) can
/// press, dimmed and inert while disabled.
pub fn button(
    style: Style,
    id: &str,
    button: Button,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let Style { theme, metrics } = style;
    let (background, border, foreground, focus) = match button.kind {
        ButtonKind::Primary => (
            theme.accent,
            theme.accent,
            theme.accent_foreground,
            theme.fg,
        ),
        ButtonKind::Secondary => (theme.surface, theme.border, theme.fg, theme.accent),
        ButtonKind::Danger => (theme.surface, theme.border, theme.danger, theme.danger),
    };
    div()
        .id(SharedString::from(format!("settings-button-{id}")))
        .role(Role::Button)
        .accessibility_id(format!("settings-button-{id}"))
        .aria_label(button.name.unwrap_or(button.label).to_owned())
        .aria_disabled(!button.enabled)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(metrics.spacing3())
        .min_w(px(0.0))
        .h(metrics.control_medium())
        .px(metrics.spacing5())
        .rounded(metrics.radius_sm())
        .bg(background)
        .border_1()
        .border_color(border)
        .text_size(metrics.font_footnote())
        .font_weight(FontWeight::MEDIUM)
        .text_color(foreground)
        .when(!button.enabled, |element| element.opacity(0.4))
        .when(button.enabled, |element| {
            element
                .cursor_pointer()
                .map(|element| match button.kind {
                    ButtonKind::Primary => element.hover(|hover| hover.opacity(0.85)),
                    ButtonKind::Secondary | ButtonKind::Danger => {
                        element.hover(|hover| hover.bg(theme.hover))
                    }
                })
                .focus(|style| style.border_color(focus))
                .button_interaction(on_click)
        })
        .when_some(button.icon, |element, symbol| {
            element.child(SymbolGlyph::new(
                symbol,
                metrics.font_footnote(),
                foreground,
            ))
        })
        .when(!button.label.is_empty(), |element| {
            element.child(
                div()
                    .min_w(px(0.0))
                    .truncate()
                    .child(SharedString::from(button.label.to_owned())),
            )
        })
}

pub fn picker(
    style: Style,
    id: &str,
    name: &str,
    choices: &[Choice],
    selected: &str,
    popover: Option<Entity<Picker>>,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let metrics = style.metrics;
    let label = choices
        .iter()
        .find(|choice| choice.value == selected)
        .map(|choice| choice.label.clone())
        .unwrap_or_default();
    let field = picker_trigger(
        style,
        id,
        name,
        &label,
        Some(CONTROL_WIDTH),
        popover.is_some(),
        on_toggle,
    );

    let Some(popover) = popover else {
        return div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .child(field)
            .into_any_element();
    };

    div()
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .child(field)
        .child(
            deferred(
                anchored()
                    .anchor(Corner::TopLeft)
                    .offset(point(
                        px(0.0),
                        metrics.control_medium() + metrics.spacing1(),
                    ))
                    .snap_to_window_with_margin(px(8.0))
                    .child(popover),
            )
            .with_priority(1),
        )
        .into_any_element()
}

pub fn picker_trigger(
    style: Style,
    id: &str,
    name: &str,
    label: &str,
    width: Option<f32>,
    open: bool,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics } = style;
    crate::accessibility::value(
        div()
            .id(SharedString::from(format!("settings-picker-{id}")))
            .role(Role::ComboBox)
            .accessibility_id(format!("settings-picker-{id}"))
            .aria_label(name.to_owned())
            .aria_expanded(open),
        label.to_owned(),
    )
    .flex()
    .flex_row()
    .items_center()
    .justify_between()
    .gap(metrics.spacing3())
    .map(|field| {
        if let Some(width) = width {
            field.w(metrics.scaled(width))
        } else {
            field.w_full()
        }
    })
    .h(metrics.control_medium())
    .px(metrics.spacing4())
    .rounded(metrics.radius_sm())
    .cursor_pointer()
    .bg(theme.surface)
    .border_1()
    .border_color(if open { theme.accent } else { theme.border })
    .hover(|hover| hover.bg(theme.hover))
    .child(
        div()
            .flex_grow()
            .min_w(px(0.0))
            .truncate()
            .text_size(metrics.font_footnote())
            .text_color(theme.fg)
            .child(SharedString::from(label.to_owned())),
    )
    .child(SymbolGlyph::new(
        "chevron.up.chevron.down",
        metrics.font_caption(),
        theme.fg_muted,
    ))
    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
    .focus(|focus| focus.border_color(theme.accent))
    .button_interaction(on_toggle)
    .into_any_element()
}

pub fn segmented(
    style: Style,
    id: &str,
    name: &str,
    choices: &[Choice],
    selected: &str,
    on_select: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics } = style;
    let handler = Rc::new(on_select);
    let mut group = div()
        .id(SharedString::from(format!("settings-segmented-{id}")))
        .role(Role::RadioGroup)
        .accessibility_id(format!("settings-segmented-{id}"))
        .aria_label(name.to_owned())
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(1.0))
        .p(px(1.0))
        .rounded(metrics.radius_sm())
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border);

    for choice in choices {
        let is_selected = choice.value == selected;
        let handler = handler.clone();
        let value = SharedString::from(choice.value.clone());
        group = group.child(
            div()
                .id(SharedString::from(format!(
                    "settings-segment-{id}-{}",
                    choice.value
                )))
                .role(Role::RadioButton)
                .accessibility_id(format!("settings-segment-{id}-{}", choice.value))
                .aria_label(choice.label.clone())
                .aria_toggled(toggled(is_selected))
                .aria_disabled(!choice.enabled)
                .debug_selector({
                    let selector = format!("settings-segment-{id}-{}", choice.value);
                    move || selector.clone()
                })
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .h(metrics.control_medium() - px(4.0))
                .px(metrics.spacing4())
                .rounded(metrics.radius_sm())
                .text_size(metrics.font_footnote())
                .font_weight(if is_selected {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if is_selected {
                    theme.accent_foreground
                } else {
                    theme.fg
                })
                .when(is_selected, |element| element.bg(theme.accent))
                .when(!choice.enabled, |element| element.opacity(0.4))
                .when(choice.enabled && !is_selected, |element| {
                    element
                        .cursor_pointer()
                        .hover(|hover| hover.bg(theme.hover))
                })
                .when(choice.enabled, |element| {
                    element
                        .focus(|focus| {
                            focus.bg(if is_selected {
                                theme.accent
                            } else {
                                theme.hover
                            })
                        })
                        .button_interaction(move |_, window, cx| handler(&value, window, cx))
                })
                .child(SharedString::from(choice.label.clone())),
        );
    }

    group.into_any_element()
}

#[derive(Debug)]
pub struct Grab {
    pub bounds: Bounds<Pixels>,
    pub position: Point<Pixels>,
}

pub fn fraction_at(bounds: Bounds<Pixels>, position: Point<Pixels>) -> f32 {
    let width = f32::from(bounds.size.width);
    if width <= 0.0 {
        return 0.0;
    }
    (f32::from(position.x - bounds.origin.x) / width).clamp(0.0, 1.0)
}

/// How far a client's step moves a [`slider`], as a fraction of its range.
const SLIDER_STEP: f32 = 0.05;

pub fn slider(
    style: Style,
    id: &str,
    name: &str,
    value: f32,
    range: (f32, f32),
    on_grab: impl Fn(&Grab, &mut Window, &mut App) + 'static,
    on_step: impl Fn(&f32, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let Style { theme, metrics } = style;
    let span = (range.1 - range.0).max(f32::EPSILON);
    let fraction = ((value - range.0) / span).clamp(0.0, 1.0);
    let track: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
    let recorder = track.clone();
    let width = metrics.scaled(SLIDER_WIDTH);
    let knob = metrics.scaled(12.0);
    let rail = metrics.scaled(4.0);
    let on_step = Rc::new(on_step);
    let on_decrement = on_step.clone();
    let step = span * SLIDER_STEP;
    let (min, max) = range;

    div()
        .id(SharedString::from(format!("settings-slider-{id}")))
        .role(Role::Slider)
        .accessibility_id(format!("settings-slider-{id}"))
        .aria_label(name.to_owned())
        .aria_numeric_value(f64::from(value))
        .aria_min_numeric_value(f64::from(min))
        .aria_max_numeric_value(f64::from(max))
        .on_a11y_action(AccessibleAction::Increment, move |_, window, cx| {
            on_step(&(value + step).min(max), window, cx);
        })
        .on_a11y_action(AccessibleAction::Decrement, move |_, window, cx| {
            on_decrement(&(value - step).max(min), window, cx);
        })
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .w(width)
        .h(metrics.control_medium())
        .cursor_pointer()
        .child(
            canvas(
                move |bounds, _, _| recorder.set(Some(bounds)),
                |_, (): (), _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .w_full()
                .h(rail)
                .rounded_full()
                .bg(theme.border),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .w(width * fraction)
                .h(rail)
                .rounded_full()
                .bg(theme.accent),
        )
        .child(
            div()
                .absolute()
                .left((width - knob) * fraction)
                .size(knob)
                .rounded_full()
                .bg(theme.accent_foreground)
                .border_1()
                .border_color(theme.border),
        )
        .on_mouse_down(
            MouseButton::Left,
            move |event: &MouseDownEvent, window, cx| {
                let Some(bounds) = track.get() else {
                    return;
                };
                on_grab(
                    &Grab {
                        bounds,
                        position: event.position,
                    },
                    window,
                    cx,
                );
            },
        )
        .into_any_element()
}

pub fn text_area(
    style: Style,
    id: &str,
    input: &Entity<TextInput>,
    content_height: Option<f32>,
) -> AnyElement {
    let Style { theme, metrics } = style;
    div()
        .id(SharedString::from(format!("settings-area-{id}")))
        .flex()
        .flex_col()
        .min_h(px(0.0))
        .p(metrics.spacing4())
        .rounded(metrics.radius_sm())
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .map(|element| match content_height {
            Some(height) => element
                .flex_none()
                .h(metrics.scaled(height) + metrics.spacing4() * 2.0 + px(2.0)),
            None => element.flex_grow(),
        })
        .child(input.clone())
        .into_any_element()
}

pub fn text_field(
    style: Style,
    id: &str,
    input: &Entity<TextInput>,
    width: Option<f32>,
) -> AnyElement {
    let Style { theme, metrics } = style;
    div()
        .id(SharedString::from(format!("settings-field-{id}")))
        .debug_selector({
            let selector = format!("settings-field-{id}");
            move || selector.clone()
        })
        .flex()
        .flex_row()
        .items_center()
        .h(metrics.control_medium())
        .min_h(metrics.control_medium())
        .max_h(metrics.control_medium())
        .px(metrics.spacing4())
        .rounded(metrics.radius_sm())
        .bg(theme.surface)
        .border_1()
        .border_color(theme.border)
        .map(|element| match width {
            Some(width) => element.flex_none().w(metrics.scaled(width)),
            None => element.flex_grow().min_w(px(0.0)),
        })
        .on_mouse_down_out({
            let input = input.clone();
            move |_, window, cx| {
                // After the press, so whatever it landed on still sees the keyboard
                // where it was, and can take it from this field.
                let focus = input.focus_handle(cx);
                window.defer(cx, move |window, _| {
                    if focus.is_focused(window) {
                        window.blur();
                    }
                });
            }
        })
        .child(crate::text_input::growing_input(input))
        .into_any_element()
}

pub fn search_field(
    style: Style,
    id: impl Into<gpui::ElementId>,
    input: &Entity<TextInput>,
) -> gpui::Stateful<gpui::Div> {
    let Style { theme, metrics } = style;
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(metrics.spacing4())
        .h(metrics.spacing10())
        .px(metrics.scaled(9.0))
        .bg(theme.bg)
        .border_1()
        .border_color(theme.border_solid())
        .rounded(metrics.scaled(5.0))
        .child(SymbolGlyph::new(
            "magnifyingglass",
            metrics.icon_md(),
            theme.fg_muted,
        ))
        .child(crate::text_input::growing_input(input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ActivateButton;
    use crate::theme::ColorScheme;
    use gpui::{Context, Render, TestAppContext};
    use std::cell::RefCell;

    struct Buttons {
        theme: Theme,
        enabled: bool,
        pressed: Rc<RefCell<Vec<ButtonKind>>>,
    }

    impl Render for Buttons {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let metrics = Metrics::new(1.0);
            let style = Style {
                theme: &self.theme,
                metrics: &metrics,
            };
            let kinds = [
                Button::primary("Save"),
                Button::secondary("Cancel"),
                Button::danger("Delete"),
            ];
            div().children(kinds.into_iter().map(|kind| {
                let pressed = self.pressed.clone();
                let label = kind.label;
                let kind = kind.enabled(self.enabled);
                button(style, label, kind, move |_, _, _| {
                    pressed.borrow_mut().push(kind.kind);
                })
            }))
        }
    }

    fn press_each_from_the_keyboard(enabled: bool, cx: &mut TestAppContext) -> Vec<ButtonKind> {
        let pressed = Rc::new(RefCell::new(Vec::new()));
        let buttons = pressed.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Buttons {
            theme: Theme::from_scheme(&ColorScheme::default()),
            enabled,
            pressed: buttons,
        });
        cx.run_until_parked();
        for _ in 0..3 {
            cx.update(|window, _| window.focus_next());
            cx.dispatch_action(ActivateButton);
        }
        pressed.take()
    }

    fn node_of(element: &impl gpui::Element) -> (Option<Role>, gpui::accesskit::Node) {
        let mut node = gpui::accesskit::Node::new(Role::Unknown);
        element.write_a11y_info(&mut node);
        (element.a11y_role(), node)
    }

    #[gpui::test]
    fn a_button_publishes_its_label_or_the_name_given_for_an_icon(cx: &mut TestAppContext) {
        cx.update(|_| {
            let theme = Theme::from_scheme(&ColorScheme::default());
            let metrics = Metrics::new(1.0);
            let style = Style {
                theme: &theme,
                metrics: &metrics,
            };
            let labelled = button(style, "save", Button::primary("Save"), |_, _, _| {});
            let (role, node) = node_of(&labelled);
            assert_eq!(role, Some(Role::Button));
            assert_eq!(node.label(), Some("Save"));
            assert_eq!(node.author_id(), Some("settings-button-save"));
            assert!(!node.is_disabled());

            let icon = button(
                style,
                "add",
                Button::secondary("").named("Add").enabled(false),
                |_, _, _| {},
            );
            let (_, node) = node_of(&icon);
            assert_eq!(node.label(), Some("Add"));
            assert!(node.is_disabled());
        });
    }

    #[test]
    fn toggled_follows_the_value() {
        assert_eq!(toggled(true), Toggled::True);
        assert_eq!(toggled(false), Toggled::False);
    }

    #[gpui::test]
    fn every_kind_of_button_is_pressed_from_the_keyboard(cx: &mut TestAppContext) {
        assert_eq!(
            press_each_from_the_keyboard(true, cx),
            [
                ButtonKind::Primary,
                ButtonKind::Secondary,
                ButtonKind::Danger
            ]
        );
    }

    #[gpui::test]
    fn a_disabled_button_cannot_be_pressed(cx: &mut TestAppContext) {
        assert!(press_each_from_the_keyboard(false, cx).is_empty());
    }
}
