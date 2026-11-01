use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use muxy_protocol::{
    AttachSnapshot, ChannelId, ClientKind, DeviceId, DeviceKind, ErrorCode, ExitReason,
    ForegroundProcess, Message, MetadataEvent, OperationId, ReplyBody, RequestId, ScreenFrame,
    SessionClient, SessionId, Size, TerminalColors, Topic,
};

use crate::notifier::Notifier;
use crate::references::{References, SharedReferences};
use crate::{AttachmentId, ServerError, SessionCommand, SessionHandle};

use super::outbox::Outbox;
use super::state::ConnectionState;

struct Attachment {
    handle: SessionHandle,
    id: AttachmentId,
    waiting: Option<RequestId>,
    resizes: VecDeque<RequestId>,
}

#[derive(Default)]
struct Inner {
    state: ConnectionState,
    outbox: Outbox,
    attachments: HashMap<ChannelId, Attachment>,
    /// The app that shares `references` with its other connections.
    owner: Option<OperationId>,
    references: SharedReferences,
    open_sessions: HashSet<SessionId>,
    created: HashSet<SessionId>,
    /// When this connection started showing each session, to order viewers.
    positions: HashMap<SessionId, u64>,
    /// Who shows which session changed; announced once the lock is released.
    positions_changed: bool,
    closed: bool,
}

/// The server's end of one app connection.
pub(crate) struct Connection {
    inner: Mutex<Inner>,
    ready: Condvar,
    notifier: Arc<Notifier>,
}

/// The locked connection; announces who-shows-what changes after unlocking.
struct Locked<'a> {
    inner: Option<MutexGuard<'a, Inner>>,
    notifier: &'a Notifier,
}

impl Deref for Locked<'_> {
    type Target = Inner;

    fn deref(&self) -> &Inner {
        self.inner.as_deref().unwrap_or_else(|| unreachable!())
    }
}

impl DerefMut for Locked<'_> {
    fn deref_mut(&mut self) -> &mut Inner {
        self.inner.as_deref_mut().unwrap_or_else(|| unreachable!())
    }
}

impl Drop for Locked<'_> {
    fn drop(&mut self) {
        let Some(mut inner) = self.inner.take() else {
            return;
        };
        let changed = std::mem::take(&mut inner.positions_changed);
        drop(inner);
        if changed {
            self.notifier.sessions_changed();
        }
    }
}

impl Connection {
    pub(crate) fn new(notifier: Arc<Notifier>) -> Self {
        Self::with_state(notifier, ConnectionState::default())
    }

    pub(super) fn for_device(notifier: Arc<Notifier>, device: DeviceId, kind: DeviceKind) -> Self {
        Self::with_state(notifier, ConnectionState::for_device(device, kind))
    }

    fn with_state(notifier: Arc<Notifier>, state: ConnectionState) -> Self {
        Self {
            inner: Mutex::new(Inner {
                state,
                ..Inner::default()
            }),
            ready: Condvar::default(),
            notifier,
        }
    }

    pub(crate) fn device(&self) -> Option<DeviceId> {
        self.lock().state.device.map(|(device, _)| device)
    }

    pub(crate) fn device_kind(&self) -> Option<DeviceKind> {
        self.lock().state.device.map(|(_, kind)| kind)
    }

    pub(crate) fn client(&self) -> SessionClient {
        self.lock().state.client
    }

    pub(super) fn identify(&self, kind: ClientKind) -> SessionClient {
        let mut inner = self.lock();
        if inner.state.client.kind != kind {
            inner.state.client.kind = kind;
            self.notifier.next_position();
            inner.positions_changed = true;
        }
        inner.state.client
    }

    /// The app this open connection shares its references with.
    pub(crate) fn owner(&self) -> Option<OperationId> {
        let inner = self.lock();
        inner.owner.filter(|_| !inner.closed)
    }

    /// The references this open connection uses.
    pub(crate) fn shared_references(&self) -> Option<SharedReferences> {
        let inner = self.lock();
        (!inner.closed).then(|| inner.references.clone())
    }

