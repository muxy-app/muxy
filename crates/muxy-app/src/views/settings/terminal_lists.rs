use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Context, Focusable, IntoElement, ParentElement, Styled, Window, div, px};
use muxy_app_core::settings::{FontMap, KeyChord, TerminalAction, TerminalEdit};
use muxy_ui::l10n::{tr_key, translate};
use muxy_ui::{controls, form, tr};

use super::pickers::FontSlot;
use super::terminal::change_button;
use super::{Change, PickerKind, SettingsEvent, SettingsView};

pub(super) const BINDING_ACTION: &str = "keybind-action";
pub(super) const BINDING_CHORD: &str = "keybind-chord";
const BINDING_PARAMETER: &str = "keybind-parameter";
const CODEPOINT_RANGE: &str = "codepoint-range";
pub(super) const EDITOR_FIELDS: [&str; 2] = [CODEPOINT_RANGE, BINDING_PARAMETER];

const ACTIONS: [(&str, &str); 17] = [
    ("text", tr_key!("Send text")),
    ("esc", tr_key!("Send Escape sequence")),
    ("csi", tr_key!("Send CSI sequence")),
    ("copy_to_clipboard", tr_key!("Copy")),
    ("paste_from_clipboard", tr_key!("Paste")),
    ("select_all", tr_key!("Select all")),
    ("clear_screen", tr_key!("Clear screen")),
    ("scroll_to_top", tr_key!("Scroll to top")),
    ("scroll_to_bottom", tr_key!("Scroll to bottom")),
    ("scroll_page_up", tr_key!("Scroll page up")),
    ("scroll_page_down", tr_key!("Scroll page down")),
    ("increase_font_size", tr_key!("Increase font size")),
    ("decrease_font_size", tr_key!("Decrease font size")),
    ("reset_font_size", tr_key!("Reset font size")),
    ("reload_config", tr_key!("Reload terminal settings")),
    ("ignore", tr_key!("Ignore key")),
    ("unbind", tr_key!("Remove built-in binding")),
];

pub(super) struct CodepointEditor {
    previous: Option<FontMap>,
    family: String,
}

impl CodepointEditor {
    pub(super) fn family(&self) -> &String {
        &self.family
    }
}

pub(super) struct BindingEditor {
    previous: Option<KeyChord>,
    chord: Option<KeyChord>,
    action: &'static str,
}

fn takes_parameter(action: &str) -> bool {
    matches!(
        action,
        "text" | "esc" | "csi" | "increase_font_size" | "decrease_font_size"
    )
}

fn action_label(action: &str) -> gpui::SharedString {
    ACTIONS
        .iter()
        .find(|(id, _)| *id == action)
        .map_or_else(|| action.to_owned().into(), |(_, label)| translate(label))
}

fn split_action(action: &TerminalAction) -> (&'static str, String) {
    let text = action.to_string();
    let (name, parameter) = text.split_once(':').unwrap_or((&text, ""));
    let name = ACTIONS
        .iter()
        .map(|(id, _)| *id)
        .find(|id| *id == name)
        .unwrap_or("text");
    (name, parameter.to_owned())
}

fn range_text(map: &FontMap) -> String {
    if map.start == map.end {
        format!("U+{:04X}", map.start)
    } else {
        format!("U+{:04X}-U+{:04X}", map.start, map.end)
    }
}

pub(super) fn action_choices(pane: &SettingsView) -> (String, Vec<(&'static str, String)>) {
    (
        pane.binding_editor
            .as_ref()
            .map_or("text", |editor| editor.action)
            .to_owned(),
        ACTIONS
            .iter()
            .map(|(id, label)| (*id, translate(label).to_string()))
            .collect(),
    )
}

fn list_row(label: String, controls: impl IntoIterator<Item = AnyElement>) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(div().flex_1().min_w(px(0.0)).child(label))
        .children(controls)
}

