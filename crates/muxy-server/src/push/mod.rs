//! Push notifications to paired phones through the muxy.app relay.
//!
//! One thread makes every relay call, so registrations, deliveries, and
//! deletions never race. Requests change the saved registration themselves
//! and ask the thread to bring the relay in line.

mod payload;
mod relay;

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use muxy_protocol::{
    ActivityEvent, DeviceId, ErrorCode, PUSH_KEY_BYTES, PushPlatform, PushRegistration, PushStatus,
    PushToken,
};

pub(crate) use payload::PushKey;
pub(crate) use relay::{Credentials, HttpRelay, Outgoing, Relay, RelayError};

use crate::remote::PushRecord;
use crate::{Registry, ServerError};

/// How long an event waits for the user before the phone is told.
const GRACE: Duration = Duration::from_secs(20);
/// A notification's lifetime at APNs and FCM, and how long it is retried.
const TTL_SECONDS: u64 = 3600;
/// How long `RegisterPush` waits for the relay before answering `Pending`.
pub(crate) const REGISTER_WAIT: Duration = Duration::from_secs(12);
const FIRST_RETRY: Duration = Duration::from_secs(5);
const LAST_RETRY: Duration = Duration::from_secs(10 * 60);
const DELETE_ATTEMPTS: u32 = 5;
/// The thread checks this often whether the server is still there.
const IDLE: Duration = Duration::from_secs(60);

enum Command {
    /// Brings the relay in line with the device's saved registration.
    Resync {
        device: DeviceId,
        reply: Option<Sender<PushStatus>>,
    },
    Event(ActivityEvent),
    Delete(Credentials),
}

/// The relay, and the thread that talks to it once the server runs it.
pub(crate) struct Push {
    relay: Arc<dyn Relay>,
    timing: Timing,
    commands: Mutex<Option<Sender<Command>>>,
}

/// How long events wait for the user, and the first wait before a retry.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Timing {
    pub(crate) grace: Duration,
    pub(crate) retry: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            grace: GRACE,
            retry: FIRST_RETRY,
        }
    }
}

impl std::fmt::Debug for Push {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Push")
            .field("timing", &self.timing)
            .finish_non_exhaustive()
    }
}

impl Default for Push {
    fn default() -> Self {
        Self::new(Arc::new(HttpRelay::from_environment()), Timing::default())
    }
}

impl Push {
    pub(crate) fn new(relay: Arc<dyn Relay>, timing: Timing) -> Self {
        Self {
            relay,
            timing,
            commands: Mutex::new(None),
        }
    }

    fn send(&self, command: Command) -> bool {
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|commands| commands.send(command).is_ok())
    }

    fn delete(&self, credentials: Option<Credentials>) {
        if let Some(credentials) = credentials {
            self.send(Command::Delete(credentials));
        }
    }

    /// The device's status once the relay answers, or `None` if it takes
    /// longer than `wait`; the thread keeps trying either way.
    fn sync(&self, device: DeviceId, wait: Duration) -> Option<PushStatus> {
        let (reply, status) = mpsc::channel();
        if !self.send(Command::Resync {
            device,
            reply: Some(reply),
        }) {
            return None;
        }
        status.recv_timeout(wait).ok()
    }
}

impl Registry {
    /// Uses `relay` for push notifications, with `timing` instead of the real waits.
    #[cfg(test)]
    pub(crate) fn with_push_relay(mut self, relay: Arc<dyn Relay>, timing: Timing) -> Self {
        self.push = Push::new(relay, timing);
        self
    }

    /// Starts the push thread, which then hears about every new activity
    /// event and retries registrations left pending.
    pub fn start_push(self: &Arc<Self>) {
        let mut commands = self
            .push
            .commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if commands.is_some() {
            log::warn!("push notifications already started");
            return;
        }
        let (sender, receiver) = mpsc::channel();
        let worker = Worker {
            registry: Arc::downgrade(self),
            relay: Arc::clone(&self.push.relay),
            timing: self.push.timing,
            receiver,
            syncs: Vec::new(),
            events: Vec::new(),
            deliveries: Vec::new(),
            deletes: Vec::new(),
        };
        if let Err(error) = thread::Builder::new()
            .name("push".into())
            .spawn(move || worker.run())
        {
            log::error!("push notifications unavailable: {error}");
            return;
        }
        let events = sender.clone();
        self.activity.observe(move |event| {
            let _ = events.send(Command::Event(event.clone()));
        });
        for (device, record) in self.remote.push_records() {
            if record.status == PushStatus::Pending {
                let _ = sender.send(Command::Resync {
                    device,
                    reply: None,
                });
            }
        }
        *commands = Some(sender);
    }