    /// Switches to `references`, and lets go of what they no longer hold.
    /// `reported` is the sessions this connection's app has open.
    pub(crate) fn use_references(
        &self,
        owner: Option<OperationId>,
        references: &SharedReferences,
        reported: Option<&[SessionId]>,
        acknowledge_creation: bool,
    ) {
        let mut inner = self.lock();
        if inner.closed {
            return;
        }
        inner.owner = owner;
        inner.references = references.clone();
        let references = references.lock();
        inner
            .created
            .retain(|session| !acknowledge_creation && !references.contains(*session));
        if let Some(sessions) = reported {
            inner.open_sessions = sessions.iter().copied().collect();
        }
        inner
            .open_sessions
            .retain(|session| references.contains(*session));
        inner.outbox.retain_control(|message| {
            session_state(message).is_none_or(|session| references.contains(session))
        });
        inner.refresh_positions(&references, &self.notifier);
    }

    pub(crate) fn created_session(&self, session: SessionId) {
        let mut inner = self.lock();
        if !inner.closed && !inner.positions.contains_key(&session) {
            inner.created.insert(session);
            inner.refresh(&self.notifier);
        }
    }

    pub(super) fn watch_catalog(&self) {
        self.lock().state.watches.catalog = true;
    }

    pub(super) fn watch_activity(&self) {
        self.lock().state.watches.activity = true;
    }

    pub(super) fn watch_remote_access(&self) {
        self.lock().state.watches.remote_access = true;
    }

    pub(crate) fn participation(&self, session: SessionId) -> Option<(u64, SessionClient)> {
        let inner = self.lock();
        if inner.closed {
            return None;
        }
        inner
            .positions
            .get(&session)
            .map(|position| (*position, inner.state.client))
    }

    pub(crate) fn references(&self, session: SessionId, include_channels: bool) -> bool {
        let inner = self.lock();
        if inner.closed {
            return false;
        }
        let references = inner.references.lock();
        references.contains(session)
            || (include_channels && inner.created.contains(&session))
            || (include_channels
                && !references.is_released(session)
                && inner
                    .attachments
                    .values()
                    .any(|attachment| attachment.handle.id() == session))
    }

    pub(crate) fn detach_session(&self, session: SessionId) {
        let mut inner = self.lock();
        inner.created.remove(&session);
        let channels: Vec<_> = inner
            .attachments
            .iter()
            .filter(|(_, attachment)| attachment.handle.id() == session)
            .map(|(channel, _)| *channel)
            .collect();
        for channel in channels {
            inner.retire(
                channel,
                &ServerError::new(ErrorCode::UnknownChannel, "pane was closed"),
            );
        }
        inner.refresh(&self.notifier);
        self.ready.notify_one();
    }

    /// Tells the client to refetch a topic it reads.
    pub(crate) fn changed(&self, topic: Topic, revision: u64) {
        let mut inner = self.lock();
        if !inner.closed && inner.state.watches.includes(topic) {
            inner
                .outbox
                .push_control(Message::Changed { topic, revision });
            self.ready.notify_one();
        }
    }

    pub(super) fn colors(&self) -> Option<TerminalColors> {
        self.lock().state.colors.clone()
    }

    pub(super) fn set_colors(&self, colors: TerminalColors) {
        let mut inner = self.lock();
        inner.state.colors = Some(colors);
        inner.apply_colors(|session| !inner.state.session_colors.contains_key(&session));
    }

    pub(super) fn set_session_colors(&self, session: SessionId, colors: Option<TerminalColors>) {
        let mut inner = self.lock();
        match colors {
            Some(colors) => inner.state.session_colors.insert(session, colors),
            None => inner.state.session_colors.remove(&session),
        };
        inner.apply_colors(|attached| attached == session);
    }

    /// Queues a control message. A session's progress and metadata reach
    /// only connections that reference the session.
    pub(crate) fn push_control(&self, message: Message) {
        let mut inner = self.lock();
        if inner.closed
            || session_state(&message)
                .is_some_and(|session| !inner.references.lock().contains(session))
        {
            return;
        }
        inner.outbox.push_control(message);
        self.ready.notify_one();
    }

