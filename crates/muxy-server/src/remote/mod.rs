//! Network access for paired devices: settings, identity, pairing, and admission.

mod hosts;
mod identity;
mod store;

use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use muxy_protocol::transport::tls::TlsIdentity;
use muxy_protocol::{
    ClientId, DeviceCredential, DeviceId, DeviceKind, ErrorCode, ListenerStatus, MAX_DEVICES,
    PairRequest, Paired, PairedDevice, PairingInvite, PairingOffer, PushStatus,
    RemoteAccessSettings, RemoteAccessState, ServerIdentity, Topic, UnreadableFile,
};

use crate::ServerError;
use crate::notifier::Notifier;
use crate::push::Credentials;
pub(crate) use store::PushRecord;
use store::{Device, Stored};

const PAIRING_SECONDS: u64 = 5 * 60;
/// How long phones' push registrations are kept while push notifications are off.
pub(crate) const PUSH_KEPT_SECONDS: u64 = 90 * 24 * 60 * 60;
const MAX_UNAUTHENTICATED: usize = 16;
const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(10);

/// Starts, restarts, or stops the network listener; installed by the executable.
pub(crate) type ListenerControl =
    dyn Fn(Option<(u16, TlsIdentity)>) -> ListenerStatus + Send + Sync;

pub struct RemoteAccess {
    state: Mutex<State>,
    store: Option<PathBuf>,
    listener: Option<Box<ListenerControl>>,
    writes: Mutex<()>,
    revision: AtomicU64,
    unauthenticated: Arc<AtomicUsize>,
    notifier: Arc<Notifier>,
    unreadable: Option<UnreadableFile>,
    /// Set as the server stops, so nothing saves after a newer server may run.
    closed: std::sync::atomic::AtomicBool,
}

impl std::fmt::Debug for RemoteAccess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RemoteAccess")
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

struct State {
    stored: Stored,
    status: ListenerStatus,
    offer: Option<Offer>,
}

struct Offer {
    secret: [u8; 16],
    expires_at: u64,
    owner: ClientId,
}

/// What switching push notifications changed.
#[derive(Debug, Default)]
pub(crate) struct PushSwitch {
    /// Relay registrations to delete, when they were switched off.
    pub(crate) deleted: Vec<Credentials>,
    /// Phones to register again, when they were switched on.
    pub(crate) resumed: Vec<DeviceId>,
}

/// Holds one of the few slots for connections that have not authenticated yet.
#[derive(Debug)]
pub struct Admission {
    count: Arc<AtomicUsize>,
    pub(crate) timeout: Duration,
}

impl Drop for Admission {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::AcqRel);
    }
}

impl RemoteAccess {
    pub fn memory() -> Self {
        Self::with_store(None, Stored::default(), None)
    }

