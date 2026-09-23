use gpui::Context;
use muxy_app_core::extensions::Action;
use muxy_app_core::settings::KeyChord;
use muxy_core::shortcuts::ShortcutSettings;
use muxy_ui::command_palette::{Command, Registry};

use super::AppModel;
use crate::views::command_palette::Handler;

/// Runs an extension command, or fires `command.<id>` for a runtime shortcut.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = muxy, no_json)]
pub(crate) struct RunCommand {
    pub owner: String,
    pub command: String,
}

impl AppModel {
    /// Key bindings for extension commands: manifest defaults (or the user's
    /// rebinding) first, then runtime shortcuts; the first claim on a key wins.
    pub(super) fn extension_bindings(&self) -> Vec<(RunCommand, KeyChord)> {
        let mut used: std::collections::HashSet<String> = muxy_core::shortcuts::ALL
            .iter()
            .flat_map(|shortcut| {
                shortcut
                    .contexts
                    .iter()
                    .flat_map(|context| self.settings.keymap.keys(shortcut.id, *context))
            })
            .collect();
        let mut bindings = Vec::new();
        for extension in self.extensions.registry.active() {
            for command in &extension.manifest.commands {
                let id = format!("extension.{}.{}", extension.name, command.id);
                let key = self.settings.keymap.binding(&id).cloned().or_else(|| {
                    command
                        .default_shortcut
                        .as_deref()
                        .and_then(super::surfaces::parse_combo)
                        .map(|(chord, _)| chord)
                });
                if let Some(key) = key
                    && used.insert(key.as_str().to_owned())
                {
                    bindings.push((
                        RunCommand {
                            owner: extension.name.clone(),
                            command: command.id.clone(),
                        },
                        key,
                    ));
                }
            }
        }
        for (owner, id, chord) in self.runtime_bindings() {
            if used.insert(chord.as_str().to_owned()) {
                bindings.push((
                    RunCommand {
                        owner: owner.into(),
                        command: id.into(),
                    },
                    chord.clone(),
                ));
            }
        }
        bindings
    }

    pub(in crate::model) fn bind_extension_keys(&self, cx: &mut Context<Self>) {
        cx.clear_key_bindings();
        crate::views::workspace::bind_keys(&self.settings.keymap, cx);
        let mut registry = muxy_ui::shortcuts::Registry::new(&self.settings.keymap);
        for (command, key) in self.extension_bindings() {
            registry.register_dynamic(key.as_str(), command, Some("WorkspaceTabs"));
        }
        cx.bind_keys(registry.into_bindings());
    }

    pub(in crate::model) fn extension_keystrokes(&self) -> Vec<gpui::Keystroke> {
        self.extension_bindings()
            .iter()
            .filter_map(|(_, key)| gpui::Keystroke::parse(key.as_str()).ok())
            .collect()
    }

    /// Palette entries for extension commands. Popover commands need an item
    /// to anchor to, so main leaves them out of the palette.
    pub(crate) fn register_extension_commands(&self, registry: &mut Registry<Handler>) {
        let bindings = self.extension_bindings();
        for extension in self.extensions.registry.active() {
            for command in &extension.manifest.commands {
                if matches!(command.action, Action::OpenPopover { .. }) {
                    continue;
                }
                let owner = extension.name.clone();
                let id = command.id.clone();
                let handler: Handler = std::rc::Rc::new(move |model, window, cx| {
                    model.run_extension_command(&owner, &id, window, cx);
                });
                let mut item = Command::new(
                    format!("extension.{}.{}", extension.name, command.id),
                    command.title.clone(),
                    handler,
                )
                .keywords(format!(
                    "{} {}",
                    extension.name,
                    command.subtitle.as_deref().unwrap_or("")
                ));
                if let Some((_, key)) = bindings.iter().find(|(binding, _)| {
                    binding.owner == extension.name && binding.command == command.id
                }) {
                    item = item.shortcut(key.to_string());
                }
                registry.register(item);
            }
        }
        let manage: Handler = std::rc::Rc::new(|model, _, cx| {
            model.show_settings(cx);
            if let Some(settings) = &model.settings_window {
                settings
                    .view
                    .update(cx, crate::views::settings::SettingsView::show_extensions);
            }
        });
        registry.register(Command::new(
            "extensions.manage",
            "Manage Extensions",
            manage,
        ));
    }
}
