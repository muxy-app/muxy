use super::{Catalog, PoisonError};
use muxy_protocol::{SessionId, SessionStatus};
use std::collections::BTreeMap;
use std::sync::{Arc, Weak};
use std::thread;
use std::time::{Duration, Instant};

const FIRST_RETRY: Duration = Duration::from_secs(1);
const LAST_RETRY: Duration = Duration::from_secs(60);

#[derive(Debug, Default)]
pub(super) struct PendingExits {
    statuses: BTreeMap<SessionId, (SessionStatus, u64)>,
    retry_after: Option<Instant>,
    /// A thread retries the write until no status is pending.
    retrying: bool,
}

impl Catalog {
    pub(crate) fn record_exit(self: &Arc<Self>, session: SessionId, status: SessionStatus) {
        self.pending_exits
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .statuses
            .insert(session, (status, super::retention::now()));
        self.retry_exits();
        let mut pending = self
            .pending_exits
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if pending.statuses.is_empty() || pending.retrying {
            return;
        }
        let catalog = Arc::downgrade(self);
        match thread::Builder::new()
            .name("catalog-exit-retry".into())
            .spawn(move || retry_until_written(&catalog))
        {
            Ok(_) => pending.retrying = true,
            Err(error) => log::error!("session exit retry could not start: {error}"),
        }
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
                pending.retry_after = Some(Instant::now() + FIRST_RETRY);
            }
        }
    }
}

/// Retries with a growing delay; stops once every status is written or the catalog is gone.
fn retry_until_written(catalog: &Weak<Catalog>) {
    let mut delay = FIRST_RETRY;
    loop {
        thread::sleep(delay);
        let Some(catalog) = catalog.upgrade() else {
            return;
        };
        catalog.retry_exits();
        let mut pending = catalog
            .pending_exits
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if pending.statuses.is_empty() {
            pending.retrying = false;
            return;
        }
        delay = (delay * 2).min(LAST_RETRY);
    }
}
