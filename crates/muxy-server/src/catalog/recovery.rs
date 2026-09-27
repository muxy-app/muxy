use super::{Catalog, PoisonError};
use muxy_protocol::{SessionId, SessionStatus};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub(super) struct PendingExits {
    statuses: BTreeMap<SessionId, (SessionStatus, u64)>,
    retry_after: Option<Instant>,
}

impl Catalog {
    pub(crate) fn record_exit(&self, session: SessionId, status: SessionStatus) {
        self.pending_exits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .statuses
            .insert(session, (status, super::retention::now()));
        self.retry_exits();
    }

    pub(super) fn retry_exits(&self) {
        let Ok(mut pending) = self.pending_exits.try_lock() else {
            return;
        };
        if pending.statuses.is_empty()
            || pending
                .retry_after
                .is_some_and(|deadline| Instant::now() < deadline)
        {
            return;
        }
        let result = self.update(|state| {
            for (id, (status, ended_at)) in &pending.statuses {
                if let Some(membership) = state.sessions.get_mut(id) {
                    membership.status = *status;
                    membership.ended_at.get_or_insert(*ended_at);
                }
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                pending.statuses.clear();
                pending.retry_after = None;
            }
            Err(error) => {
                log::error!("session exit persistence is pending: {error}");
                pending.retry_after = Some(Instant::now() + Duration::from_secs(1));
            }
        }
    }
}
