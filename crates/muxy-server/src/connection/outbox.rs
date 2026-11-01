use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use muxy_protocol::{
    AttachSnapshot, CONTROL, ChannelId, ErrorCode, ExitReason, ForegroundProcess, Message,
    MetadataEvent, ReplyBody, RequestId, ScreenFrame, SessionId, Size, TerminalColors, Topic,
};

use crate::notifier::Notifier;
use crate::{AttachmentId, ServerError, SessionCommand, SessionHandle};

use super::merge::merge;

struct Attachment {
    handle: SessionHandle,
    id: AttachmentId,
    waiting: Option<RequestId>,
    resizes: VecDeque<RequestId>,
}

#[derive(Clone, Default)]
pub(crate) struct References {
    pub(crate) owner: Option<muxy_protocol::OperationId>,
    pub(crate) revision: u64,
    sessions: HashSet<SessionId>,
    released: HashSet<SessionId>,
}

impl References {
    pub(crate) fn update(
        &mut self,
        owner: Option<muxy_protocol::OperationId>,
        revision: u64,
        sessions: Vec<SessionId>,
    ) {
        let sessions: HashSet<_> = sessions.into_iter().collect();
        self.released
            .extend(self.sessions.difference(&sessions).copied());
        self.released.retain(|session| !sessions.contains(session));
        self.owner = owner;
        self.revision = revision;
        self.sessions = sessions;
    }
}

/// Invalidations this connection asked for by reading the matching state.
#[derive(Clone, Copy, Default)]
struct Watches {
    catalog: bool,
    activity: bool,
    remote_access: bool,
}

impl Watches {
    fn includes(self, topic: Topic) -> bool {
        match topic {
            Topic::Catalog | Topic::Sessions => self.catalog,
            Topic::Activity => self.activity,
            Topic::RemoteAccess => self.remote_access,
            Topic::Unrecognized(_) => false,
        }
    }
}

#[derive(Default)]
struct State {
    client: muxy_protocol::SessionClient,
    /// The paired device on the other end of a network connection.
    device: Option<(muxy_protocol::DeviceId, muxy_protocol::DeviceKind)>,
    positions: HashMap<SessionId, u64>,
    open_sessions: HashSet<SessionId>,
    created: HashSet<SessionId>,
    /// Who shows which session changed; announced once the lock is released.
    positions_changed: bool,
    references: References,
    watches: Watches,
    colors: Option<TerminalColors>,
    session_colors: HashMap<SessionId, TerminalColors>,
    control: VecDeque<Message>,
    pending: HashMap<ChannelId, ScreenFrame>,
    metadata: HashMap<ChannelId, Vec<MetadataEvent>>,
    credit: HashMap<ChannelId, bool>,
    sent: HashMap<ChannelId, u64>,
    attachments: HashMap<ChannelId, Attachment>,
    closed: bool,
}

pub(crate) struct Outbox {
    state: Mutex<State>,
    ready: Condvar,
    notifier: Arc<Notifier>,
}

/// The locked state; announces ownership changes after unlocking.
struct Locked<'a> {
    state: Option<MutexGuard<'a, State>>,
    notifier: &'a Notifier,
}

impl Deref for Locked<'_> {
    type Target = State;

    fn deref(&self) -> &State {
        self.state.as_deref().unwrap_or_else(|| unreachable!())
    }
}

impl DerefMut for Locked<'_> {
    fn deref_mut(&mut self) -> &mut State {
        self.state.as_deref_mut().unwrap_or_else(|| unreachable!())
    }
}

impl Drop for Locked<'_> {
    fn drop(&mut self) {
        let Some(mut state) = self.state.take() else {
            return;
        };
        let changed = std::mem::take(&mut state.positions_changed);
        drop(state);
        if changed {
            self.notifier.sessions_changed();
        }
    }
}

impl Outbox {
    pub(crate) fn new(notifier: Arc<Notifier>) -> Self {
        Self {
            state: Mutex::default(),
            ready: Condvar::default(),
            notifier,
        }
    }