fn editor_buttons(
    pane: &SettingsView,
    field: &'static str,
    cx: &mut Context<SettingsView>,
) -> gpui::Div {
    div()
        .flex()
        .gap(px(6.0))
        .child(controls::button(
            pane.style(),
            &format!("{field}-save"),
            &tr!("Save"),
            true,
            cx.listener(move |pane, _, _, cx| pane.save_terminal_editor(field, cx)),
        ))
        .child(controls::button(
            pane.style(),
            &format!("{field}-cancel"),
            &tr!("Cancel"),
            true,
            cx.listener(move |pane, _, _, cx| pane.cancel_terminal_editor(field, cx)),
        ))
}

pub(super) fn fallbacks(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let names = pane
        .snapshot
        .terminal
        .font_families
        .get(1..)
        .unwrap_or_default()
        .to_vec();
    let last = names.len().saturating_sub(1);
    let rows: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let moved = |target: usize| {
                let mut names = names.clone();
                names.swap(index, target);
                Change::TerminalEdit(TerminalEdit::Fallbacks(names))
            };
            let mut removed = names.clone();
            removed.remove(index);
            list_row(
                format!("{}. {name}", index + 1),
                [
                    change_button(
                        pane,
                        &format!("fallback-up-{index}"),
                        "↑",
                        index > 0,
                        moved(index.saturating_sub(1)),
                        cx,
                    ),
                    change_button(
                        pane,
                        &format!("fallback-down-{index}"),
                        "↓",
                        index < last,
                        moved((index + 1).min(last)),
                        cx,
                    ),
                    change_button(
                        pane,
                        &format!("fallback-remove-{index}"),
                        &tr!("Remove"),
                        true,
                        Change::TerminalEdit(TerminalEdit::Fallbacks(removed)),
                        cx,
                    ),
                ],
            )
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .children(rows)
        .child(div().flex().child(pane.picker(
            PickerKind::Font(FontSlot::Fallback),
            &tr!("Add Font…"),
            cx,
        )))
        .into_any_element()
}

