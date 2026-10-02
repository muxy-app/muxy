use std::time::Duration;

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, Modifiers, ParentElement, Pixels, Styled, Task,
    div,
};
use muxy_app_core::settings::Keymap;
use muxy_core::shortcuts::ShortcutId;

use crate::model::AppModel;

#[derive(Default)]
pub(crate) struct ShortcutHints {
    modifiers: Modifiers,
    visible: bool,
    task: Option<Task<()>>,
}

impl ShortcutHints {
    pub(crate) fn label(&self, id: ShortcutId, keymap: &Keymap) -> Option<String> {
        if !self.visible {
            return None;
        }
        let chord = keymap.chord(id)?;
        let key = gpui::Keystroke::parse(chord.as_str()).ok()?;
        if self.modifiers == Modifiers::default() || key.modifiers != self.modifiers {
            return None;
        }
        Some(match key.key.as_str() {
            "enter" => "↩".into(),
            "escape" => "⎋".into(),
            "tab" => "⇥".into(),
            "space" => "␣".into(),
            "backspace" => "⌫".into(),
            "delete" => "⌦".into(),
            "insert" => "Ins".into(),
            "home" => "↖".into(),
            "end" => "↘".into(),
            "pageup" => "⇞".into(),
            "pagedown" => "⇟".into(),
            "up" => "↑".into(),
            "down" => "↓".into(),
            "left" => "←".into(),
            "right" => "→".into(),
            key => key.to_uppercase(),
        })
    }
}

impl AppModel {
    pub(crate) fn update_shortcut_hints(&mut self, modifiers: Modifiers, cx: &mut Context<Self>) {
        let hints = &mut self.shortcut_hints;
        if hints.modifiers == modifiers {
            return;
        }
        hints.modifiers = modifiers;
        if modifiers == Modifiers::default() {
            hints.task = None;
            hints.visible = false;
        } else if hints.task.is_none() && !hints.visible {
            hints.task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                let _ = this.update(cx, |model, cx| {
                    model.shortcut_hints.visible = true;
                    model.shortcut_hints.task = None;
                    cx.notify();
                });
            }));
        }
        cx.notify();
    }
}

pub(super) fn badge(label: String, size: Pixels, model: &AppModel) -> AnyElement {
    let scale = if label.chars().count() > 1 { 0.4 } else { 0.7 };
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size)
        .rounded(size * 0.25)
        .bg(model.theme.accent)
        .text_color(model.theme.bg)
        .text_size(size * scale)
        .font_weight(FontWeight::BOLD)
        .child(label)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_follow_custom_keys_and_only_appear_for_matching_modifiers() {
        let control = Modifiers {
            control: true,
            ..Default::default()
        };
        for (chord, label) in [("ctrl-x", "X"), ("ctrl-enter", "↩"), ("ctrl-f12", "F12")] {
            for id in [ShortcutId::SelectTab1, ShortcutId::SelectProject1] {
                let keymap = Keymap::default()
                    .with_binding(id.name(), Some(chord.parse().expect("chord")))
                    .expect("custom binding");
                let mut hints = ShortcutHints {
                    modifiers: control,
                    ..Default::default()
                };
                assert_eq!(hints.label(id, &keymap), None);
                hints.visible = true;
                assert_eq!(hints.label(id, &keymap).as_deref(), Some(label));
                hints.modifiers.shift = true;
                assert_eq!(hints.label(id, &keymap), None);
                hints.modifiers = Modifiers::default();
                assert_eq!(hints.label(id, &keymap), None);
            }
        }
    }
}