    /// A computer says which app it is once connected, as local apps do.
    pub(super) fn for_device(
        notifier: Arc<Notifier>,
        device: muxy_protocol::DeviceId,
        kind: muxy_protocol::DeviceKind,
    ) -> Self {
        let outbox = Self::new(notifier);
        {
            let mut state = outbox.lock();
            state.device = Some((device, kind));
            if kind != muxy_protocol::DeviceKind::Computer {
                state.client.kind = muxy_protocol::ClientKind::Mobile;
            }
        }
        outbox
    }

    pub(crate) fn device(&self) -> Option<muxy_protocol::DeviceId> {
        self.lock().device.map(|(device, _)| device)
    }

    pub(crate) fn device_kind(&self) -> Option<muxy_protocol::DeviceKind> {
        self.lock().device.map(|(_, kind)| kind)
    }

    pub(crate) fn reference_state(&self) -> Option<References> {
        let state = self.lock();
        (!state.closed).then(|| state.references.clone())
    }

    /// Returns the sessions this connection didn't reference before.
    pub(crate) fn set_references(
        &self,
        references: References,
        reported: Option<&[SessionId]>,
        acknowledge_creation: bool,
    ) -> Vec<SessionId> {
        let mut state = self.lock();
        if state.closed {
            return Vec::new();
        }
        let added = references
            .sessions
            .difference(&state.references.sessions)
            .copied()
            .collect();
        state
            .created
            .retain(|session| !acknowledge_creation && !references.sessions.contains(session));
        if let Some(sessions) = reported {
            state.open_sessions = sessions.iter().copied().collect();
        }
        state
            .open_sessions
            .retain(|session| references.sessions.contains(session));
        state.control.retain(|message| !matches!(message, Message::Progress { session, .. } | Message::SessionMetadata { session, .. } if !references.sessions.contains(session)));
        state.references = references;
        state.refresh_positions(&self.notifier);
        added
    }

    pub(crate) fn created_session(&self, session: SessionId) {
        let mut state = self.lock();
        if !state.closed && !state.positions.contains_key(&session) {
            state.created.insert(session);
            state.refresh_positions(&self.notifier);
        }
    }

    pub(super) fn watch_activity(&self) {
        self.lock().watches.activity = true;
    }
    pub(super) fn watch_remote_access(&self) {
        self.lock().watches.remote_access = true;
    }

    pub(crate) fn client(&self) -> muxy_protocol::SessionClient {
        self.lock().client
    }

    pub(super) fn identify(&self, kind: muxy_protocol::ClientKind) -> muxy_protocol::SessionClient {
        let mut state = self.lock();
        if state.client.kind != kind {
            state.client.kind = kind;
            self.notifier.next_position();
            state.positions_changed = true;
        }
        state.client
    }

    pub(crate) fn participation(
        &self,
        session: SessionId,
    ) -> Option<(u64, muxy_protocol::SessionClient)> {
        let state = self.lock();
        if state.closed {
            return None;
        }
        state
            .positions
            .get(&session)
            .map(|position| (*position, state.client))
    }

    pub(crate) fn references(&self, session: SessionId, include_channels: bool) -> bool {
        let state = self.lock();
        !state.closed
            && (state.references.sessions.contains(&session)
                || (include_channels && state.created.contains(&session))
                || (include_channels
                    && !state.references.released.contains(&session)
                    && state
                        .attachments
                        .values()
                        .any(|attachment| attachment.handle.info().id == session)))
    }

    pub(crate) fn detach_session(&self, session: SessionId) {
        let mut state = self.lock();
        state.created.remove(&session);
        let channels: Vec<_> = state
            .attachments
            .iter()
            .filter(|(_, attachment)| attachment.handle.info().id == session)
            .map(|(channel, _)| *channel)
            .collect();
        for channel in channels {
            state.retire(
                channel,
                &ServerError::new(ErrorCode::UnknownChannel, "pane was closed"),
                &self.notifier,
            );
        }
        state.refresh_positions(&self.notifier);
        self.ready.notify_one();
    }

    pub(super) fn watch_catalog(&self) {
        self.lock().watches.catalog = true;
    }