    pub(super) fn attach(
        &self,
        channel: ChannelId,
        id: AttachmentId,
        request: RequestId,
        handle: SessionHandle,
        command: SessionCommand,
    ) -> Result<(), ServerError> {
        let mut inner = self.lock();
        if inner.closed {
            return Err(ServerError::new(
                ErrorCode::BadRequest,
                "connection is closed",
            ));
        }
        let session = handle.id();
        if inner.owner.is_some() && inner.references.lock().is_released(session) {
            return Err(ServerError::unknown_session(session));
        }
        if let Some(colors) = inner.state.colors_for(session) {
            handle.send(SessionCommand::SetColors(colors.clone()))?;
        }
        if let Err(undelivered) = handle.try_send(command) {
            // Its sink reports back to this connection when dropped, which takes the lock.
            drop(inner);
            drop(undelivered);
            return Err(ServerError::unknown_session(session));
        }
        inner.references.lock().attached(session);
        inner.attachments.insert(
            channel,
            Attachment {
                handle,
                id,
                waiting: Some(request),
                resizes: VecDeque::new(),
            },
        );
        inner.outbox.open(channel);
        inner.created.remove(&session);
        inner.refresh(&self.notifier);
        Ok(())
    }

    pub(super) fn snapshot(
        &self,
        channel: ChannelId,
        snapshot: AttachSnapshot,
        process: Option<ForegroundProcess>,
    ) -> bool {
        let mut inner = self.lock();
        let Some(attachment) = inner.attachments.get_mut(&channel) else {
            return false;
        };
        if let Some(id) = attachment.waiting.take() {
            inner.outbox.push_control(Message::Reply {
                id,
                body: ReplyBody::Attached {
                    snapshot: Box::new(snapshot),
                    process,
                },
            });
            self.ready.notify_one();
        }
        true
    }

    pub(super) fn attach_failed(&self, channel: ChannelId, error: &ServerError) {
        let mut inner = self.lock();
        inner.retire(channel, error);
        inner.refresh(&self.notifier);
        self.ready.notify_one();
    }

    pub(super) fn handle(&self, channel: ChannelId) -> Option<SessionHandle> {
        self.lock()
            .attachments
            .get(&channel)
            .map(|attachment| attachment.handle.clone())
    }

    pub(super) fn resize(
        &self,
        channel: ChannelId,
        request: RequestId,
        size: Size,
    ) -> Result<(), ServerError> {
        let mut inner = self.lock();
        let attachment = inner
            .attachments
            .get_mut(&channel)
            .ok_or_else(|| ServerError::new(ErrorCode::UnknownChannel, "unknown channel"))?;
        attachment.handle.send(SessionCommand::ResizeAttachment {
            id: attachment.id,
            size,
        })?;
        attachment.resizes.push_back(request);
        Ok(())
    }

    pub(super) fn resized(&self, channel: ChannelId, frame: ScreenFrame) -> bool {
        let mut inner = self.lock();
        let Some(attachment) = inner.attachments.get_mut(&channel) else {
            return false;
        };
        if let Some(id) = attachment.resizes.pop_front() {
            inner.outbox.replace_frame(channel, frame);
            inner.outbox.push_control(Message::Reply {
                id,
                body: ReplyBody::Resized,
            });
            self.ready.notify_one();
        }
        true
    }

    pub(super) fn detach(&self, channel: ChannelId) -> Result<(), ServerError> {
        let mut inner = self.lock();
        if !inner.attachments.contains_key(&channel) {
            return Err(ServerError::new(
                ErrorCode::UnknownChannel,
                "unknown channel",
            ));
        }
        inner.retire(
            channel,
            &ServerError::new(ErrorCode::UnknownChannel, "channel detached"),
        );
        inner.refresh(&self.notifier);
        self.ready.notify_one();
        Ok(())
    }

    pub(crate) fn session_ended(&self, session: SessionId, reason: ExitReason) {
        let mut inner = self.lock();
        if inner.closed {
            return;
        }
        inner.created.remove(&session);
        inner
            .outbox
            .retain_control(|message| session_state(message) != Some(session));
        inner.references.lock().ended(session);
        inner.state.session_colors.remove(&session);
        inner.open_sessions.remove(&session);
        let channels: Vec<_> = inner
            .attachments
            .iter()
            .filter_map(|(&channel, attachment)| {
                (attachment.handle.id() == session).then_some(channel)
            })
            .collect();
        let error = ServerError::unknown_session(session);
        for channel in channels {
            inner.retire(channel, &error);
        }
        inner.refresh(&self.notifier);
        inner
            .outbox
            .push_control(Message::SessionEnded { session, reason });
        self.ready.notify_one();
    }

