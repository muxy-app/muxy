use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{AnyWindowHandle, Context, Task};
use muxy_protocol::{DeviceId, PairingOffer, RemoteAccessSettings, RemoteAccessState};

use super::{AppModel, ConnectionState, Quitting};
use crate::boot::Work;
use crate::views::settings::Change;
use crate::views::settings::mobile::Pairing;

/// Mobile access as Settings shows it; the server owns the real state.
#[derive(Default)]
pub(super) struct MobileAccess {
    pub(super) state: Option<RemoteAccessState>,
    /// The pairing code on screen, until it is used, cancelled, or expires.
    pub(super) pairing: Option<Pairing>,
    pub(super) busy: bool,
    /// The server reported a change while a request was running.
    stale: bool,
    /// The user cancelled pairing while a request was running.
    cancel: bool,
    pub(super) error: Option<String>,
    expiry: Option<Task<()>>,
}

impl AppModel {
    fn mobile_available(&self) -> bool {
        self.quitting == Quitting::Idle && self.connection == ConnectionState::Ready
    }

    pub(crate) fn read_remote_access(&mut self, cx: &mut Context<Self>) {
        if !self.mobile_available() {
            return;
        }
        if self.mobile.busy {
            self.mobile.stale = true;
            return;
        }
        self.mobile.busy = self.send(Work::ReadRemoteAccess, cx);
        self.sync_preferences(cx);
    }

    /// Paired devices connected, disconnected, or changed; refresh only while Settings is open.
    pub(super) fn remote_access_changed(&mut self, cx: &mut Context<Self>) {
        if self.settings_window.is_some() {
            self.read_remote_access(cx);
        }
    }

    pub(super) fn write_mobile_preference(
        &mut self,
        change: Change,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !self.mobile_available() {
            return Err("Connect to the server before changing mobile access".into());
        }
        if self.mobile.busy {
            return Err("Wait for the current mobile access change to finish".into());
        }
        let current = self
            .mobile
            .state
            .as_ref()
            .map(|state| state.settings)
            .ok_or("Load mobile access first")?;
        let settings = match change {
            Change::MobileAccess(enabled) => RemoteAccessSettings { enabled, ..current },
            Change::Field("mobile-port", port) => RemoteAccessSettings {
                port: port
                    .parse()
                    .map_err(|_| "Use a port from 1024 to 65535".to_owned())?,
                ..current
            },
            _ => return Err("Unknown mobile access setting".into()),
        };
        settings
            .validate()
            .map_err(|_| "Use a port from 1024 to 65535".to_owned())?;
        if settings != current {
            self.mobile.busy = self.send(Work::WriteRemoteAccess(settings), cx);
            if !self.mobile.busy {
                return Err("Could not send the change to the server".into());
            }
        }
        Ok(())
    }

    pub(crate) fn pair_phone(&mut self, cx: &mut Context<Self>) {
        if self.mobile_available() && !self.mobile.busy {
            self.mobile.error = None;
            self.mobile.busy = self.send(Work::StartPairing, cx);
            self.sync_preferences(cx);
        }
    }

    pub(crate) fn cancel_pairing(&mut self, cx: &mut Context<Self>) {
        self.mobile.pairing = None;
        self.mobile.expiry = None;
        if self.mobile.busy {
            self.mobile.cancel = true;
        } else if self.mobile_available() {
            self.mobile.busy = self.send(Work::CancelPairing, cx);
        }
        self.sync_preferences(cx);
    }

    /// A code must not stay valid on the server after Settings closes.
    pub(super) fn withdraw_pairing(&mut self, cx: &mut Context<Self>) {
        if self.mobile.pairing.is_some() {
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
        let generation = self.generation;
        self.close_prompt = Some(cx.spawn(async move |model, cx| {
            let response = crate::views::confirm::prompt_revoke(window, &name, cx).await;
            let _ = model.update(cx, |model, cx| {
                model.close_prompt = None;
                match response {
                    Ok(true) if generation == model.generation && model.mobile_available() => {
                        if model.mobile.busy {
                            model.mobile.error =
                                Some("Wait for the current mobile access change to finish".into());
                        } else {
                            model.mobile.busy = model.send(Work::RevokeDevice(device), cx);
                        }
                    }
                    Err(error) => model.mobile.error = Some(error),
                    Ok(_) => {}
                }
                model.sync_preferences(cx);
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
        match result {
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
                        model.sync_preferences(cx);
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
            self.mobile.busy = self.send(Work::CancelPairing, cx);
        }
        if std::mem::take(&mut self.mobile.stale) {
            self.read_remote_access(cx);
        }
        self.sync_preferences(cx);
        cx.notify();
    }
}
