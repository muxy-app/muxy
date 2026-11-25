use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use muxy_client::{AttachedTerminal, Attachment};
use muxy_protocol::{
    ChannelId, InputModes, MetadataEvent, ScreenFrame, ServerPath, SessionId, Size,
};

#[derive(Default)]
pub(crate) struct Terminals {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    views: HashMap<ChannelId, Arc<View>>,
    waiting: HashMap<u64, SyncSender<Arc<View>>>,
    next_tag: u64,
    last_channel: u32,
}

pub(crate) struct View {
    pub(crate) session: SessionId,
    pub(crate) channel: ChannelId,
    content: Mutex<Content>,
}

pub(crate) struct Content {
    pub(crate) terminal: AttachedTerminal,
    pub(crate) title: String,
    pub(crate) directory: ServerPath,
    pub(crate) input: InputModes,
    pub(crate) claimed: Option<Size>,
}

pub(crate) enum Delivery {
    Applied { session: SessionId, seq: u64 },
    Closed { seq: u64 },
    Unknown,
}

impl Terminals {
    pub(crate) fn start_attach(&self) -> (u64, Receiver<Arc<View>>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut state = self.lock();
        let tag = state.next_tag;
        state.next_tag = state.next_tag.wrapping_add(1);
        state.waiting.insert(tag, sender);
        (tag, receiver)
    }

    pub(crate) fn cancel_attach(&self, tag: u64) {
        self.lock().waiting.remove(&tag);
    }

    pub(crate) fn attached(
        &self,
        tag: u64,
        session: SessionId,
        attachment: Attachment,
    ) -> Option<ChannelId> {
        let channel = attachment.terminal.channel();
        let mut state = self.lock();
        state.last_channel = state.last_channel.max(channel.0);
        let Some(waiting) = state.waiting.remove(&tag) else {
            return Some(channel);
        };
        let mut terminal = attachment.terminal;
        terminal.grid.graphics = muxy_protocol::Graphics::default();
        let view = Arc::new(View {
            session,
            channel,
            content: Mutex::new(Content {
                terminal,
                title: attachment.title,
                directory: attachment.directory,
                input: InputModes::default(),
                claimed: None,
            }),
        });
        state.views.insert(channel, Arc::clone(&view));
        drop(state);
        if waiting.send(view).is_ok() {
            return None;
        }
        self.remove(channel);
        Some(channel)
    }

    pub(crate) fn frame(&self, channel: ChannelId, mut frame: ScreenFrame) -> Delivery {
        frame.graphics = None;
        let state = self.lock();
        if let Some(view) = state.views.get(&channel) {
            return Delivery::Applied {
                session: view.session,
                seq: view.lock().frame(&frame),
            };
        }
        if channel.0 <= state.last_channel {
            Delivery::Closed { seq: frame.seq }
        } else {
            Delivery::Unknown
        }
    }

    pub(crate) fn metadata(&self, channel: ChannelId, event: MetadataEvent) -> Option<SessionId> {
        let view = self.lock().views.get(&channel).cloned()?;
        view.lock().apply(event).then_some(view.session)
    }

    pub(crate) fn session(&self, channel: ChannelId) -> Option<SessionId> {
        self.lock().views.get(&channel).map(|view| view.session)
    }

    pub(crate) fn remove(&self, channel: ChannelId) {
        self.lock().views.remove(&channel);
    }

    pub(crate) fn ended(&self, session: SessionId) {
        self.lock().views.retain(|_, view| view.session != session);
    }

