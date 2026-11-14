//! The single GPUI shortcut registration interface used by app and UI modules.

use gpui::{Action, App, KeyBinding, SharedString};
use muxy_core::shortcuts::{ShortcutId, ShortcutSettings};

pub struct Registry<'a> {
    settings: &'a dyn ShortcutSettings,
    bindings: Vec<KeyBinding>,
}

impl std::fmt::Debug for Registry<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("bindings", &self.bindings)
            .finish_non_exhaustive()
    }
}

impl<'a> Registry<'a> {
    pub fn new(settings: &'a dyn ShortcutSettings) -> Self {
        Self {
            settings,
            bindings: Vec::new(),
        }
    }

    pub fn register(&mut self, id: ShortcutId, action: &(impl Action + Clone)) {
        let shortcut = id.definition();
        for context in shortcut.contexts {
            for key in self.settings.keys(shortcut.id, *context) {
                self.bindings
                    .push(KeyBinding::new(&key, action.clone(), *context));
            }
        }
    }

    /// Register a dynamic command after app settings have resolved conflicts.
    pub fn register_dynamic(&mut self, key: &str, action: impl Action, context: Option<&str>) {
        self.bindings.push(KeyBinding::new(key, action, context));
    }

    pub fn into_bindings(self) -> Vec<KeyBinding> {
        self.bindings
    }
}

/// `name` as a tooltip shows a control that has a shortcut: with `key` after
/// it in parentheses.
pub fn with_key(name: &str, key: &str) -> SharedString {
    format!("{name} ({key})").into()
}

/// The key bound to `action`, as menus and tooltips show it. An action with
/// several keys shows the first, which is the one Settings lists.
pub fn key_for(action: &dyn Action, cx: &App) -> Option<SharedString> {
    let keymap = cx.key_bindings();
    let keymap = keymap.borrow();
    let binding = keymap.bindings_for_action(action).next()?;
    let keys: Vec<_> = binding
        .keystrokes()
        .iter()
        .map(ToString::to_string)
        .collect();
    Some(keys.join(" ").into())
}

/// `name` for a tooltip, followed by the key bound to `action` when it has one.
pub fn tooltip_text(name: impl Into<SharedString>, action: &dyn Action, cx: &App) -> SharedString {
    let name = name.into();
    match key_for(action, cx) {
        Some(key) => with_key(&name, &key),
        None => name,
    }
}
