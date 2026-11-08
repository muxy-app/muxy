use std::collections::VecDeque;
use std::sync::mpsc;

use muxy_protocol::{
    AgentActivity, AgentProvider, AgentState, DeviceKind, PushEnvironment, SessionId,
};

use super::*;
use crate::connection::Connection;
use crate::{RemoteAccess, ServerSettings};

#[derive(Clone, Debug, Eq, PartialEq)]
enum Call {
    Register(String),
    Replace(String, String),
    Delete(String),
    Send(String, Outgoing),
}

/// Answers each kind of call from its script, then with success.
#[derive(Default)]
struct FakeRelay {
    calls: Mutex<Vec<Call>>,
    registers: Mutex<VecDeque<RelayError>>,
    replaces: Mutex<VecDeque<RelayError>>,
    sends: Mutex<VecDeque<RelayError>>,
    created: Mutex<u32>,
}

fn locked<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}

impl FakeRelay {
    fn calls(&self) -> Vec<Call> {
        locked(&self.calls).clone()
    }

    fn sent(&self) -> Vec<Outgoing> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                Call::Send(_, outgoing) => Some(outgoing),
                _ => None,
            })
            .collect()
    }

    fn answer(script: &Mutex<VecDeque<RelayError>>) -> Result<(), RelayError> {
        locked(script).pop_front().map_or(Ok(()), Err)
    }
}

impl Relay for FakeRelay {
    fn register(&self, token: &PushToken) -> Result<Credentials, RelayError> {
        locked(&self.calls).push(Call::Register(token.token.clone()));
        Self::answer(&self.registers)?;
        let mut created = locked(&self.created);
        *created += 1;
        credentials(*created).ok_or(RelayError::Refused)
    }

    fn replace(&self, credentials: &Credentials, token: &PushToken) -> Result<(), RelayError> {
        locked(&self.calls).push(Call::Replace(credentials.id.clone(), token.token.clone()));
        Self::answer(&self.replaces)
    }

    fn delete(&self, credentials: &Credentials) -> Result<(), RelayError> {
        locked(&self.calls).push(Call::Delete(credentials.id.clone()));
        Ok(())
    }

    fn send(&self, credentials: &Credentials, notification: &Outgoing) -> Result<(), RelayError> {
        locked(&self.calls).push(Call::Send(credentials.id.clone(), notification.clone()));
        Self::answer(&self.sends)
    }
}

fn credentials(number: u32) -> Option<Credentials> {
    Credentials::new(
        format!("01jc8q4m6x2v7s0t3n5r9k{number:04}"),
        format!("{number:A>43}"),
    )
}

fn relay_id(number: u32) -> String {
    credentials(number)
        .map(|credentials| credentials.id)
        .unwrap_or_default()
}

const TIMING: Timing = Timing {
    grace: Duration::from_millis(100),
    retry: Duration::from_millis(30),
};

struct Fixture {
    registry: Arc<Registry>,
    relay: Arc<FakeRelay>,
    phones: Vec<DeviceId>,
}

fn fixture(phones: usize) -> Fixture {
    let phones: Vec<_> = (0..phones).map(|_| DeviceId::new()).collect();
    let relay = Arc::new(FakeRelay::default());
    let (events, _) = mpsc::channel();
    let registry = Arc::new(
        Registry::new(ServerSettings::default(), events)
            .with_remote_access(RemoteAccess::with_phones(&phones))
            .with_push_relay(Arc::clone(&relay) as Arc<dyn Relay>, TIMING),
    );
    registry.start_push();
    Fixture {
        registry,
        relay,
        phones,
    }
}

fn apns(token: &str) -> PushToken {
    PushToken {
        platform: PushPlatform::Apns,
        token: token.repeat(64 / token.len()),
        environment: Some(PushEnvironment::Production),
    }
}