    /// Tells the client to refetch a topic it reads.
    pub(crate) fn changed(&self, topic: Topic, revision: u64) {
        let mut state = self.lock();
        if state.closed || !state.watches.includes(topic) {
            return;
        }
        if let Some(Message::Changed {
            revision: pending, ..
        }) = state.control.iter_mut().find(
            |message| matches!(message, Message::Changed { topic: pending, .. } if *pending == topic),
        ) {
            *pending = (*pending).max(revision);
            return;
        }
        state
            .control
            .push_back(Message::Changed { topic, revision });
        self.ready.notify_one();
    }

    pub(super) fn colors(&self) -> Option<TerminalColors> {
        self.lock().colors.clone()
    }

    pub(super) fn set_colors(&self, colors: TerminalColors) {
        let mut state = self.lock();
        state.colors = Some(colors);
        state.apply_colors(|session| !state.session_colors.contains_key(&session));
    }

    pub(super) fn set_session_colors(&self, session: SessionId, colors: Option<TerminalColors>) {
        let mut state = self.lock();
        match colors {
            Some(colors) => state.session_colors.insert(session, colors),
            None => state.session_colors.remove(&session),
        };
        state.apply_colors(|attached| attached == session);
    }

    pub(crate) fn push_control(&self, message: Message) {
        let mut state = self.lock();
        if !state.closed {
            if let Message::SessionMetadata { session, .. } | Message::Progress { session, .. } =
                &message
            {
                if !state.references.sessions.contains(session) {
                    return;
                }
                if let Some(pending) = state.control.iter_mut().find(|pending| {
                    std::mem::discriminant(*pending) == std::mem::discriminant(&message)
                        && matches!(pending, Message::SessionMetadata { session: id, .. } | Message::Progress { session: id, .. } if id == session)
                }) {
                    *pending = message;
                    return;
                }
            }
            if let Message::FilesChanged { project, changes } = &message
                && let Some(Message::FilesChanged { changes: pending, .. }) = state.control.iter_mut().find(|pending| matches!(pending, Message::FilesChanged { project: id, .. } if id == project)) {
                pending.merge(changes);
                return;
            }
            if let Message::GitChanged { project } = &message
                && state.control.iter().any(|pending| matches!(pending, Message::GitChanged { project: id } if id == project)) {
                return;
            }
            state.control.push_back(message);
            self.ready.notify_one();
        }
    }

    pub(super) fn attach(
        &self,
        channel: ChannelId,
        id: AttachmentId,
        request: RequestId,
        handle: SessionHandle,
        command: SessionCommand,
    ) -> Result<(), ServerError> {
        let mut state = self.lock();
        if state.closed {
            return Err(ServerError::new(
                ErrorCode::BadRequest,
                "connection is closed",
            ));
        }
        let session = handle.info().id;
        if state.references.owner.is_some() && state.references.released.contains(&session) {
            return Err(ServerError::unknown_session(session));
        }
        if let Some(colors) = state.colors_for(session) {
            handle.send(SessionCommand::SetColors(colors.clone()))?;
        }
        if let Err(undelivered) = handle.try_send(command) {
            // Its sink reports back to this outbox when dropped, which takes the lock.
            drop(state);
            drop(undelivered);
            return Err(ServerError::unknown_session(session));
        }
        state.references.released.remove(&session);
        state.attachments.insert(
            channel,
            Attachment {
                handle,
                id,
                waiting: Some(request),
                resizes: VecDeque::new(),
            },
        );
        state.credit.insert(channel, true);
        state.created.remove(&session);
        state.refresh_positions(&self.notifier);
        Ok(())
    }

