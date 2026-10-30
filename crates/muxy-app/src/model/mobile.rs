use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{AnyWindowHandle, ClipboardItem, Context, Task, Window};
use muxy_app_core::ServerId;
use muxy_protocol::{
    DeviceId, ListenerStatus, PairingOffer, RemoteAccessSettings, RemoteAccessState,
};
use muxy_ui::tr;

use super::{AppModel, Quitting};
use crate::boot::Work;
use crate::views::devices::Pairing;

/// Device access as the Devices tab shows it; the server owns the real state.
#[derive(Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The running request and the follow-ups it owes"
)]
pub(crate) struct MobileAccess {
    pub(crate) state: Option<RemoteAccessState>,
    /// The pairing code on screen, until it is used, cancelled, or expires.
    pub(crate) pairing: Option<Pairing>,
    pub(crate) busy: bool,
    /// The running request creates a pairing code.
    starting: bool,
    /// The server reported a change while a request was running.
    stale: bool,
    /// The user cancelled pairing while a request was running.
    cancel: bool,
    /// The pairing link goes to the clipboard once there is a code.
    copy_link: bool,
    pub(crate) error: Option<String>,
    expiry: Option<Task<()>>,
}

impl AppModel {
    pub(crate) fn device_access(&self) -> &MobileAccess {
        &self.mobile
    }

    pub(crate) fn mobile_available(&self) -> bool {
        self.quitting == Quitting::Idle && self.ready(ServerId::local())
    }

    fn devices_tab_open(&self) -> bool {
        self.state.servers_tab().is_some()
    }

    /// Reads device access when the Devices tab opens or this computer's
    /// server connects with it open, and withdraws a pairing code once the
    /// tab closes.
    pub(super) fn sync_devices_tab(&mut self, opened: bool, cx: &mut Context<Self>) {
        if !self.devices_tab_open() {
            self.withdraw_pairing(cx);
        } else if opened {
            self.read_remote_access(cx);
        }
    }

    pub(crate) fn read_remote_access(&mut self, cx: &mut Context<Self>) {
        if !self.mobile_available() {
            return;
        }
        if self.mobile.busy {
            self.mobile.stale = true;
            return;
        }
        self.mobile.busy = self.send(ServerId::local(), Work::ReadRemoteAccess, cx);
        cx.notify();
    }

    /// Paired devices connected, disconnected, or changed; refresh only while the Devices tab is open.
    pub(super) fn remote_access_changed(&mut self, cx: &mut Context<Self>) {
        if self.devices_tab_open() {
            self.read_remote_access(cx);
        }
    }

    /// Turns access for other devices on or off. A refusal shows in the tab.
    pub(crate) fn set_device_access(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let result = self.write_remote_access(
            |settings| RemoteAccessSettings {
                enabled,
                ..settings
            },
            cx,
        );
        self.finish_access_change(result, cx);
    }

    /// Moves access for other devices to the typed port.
    pub(crate) fn set_device_port(&mut self, port: &str, cx: &mut Context<Self>) {
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
        self.mobile.error = result.err();
        cx.notify();
    }

    fn write_remote_access(
        &mut self,
        change: impl FnOnce(RemoteAccessSettings) -> RemoteAccessSettings,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !self.mobile_available() {
            return Err(tr!("Connect to the server before changing device access").to_string());
        }
        if self.mobile.busy {
            return Err(tr!("Wait for the current device access change to finish").to_string());
        }
        let current = self
            .mobile
            .state
            .as_ref()
            .map(|state| state.settings)
            .ok_or_else(|| tr!("Load device access first").to_string())?;
        let settings = change(current);
        settings
            .validate()
            .map_err(|_| tr!("Use a port from 1024 to 65535").to_string())?;
        if settings != current {
            self.mobile.busy = self.send(ServerId::local(), Work::WriteRemoteAccess(settings), cx);
            if !self.mobile.busy {
                return Err(tr!("Could not send the change to the server").to_string());
            }
        }
        Ok(())
    }

    pub(crate) fn show_pairing_code(&mut self, cx: &mut Context<Self>) {
        if self.mobile_available() && !self.mobile.busy {
            self.mobile.error = None;
            self.mobile.busy = self.send(ServerId::local(), Work::StartPairing, cx);
            self.mobile.starting = self.mobile.busy;
            cx.notify();
        }
    }

