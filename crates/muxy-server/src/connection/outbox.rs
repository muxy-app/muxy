use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use muxy_protocol::wire::message_version;
use muxy_protocol::{
    AttachSnapshot, CONTROL, ChannelId, ErrorCode, ExitReason, ForegroundProcess, Message,
    MetadataEvent, ReplyBody, RequestId, ScreenFrame, SessionId, SessionProgress, Size,
    TerminalColors, Version,
};

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

#[derive(Default)]
struct State {
    client: muxy_protocol::SessionClient,
    positions: HashMap<SessionId, u64>,
    open_sessions: HashSet<SessionId>,
    created: HashSet<SessionId>,
    changes: Arc<AtomicU64>,
    references: References,
    progress: HashMap<SessionId, SessionProgress>,
    session_metadata: HashMap<SessionId, muxy_protocol::SessionMetadata>,
    catalog_watched: bool,
    activity_watched: bool,
    colors: Option<TerminalColors>,
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
    version: Version,
}

impl Outbox {
    pub(super) fn new(version: Version, changes: Arc<AtomicU64>) -> Self {
        Self {
            state: Mutex::new(State {
                changes,
                ..State::default()
            }),
            ready: Condvar::default(),
            version,
        }
    }

    pub(crate) fn referenced_sessions(&self) -> Vec<SessionId> {
        self.lock().references.sessions.iter().copied().collect()
    }

    pub(crate) fn reference_state(&self) -> Option<References> {
        let state = self.lock();
        (!state.closed).then(|| state.references.clone())
    }

    pub(crate) fn set_references(
        &self,
        references: References,
        reported: Option<&[SessionId]>,
        acknowledge_creation: bool,
    ) {
        let mut state = self.lock();
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
        state
            .progress
            .retain(|session, _| references.sessions.contains(session));
        state
            .session_metadata
            .retain(|session, _| references.sessions.contains(session));
        state.references = references;
        state.refresh_positions();
    }

    pub(crate) fn created_session(&self, session: SessionId) {
        let mut state = self.lock();
        if !state.closed && !state.positions.contains_key(&session) {
            state.created.insert(session);
            state.refresh_positions();
        }
    }

    pub(super) fn watch_activity(&self) {
        self.lock().activity_watched = true;
    }
    pub(super) fn activity_watched(&self) -> bool {
        self.lock().activity_watched
    }

    pub(crate) fn client(&self) -> muxy_protocol::SessionClient {
        self.lock().client
    }