    pub(crate) fn disconnected(&self) {
        self.lock().waiting.clear();
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl View {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Content> {
        self.content.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Content {
    pub(crate) fn claim(&mut self, size: Size) {
        self.claimed = Some(size);
    }

    pub(crate) fn claim_if_shown(&mut self, size: Size) {
        if self.terminal.grid.size == size {
            self.claim(size);
        }
    }

    pub(crate) fn reclaim(&self) -> Option<(Size, u64)> {
        self.claimed
            .filter(|size| *size != self.terminal.grid.size)
            .map(|size| (size, self.terminal.seq()))
    }

    pub(crate) fn reclaimed(&mut self, size: Size, seq: u64) {
        if self.terminal.seq() == seq {
            self.terminal.grid.resize(size);
        }
    }

    fn frame(&mut self, frame: &ScreenFrame) -> u64 {
        if self.terminal.grid.size != frame.size {
            self.terminal.grid.resize(frame.size);
        }
        self.terminal.apply(frame)
    }

    fn apply(&mut self, event: MetadataEvent) -> bool {
        match event {
            MetadataEvent::Title(title) => self.title = title,
            MetadataEvent::Directory(directory) => self.directory = directory,
            MetadataEvent::History { total_rows } => self.terminal.grid.history_total = total_rows,
            MetadataEvent::InputModes(input) => {
                let shown = |modes: InputModes| (modes.mouse_tracking, modes.alternate_scroll);
                let changed = shown(input) != shown(self.input);
                self.input = input;
                return changed;
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use muxy_client::RunGrid;
    use muxy_protocol::{AttachSnapshot, Cursor, CursorShape, Graphics, Modes, Size};

    use super::*;

    fn attachment(channel: ChannelId) -> Attachment {
        let snapshot = AttachSnapshot {
            links: None,
            graphics: Graphics::default(),
            prompts: Vec::new(),
            channel,
            size: Size { cols: 10, rows: 2 },
            rows: Vec::new(),
            cursor: Cursor {
                shape: CursorShape::Block,
                row: 0,
                col: 0,
                visible: true,
            },
            modes: Modes::default(),
            title: String::new(),
            directory: ServerPath(Vec::new()),
            history: Vec::new(),
            history_cursor: None,
            history_total: 0,
            history_graphics: None,
        };
        Attachment {
            terminal: AttachedTerminal::new(channel, RunGrid::from_snapshot(&snapshot)),
            server_input: false,
            title: snapshot.title,
            directory: snapshot.directory,
            process: None,
        }
    }

    fn resized(size: Size) -> ScreenFrame {
        resized_at(size, 1)
    }

    fn resized_at(size: Size, seq: u64) -> ScreenFrame {
        ScreenFrame {
            size,
            graphics: None,
            seq,
            reset: true,
            rows: (0..size.rows)
                .map(|index| muxy_protocol::Row {
                    index,
                    runs: Vec::new(),
                })
                .collect(),
            cursor: Cursor {
                shape: CursorShape::Block,
                row: 0,
                col: 0,
                visible: true,
            },
            modes: Modes::default(),
        }
    }

    #[test]
    fn resize_frames_replace_the_snapshot_size() {
        for size in [Size { cols: 20, rows: 4 }, Size { cols: 4, rows: 1 }] {
            let terminals = Terminals::default();
            let channel = ChannelId(1);
            let session = SessionId::new(7).unwrap();
            let (tag, attached) = terminals.start_attach();
            assert_eq!(terminals.attached(tag, session, attachment(channel)), None);
            let view = attached.try_recv().expect("view");
            assert!(matches!(
                terminals.frame(channel, resized(size)),
                Delivery::Applied { seq: 1, .. }
            ));
            assert_eq!(view.lock().terminal.grid.size, size);
            assert_eq!(view.lock().terminal.grid.rows.len(), usize::from(size.rows));
        }
    }

    #[test]
    fn the_phone_takes_back_only_a_size_it_set() {
        let terminals = Terminals::default();
        let channel = ChannelId(1);
        let session = SessionId::new(7).unwrap();
        let (tag, attached) = terminals.start_attach();
        terminals.attached(tag, session, attachment(channel));
        let view = attached.try_recv().expect("view");
        let phone = Size { cols: 4, rows: 1 };
        let desktop = Size { cols: 20, rows: 4 };

        terminals.frame(channel, resized(desktop));
        assert_eq!(view.lock().reclaim(), None);

        view.lock().claim_if_shown(phone);
        assert_eq!(view.lock().reclaim(), None);
        terminals.frame(channel, resized(phone));
        view.lock().claim_if_shown(phone);
        assert_eq!(view.lock().reclaim(), None);

        terminals.frame(channel, resized_at(desktop, 2));
        assert_eq!(view.lock().reclaim(), Some((phone, 2)));
        assert_eq!(view.lock().terminal.grid.size, desktop);
        assert_eq!(view.lock().reclaim(), Some((phone, 2)));
        view.lock().reclaimed(phone, 2);
        assert_eq!(view.lock().terminal.grid.size, phone);
        assert_eq!(view.lock().reclaim(), None);

        for after in [phone, desktop] {
            terminals.frame(channel, resized_at(desktop, 3));
            let (size, seq) = view.lock().reclaim().unwrap();
            terminals.frame(channel, resized_at(after, 4));
            view.lock().reclaimed(size, seq);
            assert_eq!(view.lock().terminal.grid.size, after);
        }
    }

    #[test]
    fn frames_after_detach_are_acknowledged_and_unknown_ones_are_not() {
        let terminals = Terminals::default();
        let session = SessionId::new(7).unwrap();
        let (tag, _attached) = terminals.start_attach();
        terminals.attached(tag, session, attachment(ChannelId(2)));
        terminals.remove(ChannelId(2));
        assert!(matches!(
            terminals.frame(ChannelId(1), resized(Size { cols: 4, rows: 1 })),
            Delivery::Closed { seq: 1 }
        ));
        assert!(matches!(
            terminals.frame(ChannelId(3), resized(Size { cols: 4, rows: 1 })),
            Delivery::Unknown
        ));
    }

    #[test]
    fn a_terminal_nobody_waits_for_is_not_kept() {
        let terminals = Terminals::default();
        let session = SessionId::new(7).unwrap();
        let (tag, attached) = terminals.start_attach();
        terminals.cancel_attach(tag);
        assert_eq!(
            terminals.attached(tag, session, attachment(ChannelId(1))),
            Some(ChannelId(1))
        );
        drop(attached);
        let (tag, attached) = terminals.start_attach();
        drop(attached);
        assert_eq!(
            terminals.attached(tag, session, attachment(ChannelId(2))),
            Some(ChannelId(2))
        );
        assert!(matches!(
            terminals.frame(ChannelId(2), resized(Size { cols: 4, rows: 1 })),
            Delivery::Closed { .. }
        ));
    }
}
