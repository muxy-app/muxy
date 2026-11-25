use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError, Weak};

use muxy_protocol::{Message, ProjectId};

use super::DebouncedWatch;
use crate::ServerError;
use crate::connection::Connection;

#[derive(Clone, Default)]
pub(crate) struct Shared(Arc<Mutex<HashMap<ProjectId, Arc<Slot>>>>);

#[derive(Default)]
struct Slot {
    watch: Mutex<Option<DebouncedWatch>>,
    subscribers: Subscribers,
}

#[derive(Clone, Default)]
pub(crate) struct Subscribers(Arc<Mutex<Vec<Weak<Connection>>>>);

impl Subscribers {
    pub(crate) fn send(&self, message: &Message) {
        let mut connections = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        connections.retain(|connection| match connection.upgrade() {
            Some(connection) => {
                connection.push_control(message.clone());
                true
            }
            None => false,
        });
    }

    fn add(&self, connection: Weak<Connection>) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(connection);
    }

    fn remove(&self, connection: &Weak<Connection>) -> bool {
        let mut connections = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = connections
            .iter()
            .position(|other| other.ptr_eq(connection))
        {
            connections.swap_remove(index);
        }
        connections.retain(|other| other.strong_count() > 0);
        !connections.is_empty()
    }
}

pub(crate) struct Subscription {
    shared: Shared,
    project: ProjectId,
    slot: Arc<Slot>,
    connection: Weak<Connection>,
}

impl Subscription {
    pub(crate) fn project(&self) -> ProjectId {
        self.project
    }
}

impl Shared {
    pub(crate) fn subscribe(
        &self,
        project: ProjectId,
        connection: &Arc<Connection>,
        start: impl FnOnce(Subscribers) -> Result<Option<DebouncedWatch>, ServerError>,
    ) -> Result<Option<Subscription>, ServerError> {
        loop {
            let slot = Arc::clone(self.slots().entry(project).or_default());
            let mut watch = slot.watch.lock().unwrap_or_else(PoisonError::into_inner);
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
            let connection = Arc::downgrade(connection);
            slot.subscribers.add(Weak::clone(&connection));
            drop(watch);
            return Ok(Some(Subscription {
                shared: self.clone(),
                project,
                slot,
                connection,
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
        let stopped_watch = {
            let mut watch = self
                .slot
                .watch
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if self.slot.subscribers.remove(&self.connection) {
                None
            } else {
                self.shared.forget(self.project, &self.slot);
                watch.take()
            }
        };
        drop(stopped_watch);
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