    pub(super) fn push_frame(&self, channel: ChannelId, frame: ScreenFrame) -> bool {
        let mut inner = self.lock();
        if inner.closed || !inner.outbox.push_frame(channel, frame) {
            return false;
        }
        self.ready.notify_one();
        true
    }

    pub(super) fn push_metadata(&self, channel: ChannelId, event: MetadataEvent) -> bool {
        let mut inner = self.lock();
        if inner.closed || !inner.outbox.push_metadata(channel, event) {
            return false;
        }
        self.ready.notify_one();
        true
    }

    pub(super) fn ack(&self, channel: ChannelId, seq: u64) {
        if self.lock().outbox.ack(channel, seq) {
            self.ready.notify_one();
        }
    }

    /// The next message to write; `None` once closed and drained of control messages.
    pub(super) fn next(&self) -> Option<(ChannelId, Message)> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(next) = inner.outbox.next_control() {
                return Some(next);
            }
            if inner.closed {
                return None;
            }
            if let Some(next) = inner.outbox.next_channel() {
                return Some(next);
            }
            inner = self
                .ready
                .wait(inner)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Takes the control messages waiting to be written.
    #[cfg(test)]
    pub(crate) fn take_control(&self) -> Vec<Message> {
        self.lock().outbox.take_control()
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.lock().closed
    }

    pub(super) fn close_with(&self, message: Message) {
        let mut inner = self.lock();
        if !inner.closed {
            inner.outbox.push_control(message);
            inner.close(&self.notifier);
            self.ready.notify_all();
        }
    }

    pub(crate) fn close(&self) {
        self.lock().close(&self.notifier);
        self.ready.notify_all();
    }

    fn lock(&self) -> Locked<'_> {
        Locked {
            inner: Some(self.inner.lock().unwrap_or_else(PoisonError::into_inner)),
            notifier: &self.notifier,
        }
    }
}

/// The session whose progress or metadata a message carries.
fn session_state(message: &Message) -> Option<SessionId> {
    match message {
        Message::Progress { session, .. } | Message::SessionMetadata { session, .. } => {
            Some(*session)
        }
        _ => None,
    }
}

impl Inner {
    /// Sends each matching attached session the colors it should use now.
    fn apply_colors(&self, matches: impl Fn(SessionId) -> bool) {
        for attachment in self.attachments.values() {
            let session = attachment.handle.id();
            if matches(session)
                && let Some(colors) = self.state.colors_for(session)
            {
                let _ = attachment
                    .handle
                    .send(SessionCommand::SetColors(colors.clone()));
            }
        }
    }

    fn refresh(&mut self, notifier: &Notifier) {
        let references = self.references.clone();
        self.refresh_positions(&references.lock(), notifier);
    }

    fn refresh_positions(&mut self, references: &References, notifier: &Notifier) {
        if self.closed {
            return;
        }
        let mut sessions = self.open_sessions.clone();
        sessions.extend(&self.created);
        sessions.extend(
            self.attachments
                .values()
                .map(|attachment| attachment.handle.id())
                .filter(|session| !references.is_released(*session)),
        );
        let previous = self.positions.len();
        self.positions
            .retain(|session, _| sessions.contains(session));
        if previous != self.positions.len() {
            notifier.next_position();
            self.positions_changed = true;
        }
        for session in sessions {
            self.positions.entry(session).or_insert_with(|| {
                self.positions_changed = true;
                notifier.next_position()
            });
        }
    }

    fn close(&mut self, notifier: &Notifier) {
        if !self.positions.is_empty() {
            self.positions.clear();
            notifier.next_position();
            self.positions_changed = true;
        }
        self.closed = true;
        for (_, attachment) in self.attachments.drain() {
            let _ = attachment
                .handle
                .send(SessionCommand::Detach(attachment.id));
        }
        self.outbox.close_channels();
    }

