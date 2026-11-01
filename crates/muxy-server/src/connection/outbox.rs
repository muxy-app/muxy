use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};

use muxy_protocol::{CONTROL, ChannelId, Message, MetadataEvent, ScreenFrame};

use super::merge::merge;

/// What a connection has yet to write. Control messages go first, folded by
/// their coalesce key; then each channel's metadata; then each channel's
/// merged frame, once the client acknowledged the one before it.
#[derive(Debug, Default)]
pub(super) struct Outbox {
    control: VecDeque<Message>,
    metadata: HashMap<ChannelId, Vec<MetadataEvent>>,
    frames: HashMap<ChannelId, ScreenFrame>,
    /// Whether each open channel may send its next frame.
    credit: HashMap<ChannelId, bool>,
    sent: HashMap<ChannelId, u64>,
}

impl Outbox {
    pub(super) fn push_control(&mut self, message: Message) {
        if let Some(key) = message.coalesce_key()
            && let Some(queued) = self
                .control
                .iter_mut()
                .find(|queued| queued.coalesce_key() == Some(key))
        {
            queued.merge(message);
            return;
        }
        self.control.push_back(message);
    }

    pub(super) fn retain_control(&mut self, keep: impl FnMut(&Message) -> bool) {
        self.control.retain(keep);
    }

    pub(super) fn open(&mut self, channel: ChannelId) {
        self.credit.insert(channel, true);
    }

    pub(super) fn push_frame(&mut self, channel: ChannelId, frame: ScreenFrame) -> bool {
        if !self.credit.contains_key(&channel) {
            return false;
        }
        match self.frames.entry(channel) {
            Entry::Occupied(mut entry) => merge(entry.get_mut(), frame),
            Entry::Vacant(entry) => {
                entry.insert(frame);
            }
        }
        true
    }

    /// Replaces whatever frame was waiting, as a resize's frame redraws everything.
    pub(super) fn replace_frame(&mut self, channel: ChannelId, frame: ScreenFrame) {
        self.frames.insert(channel, frame);
    }

    /// A newer event of the same kind replaces the waiting one.
    pub(super) fn push_metadata(&mut self, channel: ChannelId, event: MetadataEvent) -> bool {
        if !self.credit.contains_key(&channel) {
            return false;
        }
        let pending = self.metadata.entry(channel).or_default();
        if let Some(previous) = pending
            .iter_mut()
            .find(|previous| std::mem::discriminant(*previous) == std::mem::discriminant(&event))
        {
            *previous = event;
        } else {
            pending.push(event);
        }
        true
    }

    /// Whether the acknowledgement gave the channel its credit back.
    pub(super) fn ack(&mut self, channel: ChannelId, seq: u64) -> bool {
        if self.sent.get(&channel) == Some(&seq)
            && let Some(credit) = self.credit.get_mut(&channel)
        {
            *credit = true;
            return true;
        }
        false
    }

    pub(super) fn next_control(&mut self) -> Option<(ChannelId, Message)> {
        self.control.pop_front().map(|message| (CONTROL, message))
    }

    pub(super) fn next_channel(&mut self) -> Option<(ChannelId, Message)> {
        if let Some(channel) = self.metadata.keys().next().copied()
            && let Some(pending) = self.metadata.get_mut(&channel)
        {
            let event = pending.remove(0);
            if pending.is_empty() {
                self.metadata.remove(&channel);
            }
            return Some((channel, Message::Metadata(event)));
        }
        let channel = self
            .frames
            .keys()
            .find(|channel| self.credit.get(channel) == Some(&true))
            .copied()?;
        let frame = self.frames.remove(&channel)?;
        self.credit.insert(channel, false);
        self.sent.insert(channel, frame.seq);
        Some((channel, Message::Frame(frame)))
    }

    pub(super) fn close_channel(&mut self, channel: ChannelId) {
        self.frames.remove(&channel);
        self.metadata.remove(&channel);
        self.credit.remove(&channel);
        self.sent.remove(&channel);
    }

    /// Drops every channel's output; control messages still go out.
    pub(super) fn close_channels(&mut self) {
        self.frames.clear();
        self.metadata.clear();
        self.credit.clear();
        self.sent.clear();
    }

