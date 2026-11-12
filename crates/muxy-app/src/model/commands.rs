use gpui::Context;
use muxy_app_core::settings::KeyChord;
use muxy_ui::command_palette::{Command, Registry};
use muxy_ui::tr;

use super::tabs::OpenTerminal;
use super::{AppModel, ProjectStatus, Quitting};
use crate::views::command_palette::Handler;

#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = muxy, no_json)]
pub(crate) struct RunCustomCommand {
    pub id: String,
}

impl AppModel {
    pub(super) fn custom_bindings(&self) -> impl Iterator<Item = (RunCustomCommand, &KeyChord)> {
        self.settings.commands.iter().filter_map(|command| {
            self.settings
                .keymap
                .binding(&command.shortcut_id())
                .map(|chord| {
                    (
                        RunCustomCommand {
                            id: command.id.clone(),
                        },
                        chord,
                    )
                })
        })
    }

    pub(super) fn custom_shortcut_conflict(&self, id: &str, chord: &KeyChord) -> Option<String> {
        self.settings
            .commands
            .iter()
            .find(|command| {
                command.shortcut_id() != id
                    && self.settings.keymap.binding(&command.shortcut_id()) == Some(chord)
            })
            .map(|command| {
                tr!(
                    "%@ is also bound to command: %@",
                    chord.to_string(),
                    &command.name
                )
                .to_string()
            })
    }

    pub(crate) fn can_run_custom_command(&self) -> bool {
        self.quitting == Quitting::Idle
            && !self.dialogs.is_open()
            && self.close_request.is_none()
            && self.pending_close.is_none()
            && self.state.current_project().status() == ProjectStatus::Available
    }

    pub(crate) fn run_custom_command(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.can_run_custom_command() || self.overlay.is_some() {
            return;
        }
        let Some(command) = self
            .settings
            .commands
            .iter()
            .find(|command| command.id == id)
            .cloned()
        else {
            return;
        };
        if let Err(error) = self.launch_custom_command(&command, cx) {
            self.fail(error, cx);
        }
    }

    /// Opens a terminal tab running `command`. Answers `None` when the tab
    /// could not be saved, which the save already reported.
    pub(crate) fn launch_custom_command(
        &mut self,
        command: &muxy_app_core::settings::CustomCommand,
        cx: &mut Context<Self>,
    ) -> Result<Option<muxy_app_core::TabId>, String> {
        command.validate().map_err(|error| error.to_string())?;
        let opened = self.open_terminal(
            OpenTerminal {
                project: self.state.current_project().id,
                title: Some(command.name.trim()),
                command: Some(&command.command),
                directory: None,
            },
            cx,
        )?;
        Ok(opened.map(|(tab, _)| tab))
    }

    pub(crate) fn register_custom_commands(&self, registry: &mut Registry<Handler>) {
        for command in &self.settings.commands {
            let id = command.id.clone();
            let handler: Handler = std::rc::Rc::new(move |model, _, cx| {
                model.run_custom_command(&id, cx);
            });
            let mut item = Command::new(command.shortcut_id(), command.name.clone(), handler)
                .keywords(format!("custom command terminal {}", command.command))
                .disabled(!self.can_run_custom_command());
            if let Some(chord) = self.settings.keymap.binding(&command.shortcut_id()) {
                item = item.shortcut(chord.to_string());
            }
            registry.register(item);
        }
    }
}
