//! Network access for paired devices: settings, identity, pairing, and admission.

mod hosts;
mod identity;
mod store;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use muxy_protocol::transport::tls::TlsIdentity;
use muxy_protocol::{
    ClientId, DeviceCredential, DeviceId, ErrorCode, ListenerStatus, MAX_DEVICES, PairRequest,
    Paired, PairedDevice, PairingInvite, PairingOffer, RemoteAccessSettings, RemoteAccessState,
    ServerIdentity,
};

use crate::ServerError;
use store::{Device, Stored};

const PAIRING_SECONDS: u64 = 5 * 60;
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
    /// Why the saved state could not be read; access stays off until rewritten.
    unreadable: Option<String>,
}

struct Offer {
    secret: [u8; 16],
    expires_at: u64,
    owner: ClientId,
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

    /// Opens the saved state; an unreadable file keeps access off and is reported, not replaced.
    pub fn open(path: PathBuf) -> Self {
        match store::load(&path) {
            Ok(stored) => Self::with_store(Some(path), stored, None),
            Err(error) => {
                log::error!("remote access disabled: {error}");
                let unreadable = format!("{}: {error}", path.display());
                Self::with_store(Some(path), Stored::default(), Some(unreadable))
            }
        }
    }

    fn with_store(store: Option<PathBuf>, stored: Stored, unreadable: Option<String>) -> Self {
        let status = unreadable
            .clone()
            .map_or(ListenerStatus::Disabled, ListenerStatus::Failed);
        Self {
            state: Mutex::new(State {
                stored,
                status,
                offer: None,
                unreadable,
            }),
            store,
            listener: None,
            writes: Mutex::new(()),
            revision: AtomicU64::new(1),
            unauthenticated: Arc::default(),
        }
    }

    pub(crate) fn set_listener(&mut self, listener: Box<ListenerControl>) {
        self.listener = Some(listener);
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision.load(Ordering::Acquire)
    }

    pub(crate) fn changed(&self) {
        self.revision.fetch_add(1, Ordering::AcqRel);
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
                })
                .collect(),
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
            // Access is already off; keep the unreadable file for recovery.
            if state.unreadable.is_some() && !settings.enabled {
                return Ok(state.status.clone());
            }
            let mut stored = if state.unreadable.is_some() {
                Stored::default()
            } else {
                state.stored.clone()
            };
            stored.settings = settings;
            if settings.enabled && stored.identity.is_none() {
                stored.identity = Some(identity::generate()?);
            }
            self.save(&stored)?;
            state.unreadable = None;
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

    pub(crate) fn start_pairing(&self, owner: ClientId) -> Result<PairingOffer, ServerError> {
        let hosts = hosts::candidates();
        let mut state = self.lock();
        let Some(identity) = state.stored.identity.as_ref() else {
            return Err(unavailable("Mobile access is off"));
        };
        if !state.stored.settings.enabled || state.status != ListenerStatus::Listening {
            return Err(unavailable("Mobile access is not listening"));
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
        if !state.stored.settings.enabled || state.unreadable.is_some() {
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
    pub(crate) fn authenticate(&self, credential: &DeviceCredential) -> Option<DeviceId> {
        let presented = hash(&credential.token);
        let now = now();
        let mut state = self.lock();
        let usable = state.stored.settings.enabled && state.unreadable.is_none();
        let device = state
            .stored
            .devices
            .iter()
            .position(|device| device.id == credential.device);
        let expected = device.map_or([0; 32], |index| state.stored.devices[index].token_hash);
        let matched = same(&presented, &expected);
        let index = device.filter(|_| usable && matched)?;
        let mut stored = state.stored.clone();
        stored.devices[index].last_seen = Some(now);
        if let Err(error) = self.save(&stored) {
            log::warn!("last seen time not saved: {error}");
        } else {
            state.stored = stored;
        }
        drop(state);
        self.changed();
        Some(credential.device)
    }

    pub(crate) fn authorized(&self, device: DeviceId) -> bool {
        let state = self.lock();
        state.stored.settings.enabled
            && state.unreadable.is_none()
            && state
                .stored
                .devices
                .iter()
                .any(|stored| stored.id == device)
    }

    pub(crate) fn revoke(&self, device: DeviceId) -> Result<(), ServerError> {
        let mut state = self.lock();
        let mut stored = state.stored.clone();
        stored.devices.retain(|stored| stored.id != device);
        if stored.devices.len() == state.stored.devices.len() {
            return Ok(());
        }
        self.save(&stored)?;
        state.stored = stored;
        drop(state);
        self.changed();
        Ok(())
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
        match &self.store {
            Some(path) => store::save(path, stored).map_err(|error| {
                ServerError::new(
                    ErrorCode::PersistenceFailed,
                    format!("could not save mobile access: {error}"),
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
