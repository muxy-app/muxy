//! The sessions each app keeps open, shared by every connection it has.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use muxy_protocol::{OperationId, SessionId};

#[derive(Clone, Debug, Default)]
pub(crate) struct References {
    revision: u64,
    sessions: HashSet<SessionId>,
    /// Sessions the app stopped referencing and hasn't attached since.
    released: HashSet<SessionId>,
}

impl References {
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn sessions(&self) -> &HashSet<SessionId> {
        &self.sessions
    }

    pub(crate) fn contains(&self, session: SessionId) -> bool {
        self.sessions.contains(&session)
    }

    pub(crate) fn is_released(&self, session: SessionId) -> bool {
        self.released.contains(&session)
    }

    pub(crate) fn update(&mut self, revision: u64, sessions: &[SessionId]) {
        let sessions: HashSet<_> = sessions.iter().copied().collect();
        self.released
            .extend(self.sessions.difference(&sessions).copied());
        self.released.retain(|session| !sessions.contains(session));
        self.revision = revision;
        self.sessions = sessions;
    }

    pub(crate) fn attached(&mut self, session: SessionId) {
        self.released.remove(&session);
    }

    pub(crate) fn ended(&mut self, session: SessionId) {
        self.sessions.remove(&session);
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SharedReferences(Arc<Mutex<References>>);

impl SharedReferences {
    pub(crate) fn new(references: References) -> Self {
        Self(Arc::new(Mutex::new(references)))
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, References> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn is(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// Each owner's references. Connections that name no owner keep their own.
#[derive(Debug, Default)]
pub(crate) struct Owners(HashMap<OperationId, SharedReferences>);

impl Owners {
    /// The owner's references, starting from `first` for an owner no live connection has.
    pub(crate) fn get_or_insert(
        &mut self,
        owner: OperationId,
        first: impl FnOnce() -> References,
    ) -> SharedReferences {
        self.0
            .entry(owner)
            .or_insert_with(|| SharedReferences::new(first()))
            .clone()
    }

    /// Forgets owners none of whose connections are still open.
    pub(crate) fn retain(&mut self, mut connected: impl FnMut(OperationId) -> bool) {
        self.0.retain(|owner, _| connected(*owner));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(id: u64) -> SessionId {
        SessionId::new(id).unwrap_or_else(|| unreachable!())
    }

    #[test]
    fn dropped_sessions_stay_released_until_referenced_or_attached_again() {
        let mut references = References::default();
        references.update(1, &[session(1), session(2)]);
        references.update(2, &[session(2)]);
        assert!(references.is_released(session(1)));
        references.update(3, &[session(1)]);
        assert!(!references.is_released(session(1)));
        assert!(references.is_released(session(2)));
        references.attached(session(2));
        assert!(!references.is_released(session(2)));
    }
}
