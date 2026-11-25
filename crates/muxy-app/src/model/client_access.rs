use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{AnyWindowHandle, ClipboardItem, Context, Task};
use muxy_app_core::ServerId;
use muxy_protocol::{
    DeviceId, ListenerStatus, PairingOffer, RemoteAccessSettings, RemoteAccessState,
};
use muxy_ui::tr;

use super::{AppModel, Quitting};
use crate::boot::Work;
use crate::views::clients::Pairing;

#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The running request and the follow-ups it owes"
)]
pub(crate) struct ClientAccess {
    pub(crate) state: Option<RemoteAccessState>,
    pub(crate) pairing: Option<Pairing>,
    page_open: bool,
    page_shown: bool,
    copied: bool,
    pub(crate) busy: bool,
    starting: bool,
    stale: bool,
    cancel: bool,
    copy_link: bool,
    pub(super) restore_off: bool,
    pub(crate) error: Option<String>,
    expiry: Option<Task<()>>,
}

impl ClientAccess {
    pub(super) fn show_page(&mut self) {
        self.page_open = true;
    }

    pub(super) fn hide_page(&mut self) {
        self.page_open = false;
    }

    pub(super) fn after_disconnect(&self) -> Self {
        Self {
            page_open: self.page_open,
            restore_off: self.restore_off,
            ..Self::default()
        }
    }
}

impl AppModel {
    pub(crate) fn client_access(&self) -> &ClientAccess {
        &self.client_access
    }

    pub(crate) fn client_access_available(&self) -> bool {
        self.quitting == Quitting::Idle && self.ready(ServerId::local())
    }

    fn servers_page_open(&self) -> bool {
        self.client_access.page_open
            && self.state.settings_tab().is_some()
            && self
                .visible_panes()
                .into_iter()
                .any(|pane| self.state.is_settings_pane(pane))
    }