    /// Keeps the phone's token and answers with its key, registering with
    /// the relay when the token is new or changed, or wasn't accepted yet.
    pub(crate) fn register_push(
        &self,
        device: DeviceId,
        token: PushToken,
        wait: Duration,
    ) -> Result<PushRegistration, ServerError> {
        let token = normalized(token);
        if !self.remote.push_notifications() {
            self.keep_paused_token(device, &token)?;
            return Ok(disabled());
        }
        let record = loop {
            let current = self.remote.push_record(device);
            let next = match &current {
                Some(record) if record.token == token && record.status == PushStatus::Active => {
                    return Ok(PushRegistration {
                        status: PushStatus::Active,
                        key: Some(record.key),
                    });
                }
                Some(record) => PushRecord {
                    token: token.clone(),
                    status: PushStatus::Pending,
                    ..record.clone()
                },
                None => PushRecord {
                    token: token.clone(),
                    key: new_key()?,
                    relay: None,
                    status: PushStatus::Pending,
                },
            };
            if !self.remote.push_notifications() {
                return Ok(disabled());
            }
            if self
                .remote
                .replace_push(device, current.as_ref(), Some(next.clone()))?
            {
                break next;
            }
            if !self.remote.knows(device) {
                return Err(ServerError::new(
                    ErrorCode::Unauthorized,
                    "this phone is no longer paired",
                ));
            }
        };
        let status = self.push.sync(device, wait).unwrap_or(PushStatus::Pending);
        Ok(match status {
            PushStatus::Disabled => disabled(),
            status => PushRegistration {
                status,
                key: Some(record.key),
            },
        })
    }

    /// Stops the phone's notifications and forgets its key.
    pub(crate) fn unregister_push(&self, device: DeviceId) -> Result<(), ServerError> {
        loop {
            let Some(current) = self.remote.push_record(device) else {
                return Ok(());
            };
            if self.remote.replace_push(device, Some(&current), None)? {
                self.push.delete(current.relay);
                return Ok(());
            }
        }
    }

    /// A paused phone's new token, so it is the one registered when
    /// notifications come back on. A phone with nothing kept stays without.
    fn keep_paused_token(&self, device: DeviceId, token: &PushToken) -> Result<(), ServerError> {
        loop {
            let Some(current) = self.remote.push_record(device) else {
                return Ok(());
            };
            if current.token == *token {
                return Ok(());
            }
            let next = PushRecord {
                token: token.clone(),
                status: PushStatus::Pending,
                ..current.clone()
            };
            if self
                .remote
                .replace_push(device, Some(&current), Some(next))?
            {
                return Ok(());
            }
        }
    }

    /// Off deletes the relay registrations and keeps each phone's token and
    /// key; on registers those phones again with the same keys, so they need
    /// not reconnect.
    pub(crate) fn set_push_notifications(
        &self,
        on: bool,
    ) -> Result<muxy_protocol::RemoteAccessState, ServerError> {
        let switch = self.remote.set_push_notifications(on)?;
        for credentials in switch.deleted {
            self.push.delete(Some(credentials));
        }
        for device in switch.resumed {
            self.push.send(Command::Resync {
                device,
                reply: None,
            });
        }
        Ok(self.remote_access_state())
    }

    /// Forgets one phone's push registration and key; it gets a new key when
    /// it next registers.
    pub(crate) fn reset_push(
        &self,
        device: DeviceId,
    ) -> Result<muxy_protocol::RemoteAccessState, ServerError> {
        if !self.remote.knows(device) {
            return Err(ServerError::new(ErrorCode::BadRequest, "no such device"));
        }
        self.unregister_push(device)?;
        Ok(self.remote_access_state())
    }

