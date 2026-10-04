use gpui::{Context, Window};
use muxy_app_core::ServerId;

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
            Self::Connected => "Connected",
            Self::Connecting => "Connecting…",
            Self::Disconnected => "Disconnected",
            Self::Restarting => "Restarting…",
            Self::Stopping => "Stopping…",
        }
    }
}

impl AppModel {
    pub(crate) fn server_status(&self) -> ServerStatus {
        if self.updates.replacing() {
            ServerStatus::Restarting
        } else if self.server_preferences.control_busy {
            ServerStatus::Stopping
        } else {
            match self.connection(ServerId::local()) {
                ConnectionState::Ready => ServerStatus::Connected,
                ConnectionState::Connecting => ServerStatus::Connecting,
                ConnectionState::Disconnected => ServerStatus::Disconnected,
            }
        }
    }

    pub(crate) fn server_control_enabled(&self) -> bool {
        self.server_actions_enabled() && self.ready(ServerId::local())
    }

    pub(crate) fn server_connect_enabled(&self) -> bool {
        self.server_actions_enabled()
            && self.connection(ServerId::local()) == ConnectionState::Disconnected
    }

    fn server_actions_enabled(&self) -> bool {
        self.quitting == Quitting::Idle
            && self.close_prompt.is_none()
            && !self.server_preferences.control_busy
            && !self.server_preferences.busy
            && !self.updates.replacing()
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
