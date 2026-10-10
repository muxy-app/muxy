use super::{Category, Change, SettingsEvent, SettingsView};
use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, Keystroke, ParentElement, Styled, div, px,
};
use muxy_ui::{controls, tr};

pub(super) fn matching(pane: &SettingsView) -> Vec<usize> {
    pane.shortcut_names
        .iter()
        .enumerate()
        .filter_map(|(index, name)| pane.matches(Category::Keyboard, name).then_some(index))
        .collect()
}

/// Extension shortcuts that match the search, in extension order.
pub(super) fn matching_extensions(pane: &SettingsView) -> Vec<usize> {
    pane.snapshot
        .extension_shortcuts
        .iter()
        .enumerate()
        .filter_map(|(index, shortcut)| {
            let name = format!("{} {}", shortcut.extension, shortcut.title);
            pane.matches(Category::Keyboard, &name).then_some(index)
        })
        .collect()
}

/// An extension command's shortcut: record, reset to the manifest default,
/// or unassign, as on main.
pub(super) fn extension_row(
    pane: &SettingsView,
    index: usize,
    cx: &mut Context<SettingsView>,
) -> AnyElement {
    let shortcut = &pane.snapshot.extension_shortcuts[index];
    let id = shortcut.id.clone();
    let label = if pane.recording.as_deref() == Some(id.as_str()) {
        tr!("Press a shortcut…")
    } else {
        shortcut.chord.as_ref().map_or_else(
            || tr!("Not assigned"),
            |chord| chord.as_str().to_owned().into(),
        )
    };
    let focus = &pane.results.extension_focus[&id];
    let (record, reset, unassign) = (id.clone(), id.clone(), id.clone());
    let control = div()
        .flex()
        .flex_wrap()
        .gap(px(6.0))
        .child(
            controls::button(
                pane.style(),
                &id,
                &label,
                true,
                cx.listener(move |pane, _, window, cx| {
                    pane.begin_recording(&record, window, cx);
                }),
            )
            .track_focus(&focus[0]),
        )
        .child(
            controls::button(
                pane.style(),
                &format!("reset-{id}"),
                &tr!("Reset"),
                true,
                cx.listener(move |pane, _, _, cx| {
                    pane.recording = None;
                    cx.emit(SettingsEvent::Change(Change::Binding(reset.clone(), None)));
                }),
            )
            .track_focus(&focus[1]),
        )
        .child(
            controls::button(
                pane.style(),
                &format!("unassign-{id}"),
                &tr!("Unassign"),
                true,
                cx.listener(move |pane, _, _, cx| {
                    pane.recording = None;
                    cx.emit(SettingsEvent::Change(Change::Unassign(unassign.clone())));
                }),
            )
            .track_focus(&focus[2]),
        );
    pane.text_row(
        &id,
        &shortcut.title,
        None,
        control.into_any_element(),
        pane.compact,
    )
}

pub(super) fn row(pane: &SettingsView, index: usize, cx: &mut Context<SettingsView>) -> AnyElement {
    let id = muxy_core::shortcuts::ALL[index].id;
    let recording = pane.recording.as_deref() == Some(id);
    let chord = pane.snapshot.settings.keymap.binding(id);
    let label = if recording {
        tr!("Press a shortcut…")
    } else {
        chord.map_or_else(
            || tr!("Not assigned"),
            |chord| chord.as_str().to_owned().into(),
        )
    };
    let control = div()
        .flex()
        .flex_wrap()
        .gap(px(6.0))
        .child(
            controls::button(
                pane.style(),
                id,
                &label,
                true,
                cx.listener(move |pane, _, window, cx| {
                    pane.begin_recording(id, window, cx);
                }),
            )
            .track_focus(&pane.results.shortcut_focus[index][0]),
        )
        .child(
            controls::button(
                pane.style(),
                &format!("reset-{id}"),
                &tr!("Reset"),
                true,
                cx.listener(move |pane, _, _, cx| {
                    pane.recording = None;
                    cx.emit(SettingsEvent::Change(Change::Binding(id.into(), None)));
                }),
            )
            .track_focus(&pane.results.shortcut_focus[index][1]),
        );
    pane.row(id, &pane.shortcut_names[index], control.into_any_element())
}

impl SettingsView {
    pub(crate) fn begin_recording(
        &mut self,
        id: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.results.dirty = true;
        self.recording = Some(id.into());
        self.errors.remove(id);
        self.focus.focus(window);
        cx.notify();
    }

    pub(super) fn record_key(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) {
        let Some(id) = self.recording.take() else {
            return;
        };
        self.results.dirty = true;
        cx.stop_propagation();
        if keystroke.key == "escape" && !keystroke.modifiers.modified() {
            cx.notify();
            return;
        }
        match recorded_chord(keystroke) {
            Ok(chord) if id == super::terminal_lists::BINDING_CHORD => {
                self.record_terminal_chord(chord);
            }
            Ok(chord) => cx.emit(SettingsEvent::Change(Change::Binding(id, Some(chord)))),
            Err(error) => {
                self.errors.insert(id, format!("{error}"));
            }
        }
        cx.notify();
    }
}

fn recorded_chord(
    keystroke: &Keystroke,
) -> muxy_app_core::settings::Result<muxy_app_core::settings::KeyChord> {
    let modifiers = keystroke.modifiers;
    let mut value = String::new();
    for (name, enabled) in [
        ("cmd", modifiers.platform),
        ("ctrl", modifiers.control),
        ("alt", modifiers.alt),
        ("shift", modifiers.shift),
        ("fn", modifiers.function),
    ] {
        if enabled {
            value.push_str(name);
            value.push('-');
        }
    }
    value.push_str(&keystroke.key);
    value.parse()
}
