use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use muxy_core::fold::fold;
use muxy_core::shortcuts::{CATEGORIES, COMMAND, CONTROL, KeyCombo, OPTION, SHIFT, ShortcutAction};
use muxy_ui::components::SymbolGlyph;
use muxy_ui::text_input::{self, InputEvent, InputStyle, TextInput};
use muxy_ui::theme::{Metrics, Theme};

const ROW_HEIGHT: f32 = 32.0;

pub enum ShortcutEditorEvent {
    Save {
        action: ShortcutAction,
        combo: KeyCombo,
    },
    ResetAll,
    Dismiss,
}

pub struct ShortcutEditor {
    bindings: Vec<(ShortcutAction, KeyCombo)>,
    armed: Option<ShortcutAction>,
    conflict: Option<(ShortcutAction, ShortcutAction)>,
    search: Entity<TextInput>,
    query: String,
    focus_handle: FocusHandle,
    theme: Theme,
    metrics: Metrics,
    scroll: gpui::ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<ShortcutEditorEvent> for ShortcutEditor {}

impl Focusable for ShortcutEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ShortcutEditor {
    pub fn new(
        bindings: Vec<(ShortcutAction, KeyCombo)>,
        query: String,
        theme: Theme,
        metrics: Metrics,
        cx: &mut Context<Self>,
    ) -> Self {
        let style = InputStyle::field(&theme, &metrics);
        let seeded = query.clone();
        let search = cx.new(|cx| {
            TextInput::new(style, cx)
                .with_key_context(text_input::BARE_CONTEXT)
                .with_placeholder("Search shortcuts")
                .with_text(seeded)
        });
        let subscription = cx.subscribe(&search, |editor: &mut Self, input, event, cx| {
            if matches!(event, InputEvent::Changed) {
                editor.query = input.read(cx).text().to_owned();
                cx.notify();
            }
        });

        Self {
            bindings,
            armed: None,
            conflict: None,
            search,
            query,
            focus_handle: cx.focus_handle(),
            theme,
            metrics,
            scroll: gpui::ScrollHandle::default(),
            _subscriptions: vec![subscription],
        }
    }

    pub fn set_appearance(&mut self, theme: Theme, metrics: Metrics, cx: &mut Context<Self>) {
        self.theme = theme;
        self.metrics = metrics;
        let style = InputStyle::field(&self.theme, &self.metrics);
        self.search.update(cx, |input, _| input.set_style(style));
        cx.notify();
    }

    pub fn handle_escape(&mut self, cx: &mut Context<Self>) -> bool {
        if self.armed.take().is_none() {
            return false;
        }
        self.conflict = None;
        cx.notify();
        true
    }

    pub fn reset_all(&mut self, cx: &mut Context<Self>) {
        self.armed = None;
        self.conflict = None;
        cx.emit(ShortcutEditorEvent::ResetAll);
        cx.notify();
    }

    pub fn apply(&mut self, bindings: Vec<(ShortcutAction, KeyCombo)>, cx: &mut Context<Self>) {
        self.bindings = bindings;
        cx.notify();
    }

    fn combo(&self, action: ShortcutAction) -> KeyCombo {
        self.bindings
            .iter()
            .find(|(candidate, _)| *candidate == action)
            .map(|(_, combo)| combo.clone())
            .unwrap_or_else(|| KeyCombo::new("", 0))
    }

    fn conflicting(&self, combo: &KeyCombo, excluding: ShortcutAction) -> Option<ShortcutAction> {
        if !combo.is_assigned() {
            return None;
        }
        self.bindings
            .iter()
            .find(|(action, existing)| *action != excluding && existing == combo)
            .map(|(action, _)| *action)
    }

    fn commit(&mut self, action: ShortcutAction, combo: KeyCombo, cx: &mut Context<Self>) {
        if let Some(other) = self.conflicting(&combo, action) {
            self.conflict = Some((action, other));
            cx.notify();
            return;
        }
        self.conflict = None;
        self.armed = None;
        if let Some(entry) = self
            .bindings
            .iter_mut()
            .find(|(candidate, _)| *candidate == action)
        {
            entry.1 = combo.clone();
        }
        cx.emit(ShortcutEditorEvent::Save { action, combo });
        cx.notify();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.armed else {
            if event.keystroke.key == "escape" {
                cx.emit(ShortcutEditorEvent::Dismiss);
            }
            return;
        };
        cx.stop_propagation();
        let key = event.keystroke.key.as_str();
        if key == "escape" {
            self.armed = None;
            self.conflict = None;
            cx.notify();
            return;
        }
        if matches!(key, "backspace" | "delete") {
            self.commit(action, KeyCombo::new("", 0), cx);
            return;
        }
        let Some(combo) = capture(event, true) else {
            return;
        };
        self.commit(action, combo, cx);
    }

