use std::collections::{HashMap, HashSet};

use muxy_app_core::PaneId;
use muxy_protocol::{ChannelId, SessionId};

use super::{ClientEvent, CloseCheckId, Update, Work};

const MAX_DEFERRED_EVENTS: usize = 1024;

#[derive(Default, PartialEq)]
enum Disconnect {
    #[default]
    Open,
    Pending,
    Delivered,
}

/// Who waits for an attach that is under way.
enum Waiting {
    Pane { pane: PaneId, created: bool },
    CloseCheck(CloseCheckId),
}

#[derive(Default)]
pub(super) struct Delivery {
    pub(super) pending: usize,
    flushing: bool,
    waiting: HashMap<u64, Waiting>,
    /// Sessions whose terminal arrived while requests were running.
    attached: HashSet<SessionId>,
    /// Terminals that arrived for nobody, to detach.
    strays: Vec<ChannelId>,
    deferred: Vec<ClientEvent>,
    disconnect: Disconnect,
}

impl Delivery {
    /// Notes who waits for the terminal this request attaches, if it attaches one.
    pub(super) fn start(&mut self, work: &Work) {
        match work {
            Work::Attach {
                tag, pane, session, ..
            } => {
                self.waiting.insert(
                    *tag,
                    Waiting::Pane {
                        pane: *pane,
                        created: session.is_none(),
                    },
                );
            }
            Work::CheckClose { check, .. } => {
                self.waiting
                    .insert(check.tag(), Waiting::CloseCheck(*check));
            }
            _ => {}
        }
    }

    pub(super) fn take_strays(&mut self) -> Vec<ChannelId> {
        std::mem::take(&mut self.strays)
    }

    pub(super) fn event(&mut self, event: ClientEvent) -> Result<Vec<Update>, &'static str> {
        if matches!(event, ClientEvent::Disconnected) {
            return Ok(self.disconnected());
        }
        if let ClientEvent::Attached {
            tag,
            session,
            attachment,
        } = event
        {
            return Ok(self.attached(tag, session, *attachment));
        }
        if self.ready(&event) {
            return Ok(vec![Update::Event(event)]);
        }
        if let Some(key) = event.coalesce_key()
            && let Some(deferred) = self
                .deferred
                .iter_mut()
                .find(|deferred| deferred.coalesce_key() == Some(key))
        {
            deferred.merge(event);
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
        match &update {
            Some(Update::AttachFailed { pane: failed, .. }) => {
                self.waiting.retain(
                    |_, waiting| !matches!(waiting, Waiting::Pane { pane, .. } if pane == failed),
                );
            }
            Some(Update::CloseChecked { check: failed, .. }) => {
                self.waiting.retain(
                    |_, waiting| !matches!(waiting, Waiting::CloseCheck(check) if check == failed),
                );
            }
            _ => {}
        }
        let failed = match &update {
            Some(Update::AttachFailed {
                session: Some(session),
                ..
            }) => Some(*session),
            _ => None,
        };
        let mut updates: Vec<_> = update.into_iter().collect();
        self.release(&mut updates, failed);
        updates
    }

    pub(super) fn flush(&mut self) -> Vec<Update> {
        if self.busy() && !self.flushing {
            crate::diagnostics::event(
                "worker.flush",
                format_args!("pending={} deferred={}", self.pending, self.deferred.len()),
            );
        }
        self.flushing = self.busy();
        if self.flushing {
            Vec::new()
        } else {
            vec![Update::Flushed]
        }
    }

    /// Whether requests run or attached terminals are still on their way.
    fn busy(&self) -> bool {
        self.pending > 0 || !self.waiting.is_empty()
    }

    fn attached(
        &mut self,
        tag: u64,
        session: SessionId,
        attachment: muxy_client::Attachment,
    ) -> Vec<Update> {
        let update = match self.waiting.remove(&tag) {
            Some(Waiting::Pane { pane, created }) => Update::Attached {
                pane,
                session,
                attachment,
                created,
            },
            Some(Waiting::CloseCheck(check)) => Update::CloseChecked {
                check,
                session,
                result: Ok(attachment.process),
            },
            None => {
                self.strays.push(attachment.terminal.channel());
                return Vec::new();
            }
        };
        if self.busy() {
            self.attached.insert(session);
        }
        let mut updates = vec![update];
        self.release(&mut updates, None);
        updates
    }

    /// Delivers what waited, once it may go.
    fn release(&mut self, updates: &mut Vec<Update>, failed: Option<SessionId>) {
        for event in std::mem::take(&mut self.deferred) {
            if self.ready(&event)
                || matches!(&event, ClientEvent::SessionEnded { session, .. } if Some(*session) == failed)
            {
                updates.push(Update::Event(event));
            } else {
                self.deferred.push(event);
            }
        }
        if self.busy() {
            return;
        }
        self.attached.clear();
        if self.disconnect == Disconnect::Pending {
            self.disconnect = Disconnect::Delivered;
            updates.push(Update::Event(ClientEvent::Disconnected));
        }
        if self.flushing {
            self.flushing = false;
            updates.push(Update::Flushed);
        }
    }

    fn disconnected(&mut self) -> Vec<Update> {
        if self.disconnect != Disconnect::Open {
            return Vec::new();
        }
        self.waiting.clear();
        self.disconnect = Disconnect::Pending;
        let mut updates = Vec::new();
        self.release(&mut updates, None);
        updates
    }

    fn ready(&self, event: &ClientEvent) -> bool {
        !self.busy()
            || match event {
                ClientEvent::Attached { .. }
                | ClientEvent::Frame { .. }
                | ClientEvent::Metadata { .. }
                | ClientEvent::ClipboardWrite { .. } => true,
                ClientEvent::SessionEnded { session, .. } => self.attached.contains(session),
                ClientEvent::SessionMetadata { .. }
                | ClientEvent::ActivityChanged { .. }
                | ClientEvent::Progress { .. }
                | ClientEvent::FilesChanged { .. }
                | ClientEvent::GitChanged { .. }
                | ClientEvent::SessionsChanged { .. }
                | ClientEvent::CatalogChanged { .. }
                | ClientEvent::RemoteAccessChanged { .. }
                | ClientEvent::ServerRestarting
                | ClientEvent::Disconnected => false,
            }
    }
}