    /// Copies this computer's pairing link. The code shows in the Devices
    /// tab, since it is valid only while it is on screen.
    pub(crate) fn copy_pairing_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_servers_tab(window, cx);
        if self.devices_tab_open() {
            self.mobile.copy_link = true;
            self.continue_link_copy(cx);
        }
    }

    /// Copies the code's link once there is one, asks for a code once access
    /// is listening, or says why there can't be one.
    fn continue_link_copy(&mut self, cx: &mut Context<Self>) {
        if !self.mobile.copy_link || self.mobile.busy {
            return;
        }
        if !self.mobile_available() {
            self.mobile.copy_link = false;
            return;
        }
        if let Some(pairing) = &self.mobile.pairing {
            cx.write_to_clipboard(ClipboardItem::new_string(pairing.link().to_owned()));
            self.mobile.copy_link = false;
            self.show_notice(tr!("Pairing link copied").to_string(), cx);
            return;
        }
        let Some(state) = &self.mobile.state else {
            return;
        };
        match state.status {
            ListenerStatus::Listening => self.show_pairing_code(cx),
            ListenerStatus::Disabled if state.settings.enabled => {}
            _ => {
                self.mobile.copy_link = false;
                self.mobile.error =
                    Some(tr!("Turn on Allow other devices to get a pairing link.").to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn cancel_pairing(&mut self, cx: &mut Context<Self>) {
        self.mobile.copy_link = false;
        self.mobile.pairing = None;
        self.mobile.expiry = None;
        if self.mobile.busy {
            self.mobile.cancel = true;
        } else if self.mobile_available() {
            self.mobile.busy = self.send(ServerId::local(), Work::CancelPairing, cx);
        }
        cx.notify();
    }

    /// A code must not stay valid on the server after the Devices tab closes.
    fn withdraw_pairing(&mut self, cx: &mut Context<Self>) {
        if self.mobile.pairing.is_some() || self.mobile.starting {
            self.cancel_pairing(cx);
        }
    }

    pub(crate) fn confirm_revoke(
        &mut self,
        device: DeviceId,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some(name) = self.mobile.state.as_ref().and_then(|state| {
            state
                .devices
                .iter()
                .find(|paired| paired.id == device)
                .map(|paired| paired.name.clone())
        }) else {
            return;
        };
        if self.close_prompt.is_some() {
            return;
        }
        let generation = self.servers.local.generation;
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = crate::views::confirm::prompt_revoke(window, &name, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.finish_confirmation(cx);
                match response {
                    Ok(true)
                        if generation == model.servers.local.generation
                            && model.mobile_available() =>
                    {
                        if model.mobile.busy {
                            model.mobile.error = Some(
                                tr!("Wait for the current device access change to finish")
                                    .to_string(),
                            );
                        } else {
                            model.mobile.busy =
                                model.send(ServerId::local(), Work::RevokeDevice(device), cx);
                        }
                    }
                    Err(error) => model.mobile.error = Some(error),
                    Ok(_) => {}
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn receive_remote_access(
        &mut self,
        result: Result<RemoteAccessState, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.mobile.busy = false;
        match result {
            Ok(state) => {
                // The code was used, cancelled elsewhere, or expired.
                if state.pairing_expires_at.is_none() {
                    self.mobile.pairing = None;
                    self.mobile.expiry = None;
                }
                self.mobile.error = None;
                self.mobile.state = Some(state);
                self.show_device_port(cx);
            }
            Err(error) => self.mobile.error = Some(error.to_string()),
        }
        self.finish_mobile_request(cx);
    }

    pub(super) fn receive_pairing(
        &mut self,
        result: Result<PairingOffer, muxy_client::ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.mobile.busy = false;
        self.mobile.starting = false;
        match result {
            Ok(_) if self.mobile.cancel || !self.devices_tab_open() => {
                self.mobile.cancel = true;
            }
            Ok(offer) => {
                let lifetime = offer.expires_at.saturating_sub(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                );
                self.mobile.expiry = Some(cx.spawn(async move |model, cx| {
                    cx.background_executor()
                        .timer(Duration::from_secs(lifetime))
                        .await;
                    let _ = model.update(cx, |model, cx| {
                        model.mobile.pairing = None;
                        model.read_remote_access(cx);
                        cx.notify();
                    });
                }));
                self.mobile.pairing = Some(Pairing::new(&offer));
                self.mobile.error = None;
            }
            Err(error) => self.mobile.error = Some(error.to_string()),
        }
        self.finish_mobile_request(cx);
    }

    fn finish_mobile_request(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.mobile.cancel) && self.mobile_available() {
            self.mobile.busy = self.send(ServerId::local(), Work::CancelPairing, cx);
        }
        if std::mem::take(&mut self.mobile.stale) {
            self.read_remote_access(cx);
        }
        if self.mobile.error.is_some() {
            self.mobile.copy_link = false;
        }
        self.continue_link_copy(cx);
        cx.notify();
    }
}
