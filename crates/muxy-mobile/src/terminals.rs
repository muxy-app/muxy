//! Attached terminals by channel, and what arrives for a channel before its attach returns.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use muxy_client::{Attachment, RunGrid};
use muxy_protocol::{ChannelId, InputModes, MetadataEvent, ScreenFrame, ServerPath, SessionId};

/// Channels still attaching; the server allows one unacknowledged frame each.
const MAX_EARLY: usize = 16;

#[derive(Default)]
pub(crate) struct Terminals {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    views: HashMap<ChannelId, Arc<View>>,
    early: HashMap<ChannelId, Early>,
    /// Channels up to this one were attached; later ones may still be attaching.
    installed_through: u32,
}

#[derive(Default)]
struct Early {
    frame: Option<ScreenFrame>,
    metadata: Vec<MetadataEvent>,
}

pub(crate) struct View {
    pub(crate) session: SessionId,
    pub(crate) channel: ChannelId,
    content: Mutex<Content>,
}

pub(crate) struct Content {
    pub(crate) grid: RunGrid,
    pub(crate) title: String,
    pub(crate) directory: ServerPath,
    pub(crate) input: InputModes,
}

pub(crate) enum Delivery {
    /// Applied to an attached terminal; acknowledge `seq` to receive the next frame.
    Applied { session: SessionId, seq: u64 },
    /// Kept until the terminal's attach returns.
    Held,
    /// For a terminal that has been detached.
    Unknown,
}

impl Terminals {
    /// Installs an attachment and applies what arrived first; returns the frame to acknowledge.
    pub(crate) fn install(
        &self,
        session: SessionId,
        attachment: Attachment,
    ) -> (Arc<View>, Option<u64>) {
        let mut state = self.lock();
        let early = state.early.remove(&attachment.channel).unwrap_or_default();
        let mut content = Content {
            grid: attachment.grid,
            title: attachment.title,
            directory: attachment.directory,
            input: InputModes::default(),
        };
        content.grid.graphics = muxy_protocol::Graphics::default();
        let acknowledge = early.frame.map(|frame| {
            content.frame(&frame);
            frame.seq
        });
        for event in early.metadata {
            content.apply(event);
        }
        let view = Arc::new(View {
            session,
            channel: attachment.channel,
            content: Mutex::new(content),
        });
        state.views.insert(attachment.channel, Arc::clone(&view));
        state.installed_through = state.installed_through.max(attachment.channel.0);
        (view, acknowledge)
    }

    pub(crate) fn frame(&self, channel: ChannelId, mut frame: ScreenFrame) -> Delivery {
        frame.graphics = None;
        let mut state = self.lock();
        if let Some(view) = state.views.get(&channel) {
            view.lock().frame(&frame);
            return Delivery::Applied {
                session: view.session,
                seq: frame.seq,
            };
        }
        match state.early_for(channel) {
            Some(early) => {
                early.frame = Some(frame);
                Delivery::Held
            }
            None => Delivery::Unknown,
        }
    }

    /// Returns the session whose `Screen` metadata changed.
    pub(crate) fn metadata(&self, channel: ChannelId, event: MetadataEvent) -> Option<SessionId> {
        if !matches!(
            event,
            MetadataEvent::Title(_)
                | MetadataEvent::Directory(_)
                | MetadataEvent::History { .. }
                | MetadataEvent::InputModes(_)
        ) {
            return None;
        }
        let mut state = self.lock();
        if let Some(view) = state.views.get(&channel) {
            return view.lock().apply(event).then_some(view.session);
        }
        if let Some(early) = state.early_for(channel) {
            early
                .metadata
                .retain(|old| std::mem::discriminant(old) != std::mem::discriminant(&event));
            early.metadata.push(event);
        }
        None
    }

    pub(crate) fn remove(&self, channel: ChannelId) {
        self.lock().views.remove(&channel);
    }