    pub(super) fn snapshot(
        &self,
        channel: ChannelId,
        snapshot: AttachSnapshot,
        process: Option<ForegroundProcess>,
    ) -> bool {
        let mut state = self.lock();
        let Some(attachment) = state.attachments.get_mut(&channel) else {
            return false;
        };
        if let Some(id) = attachment.waiting.take() {
            state.control.push_back(Message::Reply {
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
        let mut state = self.lock();
        state.retire(channel, error, &self.notifier);
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
        let mut state = self.lock();
        let attachment = state
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
        let mut state = self.lock();
        let Some(attachment) = state.attachments.get_mut(&channel) else {
            return false;
        };
        if let Some(id) = attachment.resizes.pop_front() {
            state.pending.insert(channel, frame);
            state.control.push_back(Message::Reply {
                id,
                body: ReplyBody::Resized,
            });
            self.ready.notify_one();
        }
        true
    }

    pub(super) fn detach(&self, channel: ChannelId) -> Result<(), ServerError> {
        let mut state = self.lock();
        if !state.attachments.contains_key(&channel) {
            return Err(ServerError::new(
                ErrorCode::UnknownChannel,
                "unknown channel",
            ));
        }
        state.retire(
            channel,
            &ServerError::new(ErrorCode::UnknownChannel, "channel detached"),
            &self.notifier,
        );
        self.ready.notify_one();
        Ok(())
    }

    pub(crate) fn session_ended(&self, session: SessionId, reason: ExitReason) {
        let mut state = self.lock();
        if state.closed {
            return;
        }
        state.created.remove(&session);
        state.control.retain(
            |message| !matches!(message, Message::Progress { session: id, .. } | Message::SessionMetadata { session: id, .. } if *id == session),
        );
        state.references.sessions.remove(&session);
        state.session_colors.remove(&session);
        state.open_sessions.remove(&session);
        let channels: Vec<_> = state
            .attachments
            .iter()
            .filter_map(|(&channel, attachment)| {
                (attachment.handle.id() == session).then_some(channel)
            })
            .collect();
        let error = ServerError::unknown_session(session);
        for channel in channels {
            state.retire(channel, &error, &self.notifier);
        }
        state.refresh_positions(&self.notifier);
        state
            .control
            .push_back(Message::SessionEnded { session, reason });
        self.ready.notify_one();
    }

    pub(super) fn push_frame(&self, channel: ChannelId, frame: ScreenFrame) -> bool {
        let mut state = self.lock();
        if !state.credit.contains_key(&channel) || state.closed {
            return false;
        }
        match state.pending.entry(channel) {
            std::collections::hash_map::Entry::Occupied(mut entry) => merge(entry.get_mut(), frame),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(frame);
            }
        }
        self.ready.notify_one();
        true
    }

    pub(super) fn push_metadata(&self, channel: ChannelId, event: MetadataEvent) -> bool {
        let mut state = self.lock();
        if !state.credit.contains_key(&channel) || state.closed {
            return false;
        }
        let pending = state.metadata.entry(channel).or_default();
        if let Some(previous) = pending
            .iter_mut()
            .find(|previous| std::mem::discriminant(*previous) == std::mem::discriminant(&event))
        {
            *previous = event;
        } else {
            pending.push(event);
        }
        self.ready.notify_one();
        true
    }

    pub(super) fn ack(&self, channel: ChannelId, seq: u64) {
        let mut state = self.lock();
        if state.sent.get(&channel) == Some(&seq)
            && let Some(credit) = state.credit.get_mut(&channel)
        {
            *credit = true;
            self.ready.notify_one();
        }
    }

    pub(super) fn next(&self) -> Option<(ChannelId, Message)> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(message) = state.control.pop_front() {
                return Some((CONTROL, message));
            }
            if state.closed {
                return None;
            }
            if let Some(channel) = state.metadata.keys().next().copied()
                && let Some(pending) = state.metadata.get_mut(&channel)
            {
                let event = pending.remove(0);
                if pending.is_empty() {
                    state.metadata.remove(&channel);
                }
                return Some((channel, Message::Metadata(event)));
            }
            let channel = state
                .pending
                .keys()
                .find(|channel| state.credit.get(channel) == Some(&true))
                .copied();
            if let Some(channel) = channel
                && let Some(frame) = state.pending.remove(&channel)
            {
                state.credit.insert(channel, false);
                state.sent.insert(channel, frame.seq);
                return Some((channel, Message::Frame(frame)));
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Takes the control messages waiting to be written.
    #[cfg(test)]
    pub(crate) fn take_control(&self) -> Vec<Message> {
        self.lock().control.drain(..).collect()
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.lock().closed
    }

    pub(super) fn close_with(&self, message: Message) {
        let mut state = self.lock();
        if !state.closed {
            state.control.push_back(message);
            state.close(&self.notifier);
            self.ready.notify_all();
        }
    }

    pub(crate) fn close(&self) {
        self.lock().close(&self.notifier);
        self.ready.notify_all();
    }

    fn lock(&self) -> Locked<'_> {
        Locked {
            state: Some(self.state.lock().unwrap_or_else(PoisonError::into_inner)),
            notifier: &self.notifier,
        }
    }
}

impl State {
    fn colors_for(&self, session: SessionId) -> Option<&TerminalColors> {
        self.session_colors.get(&session).or(self.colors.as_ref())
    }

    /// Sends each matching attached session the colors it should use now.
    fn apply_colors(&self, matches: impl Fn(SessionId) -> bool) {
        for attachment in self.attachments.values() {
            let session = attachment.handle.id();
            if matches(session)
                && let Some(colors) = self.colors_for(session)
            {
                let _ = attachment
                    .handle
                    .send(SessionCommand::SetColors(colors.clone()));
            }
        }
    }

    fn refresh_positions(&mut self, notifier: &Notifier) {
        if self.closed {
            return;
        }
        let mut sessions = self.open_sessions.clone();
        sessions.extend(&self.created);
        sessions.extend(
            self.attachments
                .values()
                .map(|attachment| attachment.handle.id())
                .filter(|session| !self.references.released.contains(session)),
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
        self.pending.clear();
        self.metadata.clear();
        self.credit.clear();
        self.sent.clear();
    }

    fn retire(&mut self, channel: ChannelId, error: &ServerError, notifier: &Notifier) {
        self.pending.remove(&channel);
        self.metadata.remove(&channel);
        self.credit.remove(&channel);
        self.sent.remove(&channel);
        if let Some(attachment) = self.attachments.remove(&channel) {
            let _ = attachment
                .handle
                .send(SessionCommand::Detach(attachment.id));
            for id in attachment.waiting.into_iter().chain(attachment.resizes) {
                self.control.push_back(Message::Reply {
                    id,
                    body: ReplyBody::Error(error.to_reply()),
                });
            }
        }
        self.refresh_positions(notifier);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use muxy_protocol::{Cursor, Modes, Row};

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
        let outbox = Outbox::new(Arc::default());
        let (handle, commands) = SessionHandle::fake();
        let session = handle.id();
        outbox.set_colors(colors(1));
        outbox.set_session_colors(session, Some(colors(2)));
        let id = AttachmentId(1);
        outbox.attach(
            ChannelId(1),
            id,
            RequestId(1),
            handle,
            SessionCommand::Detach(id),
        )?;
        assert_eq!(sent(&commands), [2]);
        outbox.set_colors(colors(3));
        assert!(sent(&commands).is_empty());
        outbox.set_session_colors(session, Some(colors(4)));
        assert_eq!(sent(&commands), [4]);
        outbox.set_session_colors(session, None);
        assert_eq!(sent(&commands), [3]);
        outbox.set_colors(colors(5));
        assert_eq!(sent(&commands), [5]);
        outbox.set_session_colors(session, Some(colors(6)));
        outbox.session_ended(session, ExitReason::Exited(0));
        assert!(outbox.lock().session_colors.is_empty());
        Ok(())
    }

    #[test]
    fn file_changes_coalesce_and_rescan_for_slow_consumers() {
        let outbox = Outbox::new(Arc::default());
        let project = muxy_protocol::ProjectId::new();
        for index in 0..1000 {
            outbox.push_control(Message::FilesChanged {
                project,
                changes: muxy_protocol::FileChanges {
                    paths: vec![muxy_protocol::ServerPath::from(index.to_string())],
                    rescan: false,
                },
            });
        }
        assert_eq!(outbox.lock().control.len(), 1);
        assert_eq!(
            outbox.next(),
            Some((
                CONTROL,
                Message::FilesChanged {
                    project,
                    changes: muxy_protocol::FileChanges {
                        paths: Vec::new(),
                        rescan: true
                    }
                }
            ))
        );
    }

    fn frame(seq: u64, index: u16) -> ScreenFrame {
        ScreenFrame {
            size: Size { cols: 80, rows: 24 },
            graphics: None,
            seq,
            reset: false,
            rows: vec![Row {
                index,
                runs: vec![],
            }],
            cursor: Cursor {
                shape: muxy_protocol::CursorShape::default(),
                row: 0,
                col: 0,
                visible: true,
            },
            modes: Modes::default(),
        }
    }

    #[test]
    fn title_updates_coalesce_and_only_reach_referencing_connections() {
        let outbox = Outbox::new(Arc::default());
        let session = SessionId::from(std::num::NonZeroU64::MIN);
        let message = |title: String| Message::SessionMetadata {
            session,
            metadata: muxy_protocol::SessionMetadata {
                title,
                directory: muxy_protocol::ServerPath(b"/tmp".to_vec()),
                process: None,
            },
        };
        outbox.lock().references.sessions.insert(session);
        for i in 0..1000 {
            outbox.push_control(message(i.to_string()));
        }
        assert_eq!(outbox.lock().control.len(), 1);
        assert_eq!(outbox.next(), Some((CONTROL, message("999".into()))));
        outbox.push_control(message("pending".into()));
        outbox.set_references(References::default(), Some(&[]), false);
        outbox.push_control(message("ignored".into()));
        assert!(outbox.lock().control.is_empty());
    }

    #[test]
    fn session_state_is_pushed_once_and_introduced_to_new_references() {
        let notifier = Arc::new(Notifier::default());
        let outbox = Arc::new(Outbox::new(Arc::clone(&notifier)));
        notifier.register(&outbox);
        let (handle, _commands) = SessionHandle::fake();
        let session = handle.id();
        let published = crate::session::Published::new(handle.info(), notifier);
        let progress = |percent| muxy_protocol::SessionProgress {
            progress: Some(muxy_protocol::TerminalProgress {
                state: muxy_protocol::ProgressState::Running,
                percent: Some(percent),
            }),
            completed: 0,
        };
        let metadata = Message::SessionMetadata {
            session,
            metadata: muxy_protocol::SessionMetadata {
                title: String::new(),
                directory: handle.info().directory.clone(),
                process: None,
            },
        };
        let mut references = References::default();
        references.update(None, 1, vec![session]);

        assert_eq!(
            outbox.set_references(references.clone(), Some(&[session]), false),
            [session]
        );
        published.introduce(&outbox);
        published.set_progress(progress(10));
        published.set_progress(progress(10));
        assert_eq!(
            outbox.take_control(),
            [
                metadata.clone(),
                Message::Progress {
                    session,
                    progress: progress(10)
                }
            ]
        );

        outbox.set_references(References::default(), Some(&[]), false);
        published.set_progress(progress(20));
        assert!(outbox.take_control().is_empty());

        assert_eq!(
            outbox.set_references(references.clone(), Some(&[session]), false),
            [session]
        );
        assert!(
            outbox
                .set_references(references, Some(&[session]), false)
                .is_empty()
        );
        published.introduce(&outbox);
        assert_eq!(
            outbox.take_control(),
            [
                metadata,
                Message::Progress {
                    session,
                    progress: progress(20)
                }
            ]
        );
    }

    #[test]
    fn negotiated_development_protocol_preserves_every_control_message() {
        for message in Message::samples()
            .into_iter()
            .filter(|message| message.channel_kind() == muxy_protocol::ChannelKind::Control)
        {
            let outbox = Outbox::new(Arc::default());
            if let Message::Progress { session, .. } | Message::SessionMetadata { session, .. } =
                &message
            {
                outbox.lock().references.sessions.insert(*session);
            }
            outbox.push_control(message.clone());
            outbox.close();
            assert_eq!(outbox.next(), Some((CONTROL, message)));
            assert_eq!(outbox.next(), None);
        }
    }

    #[test]
    fn control_first_independent_credit_and_merged_pending_frames() {
        let outbox = Outbox::new(Arc::default());
        outbox
            .lock()
            .credit
            .extend([(ChannelId(1), true), (ChannelId(2), true)]);
        outbox.push_frame(ChannelId(1), frame(1, 0));
        outbox.push_control(Message::HelloReply {
            versions: muxy_protocol::SUPPORTED.to_vec(),
            server: muxy_protocol::ServerInfo::current(),
            features: Vec::new(),
        });
        assert!(matches!(
            outbox.next(),
            Some((CONTROL, Message::HelloReply { .. }))
        ));
        assert_eq!(
            outbox.next(),
            Some((ChannelId(1), Message::Frame(frame(1, 0))))
        );
        outbox.push_frame(ChannelId(1), frame(2, 1));
        outbox.push_frame(ChannelId(1), frame(4, 2));
        outbox.push_frame(ChannelId(2), frame(1, 0));
        assert_eq!(
            outbox.next(),
            Some((ChannelId(2), Message::Frame(frame(1, 0))))
        );
        outbox.ack(ChannelId(1), 0);
        outbox.ack(ChannelId(1), 5);
        assert!(!outbox.lock().credit[&ChannelId(1)]);
        assert_eq!(outbox.lock().pending.len(), 1);
        outbox.ack(ChannelId(1), 1);
        let mut merged = frame(4, 1);
        merged.rows.push(Row {
            index: 2,
            runs: vec![],
        });
        assert_eq!(outbox.next(), Some((ChannelId(1), Message::Frame(merged))));
        outbox.ack(ChannelId(1), 1);
        assert!(!outbox.lock().credit[&ChannelId(1)]);
    }

    #[test]
    fn metadata_coalesces_without_frame_credit_and_is_retired_with_its_channel() {
        let outbox = Outbox::new(Arc::default());
        let channel = ChannelId(1);
        outbox.lock().credit.insert(channel, false);
        for index in 0..1000 {
            outbox.push_metadata(channel, MetadataEvent::Title(index.to_string()));
            outbox.push_metadata(channel, MetadataEvent::Bell);
        }
        assert_eq!(outbox.lock().metadata[&channel].len(), 2);
        outbox.push_control(Message::VersionUnsupported);
        assert_eq!(outbox.next(), Some((CONTROL, Message::VersionUnsupported)));
        assert_eq!(
            outbox.next(),
            Some((
                channel,
                Message::Metadata(MetadataEvent::Title("999".into()))
            ))
        );
        assert_eq!(
            outbox.next(),
            Some((channel, Message::Metadata(MetadataEvent::Bell)))
        );
        outbox.push_metadata(channel, MetadataEvent::Bell);
        outbox.lock().retire(
            channel,
            &ServerError::new(ErrorCode::UnknownChannel, "detached"),
            &outbox.notifier,
        );
        outbox.push_metadata(channel, MetadataEvent::Bell);
        assert!(outbox.lock().metadata.is_empty());
        outbox.close();
        assert_eq!(outbox.next(), None);
    }

    #[test]
    fn clipboard_metadata_keeps_only_the_latest_write_and_is_retired() {
        let outbox = Outbox::new(Arc::default());
        let channel = ChannelId(1);
        outbox.lock().credit.insert(channel, false);
        outbox.push_metadata(channel, MetadataEvent::ClipboardWrite("first".into()));
        outbox.push_metadata(channel, MetadataEvent::ClipboardWrite("last".into()));
        assert_eq!(outbox.lock().metadata[&channel].len(), 1);
        assert_eq!(
            outbox.next(),
            Some((
                channel,
                Message::Metadata(MetadataEvent::ClipboardWrite("last".into()))
            ))
        );
        outbox.push_metadata(channel, MetadataEvent::ClipboardWrite("pending".into()));
        outbox.lock().retire(
            channel,
            &ServerError::new(ErrorCode::UnknownChannel, "detached"),
            &outbox.notifier,
        );
        assert!(outbox.lock().metadata.is_empty());
    }
}