    /// Opens the saved state; contents this build can't understand are moved
    /// aside and access starts off.
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let Some(bytes) = store::read(&path)? else {
            return Ok(Self::with_store(Some(path), Stored::default(), None));
        };
        Ok(match store::parse(&path, &bytes) {
            Ok(stored) => Self::with_store(Some(path), stored, None),
            Err(error) => {
                let unreadable = crate::unreadable::set_aside(&path, &error)?;
                Self::with_store(Some(path), Stored::default(), Some(unreadable))
            }
        })
    }

    fn with_store(
        store: Option<PathBuf>,
        stored: Stored,
        unreadable: Option<UnreadableFile>,
    ) -> Self {
        Self {
            state: Mutex::new(State {
                stored,
                status: ListenerStatus::Disabled,
                offer: None,
            }),
            store,
            listener: None,
            writes: Mutex::new(()),
            revision: AtomicU64::new(1),
            unauthenticated: Arc::default(),
            notifier: Arc::default(),
            unreadable,
            closed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Stops saving. Every save happens under the state lock, so once this
    /// returns, none is under way and none follows.
    pub(crate) fn close(&self) {
        let _state = self.lock();
        self.closed.store(true, Ordering::Release);
    }

    /// Access that is on, with `phones` paired, each with the token `[7; 32]`.
    #[cfg(test)]
    pub(crate) fn with_phones(phones: &[DeviceId]) -> Self {
        let remote = Self::memory();
        {
            let mut state = remote.lock();
            state.stored.settings.enabled = true;
            state.stored.devices = phones
                .iter()
                .map(|id| Device {
                    id: *id,
                    name: "Phone".into(),
                    token_hash: hash(&[7; 32]),
                    paired_at: 0,
                    last_seen: None,
                    kind: DeviceKind::Phone,
                    push: None,
                })
                .collect();
        }
        remote
    }

    #[cfg(test)]
    pub(crate) fn set_accepting(&self, enabled: bool) {
        self.lock().stored.settings.enabled = enabled;
    }

    /// The saved state that couldn't be read when the server started.
    pub(crate) fn unreadable(&self) -> Option<&UnreadableFile> {
        self.unreadable.as_ref()
    }

    pub(crate) fn set_listener(&mut self, listener: Box<ListenerControl>) {
        self.listener = Some(listener);
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    pub(crate) fn set_notifier(&mut self, notifier: Arc<Notifier>) {
        self.notifier = notifier;
    }

    pub(crate) fn changed(&self) {
        let revision = self.revision.fetch_add(1, Ordering::AcqRel) + 1;
        self.notifier.notify(Topic::RemoteAccess, revision);
    }

    pub(crate) fn state(&self, connected: &HashSet<DeviceId>) -> RemoteAccessState {
        let now = now();
        let state = self.lock();
        RemoteAccessState {
            revision: self.revision(),
            settings: state.stored.settings,
            status: state.status.clone(),
            pairing_expires_at: state
                .offer
                .as_ref()
                .map(|offer| offer.expires_at)
                .filter(|expires_at| *expires_at > now),
            devices: state
                .stored
                .devices
                .iter()
                .map(|device| PairedDevice {
                    id: device.id,
                    name: device.name.clone(),
                    paired_at: device.paired_at,
                    last_seen: device.last_seen,
                    connected: connected.contains(&device.id),
                    kind: device.kind,
                    push: device.push.as_ref().map(|push| {
                        if state.stored.push_notifications {
                            push.status
                        } else {
                            PushStatus::Disabled
                        }
                    }),
                })
                .collect(),
            push_notifications: state.stored.push_notifications,
            push_kept_until: state
                .stored
                .devices
                .iter()
                .any(|device| device.push.is_some())
                .then_some(state.stored.push_off_since)
                .flatten()
                .map(|since| since.saturating_add(PUSH_KEPT_SECONDS)),
        }
    }

    /// Applies the saved settings when the server starts.
    pub(crate) fn resume(&self) {
        let listening = {
            let state = self.lock();
            state
                .stored
                .settings
                .enabled
                .then(|| state.stored.identity.clone())
                .flatten()
                .map(|identity| (state.stored.settings.port, identity))
        };
        if listening.is_some() {
            self.apply(listening);
        }
    }

    pub(crate) fn configure(
        &self,
        settings: RemoteAccessSettings,
    ) -> Result<ListenerStatus, ServerError> {
        settings
            .validate()
            .map_err(|code| ServerError::new(code, "port must be between 1024 and 65535"))?;
        let _write = self.writes.lock().unwrap_or_else(PoisonError::into_inner);
        let listening = {
            let mut state = self.lock();
            let mut stored = state.stored.clone();
            stored.settings = settings;
            if settings.enabled && stored.identity.is_none() {
                stored.identity = Some(identity::generate()?);
            }
            self.save(&stored)?;
            if !settings.enabled {
                state.offer = None;
            }
            state.stored = stored;
            settings
                .enabled
                .then(|| state.stored.identity.clone())
                .flatten()
                .map(|identity| (settings.port, identity))
        };
        self.changed();
        Ok(self.apply(listening))
    }

    fn apply(&self, listening: Option<(u16, TlsIdentity)>) -> ListenerStatus {
        let status = match (&self.listener, listening) {
            (Some(listener), listening) => listener(listening),
            (None, Some(_)) => ListenerStatus::Failed("the network listener is unavailable".into()),
            (None, None) => ListenerStatus::Disabled,
        };
        self.lock().status = status.clone();
        self.changed();
        status
    }

    /// Offers a pairing code whose link lists the `given` addresses first,
    /// for a server that phones reach by another name, such as behind NAT.
    pub(crate) fn start_pairing(
        &self,
        owner: ClientId,
        given: &[String],
    ) -> Result<PairingOffer, ServerError> {
        let hosts = hosts::candidates(given);
        let mut state = self.lock();
        let Some(identity) = state.stored.identity.as_ref() else {
            return Err(unavailable("Device access is off"));
        };
        if !state.stored.settings.enabled || state.status != ListenerStatus::Listening {
            return Err(unavailable("Device access is not listening"));
        }
        if state.stored.devices.len() >= MAX_DEVICES {
            return Err(unavailable("Revoke a paired device before pairing another"));
        }
        let invite = PairingInvite {
            hosts,
            port: state.stored.settings.port,
            fingerprint: identity.fingerprint(),
            secret: random()?,
        };
        invite
            .validate()
            .map_err(|_| unavailable("No network address found for pairing"))?;
        let expires_at = now() + PAIRING_SECONDS;
        state.offer = Some(Offer {
            secret: invite.secret,
            expires_at,
            owner,
        });
        drop(state);
        self.changed();
        Ok(PairingOffer { invite, expires_at })
    }

    pub(crate) fn cancel_pairing(&self) {
        if self.lock().offer.take().is_some() {
            self.changed();
        }
    }

    /// Cancels the pending offer when the connection that created it goes away.
    pub(crate) fn release_pairing(&self, owner: ClientId) {
        let mut state = self.lock();
        if state
            .offer
            .as_ref()
            .is_some_and(|offer| offer.owner == owner)
        {
            state.offer = None;
            drop(state);
            self.changed();
        }
    }

    pub(crate) fn pair(&self, request: &PairRequest, server: ServerIdentity) -> Option<Paired> {
        self.pair_at(request, server, now())
    }

    fn pair_at(&self, request: &PairRequest, server: ServerIdentity, now: u64) -> Option<Paired> {
        let mut state = self.lock();
        if !state.stored.settings.enabled {
            return None;
        }
        let offer = state.offer.as_ref()?;
        if offer.expires_at <= now {
            state.offer = None;
            return None;
        }
        if !same(&offer.secret, &request.secret) || state.stored.devices.len() >= MAX_DEVICES {
            return None;
        }
        let token: [u8; 32] = random().ok()?;
        let id = DeviceId::new();
        let mut stored = state.stored.clone();
        stored.devices.push(Device {
            id,
            name: request.name.trim().to_owned(),
            token_hash: hash(&token),
            paired_at: now,
            last_seen: Some(now),
            kind: store::StoredKind::from(request.kind).into(),
            push: None,
        });
        if let Err(error) = self.save(&stored) {
            log::error!("pairing not saved: {error}");
            return None;
        }
        state.stored = stored;
        state.offer = None;
        drop(state);
        self.changed();
        let name = hosts::display_name();
        let paired = Paired {
            credential: DeviceCredential { device: id, token },
            server,
            name,
        };
        if paired.validate().is_ok() {
            Some(paired)
        } else {
            Some(Paired {
                name: "Muxy".into(),
                ..paired
            })
        }
    }

    /// The same answer for every failure, and the same work whether or not the device exists.
    pub(crate) fn authenticate(
        &self,
        credential: &DeviceCredential,
    ) -> Option<(DeviceId, DeviceKind)> {
        let presented = hash(&credential.token);
        let now = now();
        let mut state = self.lock();
        let usable = state.stored.settings.enabled;
        let device = state
            .stored
            .devices
            .iter()
            .position(|device| device.id == credential.device);
        let expected = device.map_or([0; 32], |index| state.stored.devices[index].token_hash);
        let matched = same(&presented, &expected);
        let index = device.filter(|_| usable && matched)?;
        let kind = state.stored.devices[index].kind;
        let mut stored = state.stored.clone();
        stored.devices[index].last_seen = Some(now);
        if let Err(error) = self.save(&stored) {
            log::warn!("last seen time not saved: {error}");
        } else {
            state.stored = stored;
        }
        drop(state);
        self.changed();
        Some((credential.device, kind))
    }

    pub(crate) fn authorized(&self, device: DeviceId) -> bool {
        let state = self.lock();
        state.stored.settings.enabled
            && state
                .stored
                .devices
                .iter()
                .any(|stored| stored.id == device)
    }

    /// Forgets the device, and hands back its push registration for deleting at the relay.
    pub(crate) fn revoke(&self, device: DeviceId) -> Result<Option<PushRecord>, ServerError> {
        let mut state = self.lock();
        let mut stored = state.stored.clone();
        let Some(index) = stored.devices.iter().position(|stored| stored.id == device) else {
            return Ok(None);
        };
        let removed = stored.devices.remove(index);
        self.save(&stored)?;
        state.stored = stored;
        drop(state);
        self.changed();
        Ok(removed.push)
    }

    /// Whether paired devices may connect, which push notifications need to be useful.
    pub(crate) fn accepting(&self) -> bool {
        self.lock().stored.settings.enabled
    }

    pub(crate) fn push_notifications(&self) -> bool {
        self.lock().stored.push_notifications
    }

    /// Turns push notifications on or off for every phone. Off pauses them:
    /// the relay registrations come back for deleting, and each phone's
    /// token and key stay for `PUSH_KEPT_SECONDS`. On names the phones to
    /// register again.
    pub(crate) fn set_push_notifications(&self, on: bool) -> Result<PushSwitch, ServerError> {
        let mut state = self.lock();
        if state.stored.push_notifications == on {
            return Ok(PushSwitch::default());
        }
        let mut stored = state.stored.clone();
        stored.push_notifications = on;
        stored.push_off_since = (!on).then(now);
        let mut switch = PushSwitch::default();
        for device in &mut stored.devices {
            let Some(push) = &mut device.push else {
                continue;
            };
            if on {
                switch.resumed.push(device.id);
            } else {
                switch.deleted.extend(push.relay.take());
                push.status = PushStatus::Pending;
            }
        }
        self.save(&stored)?;
        state.stored = stored;
        drop(state);
        self.changed();
        Ok(switch)
    }

    /// Forgets the phones' kept registrations once push notifications have
    /// been off for `PUSH_KEPT_SECONDS`. Whether any were forgotten.
    pub(crate) fn expire_paused_push(&self, now: u64) -> Result<bool, ServerError> {
        let mut state = self.lock();
        let expired = !state.stored.push_notifications
            && state
                .stored
                .push_off_since
                .is_some_and(|since| since.saturating_add(PUSH_KEPT_SECONDS) <= now)
            && state
                .stored
                .devices
                .iter()
                .any(|device| device.push.is_some());
        if !expired {
            return Ok(false);
        }
        let mut stored = state.stored.clone();
        for device in &mut stored.devices {
            device.push = None;
        }
        self.save(&stored)?;
        state.stored = stored;
        drop(state);
        self.changed();
        Ok(true)
    }

    pub(crate) fn knows(&self, device: DeviceId) -> bool {
        self.lock()
            .stored
            .devices
            .iter()
            .any(|stored| stored.id == device)
    }

    pub(crate) fn push_record(&self, device: DeviceId) -> Option<PushRecord> {
        self.lock()
            .stored
            .devices
            .iter()
            .find(|stored| stored.id == device)
            .and_then(|stored| stored.push.clone())
    }

    /// Every device's push registration.
    pub(crate) fn push_records(&self) -> Vec<(DeviceId, PushRecord)> {
        self.lock()
            .stored
            .devices
            .iter()
            .filter_map(|device| device.push.clone().map(|push| (device.id, push)))
            .collect()
    }

    /// Replaces the device's push registration only while it is still
    /// `expected`, so work based on an older one never overwrites a newer
    /// one. `false` when it changed or the device is gone.
    pub(crate) fn replace_push(
        &self,
        device: DeviceId,
        expected: Option<&PushRecord>,
        next: Option<PushRecord>,
    ) -> Result<bool, ServerError> {
        let mut state = self.lock();
        let Some(index) = state
            .stored
            .devices
            .iter()
            .position(|stored| stored.id == device)
        else {
            return Ok(false);
        };
        let current = &state.stored.devices[index].push;
        let registering = next.as_ref().is_some_and(|push| push.relay.is_some());
        if current.as_ref() != expected || (registering && !state.stored.push_notifications) {
            return Ok(false);
        }
        if *current == next {
            return Ok(true);
        }
        let shown =
            current.as_ref().map(|push| push.status) != next.as_ref().map(|push| push.status);
        let mut stored = state.stored.clone();
        stored.devices[index].push = next;
        self.save(&stored)?;
        state.stored = stored;
        drop(state);
        if shown {
            self.changed();
        }
        Ok(true)
    }

    pub(crate) fn admit(&self) -> Option<Admission> {
        self.admit_with(AUTHENTICATION_TIMEOUT)
    }

    pub(crate) fn admit_with(&self, timeout: Duration) -> Option<Admission> {
        self.unauthenticated
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_UNAUTHENTICATED).then_some(count + 1)
            })
            .ok()?;
        Some(Admission {
            count: Arc::clone(&self.unauthenticated),
            timeout,
        })
    }

    fn save(&self, stored: &Stored) -> Result<(), ServerError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(ServerError::new(
                ErrorCode::PersistenceFailed,
                "the server is stopping",
            ));
        }
        match &self.store {
            Some(path) => store::save(path, stored).map_err(|error| {
                ServerError::new(
                    ErrorCode::PersistenceFailed,
                    format!("could not save device access: {error}"),
                )
            }),
            None => Ok(()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn unavailable(message: &str) -> ServerError {
    ServerError::new(ErrorCode::BadRequest, message)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn random<const N: usize>() -> Result<[u8; N], ServerError> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|error| {
        ServerError::new(ErrorCode::BadRequest, format!("no randomness: {error}"))
    })?;
    Ok(bytes)
}

fn hash(token: &[u8]) -> [u8; 32] {
    let digest = ring::digest::digest(&ring::digest::SHA256, token);
    let mut bytes = [0; 32];
    bytes.copy_from_slice(digest.as_ref());
    bytes
}

/// Compares secrets in time independent of where they differ.
fn same<const N: usize>(left: &[u8; N], right: &[u8; N]) -> bool {
    left.iter()
        .zip(right)
        .fold(0, |difference, (left, right)| difference | (left ^ right))
        == 0
}

#[cfg(test)]
mod tests;
