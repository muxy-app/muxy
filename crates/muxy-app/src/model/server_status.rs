use gpui::{Context, Window};
use muxy_app_core::ServerId;
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

use super::{AppModel, ConnectionState, Quitting};
use crate::views::overlays::Overlay;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServerStatus {
    Connected,
    Connecting,
    Disconnected,
    Restarting,
    Stopping,
}

impl ServerStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Connected => tr_key!("Connected"),
            Self::Connecting => tr_key!("Connecting…"),
            Self::Disconnected => tr_key!("Disconnected"),
            Self::Restarting => tr_key!("Restarting…"),
            Self::Stopping => tr_key!("Stopping…"),
        }
    }
}

impl AppModel {
    /// The server of the project on screen, which the status bar shows.
    pub(crate) fn status_server(&self) -> ServerId {
        self.state.current_project().server_id
    }

    /// A server's name: settings name another computer's.
    pub(crate) fn server_label(&self, server: ServerId) -> String {
        match self.server_name(server) {
            Some(name) if !server.is_local() => name.to_owned(),
            _ if cfg!(target_os = "macos") => tr!("This Mac").to_string(),
            _ => tr!("This Computer").to_string(),
        }
    }

    pub(crate) fn server_status(&self) -> ServerStatus {
        self.status_of(self.status_server())
    }

    pub(crate) fn server_control_enabled(&self) -> bool {
        self.control_enabled(self.status_server())
    }

    pub(super) fn control_enabled(&self, server: ServerId) -> bool {
        self.server_actions_enabled(server) && self.ready(server)
    }

    pub(crate) fn server_connect_enabled(&self) -> bool {
        let server = self.status_server();
        self.server_actions_enabled(server)
            && self.connection(server) == ConnectionState::Disconnected
    }

    fn server_actions_enabled(&self, server: ServerId) -> bool {
        let idle = self.quitting == Quitting::Idle && self.close_prompt.is_none();
        if server.is_local() {
            idle && !self.server_preferences.control_busy
                && !self.server_preferences.busy
                && !self.updates.replacing()
        } else {
            idle && self.status_of(server) != ServerStatus::Stopping
        }
    }

    pub(crate) fn server_anchor(&self) -> muxy_ui::popover::PopoverAnchor {
        self.server_anchor.clone()
    }

    pub(crate) fn toggle_server_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if matches!(self.overlay, Some(Overlay::Server)) {
            self.dismiss_overlay(cx);
            return;
        }
        self.dismiss_overlay(cx);
        self.overlay = Some(Overlay::Server);
        self.overlay_focus.focus(window);
        cx.notify();
    }
}
