use gpui::Context;
use muxy_app_core::settings::KeyChord;
use muxy_ui::command_palette::{Command, Registry};

use super::{AppModel, ConnectionState, ProjectStatus, Quitting};
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
            .map(|command| format!("{chord} is also bound to command: {}", command.name))
    }

    fn can_run_custom_command(&self) -> bool {
        self.quitting == Quitting::Idle
            && self.close_prompt.is_none()
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
        else {
            return;
        };
        if let Err(error) = command.validate() {
            self.fail(error.to_string(), cx);
            return;
        }
        let command = command.clone();
        let previous = self.state.clone();
        let result = (|| {
            let tab = self
                .state
                .open_terminal_tab(self.state.current_project().id)?;
            self.state
                .set_tab_title(tab, Some(command.name.trim().into()))?;
            let pane = self
                .state
                .window()
                .active_pane
                .ok_or_else(|| "could not create command pane".to_owned())?;
            self.state.set_startup_command(pane, &command.command)?;
            Ok::<_, Box<dyn std::error::Error>>(())
        })();
        if let Err(error) = result {
            self.state = previous;
            self.fail(error.to_string(), cx);
            return;
        }
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        self.changed(cx);
        self.focus_requested = true;
        if self.connection == ConnectionState::Disconnected {
            self.connect(cx);
        }
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