fn eventually(mut done: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        if done() {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

/// Long enough for an event's grace period and a first delivery.
fn settle() {
    thread::sleep(TIMING.grace * 4);
}

fn session(id: u64) -> SessionId {
    SessionId::new(id).unwrap_or(SessionId::from(std::num::NonZeroU64::MIN))
}

impl Fixture {
    fn register(&self, phone: usize, token: PushToken) -> PushRegistration {
        match self
            .registry
            .register_push(self.phones[phone], token, Duration::from_secs(5))
        {
            Ok(registration) => registration,
            Err(error) => panic!("registering failed: {error:?}"),
        }
    }

    fn status(&self, phone: usize) -> Option<PushStatus> {
        self.registry
            .remote
            .push_record(self.phones[phone])
            .map(|record| record.status)
    }

    /// An agent in `session` starts waiting for the user; the event's id.
    fn blocked(&self, session_id: u64) -> u64 {
        self.registry.activity.update(
            AgentActivity {
                session: session(session_id),
                project: self.registry.home_project(),
                provider: AgentProvider::Claude,
                state: AgentState::Working,
            },
            false,
        );
        self.registry.activity.update(
            AgentActivity {
                session: session(session_id),
                project: self.registry.home_project(),
                provider: AgentProvider::Claude,
                state: AgentState::Blocked,
            },
            false,
        );
        self.registry
            .activity
            .snapshot()
            .events
            .iter()
            .find(|event| event.session == session(session_id))
            .map_or(0, |event| event.id)
    }

    fn connect(&self, phone: usize) -> Arc<Connection> {
        let connection = Arc::new(Connection::for_device(
            Arc::clone(&self.registry.notifier),
            self.phones[phone],
            DeviceKind::Phone,
        ));
        self.registry.notifier.register(&connection);
        connection
    }
}

#[test]
fn a_new_token_is_registered_once_and_an_unchanged_one_needs_no_relay() {
    let fixture = fixture(1);
    let first = fixture.register(0, apns("ab"));
    assert_eq!(first.status, PushStatus::Active);
    assert!(first.key.is_some());
    assert_eq!(fixture.relay.calls(), vec![Call::Register("ab".repeat(32))]);
    assert_eq!(fixture.register(0, apns("ab")), first);
    assert_eq!(fixture.register(0, apns("AB")), first);
    assert_eq!(fixture.relay.calls().len(), 1);
    let state = fixture.registry.remote_access_state();
    assert!(state.push_notifications);
    assert_eq!(state.devices[0].push, Some(PushStatus::Active));
}

#[test]
fn a_changed_token_replaces_the_registration_and_keeps_the_key() {
    let fixture = fixture(1);
    let first = fixture.register(0, apns("ab"));
    let second = fixture.register(0, apns("cd"));
    assert_eq!(second, first);
    assert_eq!(
        fixture.relay.calls()[1],
        Call::Replace(relay_id(1), "cd".repeat(32))
    );
}

#[test]
fn a_registration_the_relay_lost_is_created_again_with_the_same_key() {
    let fixture = fixture(1);
    let first = fixture.register(0, apns("ab"));
    locked(&fixture.relay.replaces).push_back(RelayError::NotFound);
    assert_eq!(fixture.register(0, apns("cd")), first);
    assert_eq!(
        fixture.relay.calls()[1..],
        [
            Call::Replace(relay_id(1), "cd".repeat(32)),
            Call::Register("cd".repeat(32)),
        ]
    );
}

#[test]
fn an_unreachable_relay_leaves_the_phone_pending_and_is_retried() {
    let fixture = fixture(1);
    locked(&fixture.relay.registers).extend([
        RelayError::Unavailable(None),
        RelayError::RateLimited(Some(Duration::from_millis(1))),
    ]);
    let registration = fixture.register(0, apns("ab"));
    assert_eq!(registration.status, PushStatus::Pending);
    assert!(registration.key.is_some());
    assert!(eventually(|| fixture.status(0) == Some(PushStatus::Active)));
    assert_eq!(fixture.relay.calls().len(), 3);
    assert_eq!(
        fixture.register(0, apns("ab")),
        PushRegistration {
            status: PushStatus::Active,
            ..registration
        }
    );
}

#[test]
fn a_rejected_token_is_refused_until_the_phone_reports_one_again() {
    let fixture = fixture(1);
    locked(&fixture.relay.registers).push_back(RelayError::Refused);
    let refused = fixture.register(0, apns("ab"));
    assert_eq!(refused.status, PushStatus::Refused);
    assert!(refused.key.is_some());
    settle();
    assert_eq!(fixture.relay.calls().len(), 1);
    assert_eq!(fixture.register(0, apns("ab")).status, PushStatus::Active);
    assert_eq!(fixture.relay.calls().len(), 2);
}

#[test]
fn an_event_reaches_each_phone_that_is_away_once_and_opens_its_pane() {
    let fixture = fixture(2);
    let away = fixture.register(0, apns("ab"));
    fixture.register(1, apns("cd"));
    let _connected = fixture.connect(1);
    let event = fixture.blocked(42);
    assert!(eventually(|| !fixture.relay.sent().is_empty()));
    settle();
    let calls: Vec<_> = fixture
        .relay
        .calls()
        .into_iter()
        .filter(|call| matches!(call, Call::Send(..)))
        .collect();
    assert_eq!(calls.len(), 1);
    let Call::Send(id, outgoing) = &calls[0] else {
        return;
    };
    assert_eq!(*id, relay_id(1));
    let Some(key) = away.key else { return };
    let project = fixture.registry.home_project().to_string();
    assert_eq!(outgoing.kind, "attention");
    assert!((3590..=3600).contains(&outgoing.ttl), "{}", outgoing.ttl);
    assert_eq!(outgoing.collapse_id, payload::collapse_id(&key, 42));
    assert_eq!(outgoing.thread_id, payload::thread_id(&key, &project));
    let server = fixture.registry.catalog.identity().to_string();
    let bytes = payload::base64url_decode(&outgoing.payload);
    let Some(plaintext) = payload::open(&key, &server, &bytes) else {
        panic!("the phone could not open the payload");
    };
    let json: serde_json::Value = serde_json::from_slice(&plaintext).unwrap_or_default();
    assert_eq!(json["v"], 1);
    assert_eq!(json["server_id"], server.as_str());
    assert_eq!(json["project_id"], project.as_str());
    assert_eq!(json["session_id"], 42);
    assert_eq!(json["event_id"], event);
    assert_eq!(json["kind"], "attention");
    assert_eq!(json["body"], "Needs your attention");
    assert!(
        json["title"]
            .as_str()
            .is_some_and(|title| title.starts_with("Claude Code · ")),
        "{json}"
    );
}

#[test]
fn nothing_is_sent_for_an_event_handled_in_time_or_while_sending_is_off() {
    let fixture = fixture(1);
    fixture.register(0, apns("ab"));
    let event = fixture.blocked(1);
    fixture.registry.activity.acknowledge(&[event]);
    settle();
    fixture.registry.remote.set_accepting(false);
    fixture.blocked(2);
    settle();
    fixture.registry.remote.set_accepting(true);
    let connection = fixture.connect(0);
    fixture.blocked(3);
    settle();
    drop(connection);
    assert!(fixture.relay.sent().is_empty());
    fixture.blocked(4);
    assert!(eventually(|| fixture.relay.sent().len() == 1));
}

#[test]
fn turning_push_off_pauses_phones_and_turning_it_on_resumes_them_with_their_keys() {
    let fixture = fixture(2);
    let first = fixture.register(0, apns("ab"));
    let second = fixture.register(1, apns("cd"));
    let Ok(off) = fixture.registry.set_push_notifications(false) else {
        panic!("push could not be switched off");
    };
    assert!(!off.push_notifications);
    assert!(
        off.devices
            .iter()
            .all(|device| device.push == Some(PushStatus::Disabled))
    );
    assert!(off.push_kept_until.is_some_and(|until| until > unix_now()));
    assert!(eventually(|| {
        let calls = fixture.relay.calls();
        calls.contains(&Call::Delete(relay_id(1))) && calls.contains(&Call::Delete(relay_id(2)))
    }));
    assert_eq!(fixture.status(0), Some(PushStatus::Pending));
    assert_eq!(
        fixture.register(0, apns("ef")),
        PushRegistration {
            status: PushStatus::Disabled,
            key: None,
        }
    );
    fixture.blocked(1);
    settle();
    assert!(fixture.relay.sent().is_empty());
    let calls = fixture.relay.calls().len();

    let Ok(on) = fixture.registry.set_push_notifications(true) else {
        panic!("push could not be switched on");
    };
    assert!(on.push_notifications);
    assert_eq!(on.push_kept_until, None);
    assert!(eventually(|| {
        fixture.status(0) == Some(PushStatus::Active)
            && fixture.status(1) == Some(PushStatus::Active)
    }));
    let registered: Vec<_> = fixture.relay.calls()[calls..].to_vec();
    assert!(registered.contains(&Call::Register("ef".repeat(32))));
    assert!(registered.contains(&Call::Register("cd".repeat(32))));
    let key = |phone: usize| {
        fixture
            .registry
            .remote
            .push_record(fixture.phones[phone])
            .map(|record| record.key)
    };
    assert_eq!(key(0), first.key);
    assert_eq!(key(1), second.key);
    fixture.blocked(2);
    assert!(eventually(|| fixture.relay.sent().len() == 2));
}

#[test]
fn registrations_kept_while_push_is_off_are_forgotten_after_ninety_days() {
    let fixture = fixture(1);
    fixture.register(0, apns("ab"));
    assert!(fixture.registry.set_push_notifications(false).is_ok());
    let remote = &fixture.registry.remote;
    let Some(until) = fixture.registry.remote_access_state().push_kept_until else {
        panic!("no expiry for the kept registration");
    };
    assert_eq!(remote.expire_paused_push(until - 1).ok(), Some(false));
    assert!(fixture.status(0).is_some());
    assert_eq!(remote.expire_paused_push(until).ok(), Some(true));
    assert_eq!(fixture.status(0), None);
    assert_eq!(fixture.registry.remote_access_state().push_kept_until, None);
    assert!(fixture.registry.set_push_notifications(true).is_ok());
    let fresh = fixture.register(0, apns("ab"));
    assert_eq!(fresh.status, PushStatus::Active);
}

#[test]
fn resetting_a_phone_forgets_its_key_but_keeps_it_paired() {
    let fixture = fixture(1);
    let first = fixture.register(0, apns("ab"));
    let Ok(state) = fixture.registry.reset_push(fixture.phones[0]) else {
        panic!("reset failed");
    };
    assert_eq!(state.devices.len(), 1);
    assert_eq!(state.devices[0].push, None);
    assert!(eventually(|| fixture
        .relay
        .calls()
        .contains(&Call::Delete(relay_id(1)))));
    let second = fixture.register(0, apns("ab"));
    assert_ne!(second.key, first.key);
    assert!(fixture.registry.reset_push(DeviceId::new()).is_err());
}

#[test]
fn unregistering_or_revoking_deletes_the_registration_and_a_new_one_gets_a_new_key() {
    let fixture = fixture(2);
    let first = fixture.register(0, apns("ab"));
    fixture.register(1, apns("cd"));
    assert!(fixture.registry.unregister_push(fixture.phones[0]).is_ok());
    assert_eq!(fixture.status(0), None);
    assert!(eventually(|| fixture
        .relay
        .calls()
        .contains(&Call::Delete(relay_id(1)))));
    let second = fixture.register(0, apns("ab"));
    assert_ne!(second.key, first.key);
    assert!(fixture.registry.revoke_device(fixture.phones[1]).is_ok());
    assert!(eventually(|| fixture
        .relay
        .calls()
        .contains(&Call::Delete(relay_id(2)))));
}

#[test]
fn a_dead_token_is_refused_and_gets_nothing_more() {
    let fixture = fixture(1);
    fixture.register(0, apns("ab"));
    locked(&fixture.relay.sends).push_back(RelayError::Gone);
    fixture.blocked(1);
    assert!(eventually(|| fixture.status(0) == Some(PushStatus::Refused)));
    fixture.blocked(2);
    settle();
    assert_eq!(fixture.relay.sent().len(), 1);
    assert_eq!(
        fixture.registry.remote_access_state().devices[0].push,
        Some(PushStatus::Refused)
    );
}

#[test]
fn a_pruned_registration_is_registered_again_and_the_event_still_arrives() {
    let fixture = fixture(1);
    let registration = fixture.register(0, apns("ab"));
    locked(&fixture.relay.sends).push_back(RelayError::NotFound);
    fixture.blocked(1);
    assert!(eventually(|| fixture.relay.sent().len() == 2));
    let calls = fixture.relay.calls();
    assert!(calls.contains(&Call::Register("ab".repeat(32))));
    assert!(matches!(calls.last(), Some(Call::Send(id, _)) if *id == relay_id(2)));
    assert_eq!(fixture.status(0), Some(PushStatus::Active));
    assert_eq!(
        fixture
            .registry
            .remote
            .push_record(fixture.phones[0])
            .map(|record| record.key),
        registration.key
    );
}

#[test]
fn a_busy_relay_is_retried_and_a_wrong_secret_waits_for_the_phone() {
    let fixture = fixture(1);
    fixture.register(0, apns("ab"));
    locked(&fixture.relay.sends).extend([
        RelayError::RateLimited(Some(Duration::from_millis(1))),
        RelayError::Unavailable(None),
    ]);
    fixture.blocked(1);
    assert!(eventually(|| fixture.relay.sent().len() == 3));
    settle();
    assert_eq!(fixture.relay.sent().len(), 3);

    locked(&fixture.relay.sends).push_back(RelayError::Unauthorized);
    fixture.blocked(2);
    assert!(eventually(|| fixture.status(0) == Some(PushStatus::Pending)));
    settle();
    let calls = fixture.relay.calls().len();
    assert_eq!(
        fixture
            .registry
            .remote
            .push_record(fixture.phones[0])
            .and_then(|record| record.relay),
        None
    );
    assert_eq!(fixture.register(0, apns("ab")).status, PushStatus::Active);
    assert_eq!(
        fixture.relay.calls()[calls],
        Call::Register("ab".repeat(32))
    );
}

#[test]
fn starting_twice_keeps_one_worker_and_a_switched_off_server_saves_no_relay_registration() {
    let fixture = fixture(1);
    fixture.registry.start_push();
    fixture.register(0, apns("ab"));
    assert_eq!(fixture.relay.calls().len(), 1);
    assert!(fixture.registry.set_push_notifications(false).is_ok());
    let Some(paused) = fixture.registry.remote.push_record(fixture.phones[0]) else {
        panic!("the registration was not kept");
    };
    let registered = PushRecord {
        relay: credentials(9),
        status: PushStatus::Active,
        ..paused.clone()
    };
    assert_eq!(
        fixture
            .registry
            .remote
            .replace_push(fixture.phones[0], Some(&paused), Some(registered))
            .ok(),
        Some(false)
    );
    assert_eq!(fixture.status(0), Some(PushStatus::Pending));
}

#[test]
fn backoff_doubles_to_a_limit_and_follows_the_relay() {
    let first = Duration::from_secs(5);
    assert_eq!(backoff(first, 0, None), first);
    assert_eq!(backoff(first, 3, None), Duration::from_secs(40));
    assert_eq!(backoff(first, 30, None), LAST_RETRY);
    assert_eq!(
        backoff(first, 3, Some(Duration::from_secs(7))),
        Duration::from_secs(7)
    );
    assert_eq!(
        backoff(first, 0, Some(Duration::from_secs(1_000_000))),
        Duration::from_secs(TTL_SECONDS)
    );
}

#[test]
fn a_paired_phone_registers_over_its_connection_and_a_local_client_may_not()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::net::UnixStream;

    use muxy_protocol::wire::{Decoder, Encoder};
    use muxy_protocol::{CONTROL, DeviceCredential, Message, ReplyBody, RequestBody, RequestId};

    let fixture = fixture(1);
    let request = |encoder: &mut Encoder<UnixStream>, id, body| {
        encoder.send(
            CONTROL,
            &Message::Request {
                id: RequestId(id),
                body,
            },
        )
    };
    let reply = |decoder: &mut Decoder<UnixStream>| loop {
        match decoder.next() {
            Ok((CONTROL, Message::Reply { body, .. })) => return Some(body),
            Ok(_) => {}
            Err(_) => return None,
        }
    };
    let hello = |encoder: &mut Encoder<UnixStream>, decoder: &mut Decoder<UnixStream>| {
        let sent = encoder.send(
            CONTROL,
            &Message::Hello {
                versions: muxy_protocol::SUPPORTED.to_vec(),
            },
        );
        sent.is_ok() && matches!(decoder.next(), Ok((CONTROL, Message::HelloReply { .. })))
    };

    let (phone, server) = UnixStream::pair()?;
    phone.set_read_timeout(Some(Duration::from_secs(10)))?;
    let Some(admission) = fixture.registry.admit_remote() else {
        panic!("no admission");
    };
    let serving = Arc::clone(&fixture.registry);
    thread::spawn(move || crate::connection::serve_remote(Box::new(server), serving, admission));
    let mut encoder = Encoder::new(phone.try_clone()?);
    let mut decoder = Decoder::new(phone.try_clone()?);
    assert!(hello(&mut encoder, &mut decoder));
    request(
        &mut encoder,
        1,
        RequestBody::Authenticate(DeviceCredential {
            device: fixture.phones[0],
            token: [7; 32],
        }),
    )?;
    assert_eq!(reply(&mut decoder), Some(ReplyBody::Authenticated));
    request(&mut encoder, 2, RequestBody::RegisterPush(apns("ab")))?;
    let Some(ReplyBody::PushRegistered(registration)) = reply(&mut decoder) else {
        panic!("the phone was not registered");
    };
    assert_eq!(registration.status, PushStatus::Active);
    assert_eq!(
        fixture
            .registry
            .remote
            .push_record(fixture.phones[0])
            .map(|record| record.key),
        registration.key
    );
    request(&mut encoder, 3, RequestBody::UnregisterPush)?;
    assert_eq!(reply(&mut decoder), Some(ReplyBody::PushUnregistered));
    assert_eq!(fixture.status(0), None);

    let (desktop, server) = UnixStream::pair()?;
    desktop.set_read_timeout(Some(Duration::from_secs(10)))?;
    let serving = Arc::clone(&fixture.registry);
    thread::spawn(move || crate::connection::serve(Box::new(server), serving));
    let mut encoder = Encoder::new(desktop.try_clone()?);
    let mut decoder = Decoder::new(desktop.try_clone()?);
    assert!(hello(&mut encoder, &mut decoder));
    request(&mut encoder, 1, RequestBody::RegisterPush(apns("ab")))?;
    assert!(matches!(
        reply(&mut decoder),
        Some(ReplyBody::Error(error)) if error.code == ErrorCode::Unauthorized
    ));
    request(&mut encoder, 2, RequestBody::SetPushNotifications(false))?;
    assert!(matches!(
        reply(&mut decoder),
        Some(ReplyBody::RemoteAccess(state)) if !state.push_notifications
    ));
    Ok(())
}