    fn retire(&mut self, channel: ChannelId, error: &ServerError) {
        self.outbox.close_channel(channel);
        if let Some(attachment) = self.attachments.remove(&channel) {
            let _ = attachment
                .handle
                .send(SessionCommand::Detach(attachment.id));
            for id in attachment.waiting.into_iter().chain(attachment.resizes) {
                self.outbox.push_control(Message::Reply {
                    id,
                    body: ReplyBody::Error(error.to_reply()),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn a_session_keeps_its_own_colors_until_cleared() -> Result<(), ServerError> {
        let colors = |shade| TerminalColors {
            background: [shade; 3],
            ..TerminalColors::default()
        };
        let sent = |commands: &std::sync::mpsc::Receiver<crate::session::OwnerEvent>| {
            commands
                .try_iter()
                .filter_map(|event| match event {
                    crate::session::OwnerEvent::Command(SessionCommand::SetColors(colors)) => {
                        Some(colors.background[0])
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let connection = Connection::new(Arc::default());
        let (handle, commands) = SessionHandle::fake();
        let session = handle.id();
        connection.set_colors(colors(1));
        connection.set_session_colors(session, Some(colors(2)));
        let id = AttachmentId(1);
        connection.attach(
            ChannelId(1),
            id,
            RequestId(1),
            handle,
            SessionCommand::Detach(id),
        )?;
        assert_eq!(sent(&commands), [2]);
        connection.set_colors(colors(3));
        assert!(sent(&commands).is_empty());
        connection.set_session_colors(session, Some(colors(4)));
        assert_eq!(sent(&commands), [4]);
        connection.set_session_colors(session, None);
        assert_eq!(sent(&commands), [3]);
        connection.set_colors(colors(5));
        assert_eq!(sent(&commands), [5]);
        connection.set_session_colors(session, Some(colors(6)));
        connection.session_ended(session, ExitReason::Exited(0));
        assert!(connection.lock().state.session_colors.is_empty());
        Ok(())
    }

    fn referencing(connection: &Connection, sessions: &[SessionId]) {
        let mut references = References::default();
        references.update(1, sessions);
        connection.use_references(
            None,
            &SharedReferences::new(references),
            Some(sessions),
            false,
        );
    }

    #[test]
    fn session_state_only_reaches_connections_that_reference_the_session() {
        let connection = Connection::new(Arc::default());
        let session = SessionId::from(std::num::NonZeroU64::MIN);
        let message = |title: String| Message::SessionMetadata {
            session,
            metadata: muxy_protocol::SessionMetadata {
                title,
                directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
                process: None,
            },
        };
        connection.push_control(message("ignored".into()));
        assert!(connection.take_control().is_empty());
        referencing(&connection, &[session]);
        connection.push_control(message("first".into()));
        connection.push_control(message("latest".into()));
        assert_eq!(connection.take_control(), [message("latest".into())]);
        connection.push_control(message("pending".into()));
        referencing(&connection, &[]);
        connection.push_control(message("ignored".into()));
        assert!(connection.take_control().is_empty());
    }

    #[test]
    fn every_control_message_is_written_before_a_closed_connection_ends() {
        for message in Message::samples()
            .into_iter()
            .filter(|message| message.channel_kind() == muxy_protocol::ChannelKind::Control)
        {
            let connection = Connection::new(Arc::default());
            if let Some(session) = session_state(&message) {
                referencing(&connection, &[session]);
            }
            connection.push_control(message.clone());
            connection.close();
            assert_eq!(connection.next(), Some((muxy_protocol::CONTROL, message)));
            assert_eq!(connection.next(), None);
        }
    }

    #[test]
    fn a_retired_channel_drops_what_waits_and_fails_its_requests() -> Result<(), ServerError> {
        let connection = Connection::new(Arc::default());
        let (handle, _commands) = SessionHandle::fake();
        let channel = ChannelId(1);
        let id = AttachmentId(1);
        connection.attach(
            channel,
            id,
            RequestId(7),
            handle,
            SessionCommand::Detach(id),
        )?;
        assert!(connection.push_metadata(channel, MetadataEvent::Bell));
        connection.detach(channel)?;
        assert!(!connection.push_metadata(channel, MetadataEvent::Bell));
        assert!(matches!(
            connection.take_control().as_slice(),
            [Message::Reply {
                id: RequestId(7),
                body: ReplyBody::Error(_)
            }]
        ));
        connection.close();
        assert_eq!(connection.next(), None);
        Ok(())
    }
}
