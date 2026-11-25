use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use muxy_protocol::{ExitReason, Message, SessionId, Topic};

use crate::connection::Connection;

#[derive(Debug, Default)]
pub(crate) struct Notifier {
    connections: Mutex<Vec<Weak<Connection>>>,
    catalog: AtomicU64,
    positions: AtomicU64,
}

impl Notifier {
    pub(crate) fn register(&self, connection: &Arc<Connection>) {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        connections.retain(|connection| connection.strong_count() > 0);
        connections.push(Arc::downgrade(connection));
    }

    pub(crate) fn connections(&self) -> Vec<Arc<Connection>> {
        self.connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter_map(Weak::upgrade)
            .collect()
    }

    pub(crate) fn notify(&self, topic: Topic, revision: u64) {
        for connection in self.connections() {
            connection.changed(topic, revision);
        }
    }

    pub(crate) fn catalog_changed(&self, revision: u64) {
        self.catalog.fetch_max(revision, Ordering::AcqRel);
        self.notify(Topic::Catalog, revision);
        self.sessions_changed();
    }

    pub(crate) fn sessions_changed(&self) {
        self.notify(Topic::Sessions, self.sessions_revision());
    }

    pub(crate) fn sessions_revision(&self) -> u64 {
        self.catalog
            .load(Ordering::Acquire)
            .saturating_add(self.positions.load(Ordering::Acquire))
    }

    pub(crate) fn next_position(&self) -> u64 {
        self.positions.fetch_add(1, Ordering::AcqRel)
    }

    pub(crate) fn session_ended(&self, session: SessionId, reason: ExitReason) {
        for connection in self.connections() {
            connection.session_ended(session, reason);
        }
    }

    pub(crate) fn broadcast(&self, message: &Message) {
        for connection in self.connections() {
            connection.push_control(message.clone());
        }
    }

    pub(crate) fn close_all(&self) {
        for connection in self.connections() {
            connection.close();
        }
    }
}
