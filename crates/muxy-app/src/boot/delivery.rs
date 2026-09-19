use super::{ClientEvent, Update};

const MAX_DEFERRED_EVENTS: usize = 1024;

#[derive(Default, PartialEq)]
enum Disconnect {
    #[default]
    Open,
    Pending,
    Delivered,
}

#[derive(Default)]
pub(super) struct Delivery {
    pub(super) pending: usize,
    flushing: bool,
    installed_through: u32,
    deferred: Vec<ClientEvent>,
    disconnect: Disconnect,
}

impl Delivery {
    pub(super) fn event(&mut self, event: ClientEvent) -> Result<Vec<Update>, &'static str> {
        if matches!(event, ClientEvent::Disconnected) {
            return Ok(self.disconnected());
        }
        if self.ready(&event) {
            return Ok(vec![Update::Event(event)]);
        }
        if let ClientEvent::Metadata { channel, event: metadata } = &event
            && let Some(previous) = self.deferred.iter_mut().find(|previous| {
                matches!(previous, ClientEvent::Metadata { channel: old_channel, event: old }
                    if old_channel == channel && std::mem::discriminant(old) == std::mem::discriminant(metadata))
            })
        {
            *previous = event;
            return Ok(Vec::new());
        }
        if let ClientEvent::ActivityChanged { revision } = &event
            && let Some(ClientEvent::ActivityChanged { revision: pending }) = self
                .deferred
                .iter_mut()
                .find(|event| matches!(event, ClientEvent::ActivityChanged { .. }))
        {
            *pending = (*pending).max(*revision);
            return Ok(Vec::new());
        }
        if let ClientEvent::SessionMetadata { session, .. } = &event
            && let Some(previous) = self.deferred.iter_mut().find(|previous| matches!(previous, ClientEvent::SessionMetadata { session: id, .. } if id == session)) {
            *previous = event;
            return Ok(Vec::new());
        }
        if let ClientEvent::Progress { session, .. } = &event
            && let Some(previous) = self.deferred.iter_mut().find(|previous| matches!(previous, ClientEvent::Progress { session: id, .. } if id == session)) {
            *previous = event;
            return Ok(Vec::new());
        }
        if let ClientEvent::GitChanged { project } = &event
            && self.deferred.iter().any(|pending| matches!(pending, ClientEvent::GitChanged { project: id } if id == project)) {
            return Ok(Vec::new());
        }
        if let ClientEvent::SessionsChanged { revision } = &event
            && let Some(ClientEvent::SessionsChanged { revision: previous }) = self
                .deferred
                .iter_mut()
                .find(|previous| matches!(previous, ClientEvent::SessionsChanged { .. }))
        {
            *previous = (*previous).max(*revision);
            return Ok(Vec::new());
        }
        if let ClientEvent::CatalogChanged { revision } = &event
            && let Some(ClientEvent::CatalogChanged { revision: previous }) = self
                .deferred
                .iter_mut()
                .find(|previous| matches!(previous, ClientEvent::CatalogChanged { .. }))
        {
            *previous = (*previous).max(*revision);
            return Ok(Vec::new());
        }
        if self.deferred.len() == MAX_DEFERRED_EVENTS {
            return Err("too many events arrived before request completion");
        }
        let updates = match &event {
            ClientEvent::SessionEnded { session, .. } => vec![Update::CloseSessionPanes(*session)],
            _ => Vec::new(),
        };
        self.deferred.push(event);
        Ok(updates)
    }

    pub(super) fn complete(&mut self, update: Option<Update>) -> Vec<Update> {
        crate::diagnostics::event(
            "worker.complete",
            format_args!(
                "pending={} deferred={} flushing={}",
                self.pending,
                self.deferred.len(),
                self.flushing
            ),
        );
        self.pending = self.pending.saturating_sub(1);
        if let Some(Update::Attached { attachment, .. }) = &update {
            self.installed_through = self.installed_through.max(attachment.channel.0);
        }
        let attached = match &update {
            Some(
                Update::Attached { session, .. }
                | Update::AttachFailed {
                    session: Some(session),
                    ..
                },
            ) => Some(*session),
            _ => None,
        };
        let mut updates: Vec<_> = update.into_iter().collect();
        for event in std::mem::take(&mut self.deferred) {
            if self.ready(&event)
                || matches!(&event, ClientEvent::SessionEnded { session, .. } if Some(*session) == attached)
            {
                updates.push(Update::Event(event));
            } else {
                self.deferred.push(event);
            }
        }
        if self.pending == 0 && self.disconnect == Disconnect::Pending {
            self.disconnect = Disconnect::Delivered;
            updates.push(Update::Event(ClientEvent::Disconnected));
        }
        if self.pending == 0 && self.flushing {
            self.flushing = false;
            updates.push(Update::Flushed);
        }
        updates
    }

    pub(super) fn flush(&mut self) -> Vec<Update> {
        if self.pending > 0 && !self.flushing {
            crate::diagnostics::event(
                "worker.flush",
                format_args!("pending={} deferred={}", self.pending, self.deferred.len()),
            );
        }
        self.flushing = self.pending > 0;
        if self.flushing {
            Vec::new()
        } else {
            vec![Update::Flushed]
        }
    }

    fn disconnected(&mut self) -> Vec<Update> {
        if self.disconnect != Disconnect::Open {
            return Vec::new();
        }
        if self.pending > 0 {
            self.disconnect = Disconnect::Pending;
            Vec::new()
        } else {
            self.disconnect = Disconnect::Delivered;
            vec![Update::Event(ClientEvent::Disconnected)]
        }
    }

    fn ready(&self, event: &ClientEvent) -> bool {
        self.pending == 0
            || match event {
                ClientEvent::Frame { channel, .. } | ClientEvent::Metadata { channel, .. } => {
                    channel.0 <= self.installed_through
                }
                ClientEvent::SessionMetadata { .. }
                | ClientEvent::ActivityChanged { .. }
                | ClientEvent::Progress { .. }
                | ClientEvent::FilesChanged { .. }
                | ClientEvent::GitChanged { .. }
                | ClientEvent::SessionsChanged { .. }
                | ClientEvent::CatalogChanged { .. }
                | ClientEvent::SessionEnded { .. }
                | ClientEvent::ServerRestarting
                | ClientEvent::Disconnected => false,
            }
    }
}
