use std::time::Duration;

use gpui::{
    AnyElement, Context, FontWeight, IntoElement, Modifiers, ParentElement, Pixels, Styled, Task,
    div,
};
use muxy_app_core::settings::Keymap;
use muxy_core::shortcuts::ShortcutId;

use crate::model::AppModel;

const REVEAL_DELAY: Duration = Duration::from_millis(500);
const HELD_CHECK_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Default)]
pub(crate) struct ShortcutHints {
    modifiers: Modifiers,
    visible: bool,
    task: Option<Task<()>>,
    #[cfg(test)]
    pub(crate) held: Modifiers,
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
        Some(key_glyph(&key.key))
    }
}

/// How one key reads on screen: its symbol, or its name in capitals.
pub(crate) fn key_glyph(key: &str) -> String {
    match key {
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
    }
}

impl AppModel {
    /// `name` as the tooltip of a control that runs the command `id`: with
    /// the command's key after it in parentheses, when it has one.
    pub(crate) fn tooltip(
        &self,
        name: impl Into<gpui::SharedString>,
        id: ShortcutId,
    ) -> gpui::SharedString {
        let name = name.into();
        match self.shortcut_label(id) {
            Some(key) => muxy_ui::shortcuts::with_key(&name, &key),
            None => name,
        }
    }

    /// The key of the command `id`, as menus and tooltips show it.
    pub(crate) fn shortcut_label(&self, id: ShortcutId) -> Option<gpui::SharedString> {
        let chord = self.settings.keymap.chord(id)?;
        Some(super::command_palette::chord_label(chord).into())
    }

    pub(crate) fn update_shortcut_hints(&mut self, modifiers: Modifiers, cx: &mut Context<Self>) {
        let hints = &mut self.shortcut_hints;
        if hints.modifiers == modifiers {
            return;
        }
        hints.modifiers = modifiers;
        if modifiers == Modifiers::default() {
            hints.task = None;
            hints.visible = false;
        } else if hints.task.is_none() {
            hints.task = Some(cx.spawn(async move |this, cx| {
                let mut wait = REVEAL_DELAY;
                loop {
                    cx.background_executor().timer(wait).await;
                    let held = this.update(cx, Self::recheck_shortcut_hints);
                    if !matches!(held, Ok(true)) {
                        break;
                    }
                    wait = HELD_CHECK_INTERVAL;
                }
            }));
        }
        cx.notify();
    }

    // The system can swallow a modifier's release, so no event ever reports it.
    fn recheck_shortcut_hints(&mut self, cx: &mut Context<Self>) -> bool {
        #[cfg(test)]
        let held = self.shortcut_hints.held;
        #[cfg(all(target_os = "macos", not(test)))]
        let held = muxy_ui::keyboard::held_modifiers();
        #[cfg(not(any(target_os = "macos", test)))]
        let held = self.shortcut_hints.modifiers;
        self.update_shortcut_hints(held, cx);
        let hints = &mut self.shortcut_hints;
        let tracking = hints.task.is_some();
        if tracking && !hints.visible {
            hints.visible = true;
            cx.notify();
        }
        tracking
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
