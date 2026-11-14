use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use muxy_protocol::{
    ACTIVITY_HISTORY_LIMIT, ActivityEvent, ActivityKind, ActivitySnapshot, AgentActivity,
    AgentState, ProjectId, SessionId, Topic,
};

use crate::notifier::Notifier;

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

/// Told about each new event, outside the activity lock.
type Observer = Box<dyn Fn(&ActivityEvent) + Send + Sync>;

/// Live agent state and at most one unread indicator event per session, held only in memory.
#[derive(Default)]
pub(crate) struct Activity {
    state: Mutex<State>,
    notifier: Arc<Notifier>,
    observer: OnceLock<Observer>,
}

impl std::fmt::Debug for Activity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Activity")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Activity {
    pub(crate) fn new(notifier: Arc<Notifier>) -> Self {
        Self {
            state: Mutex::default(),
            notifier,
            observer: OnceLock::new(),
        }
    }

    /// Installs the one observer of new events.
    pub(crate) fn observe(&self, observer: impl Fn(&ActivityEvent) + Send + Sync + 'static) {
        if self.observer.set(Box::new(observer)).is_err() {
            log::warn!("activity already has an observer");
        }
    }

    /// The event while it is still unread: not acknowledged, replaced by a
    /// newer one for its session, or gone with the session.
    pub(crate) fn event(&self, id: u64) -> Option<ActivityEvent> {
        lock(&self.state)
            .events
            .iter()
            .find(|event| event.id == id)
            .cloned()
    }

    pub(crate) fn open(directory: &Path, notifier: Arc<Notifier>) -> io::Result<Self> {
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
            notifier,
            observer: OnceLock::new(),
        })
    }

    /// Bumps the revision and tells watching connections once the lock is released.
    fn changed(&self, mut state: MutexGuard<'_, State>) {
        state.revision += 1;
        let revision = state.revision;
        drop(state);
        self.notifier.notify(Topic::Activity, revision);
    }

    #[cfg(test)]
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
        let created = kind.map(|kind| {
            state.retain_events(|event| event.session != activity.session);
            state.next_id += 1;
            let event = ActivityEvent {
                id: state.next_id,
                session: activity.session,
                project: activity.project,
                provider: activity.provider,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                kind,
                read: false,
            };
            state.events.insert(0, event.clone());
            if state.events.len() > ACTIVITY_HISTORY_LIMIT
                && let Some(expired) = state.events.pop()
            {
                state.claimed.remove(&expired.id);
            }
            event
        });
        state.agents.insert(activity.session, activity);
        self.changed(state);
        if let (Some(event), Some(observer)) = (created, self.observer.get()) {
            observer(&event);
        }
    }

    pub(crate) fn clear_agent(&self, session: SessionId) {
        let mut state = lock(&self.state);
        if state.agents.remove(&session).is_some() {
            self.changed(state);
        }
    }

    pub(crate) fn remove(&self, session: SessionId) {
        let mut state = lock(&self.state);
        let agent_removed = state.agents.remove(&session).is_some();
        let event_removed = state.retain_events(|event| event.session != session);
        if agent_removed || event_removed {
            self.changed(state);
        }
    }

    /// Attributes a session's agent and unread event to another project.
    pub(crate) fn move_session(&self, session: SessionId, project: ProjectId) {
        let mut state = lock(&self.state);
        let mut moved = false;
        if let Some(agent) = state.agents.get_mut(&session)
            && agent.project != project
        {
            agent.project = project;
            moved = true;
        }
        for event in state
            .events
            .iter_mut()
            .filter(|event| event.session == session && event.project != project)
        {
            event.project = project;
            moved = true;
        }
        if moved {
            self.changed(state);
        }
    }

    pub(crate) fn acknowledge(&self, ids: &[u64]) {
        let mut state = lock(&self.state);
        if state.retain_events(|event| !ids.contains(&event.id)) {
            self.changed(state);
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
    use muxy_protocol::AgentProvider;

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
    fn the_observer_hears_each_new_event_once_and_events_stay_readable_until_gone() {
        let activity = Activity::default();
        let (sender, events) = std::sync::mpsc::channel();
        activity.observe(move |event| {
            let _ = sender.send(event.clone());
        });
        activity.update(agent(AgentState::Working), false);
        activity.update(agent(AgentState::Blocked), false);
        activity.update(agent(AgentState::Blocked), false);
        let attention = events.try_recv().ok();
        assert_eq!(
            attention.as_ref().map(|event| event.kind),
            Some(ActivityKind::Attention)
        );
        assert!(events.try_recv().is_err());
        let Some(attention) = attention else { return };
        assert_eq!(activity.event(attention.id), Some(attention.clone()));
        activity.update(agent(AgentState::Working), false);
        activity.update(agent(AgentState::Idle), true);
        let completed = events.try_recv().ok();
        assert_eq!(
            completed.as_ref().map(|event| event.kind),
            Some(ActivityKind::Completed)
        );
        assert_eq!(activity.event(attention.id), None);
        let Some(completed) = completed else { return };
        activity.acknowledge(&[completed.id]);
        assert_eq!(activity.event(completed.id), None);
    }

    #[test]
    fn a_moved_session_takes_its_agent_and_unread_event_along() {
        let activity = Activity::default();
        let blocked = agent(AgentState::Blocked);
        activity.update(blocked.clone(), false);
        let revision = activity.revision();
        let project = ProjectId::from_u128(2);
        activity.move_session(blocked.session, project);
        let snapshot = activity.snapshot();
        assert_eq!(snapshot.agents[0].project, project);
        assert_eq!(snapshot.events[0].project, project);
        assert!(activity.revision() > revision);
        let revision = activity.revision();
        activity.move_session(blocked.session, project);
        assert_eq!(activity.revision(), revision);
    }
}