    #[cfg(test)]
    pub(super) fn take_control(&mut self) -> Vec<Message> {
        self.control.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use muxy_protocol::{
        Cursor, FileChanges, Modes, ProgressState, ProjectId, Row, ServerPath, SessionId,
        SessionProgress, Size, TerminalProgress, Topic,
    };

    use super::*;

    fn session(id: u64) -> SessionId {
        SessionId::new(id).unwrap()
    }

    fn progress(session: SessionId, percent: u8) -> Message {
        Message::Progress {
            session,
            progress: SessionProgress {
                progress: Some(TerminalProgress {
                    state: ProgressState::Running,
                    percent: Some(percent),
                }),
                completed: 0,
            },
        }
    }

    fn drain(outbox: &mut Outbox) -> Vec<Message> {
        std::iter::from_fn(|| outbox.next_control())
            .map(|(_, message)| message)
            .collect()
    }

    #[test]
    fn a_newer_message_takes_the_place_of_the_one_with_its_key() {
        let mut outbox = Outbox::default();
        outbox.push_control(progress(session(1), 10));
        outbox.push_control(progress(session(2), 10));
        outbox.push_control(Message::ServerRestarting);
        outbox.push_control(progress(session(1), 20));
        outbox.push_control(Message::ServerRestarting);
        assert_eq!(
            drain(&mut outbox),
            [
                progress(session(1), 20),
                progress(session(2), 10),
                Message::ServerRestarting,
                Message::ServerRestarting,
            ]
        );
    }

    #[test]
    fn changes_keep_the_highest_revision_and_combine_file_changes() {
        let mut outbox = Outbox::default();
        let project = ProjectId::new();
        let files = |paths: &[&str]| Message::FilesChanged {
            project,
            changes: FileChanges {
                paths: paths.iter().copied().map(ServerPath::from).collect(),
                rescan: false,
            },
        };
        let changed = |topic, revision| Message::Changed { topic, revision };
        for message in [
            changed(Topic::Catalog, 3),
            files(&["a"]),
            Message::GitChanged { project },
            changed(Topic::Activity, 1),
            changed(Topic::Catalog, 2),
            files(&["b", "a"]),
            Message::GitChanged { project },
        ] {
            outbox.push_control(message);
        }
        assert_eq!(
            drain(&mut outbox),
            [
                changed(Topic::Catalog, 3),
                files(&["a", "b"]),
                Message::GitChanged { project },
                changed(Topic::Activity, 1),
            ]
        );
    }

    #[test]
    fn file_changes_turn_into_a_rescan_for_slow_consumers() {
        let mut outbox = Outbox::default();
        let project = ProjectId::new();
        for index in 0..1000 {
            outbox.push_control(Message::FilesChanged {
                project,
                changes: FileChanges {
                    paths: vec![ServerPath::from(index.to_string())],
                    rescan: false,
                },
            });
        }
        assert_eq!(
            drain(&mut outbox),
            [Message::FilesChanged {
                project,
                changes: FileChanges {
                    paths: Vec::new(),
                    rescan: true
                }
            }]
        );
    }

    #[test]
    fn every_unkeyed_control_message_is_kept() {
        let mut outbox = Outbox::default();
        let messages: Vec<_> = Message::samples()
            .into_iter()
            .filter(|message| {
                message.channel_kind() == muxy_protocol::ChannelKind::Control
                    && message.coalesce_key().is_none()
            })
            .collect();
        for message in messages.iter().chain(&messages) {
            outbox.push_control(message.clone());
        }
        assert_eq!(drain(&mut outbox), [messages.clone(), messages].concat());
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
    fn each_channel_waits_for_its_own_credit_and_merges_what_waits() {
        let mut outbox = Outbox::default();
        outbox.open(ChannelId(1));
        outbox.open(ChannelId(2));
        outbox.push_frame(ChannelId(1), frame(1, 0));
        assert_eq!(
            outbox.next_channel(),
            Some((ChannelId(1), Message::Frame(frame(1, 0))))
        );
        outbox.push_frame(ChannelId(1), frame(2, 1));
        outbox.push_frame(ChannelId(1), frame(4, 2));
        outbox.push_frame(ChannelId(2), frame(1, 0));
        assert_eq!(
            outbox.next_channel(),
            Some((ChannelId(2), Message::Frame(frame(1, 0))))
        );
        assert_eq!(outbox.next_channel(), None);
        assert!(!outbox.ack(ChannelId(1), 0));
        assert!(!outbox.ack(ChannelId(1), 5));
        assert!(outbox.ack(ChannelId(1), 1));
        let mut merged = frame(4, 1);
        merged.rows.push(Row {
            index: 2,
            runs: vec![],
        });
        assert_eq!(
            outbox.next_channel(),
            Some((ChannelId(1), Message::Frame(merged)))
        );
        assert!(!outbox.ack(ChannelId(1), 1));
    }

    #[test]
    fn metadata_coalesces_by_kind_without_credit_and_closes_with_its_channel() {
        let mut outbox = Outbox::default();
        let channel = ChannelId(1);
        assert!(!outbox.push_metadata(channel, MetadataEvent::Bell));
        outbox.open(channel);
        outbox.push_frame(channel, frame(1, 0));
        outbox.next_channel();
        for index in 0..1000 {
            outbox.push_metadata(channel, MetadataEvent::Title(index.to_string()));
            outbox.push_metadata(channel, MetadataEvent::Bell);
            outbox.push_metadata(channel, MetadataEvent::ClipboardWrite(index.to_string()));
        }
        for event in [
            MetadataEvent::Title("999".into()),
            MetadataEvent::Bell,
            MetadataEvent::ClipboardWrite("999".into()),
        ] {
            assert_eq!(
                outbox.next_channel(),
                Some((channel, Message::Metadata(event)))
            );
        }
        outbox.push_metadata(channel, MetadataEvent::Bell);
        outbox.close_channel(channel);
        assert!(!outbox.push_metadata(channel, MetadataEvent::Bell));
        assert_eq!(outbox.next_channel(), None);
    }
}