    pub(crate) fn ended(&self, session: SessionId) {
        self.lock().views.retain(|_, view| view.session != session);
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    fn early_for(&mut self, channel: ChannelId) -> Option<&mut Early> {
        let attaching = channel.0 > self.installed_through
            && (self.early.len() < MAX_EARLY || self.early.contains_key(&channel));
        attaching.then(|| self.early.entry(channel).or_default())
    }
}

impl View {
    pub(crate) fn lock(&self) -> MutexGuard<'_, Content> {
        self.content.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Content {
    fn frame(&mut self, frame: &ScreenFrame) {
        if self.grid.size != frame.size {
            self.grid.resize(frame.size);
        }
        self.grid.apply(frame);
    }

    /// Returns whether the change shows in `Screen`.
    fn apply(&mut self, event: MetadataEvent) -> bool {
        match event {
            MetadataEvent::Title(title) => self.title = title,
            MetadataEvent::Directory(directory) => self.directory = directory,
            MetadataEvent::History { total_rows } => self.grid.history_total = total_rows,
            MetadataEvent::InputModes(input) => {
                // Focus reporting alone doesn't show.
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
    use muxy_protocol::{AttachSnapshot, Cursor, CursorShape, Graphics, Modes, Size};

    use super::*;

    fn attachment(channel: ChannelId) -> Attachment {
        let snapshot = AttachSnapshot {
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
        };
        Attachment {
            channel,
            grid: RunGrid::from_snapshot(&snapshot),
            title: snapshot.title,
            directory: snapshot.directory,
            process: None,
        }
    }

    fn resized(size: Size) -> ScreenFrame {
        ScreenFrame {
            size,
            graphics: None,
            seq: 1,
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
    fn resize_frames_before_attach_returns_replace_the_snapshot_size() {
        for size in [Size { cols: 20, rows: 4 }, Size { cols: 4, rows: 1 }] {
            let terminals = Terminals::default();
            let channel = ChannelId(1);
            let session = SessionId::new(7).unwrap();
            assert!(matches!(
                terminals.frame(channel, resized(size)),
                Delivery::Held
            ));
            let (view, acknowledge) = terminals.install(session, attachment(channel));
            assert_eq!(acknowledge, Some(1));
            assert_eq!(view.lock().grid.size, size);
            assert_eq!(view.lock().grid.rows.len(), usize::from(size.rows));
        }
    }

    #[test]
    fn blank_resets_track_both_dimensions_and_deltas_keep_other_rows() {
        let terminals = Terminals::default();
        let channel = ChannelId(1);
        let session = SessionId::new(7).unwrap();
        let (view, _) = terminals.install(session, attachment(channel));
        for size in [Size { cols: 20, rows: 4 }, Size { cols: 4, rows: 1 }] {
            assert!(matches!(
                terminals.frame(channel, resized(size)),
                Delivery::Applied { .. }
            ));
            assert_eq!(view.lock().grid.size, size);
            assert_eq!(view.lock().grid.rows.len(), usize::from(size.rows));
        }
        let mut delta = resized(Size { cols: 4, rows: 1 });
        delta.reset = false;
        delta.rows[0].runs.push(muxy_protocol::Run {
            text: "text".into(),
            width: 4,
            style: muxy_protocol::Style::default(),
        });
        terminals.frame(channel, delta.clone());
        delta.rows.clear();
        terminals.frame(channel, delta);
        assert_eq!(view.lock().grid.row_text(0), "text");
    }

    fn modes(mouse_tracking: bool, focus_events: bool) -> MetadataEvent {
        MetadataEvent::InputModes(InputModes {
            mouse_tracking,
            alternate_scroll: false,
            focus_events,
        })
    }

    #[test]
    fn mouse_modes_sent_before_attach_returns_are_kept() {
        let terminals = Terminals::default();
        let channel = ChannelId(1);
        let session = SessionId::new(7).unwrap();
        assert_eq!(terminals.metadata(channel, modes(true, false)), None);
        let (view, _) = terminals.install(session, attachment(channel));
        assert!(view.lock().input.mouse_tracking);
    }

    #[test]
    fn only_mode_changes_the_screen_shows_are_reported() {
        let terminals = Terminals::default();
        let channel = ChannelId(1);
        let session = SessionId::new(7).unwrap();
        terminals.install(session, attachment(channel));
        assert_eq!(terminals.metadata(channel, modes(false, true)), None);
        assert_eq!(
            terminals.metadata(channel, modes(true, true)),
            Some(session)
        );
        assert_eq!(terminals.metadata(channel, modes(true, true)), None);
    }
}
