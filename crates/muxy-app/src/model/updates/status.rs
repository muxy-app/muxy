use gpui::{Context, Window};
use muxy_app_core::ServerId;
use muxy_ui::l10n::tr_key;
use muxy_ui::tr;

use super::{AppModel, AppUpdatePhase, ConnectionState, Quitting, ServerUpdatePhase};
use crate::views::overlays::Overlay;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UpdateAction {
    ReviewApp,
    ManageSchedule,
    CancelSchedule,
    RetryApp,
    RestartServer,
    RetryServer,
    Connect,
}

pub(crate) struct UpdateDetails {
    pub(crate) label: String,
    pub(crate) description: String,
    /// The action and its label, an English key shown translated.
    pub(crate) action: Option<(UpdateAction, &'static str)>,
    pub(crate) failed: bool,
}

impl UpdateDetails {
    fn new(label: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: description.into(),
            action: None,
            failed: false,
        }
    }

    fn action(mut self, action: UpdateAction, label: &'static str) -> Self {
        self.action = Some((action, label));
        self
    }

    fn failure(mut self) -> Self {
        self.failed = true;
        self
    }
}

impl AppModel {
    pub(crate) fn update_details(&self) -> Option<UpdateDetails> {
        if self.quitting == Quitting::Update {
            return Some(UpdateDetails::new(
                tr!("Updating app…"),
                tr!("Installing the app update and restarting Muxy."),
            ));
        }
        let waiting_for_sessions = self.updates.phase == ServerUpdatePhase::Replacing
            && !self.server_preferences.control_busy
            && self.updates.sessions > 0;
        if self.updates.replacing() && !waiting_for_sessions {
            return Some(UpdateDetails::new(
                tr!("Restarting server…"),
                tr!("Starting the server and reconnecting to Muxy."),
            ));
        }
        if self.updates.phase == ServerUpdatePhase::Failed {
            let action = if self.connection(ServerId::local()) == ConnectionState::Disconnected {
                tr_key!("Retry connection")
            } else if self.server_update_pending() {
                tr_key!("Restart server…")
            } else {
                tr_key!("Retry server update")
            };
            return Some(
                UpdateDetails::new(
                    tr!("Server update paused"),
                    self.updates
                        .server_error
                        .clone()
                        .unwrap_or_else(|| tr!("The server update could not finish.").to_string()),
                )
                .action(UpdateAction::RetryServer, action)
                .failure(),
            );
        }
        if self.updates.checking() {
            return Some(if self.updates.download == AppUpdatePhase::Downloading {
                UpdateDetails::new(
                    tr!("Downloading app update…"),
                    tr!("Downloading and verifying the latest release. You can keep working."),
                )
            } else {
                UpdateDetails::new(
                    tr!("Checking for updates…"),
                    tr!("Looking for the newest release on your update channel."),
                )
            });
        }
        if let Some(error) = &self.updates.app_error {
            return Some(
                UpdateDetails::new(tr!("App update failed"), error.clone())
                    .action(UpdateAction::RetryApp, tr_key!("Retry app update"))
                    .failure(),
            );
        }
        if self.updates.scheduled {
            return Some(scheduled_details(self.updates.sessions));
        }
        if self.updates.retry_preparation.is_some() {
            return Some(UpdateDetails::new(
                tr!("App update waiting"),
                tr!("Another update is in progress. Muxy will retry when it finishes."),
            ));
        }
        if let Some(update) = &self.updates.ready {
            if self.connection(ServerId::local()) != ConnectionState::Ready {
                return Some(UpdateDetails::new(tr!("App update ready"), tr!("Connect to the server so Muxy can check whether terminal sessions can be preserved."))
                    .action(UpdateAction::Connect, tr_key!("Connect to server")));
            }
            let compatible = self
                .updates
                .server
                .as_ref()
                .is_some_and(|server| update.compatible_with(server));
            return Some(if compatible {
                UpdateDetails::new(tr!("App update ready"), tr!("Restart the app to install the update. Your terminal sessions will keep running. You can update the server afterward."))
                    .action(UpdateAction::ReviewApp, tr_key!("Update and restart app…"))
            } else {
                UpdateDetails::new(tr!("App update requires a server restart"), tr!("This release needs a new server. Wait for all terminal sessions to end, or confirm ending them to update now."))
                    .action(UpdateAction::ReviewApp, tr_key!("Review app update…"))
            });
        }
        if self.server_update_pending() {
            let detail = UpdateDetails::new(
                tr!("Server update pending"),
                tr!(
                    "The app is updated. The server will update when all terminal sessions end, or you can restart it now. Restarting ends all sessions, including idle and detached sessions."
                ),
            );
            return Some(
                if self.connection(ServerId::local()) == ConnectionState::Ready {
                    detail.action(UpdateAction::RestartServer, tr_key!("Restart server…"))
                } else {
                    detail.action(UpdateAction::Connect, tr_key!("Connect to server"))
                },
            );
        }
        None
    }