    /// Deletes a revoked phone's registration at the relay.
    pub(crate) fn forget_push(&self, record: Option<PushRecord>) {
        self.push.delete(record.and_then(|record| record.relay));
    }
}

fn disabled() -> PushRegistration {
    PushRegistration {
        status: PushStatus::Disabled,
        key: None,
    }
}

/// APNs tokens are hex in any case; the relay keeps them lowercase.
fn normalized(mut token: PushToken) -> PushToken {
    if token.platform == PushPlatform::Apns {
        token.token.make_ascii_lowercase();
    }
    token
}

fn new_key() -> Result<PushKey, ServerError> {
    let mut key = [0; PUSH_KEY_BYTES];
    getrandom::fill(&mut key).map_err(|error| {
        ServerError::new(ErrorCode::BadRequest, format!("no randomness: {error}"))
    })?;
    Ok(key)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Doubles from `first` up to `LAST_RETRY`, unless the relay said when.
fn backoff(first: Duration, attempt: u32, after: Option<Duration>) -> Duration {
    after.map_or_else(
        || first.saturating_mul(1 << attempt.min(16)).min(LAST_RETRY),
        |after| after.clamp(Duration::from_secs(1), Duration::from_secs(TTL_SECONDS)),
    )
}

struct Resync {
    device: DeviceId,
    due: Instant,
    attempt: u32,
    replies: Vec<Sender<PushStatus>>,
}

struct Pending {
    event: u64,
    due: Instant,
}

#[derive(Clone, Copy)]
struct Delivery {
    event: u64,
    device: DeviceId,
    due: Instant,
    attempt: u32,
}

struct Deletion {
    credentials: Credentials,
    due: Instant,
    attempt: u32,
}

struct Worker {
    registry: Weak<Registry>,
    relay: Arc<dyn Relay>,
    timing: Timing,
    receiver: Receiver<Command>,
    syncs: Vec<Resync>,
    events: Vec<Pending>,
    deliveries: Vec<Delivery>,
    deletes: Vec<Deletion>,
}

impl Worker {
    fn run(mut self) {
        loop {
            let wait = self
                .next_due()
                .map_or(IDLE, |due| due.saturating_duration_since(Instant::now()))
                .min(IDLE);
            match self.receiver.recv_timeout(wait) {
                Ok(command) => self.accept(command),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            while let Ok(command) = self.receiver.try_recv() {
                self.accept(command);
            }
            let Some(registry) = self.registry.upgrade() else {
                return;
            };
            self.work(&registry);
        }
    }

    fn next_due(&self) -> Option<Instant> {
        self.syncs
            .iter()
            .map(|sync| sync.due)
            .chain(self.events.iter().map(|event| event.due))
            .chain(self.deliveries.iter().map(|delivery| delivery.due))
            .chain(self.deletes.iter().map(|deletion| deletion.due))
            .min()
    }

    fn accept(&mut self, command: Command) {
        let now = Instant::now();
        match command {
            Command::Resync { device, reply } => self.schedule_sync(device, now, reply),
            Command::Event(event) => self.events.push(Pending {
                event: event.id,
                due: now + self.timing.grace,
            }),
            Command::Delete(credentials) => self.deletes.push(Deletion {
                credentials,
                due: now,
                attempt: 0,
            }),
        }
    }

    /// One sync per device: a newer request joins the one waiting, and runs now.
    fn schedule_sync(&mut self, device: DeviceId, due: Instant, reply: Option<Sender<PushStatus>>) {
        if let Some(sync) = self.syncs.iter_mut().find(|sync| sync.device == device) {
            sync.due = sync.due.min(due);
            sync.attempt = 0;
            sync.replies.extend(reply);
            return;
        }
        self.syncs.push(Resync {
            device,
            due,
            attempt: 0,
            replies: reply.into_iter().collect(),
        });
    }

    fn work(&mut self, registry: &Registry) {
        if let Err(error) = registry.remote.expire_paused_push(unix_now()) {
            log::error!("paused push registrations not forgotten: {error}");
        }
        let now = Instant::now();
        for sync in take_due(&mut self.syncs, now, |sync| sync.due) {
            self.sync(registry, sync);
        }
        for deletion in take_due(&mut self.deletes, now, |deletion| deletion.due) {
            self.delete(deletion);
        }
        for pending in take_due(&mut self.events, now, |pending| pending.due) {
            self.expand(registry, pending.event);
        }
        for delivery in take_due(&mut self.deliveries, now, |delivery| delivery.due) {
            self.deliver(registry, delivery);
        }
    }

    fn sync(&mut self, registry: &Registry, sync: Resync) {
        let (status, after) = self.sync_device(registry, &sync);
        let waiting = self.syncs.iter().any(|queued| queued.device == sync.device);
        if status == PushStatus::Pending
            && !waiting
            && registry.remote.push_record(sync.device).is_some()
        {
            self.syncs.push(Resync {
                due: Instant::now() + backoff(self.timing.retry, sync.attempt, after),
                attempt: sync.attempt + 1,
                replies: Vec::new(),
                device: sync.device,
            });
        }
        for reply in sync.replies {
            let _ = reply.send(status);
        }
    }

    /// The device's status afterwards, and when the relay asked to be tried again.
    fn sync_device(
        &mut self,
        registry: &Registry,
        sync: &Resync,
    ) -> (PushStatus, Option<Duration>) {
        if !registry.remote.push_notifications() {
            return (PushStatus::Disabled, None);
        }
        let Some(before) = registry.remote.push_record(sync.device) else {
            return (PushStatus::Disabled, None);
        };
        let outcome = match &before.relay {
            None => self.relay.register(&before.token).map(Some),
            Some(credentials) => match self.relay.replace(credentials, &before.token) {
                Ok(()) => Ok(None),
                Err(RelayError::Unauthorized | RelayError::NotFound) => {
                    self.relay.register(&before.token).map(Some)
                }
                Err(error) => Err(error),
            },
        };
        let mut after = before.clone();
        let mut retry_after = None;
        let created = match outcome {
            Ok(created) => {
                after.status = PushStatus::Active;
                if created.is_some() {
                    after.relay.clone_from(&created);
                }
                created
            }
            Err(RelayError::Gone | RelayError::Refused) => {
                after.status = PushStatus::Refused;
                None
            }
            Err(error) => {
                log::warn!("push registration pending: {error:?}");
                after.status = PushStatus::Pending;
                retry_after = error.retry_after();
                None
            }
        };
        let status =
            match registry
                .remote
                .replace_push(sync.device, Some(&before), Some(after.clone()))
            {
                Ok(true) => after.status,
                Ok(false) => {
                    self.forget(created);
                    registry
                        .remote
                        .push_record(sync.device)
                        .map_or(PushStatus::Disabled, |current| current.status)
                }
                Err(error) => {
                    log::error!("push registration not saved: {error}");
                    self.forget(created);
                    PushStatus::Pending
                }
            };
        (status, retry_after)
    }

    /// Deletes a registration the relay made for a record that changed meanwhile.
    fn forget(&mut self, created: Option<Credentials>) {
        if let Some(credentials) = created {
            self.deletes.push(Deletion {
                credentials,
                due: Instant::now(),
                attempt: 0,
            });
        }
    }

    fn delete(&mut self, deletion: Deletion) {
        let Err(error) = self.relay.delete(&deletion.credentials) else {
            return;
        };
        if error.retryable() && deletion.attempt + 1 < DELETE_ATTEMPTS {
            self.deletes.push(Deletion {
                due: Instant::now()
                    + backoff(self.timing.retry, deletion.attempt, error.retry_after()),
                attempt: deletion.attempt + 1,
                credentials: deletion.credentials,
            });
        }
    }

    /// The event outlived the grace period: one delivery per registered phone.
    fn expand(&mut self, registry: &Registry, event: u64) {
        if registry.activity.event(event).is_none() || !sending(registry) {
            return;
        }
        let connected = registry.connected_devices();
        let now = Instant::now();
        for (device, record) in registry.remote.push_records() {
            if record.status != PushStatus::Refused && !connected.contains(&device) {
                self.deliveries.push(Delivery {
                    event,
                    device,
                    due: now,
                    attempt: 0,
                });
            }
        }
    }

    /// Checks every rule again right before the call, since earlier calls take time.
    fn deliver(&mut self, registry: &Registry, delivery: Delivery) {
        let Some(event) = registry.activity.event(delivery.event) else {
            return;
        };
        if !sending(registry) || registry.connected_devices().contains(&delivery.device) {
            return;
        }
        let Some(record) = registry.remote.push_record(delivery.device) else {
            return;
        };
        let age = unix_now().saturating_sub(event.timestamp);
        if age >= TTL_SECONDS || record.status == PushStatus::Refused {
            return;
        }
        let Some(credentials) = record.relay.clone() else {
            self.retry(delivery, None);
            return;
        };
        let Some(outgoing) = outgoing(registry, &record.key, &event, TTL_SECONDS - age) else {
            log::error!(
                "push notification for event {} could not be built",
                event.id
            );
            return;
        };
        let changed = |status, relay| PushRecord {
            relay,
            status,
            ..record.clone()
        };
        match self.relay.send(&credentials, &outgoing) {
            Ok(()) | Err(RelayError::Refused) => {}
            Err(RelayError::Gone) => {
                let refused = changed(PushStatus::Refused, record.relay.clone());
                replace(registry, delivery.device, &record, refused);
            }
            Err(RelayError::Unauthorized) => {
                replace(
                    registry,
                    delivery.device,
                    &record,
                    changed(PushStatus::Pending, None),
                );
            }
            Err(RelayError::NotFound) => {
                if replace(
                    registry,
                    delivery.device,
                    &record,
                    changed(PushStatus::Pending, None),
                ) {
                    self.schedule_sync(delivery.device, Instant::now(), None);
                }
                self.retry(delivery, None);
            }
            Err(error @ (RelayError::RateLimited(after) | RelayError::Unavailable(after))) => {
                log::warn!("push notification delayed: {error:?}");
                self.retry(delivery, after);
            }
        }
    }

    fn retry(&mut self, delivery: Delivery, after: Option<Duration>) {
        self.deliveries.push(Delivery {
            due: Instant::now() + backoff(self.timing.retry, delivery.attempt, after),
            attempt: delivery.attempt + 1,
            ..delivery
        });
    }
}

fn take_due<T>(items: &mut Vec<T>, now: Instant, due: impl Fn(&T) -> Instant) -> Vec<T> {
    let (ready, waiting) = std::mem::take(items)
        .into_iter()
        .partition(|item| due(item) <= now);
    *items = waiting;
    ready
}

/// Push is on, and phones may connect to open what they are told about.
fn sending(registry: &Registry) -> bool {
    registry.remote.push_notifications() && registry.remote.accepting()
}

fn replace(registry: &Registry, device: DeviceId, before: &PushRecord, after: PushRecord) -> bool {
    registry
        .remote
        .replace_push(device, Some(before), Some(after))
        .unwrap_or_else(|error| {
            log::error!("push registration not saved: {error}");
            false
        })
}

fn outgoing(
    registry: &Registry,
    key: &PushKey,
    event: &ActivityEvent,
    ttl: u64,
) -> Option<Outgoing> {
    let kind = payload::kind_name(event.kind)?;
    let project_id = event.project.to_string();
    let project = registry
        .catalog
        .project(event.project)
        .map_or_else(|_| "Muxy".to_owned(), |project| project.name);
    let notification = payload::Notification {
        server_id: registry.catalog.identity().to_string(),
        project_id: project_id.clone(),
        session_id: event.session.get(),
        event_id: event.id,
        kind: event.kind,
        title: format!("{} · {project}", event.provider.name()),
        body: event.kind.description().to_owned(),
        ts: event.timestamp,
    };
    Some(Outgoing {
        kind,
        payload: payload::encrypt(key, &notification)?,
        collapse_id: payload::collapse_id(key, event.session.get()),
        thread_id: payload::thread_id(key, &project_id),
        ttl,
    })
}

#[cfg(test)]
mod tests;