    fn visible(&self, action: ShortcutAction) -> bool {
        let query = fold(self.query.trim());
        query.is_empty()
            || fold(action.display_name()).contains(&query)
            || fold(action.category()).contains(&query)
    }

    fn rows(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut list = div().flex().flex_col();
        for category in CATEGORIES {
            let actions: Vec<ShortcutAction> = self
                .bindings
                .iter()
                .map(|(action, _)| *action)
                .filter(|action| action.category() == category)
                .filter(|action| self.visible(*action))
                .collect();
            if actions.is_empty() {
                continue;
            }
            list = list.child(self.header(category));
            for action in actions {
                list = list.child(self.row(action, cx));
            }
        }
        div()
            .id("shortcut-editor-rows")
            .flex()
            .flex_col()
            .flex_grow()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(list)
            .into_any_element()
    }

    fn header(&self, title: &str) -> AnyElement {
        let metrics = &self.metrics;
        div()
            .flex()
            .flex_row()
            .items_center()
            .px(metrics.spacing6())
            .pt(metrics.spacing4())
            .pb(metrics.scaled(3.0))
            .child(
                div()
                    .text_size(metrics.font_caption())
                    .font_weight(FontWeight::BOLD)
                    .text_color(self.theme.fg_dim)
                    .child(SharedString::from(title.to_uppercase())),
            )
            .into_any_element()
    }

    fn row(&self, action: ShortcutAction, cx: &mut Context<Self>) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        let armed = self.armed == Some(action);
        let combo = self.combo(action);
        let conflict = self
            .conflict
            .filter(|(subject, _)| *subject == action)
            .map(|(_, other)| other);

        let value = if armed {
            "Press a shortcut…".to_owned()
        } else {
            combo.display()
        };

        let mut trailing = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(metrics.spacing3());
        if let Some(other) = conflict {
            trailing = trailing.child(
                div()
                    .text_size(metrics.font_footnote())
                    .text_color(theme.danger)
                    .child(SharedString::from(format!(
                        "Conflicts with \"{}\". Press a different shortcut or Esc to cancel.",
                        other.display_name()
                    ))),
            );
        }
        trailing = trailing
            .child(self.row_action(action, "unassign", "xmark", cx))
            .child(self.row_action(action, "reset", "arrow.counterclockwise", cx));
        trailing = trailing.child(
            div()
                .px(metrics.scaled(6.0))
                .py(metrics.scaled(2.0))
                .rounded(metrics.radius_sm())
                .bg(theme.surface)
                .border_1()
                .border_color(if armed { theme.accent } else { theme.border })
                .text_size(metrics.font_footnote())
                .text_color(if combo.is_assigned() {
                    theme.fg
                } else {
                    theme.fg_dim
                })
                .child(SharedString::from(value)),
        );

