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

/// Client access as the Servers page shows it; the server owns the real state.
#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The running request and the follow-ups it owes"
)]
pub(crate) struct ClientAccess {
    pub(crate) state: Option<RemoteAccessState>,
    /// The current pairing code, until it is used, cancelled, or expires.
    pub(crate) pairing: Option<Pairing>,
    /// Settings is on the Servers page, whether or not its tab is displayed.
    page_open: bool,
    /// The Servers page was on screen when last checked.
    page_shown: bool,
    /// The code's link was copied, so it stays valid away from the Servers
    /// page until it is used or expires.
    copied: bool,
    pub(crate) busy: bool,
    /// The running request creates a pairing code.
    starting: bool,
    /// The server reported a change while a request was running.
    stale: bool,
    /// The user cancelled pairing while a request was running.
    cancel: bool,
    /// The pairing link goes to the clipboard once there is a fresh code.
    copy_link: bool,
    /// The welcome tour turned access on and nothing paired, so it goes back
    /// off once no request is running.
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

    /// What survives losing the server: the Servers page staying on screen,
    /// and the promise to turn access back off, which is kept once it returns.
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

    /// The Servers page is on screen, which focus elsewhere doesn't change.
    fn servers_page_open(&self) -> bool {
        self.client_access.page_open
            && self.state.settings_tab().is_some()
            && self
                .visible_panes()
                .into_iter()
                .any(|pane| self.state.is_settings_pane(pane))
    }

    /// Settings moved to the Servers page or away from it.
    pub(crate) fn set_servers_page_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if self.client_access.page_open != open {
            self.client_access.page_open = open;
            self.sync_servers_page(open, cx);
        }
    }

    /// Client access is on screen, on the Servers page or in the welcome tour.
    fn client_access_shown(&self) -> bool {
        self.servers_page_open() || self.onboarding_pairing_shown()
    }

    /// Reads client access when the Servers page comes on screen or this
    /// computer's server connects with it open, and withdraws a pairing code
    /// that was never copied once it leaves the screen.
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

    /// Paired clients connected, disconnected, or changed; refresh only while client access is on screen.
    pub(super) fn remote_access_changed(&mut self, cx: &mut Context<Self>) {
        if self.client_access_shown() {
            self.read_remote_access(cx);
        }
    }

    /// Turns client access on or off. A refusal shows on the page.
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

    /// Whether this computer's server can send push notifications to paired phones.
    pub(crate) fn push_notifications_available(&self) -> bool {
        self.client_access_available()
            && self.server_supports(
                ServerId::local(),
                muxy_protocol::Feature::PUSH_NOTIFICATIONS,
            )
    }

    /// Turns push notifications to paired phones on or off. Pausing phones
    /// that get them asks first, since they stop hearing about agents.
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

    /// Sends the switch; a refusal shows on the page.
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

    /// Forgets one phone's notification key after asking; it stays paired.
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

    /// Moves client access to the typed port.
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

    /// Copies this computer's pairing link where the user is, making a code
    /// when there is none.
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

    /// Copies the code's link once there is one, asks for a code once access
    /// is listening, or says why there can't be one.
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

    /// A code that was only shown must not stay valid on the server once it
    /// leaves the screen.
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
                // The code was used, cancelled elsewhere, or expired.
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

    /// Turns access back off once the server can take it, as the welcome tour
    /// asked. The promise is kept until the server reports access off. Without
    /// the server it waits for it to return, and after a refusal, for the next
    /// good answer.
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