pub(super) fn codepoint_maps(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let editing = pane.codepoint_editor.is_some();
    let rows: Vec<_> = pane
        .snapshot
        .terminal
        .font
        .codepoints
        .iter()
        .enumerate()
        .map(|(index, map)| {
            let edit = map.clone();
            list_row(
                format!("{} → {}", range_text(map), map.family),
                [
                    controls::button(
                        pane.style(),
                        &format!("codepoint-edit-{index}"),
                        &tr!("Edit"),
                        !editing,
                        cx.listener(move |pane, _, window, cx| {
                            pane.edit_codepoint_map(Some(edit.clone()), window, cx);
                        }),
                    )
                    .into_any_element(),
                    change_button(
                        pane,
                        &format!("codepoint-delete-{index}"),
                        &tr!("Delete"),
                        !editing,
                        Change::TerminalEdit(TerminalEdit::CodepointMap {
                            previous: Some(map.clone()),
                            map: None,
                        }),
                        cx,
                    ),
                ],
            )
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .children(rows)
        .child(if editing {
            codepoint_editor(pane, cx)
        } else {
            div()
                .flex()
                .child(controls::button(
                    pane.style(),
                    "codepoint-add",
                    &tr!("Add Map"),
                    true,
                    cx.listener(|pane, _, window, cx| pane.edit_codepoint_map(None, window, cx)),
                ))
                .into_any_element()
        })
        .into_any_element()
}

fn codepoint_editor(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let family = pane
        .codepoint_editor
        .as_ref()
        .map_or("", |editor| editor.family.as_str());
    let choose = tr!("Choose Font…");
    div()
        .flex()
        .flex_col()
        .child(form::row(
            pane.layout_style(),
            &tr!("Range"),
            Some(tr!("A Unicode range, such as U+E000-U+F8FF.").as_ref()),
            pane.field(CODEPOINT_RANGE),
            pane.compact,
        ))
        .child(form::row(
            pane.layout_style(),
            &tr!("Font"),
            None,
            pane.picker(
                PickerKind::Font(FontSlot::Codepoint),
                if family.is_empty() { &choose } else { family },
                cx,
            ),
            pane.compact,
        ))
        .child(editor_buttons(pane, CODEPOINT_RANGE, cx))
        .into_any_element()
}

pub(super) fn bindings(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let editing = pane.binding_editor.is_some();
    let bindings = &pane.snapshot.terminal.keybindings.bindings;
    let rows: Vec<_> = bindings
        .iter()
        .enumerate()
        .map(|(index, (chord, action))| {
            let (name, parameter) = split_action(action);
            let label = action_label(name);
            let description = if parameter.is_empty() {
                label.to_string()
            } else {
                format!("{label}: {parameter}")
            };
            let edit = (chord.clone(), action.clone());
            list_row(
                format!("{chord}  ·  {description}"),
                [
                    controls::button(
                        pane.style(),
                        &format!("keybind-edit-{index}"),
                        &tr!("Edit"),
                        !editing,
                        cx.listener(move |pane, _, window, cx| {
                            pane.edit_binding(Some(edit.clone()), window, cx);
                        }),
                    )
                    .into_any_element(),
                    change_button(
                        pane,
                        &format!("keybind-delete-{index}"),
                        &tr!("Delete"),
                        !editing,
                        Change::TerminalEdit(TerminalEdit::Binding {
                            previous: Some(chord.clone()),
                            binding: None,
                        }),
                        cx,
                    ),
                ],
            )
        })
        .collect();
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .when(bindings.is_empty() && !editing, |list| {
            list.child(pane.note(&tr!("No custom key bindings yet."), false))
        })
        .children(rows)
        .child(if editing {
            binding_editor(pane, cx)
        } else {
            div()
                .flex()
                .child(controls::button(
                    pane.style(),
                    "keybind-add",
                    &tr!("Add Binding"),
                    true,
                    cx.listener(|pane, _, window, cx| pane.edit_binding(None, window, cx)),
                ))
                .into_any_element()
        })
        .into_any_element()
}

fn binding_editor(pane: &SettingsView, cx: &mut Context<SettingsView>) -> AnyElement {
    let Some(editor) = &pane.binding_editor else {
        return div().into_any_element();
    };
    let key = if pane.recording.as_deref() == Some(BINDING_CHORD) {
        tr!("Press a key…")
    } else {
        editor.chord.as_ref().map_or_else(
            || tr!("Record Key"),
            |chord| chord.as_str().to_owned().into(),
        )
    };
    let hint = if editor.action.ends_with("font_size") {
        tr!("Points to change the font size by, such as 1.")
    } else {
        tr!(
            "Text to send. Write special keys as escapes, such as \\e for Escape or \\x01 for Control-A."
        )
    };
    div()
        .flex()
        .flex_col()
        .child(form::row(
            pane.layout_style(),
            &tr!("Key"),
            None,
            controls::button(
                pane.style(),
                BINDING_CHORD,
                &key,
                true,
                cx.listener(|pane, _, window, cx| pane.begin_recording(BINDING_CHORD, window, cx)),
            )
            .into_any_element(),
            pane.compact,
        ))
        .child(form::row(
            pane.layout_style(),
            &tr!("Action"),
            None,
            pane.picker(
                PickerKind::Terminal(BINDING_ACTION),
                &action_label(editor.action),
                cx,
            ),
            pane.compact,
        ))
        .when(takes_parameter(editor.action), |body| {
            body.child(form::row(
                pane.layout_style(),
                &tr!("Value"),
                Some(hint.as_ref()),
                pane.field(BINDING_PARAMETER),
                pane.compact,
            ))
        })
        .when_some(pane.errors.get(BINDING_CHORD), |body, error| {
            body.child(pane.note(error, true))
        })
        .child(editor_buttons(pane, BINDING_PARAMETER, cx))
        .into_any_element()
}

impl SettingsView {
    pub(super) fn add_fallback(&mut self, name: String, cx: &mut Context<Self>) {
        let mut names = self
            .snapshot
            .terminal
            .font_families
            .get(1..)
            .unwrap_or_default()
            .to_vec();
        if names.contains(&name) {
            return;
        }
        names.push(name);
        cx.emit(SettingsEvent::Change(Change::TerminalEdit(
            TerminalEdit::Fallbacks(names),
        )));
    }

    pub(super) fn choose_codepoint_font(&mut self, name: String, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.codepoint_editor {
            editor.family = name;
        }
        self.results.dirty = true;
        cx.notify();
    }

    pub(super) fn choose_binding_action(&mut self, action: &str, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.binding_editor
            && let Some((id, _)) = ACTIONS.iter().find(|(id, _)| *id == action)
        {
            editor.action = id;
        }
        self.results.dirty = true;
        cx.notify();
    }

    pub(super) fn record_terminal_chord(&mut self, chord: KeyChord) {
        if let Some(editor) = &mut self.binding_editor {
            editor.chord = Some(chord);
        }
        self.errors.remove(BINDING_CHORD);
    }

    fn edit_codepoint_map(
        &mut self,
        previous: Option<FontMap>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = previous.as_ref().map_or_else(String::new, range_text);
        self.fields[CODEPOINT_RANGE].update(cx, |input, cx| input.set_text(range, cx));
        self.codepoint_editor = Some(CodepointEditor {
            family: previous
                .as_ref()
                .map_or_else(String::new, |map| map.family.clone()),
            previous,
        });
        self.errors.remove("font-codepoint-map");
        self.results.dirty = true;
        self.fields[CODEPOINT_RANGE].focus_handle(cx).focus(window);
        cx.notify();
    }

    fn edit_binding(
        &mut self,
        previous: Option<(KeyChord, TerminalAction)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (action, parameter) = previous
            .as_ref()
            .map_or(("text", String::new()), |(_, action)| split_action(action));
        self.fields[BINDING_PARAMETER].update(cx, |input, cx| input.set_text(parameter, cx));
        let chord = previous.map(|(chord, _)| chord);
        self.binding_editor = Some(BindingEditor {
            previous: chord.clone(),
            chord,
            action,
        });
        self.errors.remove("terminal-keybindings");
        self.errors.remove(BINDING_CHORD);
        if self
            .binding_editor
            .as_ref()
            .is_some_and(|editor| editor.chord.is_none())
        {
            self.begin_recording(BINDING_CHORD, window, cx);
        } else {
            self.results.dirty = true;
            cx.notify();
        }
    }

    pub(super) fn save_terminal_editor(&mut self, field: &'static str, cx: &mut Context<Self>) {
        let parameter = self.fields[field].read(cx).text().to_owned();
        let edit = if field == CODEPOINT_RANGE {
            let Some(editor) = &self.codepoint_editor else {
                return;
            };
            TerminalEdit::CodepointMap {
                previous: editor.previous.clone(),
                map: Some((parameter, editor.family.clone())),
            }
        } else {
            let Some(editor) = &self.binding_editor else {
                return;
            };
            let Some(chord) = editor.chord.clone() else {
                self.errors
                    .insert(BINDING_CHORD.into(), tr!("Record a key first.").to_string());
                self.results.dirty = true;
                cx.notify();
                return;
            };
            let action = match editor.action {
                "esc" => format!("text:\\e{parameter}"),
                "csi" => format!("text:\\e[{parameter}"),
                action if takes_parameter(action) => format!("{action}:{parameter}"),
                action => action.to_owned(),
            };
            TerminalEdit::Binding {
                previous: editor.previous.clone(),
                binding: Some((chord, action)),
            }
        };
        self.recording = None;
        cx.emit(SettingsEvent::Change(Change::TerminalEdit(edit)));
    }

    pub(super) fn cancel_terminal_editor(&mut self, field: &'static str, cx: &mut Context<Self>) {
        if field == CODEPOINT_RANGE {
            self.codepoint_editor = None;
            self.errors.remove("font-codepoint-map");
        } else {
            self.binding_editor = None;
            self.recording = None;
            self.errors.remove("terminal-keybindings");
            self.errors.remove(BINDING_CHORD);
        }
        self.results.dirty = true;
        cx.notify();
    }
}
