use super::Composer;
use gpui::Context;
use muxy_app_core::settings::ComposerPresentation;
use muxy_ui::native_menu::Item;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Command {
    Pause,
    CancelVoice,
    Broadcast,
    Insert,
    ClearAfter,
    ClearClose,
    Presentation,
    ResetSize,
}

impl Composer {
    pub(crate) fn menu_items(&self) -> Vec<(Item, Command)> {
        let mut items = Vec::new();
        if self.recording() && self.voice_snapshot.phase != muxy_ui::voice::Phase::Starting {
            items.push((
                Item::new(
                    if self.voice_snapshot.phase == muxy_ui::voice::Phase::Paused {
                        "Resume Dictation"
                    } else {
                        "Pause Dictation"
                    },
                ),
                Command::Pause,
            ));
            items.push((Item::new("Cancel Dictation"), Command::CancelVoice));
        }
        let mut broadcast = Item::new(if self.settings.broadcast {
            "Send to Active Pane"
        } else {
            "Send to All Split Panes"
        });
        broadcast.separator = !items.is_empty();
        items.push((broadcast, Command::Broadcast));
        let mut insert = Item::new("Send Without Enter");
        insert.enabled = !self.recording() && !self.sending;
        items.push((insert, Command::Insert));
        let mut clear_after = Item::new("Clear After Sending");
        clear_after.separator = true;
        clear_after.checked = self.settings.clear_after_sending;
        items.push((clear_after, Command::ClearAfter));
        let mut clear_close = Item::new("Clear on Close");
        clear_close.checked = self.settings.clear_on_close;
        items.push((clear_close, Command::ClearClose));
        let floating = self.settings.presentation == ComposerPresentation::Floating;
        let mut presentation = Item::new(if floating {
            "Use Composer Panel"
        } else {
            "Use Floating Composer"
        });
        presentation.separator = true;
        items.push((presentation, Command::Presentation));
        if floating {
            items.push((Item::new("Reset Composer Size"), Command::ResetSize));
        }
        items
    }

    pub(crate) fn menu_action(&mut self, command: Command, cx: &mut Context<Self>) {
        match command {
            Command::Pause => {
                self.pause_voice(cx);
                return;
            }
            Command::CancelVoice => {
                self.cancel_voice(cx);
                return;
            }
            Command::Insert => {
                self.submit(false, cx);
                return;
            }
            Command::Broadcast => self.settings.broadcast = !self.settings.broadcast,
            Command::ClearAfter => {
                self.settings.clear_after_sending = !self.settings.clear_after_sending;
            }
            Command::ClearClose => self.settings.clear_on_close = !self.settings.clear_on_close,
            Command::Presentation => {
                self.settings.presentation =
                    if self.settings.presentation == ComposerPresentation::Panel {
                        ComposerPresentation::Floating
                    } else {
                        ComposerPresentation::Panel
                    }
            }
            Command::ResetSize => {
                self.settings.expanded = false;
                self.settings.floating_width = 570.0;
                self.settings.floating_height = 236.0;
            }
        }
        self.preferences(cx);
    }
}