        div()
            .id(SharedString::from(format!(
                "shortcut-row-{}",
                action.display_name()
            )))
            .group("shortcut-row")
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .gap(metrics.spacing4())
            .px(metrics.spacing6())
            .h(metrics.scaled(ROW_HEIGHT))
            .flex_none()
            .cursor_pointer()
            .when(armed, |element| element.bg(theme.surface))
            .when(!armed, |element| {
                element.hover(|style| style.bg(theme.hover))
            })
            .on_click(
                cx.listener(move |editor: &mut Self, _, window: &mut Window, cx| {
                    editor.armed = Some(action);
                    editor.conflict = None;
                    window.focus(&editor.focus_handle);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .flex_grow()
                    .min_w(px(0.0))
                    .truncate()
                    .text_size(metrics.font_body())
                    .text_color(theme.fg)
                    .child(SharedString::from(action.display_name())),
            )
            .child(trailing)
            .into_any_element()
    }

    fn row_action(
        &self,
        action: ShortcutAction,
        kind: &'static str,
        symbol: &'static str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        div()
            .id(SharedString::from(format!(
                "shortcut-{kind}-{}",
                action.display_name()
            )))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(metrics.control_small())
            .rounded(metrics.radius_sm())
            .cursor_pointer()
            .invisible()
            .group_hover("shortcut-row", |style| style.visible())
            .hover(|style| style.bg(theme.hover))
            .child(SymbolGlyph::new(
                symbol,
                metrics.font_caption(),
                theme.fg_muted,
            ))
            .on_click(cx.listener(move |editor: &mut Self, _, _, cx| {
                let combo = if kind == "unassign" {
                    KeyCombo::new("", 0)
                } else {
                    muxy_core::shortcuts::default_combo(action)
                };
                editor.armed = None;
                editor.conflict = None;
                editor.commit(action, combo, cx);
            }))
            .into_any_element()
    }

    fn header_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let metrics = &self.metrics;
        let theme = &self.theme;
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(metrics.spacing4())
            .flex_none()
            .px(metrics.spacing6())
            .py(metrics.spacing4())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(metrics.spacing3())
                    .flex_grow()
                    .min_w(px(0.0))
                    .px(metrics.scaled(9.0))
                    .py(metrics.scaled(5.0))
                    .rounded(metrics.radius_lg())
                    .bg(theme.surface)
                    .child(SymbolGlyph::new(
                        "magnifyingglass",
                        metrics.font_body(),
                        theme.fg_muted,
                    ))
                    .child(muxy_ui::text_input::growing_input(&self.search)),
            )
            .child(
                div()
                    .id("shortcut-reset-all")
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(metrics.control_medium())
                    .px(metrics.spacing4())
                    .rounded(metrics.radius_sm())
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.hover))
                    .text_size(metrics.font_footnote())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.accent)
                    .child(SharedString::from("Reset All"))
                    .on_click(cx.listener(|editor: &mut Self, _, _, cx| editor.reset_all(cx))),
            )
            .into_any_element()
    }
}

impl Render for ShortcutEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .flex()
            .flex_col()
            .flex_grow()
            .min_h(px(0.0))
            .child(self.header_bar(cx))
            .child(div().h(px(1.0)).flex_none().bg(theme.border))
            .child(self.rows(cx))
    }
}

pub(crate) fn capture(event: &KeyDownEvent, requires_modifier: bool) -> Option<KeyCombo> {
    let modifiers = &event.keystroke.modifiers;
    let mut bits = 0u64;
    if modifiers.control {
        bits |= CONTROL;
    }
    if modifiers.alt {
        bits |= OPTION;
    }
    if modifiers.shift {
        bits |= SHIFT;
    }
    if modifiers.platform {
        bits |= COMMAND;
    }
    if requires_modifier && bits & (CONTROL | OPTION | COMMAND) == 0 {
        return None;
    }
    let key = match event.keystroke.key.as_str() {
        "left" => "leftarrow",
        "right" => "rightarrow",
        "up" => "uparrow",
        "down" => "downarrow",
        "enter" => "return",
        key if key.chars().count() == 1 || matches!(key, "tab" | "space") => key,
        _ => return None,
    };
    Some(KeyCombo::new(&key.to_lowercase(), bits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Keystroke, Modifiers};

    fn event(key: &str, modifiers: Modifiers) -> KeyDownEvent {
        KeyDownEvent {
            keystroke: Keystroke {
                modifiers,
                key: key.to_owned(),
                key_char: None,
            },
            is_held: false,
        }
    }

    #[test]
    fn capture_requires_a_command_control_or_option_modifier() {
        assert!(capture(&event("t", Modifiers::shift()), true).is_none());
        assert!(capture(&event("t", Modifiers::none()), true).is_none());
        assert_eq!(
            capture(&event("t", Modifiers::command()), true),
            Some(KeyCombo::new("t", COMMAND))
        );
    }

    #[test]
    fn capture_accepts_a_bare_letter_when_no_modifier_is_required() {
        assert_eq!(
            capture(&event("t", Modifiers::none()), false),
            Some(KeyCombo::new("t", 0))
        );
        assert_eq!(
            capture(&event("t", Modifiers::shift()), false),
            Some(KeyCombo::new("t", SHIFT))
        );
    }

    #[test]
    fn capture_maps_arrow_and_return_keys_to_the_swift_names() {
        assert_eq!(
            capture(&event("left", Modifiers::command()), true),
            Some(KeyCombo::new("leftarrow", COMMAND))
        );
        assert_eq!(
            capture(&event("enter", Modifiers::command()), true),
            Some(KeyCombo::new("return", COMMAND))
        );
        assert_eq!(
            capture(&event("tab", Modifiers::control()), true),
            Some(KeyCombo::new("tab", CONTROL))
        );
        assert!(capture(&event("f13", Modifiers::command()), true).is_none());
    }
}