    pub(crate) fn update_versions(&self) -> (String, String) {
        let installed = env!("CARGO_PKG_VERSION");
        let target = self
            .updates
            .ready
            .as_ref()
            .map_or(installed, |update| update.version.as_str());
        let app = version_transition(installed, target);
        let server = self.updates.server.as_ref().map_or_else(
            || tr!("Not connected").to_string(),
            |server| {
                let version = version_transition(&server.build.version, installed);
                if self.connection(ServerId::local()) == ConnectionState::Ready {
                    version
                } else {
                    format!("{version} · {}", tr!("Disconnected"))
                }
            },
        );
        (
            app,
            if self.connection(ServerId::local()) == ConnectionState::Connecting {
                tr!("Connecting…").to_string()
            } else {
                server
            },
        )
    }

    pub(crate) fn toggle_update_popover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_prompt.is_some() {
            return;
        }
        if matches!(self.overlay, Some(Overlay::Updates)) {
            self.dismiss_overlay(cx);
            return;
        }
        self.dismiss_overlay(cx);
        self.overlay = Some(Overlay::Updates);
        self.overlay_focus.focus(window);
        cx.notify();
    }

    pub(crate) fn update_anchor(&self) -> muxy_ui::popover::PopoverAnchor {
        self.updates.anchor.clone()
    }

    pub(crate) fn server_update_action(&self) -> (UpdateAction, &'static str) {
        if self.connection(ServerId::local()) == ConnectionState::Ready {
            (UpdateAction::RestartServer, tr_key!("Restart server…"))
        } else {
            (UpdateAction::Connect, tr_key!("Connect to server"))
        }
    }

    pub(crate) fn update_scheduled(&self) -> bool {
        self.updates.scheduled
    }

    pub(crate) fn update_action_enabled(&self, action: UpdateAction) -> bool {
        if action == UpdateAction::CancelSchedule {
            return self.quitting == Quitting::Idle
                && self.close_prompt.is_none()
                && self.updates.scheduled;
        }
        self.quitting == Quitting::Idle
            && self.close_prompt.is_none()
            && (matches!(
                action,
                UpdateAction::RestartServer | UpdateAction::RetryServer | UpdateAction::Connect
            ) || !self.updates.checking())
            && !self.updates.replacing()
            && !self.server_preferences.control_busy
            && !self.server_preferences.busy
            && self.connection(ServerId::local()) != ConnectionState::Connecting
    }

    pub(crate) fn perform_update_action(&mut self, action: UpdateAction, cx: &mut Context<Self>) {
        if !self.update_action_enabled(action) {
            return;
        }
        match action {
            UpdateAction::ReviewApp | UpdateAction::RetryApp => self.check_for_updates(true, cx),
            UpdateAction::ManageSchedule => {
                self.dismiss_overlay(cx);
                self.confirm_update(cx);
            }
            UpdateAction::CancelSchedule => self.schedule_update(false, cx),
            UpdateAction::RestartServer => {
                self.confirm_server_control(ServerId::local(), true, self.window, cx);
            }
            UpdateAction::RetryServer => {
                self.updates.phase = ServerUpdatePhase::Idle;
                self.updates.server_error = None;
                if self.connection(ServerId::local()) == ConnectionState::Disconnected {
                    self.updates.phase = ServerUpdatePhase::Reconnecting;
                    self.connect_to_server(ServerId::local(), true, cx);
                } else if self.server_update_pending() {
                    self.confirm_server_control(ServerId::local(), true, self.window, cx);
                } else {
                    self.reconcile_server_update(cx);
                }
            }
            UpdateAction::Connect => self.connect(cx),
        }
        cx.notify();
    }
}

fn scheduled_details(sessions: usize) -> UpdateDetails {
    let (label, description) = if sessions == 1 {
        (
            tr!("App update waiting for %lld session", sessions),
            tr!(
                "Muxy will update and restart when all %lld terminal session end, including idle and detached sessions. Keep Muxy open to finish the update.",
                sessions
            ),
        )
    } else {
        (
            tr!("App update waiting for %lld sessions", sessions),
            tr!(
                "Muxy will update and restart when all %lld terminal sessions end, including idle and detached sessions. Keep Muxy open to finish the update.",
                sessions
            ),
        )
    };
    UpdateDetails::new(label, description).action(
        UpdateAction::ManageSchedule,
        tr_key!("Manage scheduled update…"),
    )
}

fn version_transition(current: &str, target: &str) -> String {
    if crate::updater::build_number(target) > crate::updater::build_number(current) {
        format!("{current} → {target}")
    } else {
        current.into()
    }
}
