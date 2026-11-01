use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError, Weak};

use muxy_protocol::{Message, ProjectId};

use super::DebouncedWatch;
use crate::ServerError;
use crate::connection::Outbox;

/// One watch per project, whoever asks for it. A watch stops when its last
/// subscription is dropped.
#[derive(Clone, Default)]
pub(crate) struct Shared(Arc<Mutex<HashMap<ProjectId, Arc<Slot>>>>);

#[derive(Default)]
struct Slot {
    watch: Mutex<Option<DebouncedWatch>>,
    subscribers: Subscribers,
}

/// The connections a watch reports to, held weakly so a closed connection
/// never stays alive for its watch.
#[derive(Clone, Default)]
pub(crate) struct Subscribers(Arc<Mutex<Vec<Weak<Outbox>>>>);

impl Subscribers {
    pub(crate) fn send(&self, message: &Message) {
        let mut outboxes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        outboxes.retain(|outbox| match outbox.upgrade() {
            Some(outbox) => {
                outbox.push_control(message.clone());
                true
            }
            None => false,
        });
    }

    fn add(&self, outbox: Weak<Outbox>) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(outbox);
    }

    /// Returns whether anyone is still subscribed.
    fn remove(&self, outbox: &Weak<Outbox>) -> bool {
        let mut outboxes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = outboxes.iter().position(|other| other.ptr_eq(outbox)) {
            outboxes.swap_remove(index);
        }
        outboxes.retain(|other| other.strong_count() > 0);
        !outboxes.is_empty()
    }
}

pub(crate) struct Subscription {
    shared: Shared,
    project: ProjectId,
    slot: Arc<Slot>,
    outbox: Weak<Outbox>,
}

impl Subscription {
    pub(crate) fn project(&self) -> ProjectId {
        self.project
    }
}

impl Shared {
    /// Subscribes `outbox` to the project's watch, starting it with `start`
    /// when nobody watches the project yet. `start` returns `None` when there
    /// is nothing to watch.
    pub(crate) fn subscribe(
        &self,
        project: ProjectId,
        outbox: &Arc<Outbox>,
        start: impl FnOnce(Subscribers) -> Result<Option<DebouncedWatch>, ServerError>,
    ) -> Result<Option<Subscription>, ServerError> {
        loop {
            let slot = Arc::clone(self.slots().entry(project).or_default());
            let mut watch = slot.watch.lock().unwrap_or_else(PoisonError::into_inner);
            // The last subscriber left between the lookup and the lock.
            if !self.holds(project, &slot) {
                continue;
            }
            if watch.is_none() {
                match start(slot.subscribers.clone()) {
                    Ok(Some(started)) => *watch = Some(started),
                    Ok(None) => {
                        self.forget(project, &slot);
                        return Ok(None);
                    }
                    Err(error) => {
                        self.forget(project, &slot);
                        return Err(error);
                    }
                }
            }
            let outbox = Arc::downgrade(outbox);
            slot.subscribers.add(Weak::clone(&outbox));
            drop(watch);
            return Ok(Some(Subscription {
                shared: self.clone(),
                project,
                slot,
                outbox,
            }));
        }
    }

    fn slots(&self) -> std::sync::MutexGuard<'_, HashMap<ProjectId, Arc<Slot>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn holds(&self, project: ProjectId, slot: &Arc<Slot>) -> bool {
        self.slots()
            .get(&project)
            .is_some_and(|held| Arc::ptr_eq(held, slot))
    }

    /// Called with the slot's watch locked, so nobody joins it meanwhile.
    fn forget(&self, project: ProjectId, slot: &Arc<Slot>) {
        let mut slots = self.slots();
        if slots
            .get(&project)
            .is_some_and(|held| Arc::ptr_eq(held, slot))
        {
            slots.remove(&project);
        }
    }

    #[cfg(test)]
    pub(crate) fn watching(&self) -> usize {
        self.slots().len()
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let stopped = {
            let mut watch = self
                .slot
                .watch
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if self.slot.subscribers.remove(&self.outbox) {
                None
            } else {
                self.shared.forget(self.project, &self.slot);
                watch.take()
            }
        };
        // Joins the watch's thread, so it happens without holding the lock.
        drop(stopped);
    }
}

impl std::fmt::Debug for Shared {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Shared")
            .field("projects", &self.slots().len())
            .finish()
    }
}
