use gpui::{
    AnyElement, AppContext, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use muxy_app_core::extensions::Icon;
use muxy_app_core::settings::KeyChord;
use muxy_core::shortcuts::ShortcutSettings;
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::components::{ButtonInteraction, SymbolGlyph, Tooltip};

use super::AppModel;
use crate::views::command_palette::Handler;

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = muxy, no_json)]
pub(crate) struct RunCommand {
    pub owner: String,
    pub command: String,
}

impl AppModel {
    fn extension_bindings(&self) -> Vec<(RunCommand, KeyChord)> {
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
                        .as_ref()
                        .and_then(|key| key.replace('+', "-").parse().ok())
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

    pub(crate) fn register_extension_commands(&self, registry: &mut Registry<Handler>) {
        let bindings = self.extension_bindings();
        for extension in self.extensions.registry.active() {
            for command in &extension.manifest.commands {
                let owner = extension.name.clone();
                let id = command.id.clone();
                let handler: Handler = std::rc::Rc::new(move |model, window, cx| {
                    model.run_extension_command(&owner, &id, window, cx);
                });
                let mut item = Command::new(
                    format!("extension.{}.{}", extension.name, command.id),
                    command.title.clone(),
                    handler,
                );
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

    pub(crate) fn extension_toolbar_width(&self) -> f32 {
        let count = self
            .extensions
            .registry
            .active()
            .map(|extension| extension.manifest.topbar_items.len())
            .sum::<usize>();
        f32::from(u16::try_from(count).unwrap_or(u16::MAX)) * 28.0
    }

    pub(crate) fn extension_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut bar = div().flex().items_center().gap(px(2.0));
        for extension in self.extensions.registry.active() {
            for item in &extension.manifest.topbar_items {
                let owner = extension.name.clone();
                let command = item.command.clone();
                let symbol = match &item.icon {
                    Icon::Name(name) | Icon::Symbol { symbol: name } => name.as_str(),
                    Icon::Svg { .. } => "puzzlepiece.extension",
                };
                let tooltip = item.tooltip.clone().unwrap_or_else(|| item.command.clone());
                let theme = self.theme.clone();
                bar = bar.child(
                    div()
                        .id(gpui::SharedString::from(format!(
                            "extension-toolbar-{}-{}",
                            extension.name, item.id
                        )))
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(26.0))
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .hover(|view| view.bg(self.theme.hover))
                        .tooltip(move |_, cx| {
                            cx.new(|_| {
                                Tooltip::new(
                                    tooltip.clone(),
                                    theme.raised(),
                                    theme.fg,
                                    theme.border,
                                )
                            })
                            .into()
                        })
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .button_interaction(cx.listener(move |model, _, window, cx| {
                            model.run_extension_command(&owner, &command, window, cx);
                        }))
                        .child(SymbolGlyph::new(
                            symbol.to_owned(),
                            px(14.0),
                            self.theme.fg_muted,
                        )),
                );
            }
        }
        bar.into_any_element()
    }
}
