//! Attached terminals by channel, and what arrives for a channel before its attach returns.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use muxy_client::{Attachment, RunGrid};
use muxy_protocol::{ChannelId, MetadataEvent, ScreenFrame, ServerPath, SessionId};

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
        };
        content.grid.graphics = muxy_protocol::Graphics::default();
        let acknowledge = early.frame.map(|frame| {
            content.grid.apply(&frame);
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
            view.lock().grid.apply(&frame);
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

    /// Returns the session whose title or directory changed.
    pub(crate) fn metadata(&self, channel: ChannelId, event: MetadataEvent) -> Option<SessionId> {
        if !matches!(
            event,
            MetadataEvent::Title(_) | MetadataEvent::Directory(_) | MetadataEvent::History { .. }
        ) {
            return None;
        }
        let mut state = self.lock();
        if let Some(view) = state.views.get(&channel) {
            view.lock().apply(event);
            return Some(view.session);
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
    fn apply(&mut self, event: MetadataEvent) {
        match event {
            MetadataEvent::Title(title) => self.title = title,
            MetadataEvent::Directory(directory) => self.directory = directory,
            MetadataEvent::History { total_rows } => self.grid.history_total = total_rows,
            _ => {}
        }
    }
}