    pub(super) fn identify(&self, kind: muxy_protocol::ClientKind) -> muxy_protocol::SessionClient {
        let mut state = self.lock();
        if state.client.kind != kind {
            state.client.kind = kind;
            state.changes.fetch_add(1, Ordering::AcqRel);
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
            );
        }
        state.refresh_positions();
        self.ready.notify_one();
    }

    pub(super) fn watch_catalog(&self) {
        self.lock().catalog_watched = true;
    }
    pub(super) fn catalog_watched(&self) -> bool {
        self.lock().catalog_watched
    }

    pub(super) fn colors(&self) -> Option<TerminalColors> {
        self.lock().colors.clone()
    }

    pub(super) fn set_colors(&self, colors: TerminalColors) {
        let mut state = self.lock();
        for attachment in state.attachments.values() {
            let _ = attachment
                .handle
                .send(SessionCommand::SetColors(colors.clone()));
        }
        state.colors = Some(colors);
    }

    pub(super) fn push_control(&self, message: Message) {
        let Some(message) = self.compatible(message) else {
            return;
        };
        let mut state = self.lock();
        if !state.closed {
            if let Message::ActivityChanged { revision } = &message
                && let Some(Message::ActivityChanged { revision: pending }) = state
                    .control
                    .iter_mut()
                    .find(|message| matches!(message, Message::ActivityChanged { .. }))
            {
                *pending = (*pending).max(*revision);
                return;
            }
            if let Message::SessionMetadata { session, metadata } = &message {
                if !state.references.sessions.contains(session)
                    || state.session_metadata.get(session) == Some(metadata)
                {
                    return;
                }
                state.session_metadata.insert(*session, metadata.clone());
                if let Some(pending) = state.control.iter_mut().find(|pending| matches!(pending, Message::SessionMetadata { session: id, .. } if id == session)) {
                    *pending = message;
                    return;
                }
            }
            if let Message::Progress { session, progress } = &message {
                if !state.references.sessions.contains(session)
                    || *progress == state.progress.get(session).copied().unwrap_or_default()
                {
                    return;
                }
                state.progress.insert(*session, *progress);
            }
            if let Message::Progress { session, .. } = &message
                && let Some(pending) = state.control.iter_mut().find(|pending| matches!(pending, Message::Progress { session: id, .. } if id == session)) {
                *pending = message;
                return;
            }
            if let Message::GitChanged { project } = &message
                && state.control.iter().any(|pending| matches!(pending, Message::GitChanged { project: id } if id == project)) {
                return;
            }
            if let Message::CatalogChanged { revision } = &message
                && let Some(Message::CatalogChanged { revision: pending }) = state
                    .control
                    .iter_mut()
                    .find(|message| matches!(message, Message::CatalogChanged { .. }))
            {
                *pending = (*pending).max(*revision);
                return;
            }
            if let Message::SessionsChanged { revision } = &message
                && let Some(Message::SessionsChanged { revision: pending }) = state
                    .control
                    .iter_mut()
                    .find(|message| matches!(message, Message::SessionsChanged { .. }))
            {
                *pending = (*pending).max(*revision);
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
        if let Some(colors) = &state.colors {
            handle.send(SessionCommand::SetColors(colors.clone()))?;
        }
        handle.send(command)?;
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
        state.refresh_positions();
        Ok(())
    }

    pub(super) fn snapshot(
        &self,
        channel: ChannelId,
        snapshot: AttachSnapshot,
        process: Option<ForegroundProcess>,
    ) {
        let mut state = self.lock();
        if let Some(attachment) = state.attachments.get_mut(&channel)
            && let Some(id) = attachment.waiting.take()
        {
            if let Some(message) = self.compatible(Message::Reply {
                id,
                body: ReplyBody::Attached {
                    snapshot: Box::new(snapshot),
                    process,
                },
            }) {
                state.control.push_back(message);
            }
            self.ready.notify_one();
        }
    }

    pub(super) fn attach_failed(&self, channel: ChannelId, error: &ServerError) {
        let mut state = self.lock();
        state.retire(channel, error);
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

    pub(super) fn resized(&self, channel: ChannelId, frame: ScreenFrame) {
        let mut state = self.lock();
        if let Some(attachment) = state.attachments.get_mut(&channel)
            && let Some(id) = attachment.resizes.pop_front()
        {
            state.pending.insert(channel, frame);
            state.control.push_back(Message::Reply {
                id,
                body: ReplyBody::Resized,
            });
            self.ready.notify_one();
        }
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
        );
        self.ready.notify_one();
        Ok(())
    }

    pub(super) fn session_ended(&self, session: SessionId, reason: ExitReason) {
        let mut state = self.lock();
        if state.closed {
            return;
        }
        state.created.remove(&session);
        state.control.retain(
            |message| !matches!(message, Message::Progress { session: id, .. } | Message::SessionMetadata { session: id, .. } if *id == session),
        );
        state.references.sessions.remove(&session);
        state.progress.remove(&session);
        state.session_metadata.remove(&session);
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
            state.retire(channel, &error);
        }
        state.refresh_positions();
        state
            .control
            .push_back(Message::SessionEnded { session, reason });
        self.ready.notify_one();
    }

    pub(super) fn push_frame(&self, channel: ChannelId, frame: ScreenFrame) {
        let mut state = self.lock();
        if !state.credit.contains_key(&channel) || state.closed {
            return;
        }
        match state.pending.entry(channel) {
            std::collections::hash_map::Entry::Occupied(mut entry) => merge(entry.get_mut(), frame),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(frame);
            }
        }
        self.ready.notify_one();
    }

    pub(super) fn push_metadata(&self, channel: ChannelId, event: MetadataEvent) {
        let Some(Message::Metadata(event)) = self.compatible(Message::Metadata(event)) else {
            return;
        };
        let mut state = self.lock();
        if !state.credit.contains_key(&channel) || state.closed {
            return;
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
        let mut state = self.lock();
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

    pub(crate) fn is_closed(&self) -> bool {
        self.lock().closed
    }

    pub(super) fn close_with(&self, message: Message) {
        let mut state = self.lock();
        if !state.closed {
            if let Some(message) = self.compatible(message) {
                state.control.push_back(message);
            }
            state.close();
            self.ready.notify_all();
        }
    }

    pub(super) fn close(&self) {
        self.lock().close();
        self.ready.notify_all();
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn compatible(&self, message: Message) -> Option<Message> {
        (message_version(&message) <= self.version).then_some(message)
    }
}

impl State {
    fn refresh_positions(&mut self) {
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
            self.changes.fetch_add(1, Ordering::AcqRel);
        }
        for session in sessions {
            self.positions
                .entry(session)
                .or_insert_with(|| self.changes.fetch_add(1, Ordering::AcqRel));
        }
    }

    fn close(&mut self) {
        if !self.positions.is_empty() {
            self.positions.clear();
            self.changes.fetch_add(1, Ordering::AcqRel);
        }
        self.closed = true;
        for (_, attachment) in self.attachments.drain() {
            let _ = attachment
                .handle
                .send(SessionCommand::Detach(attachment.id));
        }
        self.pending.clear();
        self.progress.clear();
        self.metadata.clear();
        self.credit.clear();
        self.sent.clear();
    }

    fn retire(&mut self, channel: ChannelId, error: &ServerError) {
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
        self.refresh_positions();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, mpsc};
    use std::thread;
    use std::time::Duration;

    use muxy_protocol::{Cursor, Modes, Row};

    use super::*;

    #[test]
    fn slow_activity_consumers_keep_one_latest_invalidation_without_frame_credit() {
        let outbox = Outbox::new(muxy_protocol::V1, Arc::default());
        for revision in 1..=1000 {
            outbox.push_control(Message::ActivityChanged { revision });
        }
        assert_eq!(outbox.lock().control.len(), 1);
        assert_eq!(
            outbox.next(),
            Some((CONTROL, Message::ActivityChanged { revision: 1000 }))
        );
    }

    fn frame(seq: u64, index: u16) -> ScreenFrame {
        ScreenFrame {
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
    fn prompt_metadata_is_bounded_and_keeps_the_merged_frame_watermark() {
        let outbox = Outbox::new(muxy_protocol::V1, Arc::default());
        let channel = ChannelId(1);
        outbox.lock().credit.insert(channel, false);
        for seq in 1..1000 {
            outbox.push_metadata(channel, MetadataEvent::ScreenPrompts { seq, rows: vec![1] });
            outbox.push_frame(channel, frame(seq, 0));
        }
        assert_eq!(outbox.lock().metadata[&channel].len(), 1);
        assert_eq!(
            outbox.next(),
            Some((
                channel,
                Message::Metadata(MetadataEvent::ScreenPrompts {
                    seq: 999,
                    rows: vec![1]
                })
            ))
        );
        outbox.lock().credit.insert(channel, true);
        assert_eq!(
            outbox.next(),
            Some((channel, Message::Frame(frame(999, 0))))
        );
    }

    #[test]
    fn history_counts_are_coalesced() {
        for version in muxy_protocol::SUPPORTED.iter().copied() {
            let outbox = Outbox::new(version, Arc::default());
            let channel = ChannelId(1);
            outbox.lock().credit.insert(channel, true);
            for total_rows in [10, 20, 30] {
                outbox.push_metadata(channel, MetadataEvent::History { total_rows });
            }
            let state = outbox.lock();
            if version == muxy_protocol::V1 {
                assert_eq!(
                    state.metadata[&channel],
                    [MetadataEvent::History { total_rows: 30 }]
                );
            } else {
                assert!(state.metadata.is_empty());
            }
        }
    }

    #[test]
    fn cursor_blinking_is_coalesced_without_credit() {
        for version in muxy_protocol::SUPPORTED.iter().copied() {
            let outbox = Outbox::new(version, Arc::default());
            let channel = ChannelId(1);
            outbox.lock().credit.insert(channel, false);
            for blinking in [false, true, false] {
                outbox.push_metadata(channel, MetadataEvent::CursorBlinking(blinking));
            }
            let state = outbox.lock();
            if version == muxy_protocol::V1 {
                assert_eq!(
                    state.metadata[&channel],
                    [MetadataEvent::CursorBlinking(false)]
                );
            } else {
                assert!(state.metadata.is_empty());
            }
        }
    }

    #[test]
    fn input_modes_are_coalesced_without_credit() {
        for version in muxy_protocol::SUPPORTED.iter().copied() {
            let outbox = Outbox::new(version, Arc::default());
            let channel = ChannelId(1);
            outbox.lock().credit.insert(channel, false);
            let modes = muxy_protocol::InputModes {
                mouse_tracking: true,
                alternate_scroll: true,
                focus_events: true,
            };
            outbox.push_metadata(
                channel,
                MetadataEvent::InputModes(muxy_protocol::InputModes::default()),
            );
            outbox.push_metadata(channel, MetadataEvent::InputModes(modes));
            let state = outbox.lock();
            if version == muxy_protocol::V1 {
                assert_eq!(state.metadata[&channel], [MetadataEvent::InputModes(modes)]);
            } else {
                assert!(state.metadata.is_empty());
            }
        }
    }

    #[test]
    fn title_updates_coalesce_and_resubscribe_without_screen_credit() {
        let outbox = Outbox::new(muxy_protocol::V1, Arc::default());
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
        outbox.push_control(message("999".into()));
        assert!(outbox.lock().control.is_empty());
        outbox.push_control(message("pending".into()));
        outbox.set_references(References::default(), Some(&[]), false);
        outbox.push_control(message("ignored".into()));
        assert!(outbox.lock().control.is_empty());
        outbox.lock().references.sessions.insert(session);
        outbox.push_control(message("pending".into()));
        assert_eq!(outbox.next(), Some((CONTROL, message("pending".into()))));
    }

    #[test]
    fn progress_coalesces_completions_and_releases_pending_updates_on_unsubscribe() {
        let outbox = Outbox::new(muxy_protocol::V1, Arc::default());
        let session = SessionId::from(std::num::NonZeroU64::MIN);
        let message = |completed| Message::Progress {
            session,
            progress: SessionProgress {
                progress: None,
                completed,
            },
        };
        outbox.lock().references.sessions.insert(session);
        for completed in 1..=1000 {
            outbox.push_control(message(completed));
        }
        assert_eq!(outbox.lock().control.len(), 1);
        assert_eq!(outbox.next(), Some((CONTROL, message(1000))));
        outbox.push_control(message(1001));
        outbox.set_references(References::default(), Some(&[]), false);
        outbox.push_control(message(1002));
        assert!(outbox.lock().control.is_empty());
    }

    #[test]
    fn progress_resubscription_restores_unchanged_state_between_polls() {
        for delivered in [false, true] {
            let outbox = Outbox::new(muxy_protocol::V1, Arc::default());
            let session = SessionId::from(std::num::NonZeroU64::MIN);
            let message = Message::Progress {
                session,
                progress: SessionProgress {
                    progress: Some(muxy_protocol::TerminalProgress {
                        state: muxy_protocol::ProgressState::Indeterminate,
                        percent: None,
                    }),
                    completed: 0,
                },
            };
            let mut references = References::default();
            references.update(None, 1, vec![session]);
            outbox.set_references(references.clone(), Some(&[session]), false);
            outbox.push_control(message.clone());
            if delivered {
                assert_eq!(outbox.next(), Some((CONTROL, message.clone())));
            }
            outbox.set_references(References::default(), Some(&[]), false);
            outbox.push_control(message.clone());
            assert!(outbox.lock().control.is_empty());
            outbox.set_references(references.clone(), Some(&[session]), false);
            outbox.push_control(message.clone());
            assert_eq!(outbox.lock().control.len(), 1);
            assert_eq!(outbox.next(), Some((CONTROL, message.clone())));
            outbox.set_references(references, Some(&[session]), false);
            outbox.push_control(message);
            assert!(outbox.lock().control.is_empty());
        }
    }

    #[test]
    fn negotiated_development_protocol_preserves_every_control_message() {
        for message in Message::samples()
            .into_iter()
            .filter(|message| message.channel_kind() == muxy_protocol::ChannelKind::Control)
        {
            let outbox = Outbox::new(muxy_protocol::V1, Arc::default());
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
        let outbox = Outbox::new(muxy_protocol::V1, Arc::default());
        outbox
            .lock()
            .credit
            .extend([(ChannelId(1), true), (ChannelId(2), true)]);
        outbox.push_frame(ChannelId(1), frame(1, 0));
        outbox.push_control(Message::HelloReply {
            versions: vec![muxy_protocol::V1],
            server: muxy_protocol::ServerInfo::current(),
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
    fn fatal_seals_the_control_queue_before_other_producers_can_append() {
        let outbox = Outbox::new(muxy_protocol::V1, Arc::default());
        let fatal = super::super::handshake::fatal("invalid message");
        outbox.close_with(fatal.clone());
        outbox.push_control(Message::VersionUnsupported);
        outbox.close_with(Message::VersionUnsupported);
        assert_eq!(outbox.next(), Some((CONTROL, fatal)));
        assert_eq!(outbox.next(), None);
    }

    #[test]
    fn metadata_coalesces_without_frame_credit_and_is_retired_with_its_channel() {
        let outbox = Outbox::new(muxy_protocol::V1, Arc::default());
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
        );
        outbox.push_metadata(channel, MetadataEvent::Bell);
        assert!(outbox.lock().metadata.is_empty());
        outbox.close();
        assert_eq!(outbox.next(), None);
    }

    #[test]
    fn closing_wakes_writer_and_discards_pending_but_drains_control()
    -> Result<(), Box<dyn std::error::Error>> {
        let outbox = Arc::new(Outbox::new(muxy_protocol::V1, Arc::default()));
        outbox.lock().credit.insert(ChannelId(1), false);
        outbox.push_frame(ChannelId(1), frame(1, 0));
        let (sender, receiver) = mpsc::channel();
        let waiting = Arc::clone(&outbox);
        let writer = thread::spawn(move || {
            let _ = sender.send(waiting.next());
        });
        assert!(receiver.recv_timeout(Duration::from_millis(50)).is_err());
        outbox.push_control(Message::VersionUnsupported);
        outbox.close();
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2))?,
            Some((CONTROL, Message::VersionUnsupported))
        );
        writer.join().map_err(|_| "writer panicked")?;
        assert_eq!(outbox.next(), None);
        outbox.push_frame(ChannelId(1), frame(2, 0));
        outbox.push_control(Message::VersionUnsupported);
        assert_eq!(outbox.next(), None);
        Ok(())
    }
    #[test]
    fn hyperlink_replacements_coalesce_before_frames_even_without_credit() {
        let outbox = Outbox::new(muxy_protocol::V1, Arc::default());
        let channel = ChannelId(1);
        outbox.lock().credit.insert(channel, false);
        for seq in 1..=3 {
            outbox.push_metadata(
                channel,
                MetadataEvent::Links {
                    seq,
                    rows: vec![muxy_protocol::LinkRow {
                        row: 0,
                        spans: vec![muxy_protocol::LinkSpan {
                            start: 0,
                            end: 1,
                            uri: format!("https://example.com/{seq}"),
                        }],
                    }],
                },
            );
            outbox.push_frame(channel, frame(seq, 0));
        }
        outbox.push_metadata(
            channel,
            MetadataEvent::Links {
                seq: 4,
                rows: vec![],
            },
        );
        assert_eq!(
            outbox.next(),
            Some((
                channel,
                Message::Metadata(MetadataEvent::Links {
                    seq: 4,
                    rows: vec![]
                })
            ))
        );
        assert!(outbox.lock().metadata.is_empty());
        assert_eq!(outbox.lock().pending.len(), 1);
    }
}