    pub(crate) fn set_servers_page_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.client_access.page_open != open {
            self.client_access.page_open = open;
            self.sync_servers_page(open, cx);
        }
    }

    fn client_access_shown(&self) -> bool {
        self.servers_page_open() || self.onboarding_pairing_shown()
    }

    pub(crate) fn sync_servers_page(&mut self, opened: bool, cx: &mut Context<Self>) {
        let page = self.servers_page_open();
        let returned = page && !self.client_access.page_shown;
        self.client_access.page_shown = page;
        if !self.client_access_shown() {
            self.withdraw_pairing(cx);
        } else if opened || returned {
            self.read_remote_access(cx);
        }
    }

    pub(crate) fn read_remote_access(&mut self, cx: &mut Context<Self>) {
        if !self.client_access_available() {
            return;
        }
        if self.client_access.busy {
            self.client_access.stale = true;
            return;
        }
        self.client_access.busy = self.send(ServerId::local(), Work::ReadRemoteAccess, cx);
        cx.notify();
    }

    pub(super) fn remote_access_changed(&mut self, cx: &mut Context<Self>) {
        if self.client_access_shown() {
            self.read_remote_access(cx);
        }
    }

    pub(crate) fn set_client_access(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let result = self.write_remote_access(
            |settings| RemoteAccessSettings {
                enabled,
                ..settings
            },
            cx,
        );
        self.finish_access_change(result, cx);
    }

    pub(crate) fn push_notifications_available(&self) -> bool {
        self.client_access_available()
            && self.server_supports(
                ServerId::local(),
                muxy_protocol::Feature::PUSH_NOTIFICATIONS,
            )
    }

    pub(crate) fn set_push_notifications(
        &mut self,
        on: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let affected = self.client_access.state.as_ref().is_some_and(|state| {
            state.push_notifications && state.devices.iter().any(|device| device.push.is_some())
        });
        if on || !affected {
            self.switch_push_notifications(on, cx);
            return;
        }
        let generation = self.servers.local.generation;
        self.ask_once(
            super::dialogs::Question::PausePush,
            window,
            async move |parent, cx| crate::views::confirm::prompt_pause_push(parent, cx).await,
            move |model, response, cx| match response {
                Ok(true) if generation == model.servers.local.generation => {
                    model.switch_push_notifications(false, cx);
                }
                Err(error) => model.client_access.error = Some(error),
                Ok(_) => {}
            },
            cx,
        );
    }

    pub(crate) fn switch_push_notifications(&mut self, on: bool, cx: &mut Context<Self>) {
        let result = if !self.push_notifications_available() {
            Err(tr!("Connect to the server before changing client access").to_string())
        } else if self.client_access.busy {
            Err(tr!("Wait for the current client access change to finish").to_string())
        } else if self
            .client_access
            .state
            .as_ref()
            .is_some_and(|state| state.push_notifications != on)
        {
            self.client_access.busy =
                self.send(ServerId::local(), Work::SetPushNotifications(on), cx);
            if self.client_access.busy {
                Ok(())
            } else {
                Err(tr!("Could not send the change to the server").to_string())
            }
        } else {
            Ok(())
        };
        self.finish_access_change(result, cx);
    }

    pub(crate) fn confirm_reset_push(
        &mut self,
        device: DeviceId,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self.client_access.state.as_ref().and_then(|state| {
            state
                .devices
                .iter()
                .find(|paired| paired.id == device)
                .map(|paired| paired.name.clone())
        }) else {
            return;
        };
        let generation = self.servers.local.generation;
        self.ask_once(
            super::dialogs::Question::ResetPush(device),
            window,
            async move |parent, cx| {
                crate::views::confirm::prompt_reset_push(parent, &name, cx).await
            },
            move |model, response, cx| match response {
                Ok(true)
                    if generation == model.servers.local.generation
                        && model.push_notifications_available() =>
                {
                    if model.client_access.busy {
                        model.client_access.error = Some(
                            tr!("Wait for the current client access change to finish").to_string(),
                        );
                    } else {
                        model.client_access.busy =
                            model.send(ServerId::local(), Work::ResetPush(device), cx);
                    }
                }
                Err(error) => model.client_access.error = Some(error),
                Ok(_) => {}
            },
            cx,
        );
    }

    pub(crate) fn set_client_port(&mut self, port: &str, cx: &mut Context<Self>) {
        let result = port
            .trim()
            .parse()
            .map_err(|_| tr!("Use a port from 1024 to 65535").to_string())
            .and_then(|port| {
                self.write_remote_access(|settings| RemoteAccessSettings { port, ..settings }, cx)
            });
        self.finish_access_change(result, cx);
    }

    fn finish_access_change(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.client_access.error = result.err();
        cx.notify();
    }

    fn write_remote_access(
        &mut self,
        change: impl FnOnce(RemoteAccessSettings) -> RemoteAccessSettings,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !self.client_access_available() {
            return Err(tr!("Connect to the server before changing client access").to_string());
        }
        if self.client_access.busy {
            return Err(tr!("Wait for the current client access change to finish").to_string());
        }
        let current = self
            .client_access
            .state
            .as_ref()
            .map(|state| state.settings)
            .ok_or_else(|| tr!("Load client access first").to_string())?;
        let settings = change(current);
        settings
            .validate()
            .map_err(|_| tr!("Use a port from 1024 to 65535").to_string())?;
        if settings != current {
            self.client_access.busy =
                self.send(ServerId::local(), Work::WriteRemoteAccess(settings), cx);
            if !self.client_access.busy {
                return Err(tr!("Could not send the change to the server").to_string());
            }
        }
        Ok(())
    }

    pub(crate) fn show_pairing_code(&mut self, cx: &mut Context<Self>) {
        if self.client_access_available() && !self.client_access.busy {
            self.client_access.error = None;
            self.client_access.busy = self.send(ServerId::local(), Work::StartPairing, cx);
            self.client_access.starting = self.client_access.busy;
            cx.notify();
        }
    }

    pub(crate) fn copy_pairing_link(&mut self, cx: &mut Context<Self>) {
        self.dismiss_overlay(cx);
        if !self.client_access_available() {
            self.fail(
                tr!("Connect to this computer's server to copy its pairing link.").to_string(),
                cx,
            );
            return;
        }
        self.client_access.copy_link = true;
        let code_may_be_stale = !self.servers_page_open();
        if code_may_be_stale && !self.client_access.busy {
            self.read_remote_access(cx);
        }
        self.continue_link_copy(cx);
    }

    fn continue_link_copy(&mut self, cx: &mut Context<Self>) {
        if !self.client_access.copy_link || self.client_access.busy {
            return;
        }
        if let Some(pairing) = &self.client_access.pairing {
            cx.write_to_clipboard(ClipboardItem::new_string(pairing.link().to_owned()));
            self.client_access.copy_link = false;
            self.client_access.copied = true;
            self.show_notice(tr!("Pairing link copied").to_string(), cx);
            return;
        }
        let Some(state) = &self.client_access.state else {
            return;
        };
        if state.status == ListenerStatus::Listening {
            self.show_pairing_code(cx);
            return;
        }
        self.client_access.copy_link = false;
        let reason = if state.settings.enabled {
            tr!("This computer isn't listening for clients yet. Try again in a moment.")
        } else {
            tr!("Turn on Allow clients in Settings → Servers to copy a pairing link.")
        };
        self.fail(reason.to_string(), cx);
    }

    pub(crate) fn cancel_pairing(&mut self, cx: &mut Context<Self>) {
        self.client_access.copy_link = false;
        self.client_access.copied = false;
        self.client_access.pairing = None;
        self.client_access.expiry = None;
        if self.client_access.busy {
            self.client_access.cancel = true;
        } else if self.client_access_available() {
            self.client_access.busy = self.send(ServerId::local(), Work::CancelPairing, cx);
        }
        cx.notify();
    }

    pub(super) fn withdraw_pairing(&mut self, cx: &mut Context<Self>) {
        let shown = self.client_access.pairing.is_some() && !self.client_access.copied;
        let starting = self.client_access.starting && !self.client_access.copy_link;
        if shown || starting {
            self.cancel_pairing(cx);
        }
    }

    pub(crate) fn confirm_revoke(
        &mut self,
        device: DeviceId,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self.client_access.state.as_ref().and_then(|state| {
            state
                .devices
                .iter()
                .find(|paired| paired.id == device)
                .map(|paired| paired.name.clone())
        }) else {
            return;
        };
        let generation = self.servers.local.generation;
        self.ask_once(
            super::dialogs::Question::RevokeDevice(device),
            window,
            async move |parent, cx| crate::views::confirm::prompt_revoke(parent, &name, cx).await,
            move |model, response, cx| match response {
                Ok(true)
                    if generation == model.servers.local.generation
                        && model.client_access_available() =>
                {
                    if model.client_access.busy {
                        model.client_access.error = Some(
                            tr!("Wait for the current client access change to finish").to_string(),
                        );
                    } else {
                        model.client_access.busy =
                            model.send(ServerId::local(), Work::RevokeDevice(device), cx);
                    }
                }
                Err(error) => model.client_access.error = Some(error),
                Ok(_) => {}
            },
            cx,
        );
    }

    pub(super) fn receive_remote_access(
        &mut self,
        result: Result<RemoteAccessState, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.client_access.busy = false;
        match result {
            Ok(state) => {
                if state.pairing_expires_at.is_none() {
                    self.client_access.pairing = None;
                    self.client_access.copied = false;
                    self.client_access.expiry = None;
                }
                self.client_access.error = None;
                self.client_access.state = Some(state);
                self.show_client_port(cx);
            }
            Err(error) => self.client_access.error = Some(error.to_string()),
        }
        self.finish_client_access_request(cx);
    }

    pub(super) fn receive_pairing(
        &mut self,
        result: Result<PairingOffer, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.client_access.busy = false;
        self.client_access.starting = false;
        match result {
            Ok(_)
                if self.client_access.cancel
                    || !(self.client_access_shown() || self.client_access.copy_link) =>
            {
                self.client_access.cancel = true;
            }
            Ok(offer) => {
                let lifetime = offer.expires_at.saturating_sub(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                );
                self.client_access.expiry = Some(cx.spawn(async move |model, cx| {
                    cx.background_executor()
                        .timer(Duration::from_secs(lifetime))
                        .await;
                    let _ = model.update(cx, |model, cx| {
                        model.client_access.pairing = None;
                        model.client_access.copied = false;
                        model.read_remote_access(cx);
                        cx.notify();
                    });
                }));
                self.client_access.pairing = Some(Pairing::new(&offer));
                self.client_access.copied = false;
                self.client_access.error = None;
            }
            Err(error) => self.client_access.error = Some(error.to_string()),
        }
        self.finish_client_access_request(cx);
    }

    fn finish_client_access_request(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.client_access.cancel) && self.client_access_available() {
            self.client_access.busy = self.send(ServerId::local(), Work::CancelPairing, cx);
        }
        if std::mem::take(&mut self.client_access.stale) {
            self.read_remote_access(cx);
        }
        if let Some(error) = self
            .client_access
            .error
            .clone()
            .filter(|_| self.client_access.copy_link)
        {
            self.client_access.copy_link = false;
            self.fail(error, cx);
        }
        self.continue_link_copy(cx);
        self.settle_restored_access(cx);
        self.continue_onboarding_pairing(cx);
        cx.notify();
    }

    pub(super) fn settle_restored_access(&mut self, cx: &mut Context<Self>) {
        if !self.client_access.restore_off
            || self.client_access.busy
            || !self.client_access_available()
        {
            return;
        }
        let Some(enabled) = self
            .client_access
            .state
            .as_ref()
            .map(|state| state.settings.enabled)
        else {
            if self.client_access.error.is_none() {
                self.read_remote_access(cx);
            }
            return;
        };
        if !enabled {
            self.client_access.restore_off = false;
        } else if self.client_access.error.is_none() {
            self.set_client_access(false, cx);
        }
    }
}
