use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::{SystemTime, UNIX_EPOCH};

use muxy_protocol::{
    ACTIVITY_HISTORY_LIMIT, ActivityEvent, ActivityKind, ActivitySnapshot, AgentActivity,
    AgentState, SessionId,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct History {
    next_id: u64,
    events: Vec<ActivityEvent>,
}

#[derive(Debug, Default)]
struct State {
    revision: u64,
    agents: BTreeMap<SessionId, AgentActivity>,
    history: History,
    claimed: HashSet<u64>,
}

/// Detection only changes memory and wakes the coalescing persistence worker.
#[derive(Debug, Default)]
pub(crate) struct Activity {
    state: Arc<Mutex<State>>,
    writer: Option<Arc<Writer>>,
    wake: Option<mpsc::SyncSender<()>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

#[derive(Debug)]
struct Writer {
    path: PathBuf,
    serial: Mutex<()>,
}

impl Activity {
    pub(crate) fn open(directory: &Path) -> io::Result<Self> {
        let path = directory.join("activity.json");
        let mut history: History = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => History::default(),
            Err(error) => return Err(error),
        };
        history.events.truncate(ACTIVITY_HISTORY_LIMIT);
        history.next_id = history
            .next_id
            .max(history.events.iter().map(|e| e.id).max().unwrap_or(0));
        let state = Arc::new(Mutex::new(State {
            revision: 1,
            history,
            ..State::default()
        }));
        let writer = Arc::new(Writer {
            path,
            serial: Mutex::new(()),
        });
        let (wake, receiver) = mpsc::sync_channel(1);
        let pending = Arc::clone(&state);
        let output = Arc::clone(&writer);
        let worker = std::thread::Builder::new()
            .name("activity-history".into())
            .spawn(move || {
                while receiver.recv().is_ok() {
                    if let Err(error) = output.save(&pending) {
                        log::error!("Could not save activity history: {error}");
                    }
                }
                if let Err(error) = output.save(&pending) {
                    log::error!("Could not save final activity history: {error}");
                }
            })?;
        Ok(Self {
            state,
            writer: Some(writer),
            wake: Some(wake),
            worker: Some(worker),
        })
    }

    pub(crate) fn revision(&self) -> u64 {
        lock(&self.state).revision
    }

    pub(crate) fn snapshot(&self) -> ActivitySnapshot {
        let state = lock(&self.state);
        ActivitySnapshot {
            revision: state.revision,
            agents: state.agents.values().cloned().collect(),
            events: state.history.events.clone(),
        }
    }

    pub(crate) fn update(&self, activity: AgentActivity, completed: bool) {
        let mut state = lock(&self.state);
        let previous = state.agents.get(&activity.session);
        if previous == Some(&activity) {
            return;
        }
        let kind = if activity.state == AgentState::Blocked
            && previous.is_none_or(|old| {
                old.state != AgentState::Blocked || old.provider != activity.provider
            }) {
            Some(ActivityKind::Attention)
        } else if completed {
            Some(ActivityKind::Completed)
        } else {
            None
        };
        if let Some(kind) = kind {
            state.history.next_id += 1;
            let id = state.history.next_id;
            state.history.events.insert(
                0,
                ActivityEvent {
                    id,
                    session: activity.session,
                    project: activity.project,
                    provider: activity.provider,
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    kind,
                    read: false,
                },
            );
            state.history.events.truncate(ACTIVITY_HISTORY_LIMIT);
            let retained: HashSet<_> = state.history.events.iter().map(|event| event.id).collect();
            state.claimed.retain(|id| retained.contains(id));
            self.save_later();
        }
        state.agents.insert(activity.session, activity);
        state.revision += 1;
    }

    pub(crate) fn remove(&self, session: SessionId) {
        let mut state = lock(&self.state);
        if state.agents.remove(&session).is_some() {
            state.revision += 1;
        }
    }

    pub(crate) fn acknowledge(&self, ids: &[u64]) -> io::Result<()> {
        let _serial = self.writer.as_ref().map(|writer| lock(&writer.serial));
        if let Some(writer) = &self.writer {
            let mut history = lock(&self.state).history.clone();
            for event in &mut history.events {
                if ids.contains(&event.id) {
                    event.read = true;
                }
            }
            writer.write(&history)?;
        }
        let mut state = lock(&self.state);
        let mut changed = false;
        for event in &mut state.history.events {
            if ids.contains(&event.id) && !event.read {
                event.read = true;
                changed = true;
            }
        }
        if changed {
            state.revision += 1;
        }
        Ok(())
    }

    pub(crate) fn claim(&self, ids: &[u64]) -> Vec<u64> {
        let mut state = lock(&self.state);
        let eligible: Vec<_> = state
            .history
            .events
            .iter()
            .filter(|event| !event.read && ids.contains(&event.id))
            .map(|event| event.id)
            .collect();
        eligible
            .into_iter()
            .filter(|id| state.claimed.insert(*id))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn flush(&self) -> io::Result<()> {
        self.writer
            .as_ref()
            .map_or(Ok(()), |writer| writer.save(&self.state))
    }

    fn save_later(&self) {
        if let Some(wake) = &self.wake {
            let _ = wake.try_send(());
        }
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        self.wake.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Writer {
    fn save(&self, state: &Mutex<State>) -> io::Result<()> {
        let _serial = lock(&self.serial);
        let history = lock(state).history.clone();
        self.write(&history)
    }

    fn write(&self, history: &History) -> io::Result<()> {
        let bytes = serde_json::to_vec(history)?;
        let temporary = self
            .path
            .with_extension(format!("{}.tmp", muxy_protocol::OperationId::new()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, &self.path)?;
            File::open(
                self.path
                    .parent()
                    .ok_or_else(|| io::Error::other("activity path has no parent"))?,
            )?
            .sync_all()
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_protocol::{AgentProvider, ProjectId};

    fn agent(state: AgentState) -> AgentActivity {
        AgentActivity {
            session: std::num::NonZeroU64::MIN.into(),
            project: ProjectId::from_u128(1),
            provider: AgentProvider::Xal,
            state,
        }
    }

    #[test]
    fn event_identity_read_acknowledgement_and_delivery_are_shared() -> io::Result<()> {
        let activity = Activity::default();
        activity.update(agent(AgentState::Working), false);
        activity.update(agent(AgentState::Blocked), false);
        activity.update(agent(AgentState::Blocked), false);
        let first = activity.snapshot().events[0].id;
        assert_eq!(activity.snapshot().events.len(), 1);
        activity.update(agent(AgentState::Working), false);
        activity.update(agent(AgentState::Idle), true);
        let second = activity.snapshot().events[0].id;
        activity.acknowledge(&[first])?;
        assert!(!activity.snapshot().events[0].read);
        assert!(activity.snapshot().events[1].read);
        assert_eq!(activity.claim(&[first, second]), vec![second]);
        assert!(activity.claim(&[second]).is_empty());
        activity.update(agent(AgentState::Blocked), false);
        let blocked = activity.snapshot().events[0].id;
        activity.acknowledge(&[blocked])?;
        assert_eq!(activity.snapshot().agents[0].state, AgentState::Blocked);
        activity.remove(agent(AgentState::Idle).session);
        assert!(activity.snapshot().agents.is_empty());
        assert_eq!(activity.snapshot().events.len(), 3);
        Ok(())
    }

    #[test]
    fn bounded_history_survives_restart_without_reviving_live_state() -> io::Result<()> {
        let directory = std::env::temp_dir().join(format!(
            "muxy-activity-{}",
            muxy_protocol::OperationId::new()
        ));
        fs::create_dir(&directory)?;
        let activity = Activity::open(&directory)?;
        for _ in 0..220 {
            activity.update(agent(AgentState::Working), false);
            activity.update(agent(AgentState::Idle), true);
        }
        let snapshot = activity.snapshot();
        assert_eq!(snapshot.events.len(), ACTIVITY_HISTORY_LIMIT);
        activity.acknowledge(&[snapshot.events[0].id])?;
        let saved = activity.snapshot().events;
        drop(activity);
        let restored = Activity::open(&directory)?;
        assert!(restored.snapshot().agents.is_empty());
        assert_eq!(restored.snapshot().events, saved);
        restored.update(agent(AgentState::Blocked), false);
        assert!(restored.snapshot().events[0].id > snapshot.events[0].id);
        restored.flush()?;
        drop(restored);
        fs::remove_dir_all(directory)?;
        Ok(())
    }
}
