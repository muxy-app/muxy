use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use muxy_protocol::{
    ACTIVITY_HISTORY_LIMIT, ActivityEvent, ActivityKind, ActivitySnapshot, AgentActivity,
    AgentState, SessionId,
};

#[derive(Debug, Default)]
struct State {
    revision: u64,
    next_id: u64,
    agents: BTreeMap<SessionId, AgentActivity>,
    events: Vec<ActivityEvent>,
    claimed: HashSet<u64>,
}

impl State {
    fn retain_events(&mut self, mut keep: impl FnMut(&ActivityEvent) -> bool) -> bool {
        let previous = self.events.len();
        self.events.retain(|event| {
            if keep(event) {
                true
            } else {
                self.claimed.remove(&event.id);
                false
            }
        });
        previous != self.events.len()
    }
}

/// Live agent state and at most one unread indicator event per session, held only in memory.
#[derive(Debug, Default)]
pub(crate) struct Activity {
    state: Mutex<State>,
}

impl Activity {
    pub(crate) fn open(directory: &Path) -> io::Result<Self> {
        match fs::remove_file(directory.join("activity.json")) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        // Native notifications from an earlier server must never target a new event.
        let next_id = u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros(),
        )
        .unwrap_or(0);
        Ok(Self {
            state: Mutex::new(State {
                revision: 1,
                next_id,
                ..State::default()
            }),
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
            events: state.events.clone(),
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
            state.retain_events(|event| event.session != activity.session);
            state.next_id += 1;
            let id = state.next_id;
            state.events.insert(
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
            if state.events.len() > ACTIVITY_HISTORY_LIMIT
                && let Some(expired) = state.events.pop()
            {
                state.claimed.remove(&expired.id);
            }
        }
        state.agents.insert(activity.session, activity);
        state.revision += 1;
    }

    pub(crate) fn clear_agent(&self, session: SessionId) {
        let mut state = lock(&self.state);
        if state.agents.remove(&session).is_some() {
            state.revision += 1;
        }
    }

    pub(crate) fn remove(&self, session: SessionId) {
        let mut state = lock(&self.state);
        let agent_removed = state.agents.remove(&session).is_some();
        let event_removed = state.retain_events(|event| event.session != session);
        if agent_removed || event_removed {
            state.revision += 1;
        }
    }

    pub(crate) fn acknowledge(&self, ids: &[u64]) {
        let mut state = lock(&self.state);
        if state.retain_events(|event| !ids.contains(&event.id)) {
            state.revision += 1;
        }
    }

    pub(crate) fn claim(&self, ids: &[u64]) -> Vec<u64> {
        let mut state = lock(&self.state);
        let eligible: Vec<_> = state
            .events
            .iter()
            .filter(|event| ids.contains(&event.id))
            .map(|event| event.id)
            .collect();
        eligible
            .into_iter()
            .filter(|id| state.claimed.insert(*id))
            .collect()
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
    fn unread_events_are_replaced_acknowledged_and_removed_with_the_session() {
        let activity = Activity::default();
        activity.update(agent(AgentState::Blocked), false);
        let first = activity.snapshot().events[0].id;
        assert_eq!(activity.claim(&[first]), vec![first]);
        assert!(activity.claim(&[first]).is_empty());
        activity.update(agent(AgentState::Working), false);
        activity.update(agent(AgentState::Idle), true);
        let snapshot = activity.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        let second = snapshot.events[0].id;
        assert!(second > first);
        assert!(lock(&activity.state).claimed.is_empty());
        activity.acknowledge(&[first]);
        assert_eq!(activity.snapshot().events, snapshot.events);
        activity.acknowledge(&[second]);
        assert!(activity.snapshot().events.is_empty());
        activity.update(agent(AgentState::Blocked), false);
        let blocked = activity.snapshot().events[0].id;
        activity.acknowledge(&[blocked]);
        assert_eq!(activity.snapshot().agents[0].state, AgentState::Blocked);
        activity.update(agent(AgentState::Working), false);
        activity.update(agent(AgentState::Idle), true);
        let completed = activity.snapshot().events[0].id;
        activity.claim(&[completed]);
        activity.clear_agent(agent(AgentState::Idle).session);
        assert!(activity.snapshot().agents.is_empty());
        assert_eq!(activity.snapshot().events.len(), 1);
        let revision = activity.revision();
        activity.remove(agent(AgentState::Idle).session);
        assert!(activity.snapshot().events.is_empty());
        assert!(lock(&activity.state).claimed.is_empty());
        assert!(activity.revision() > revision);
    }

    #[test]
    fn pending_events_are_bounded_and_legacy_history_is_not_restored() -> io::Result<()> {
        let directory = std::env::temp_dir().join(format!(
            "muxy-activity-{}",
            muxy_protocol::OperationId::new()
        ));
        fs::create_dir(&directory)?;
        fs::write(directory.join("activity.json"), b"obsolete history")?;
        let activity = Activity::open(&directory)?;
        assert!(activity.snapshot().events.is_empty());
        assert!(!directory.join("activity.json").exists());
        for id in 1..=220 {
            let mut state = agent(AgentState::Blocked);
            state.session = SessionId::new(id).expect("session");
            activity.update(state, false);
        }
        assert_eq!(activity.snapshot().events.len(), ACTIVITY_HISTORY_LIMIT);
        drop(activity);
        let restored = Activity::open(&directory)?;
        assert!(restored.snapshot().agents.is_empty());
        assert!(restored.snapshot().events.is_empty());
        assert!(!directory.join("activity.json").exists());
        fs::remove_dir_all(directory)
    }
}
