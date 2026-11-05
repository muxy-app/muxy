use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use muxy_protocol::{
    ACTIVITY_HISTORY_LIMIT, ActivityEvent, ActivityKind, ActivitySnapshot, AgentActivity,
    AgentState, SessionId, Topic,
};

use crate::notifier::Notifier;

#[derive(Debug, Default)]
struct State {
    revision: u64,
    next_id: u64,
    agents: BTreeMap<SessionId, AgentActivity>,
    events: Vec<ActivityEvent>,
    claimed: HashSet<u64>,
    harness_panes: BTreeMap<SessionId, crate::harness::Pane>,
    harness_report: crate::harness::Report,
    harness: crate::harness::Reconciled,
    /// The terminals changed since the poller last read them.
    harness_stale: bool,
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
pub(crate) struct Activity {
    state: Mutex<State>,
    notifier: Arc<Notifier>,
    poller: std::sync::Once,
    wake: std::sync::Condvar,
}

impl std::fmt::Debug for Activity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Activity")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Default for Activity {
    fn default() -> Self {
        Self::new(Arc::default())
    }
}

impl Activity {
    pub(crate) fn new(notifier: Arc<Notifier>) -> Self {
        Self {
            state: Mutex::default(),
            notifier,
            poller: std::sync::Once::new(),
            wake: std::sync::Condvar::new(),
        }
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
            poller: std::sync::Once::new(),
            wake: std::sync::Condvar::new(),
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
            agents: state
                .agents
                .values()
                .map(|agent| AgentActivity {
                    title: state.harness.titles.get(&agent.session).cloned(),
                    ..agent.clone()
                })
                .collect(),
            events: state.events.clone(),
            tabs: state.harness.tabs.clone(),
        }
    }

    /// Records what a terminal running an agent with its own tabs shows, and
    /// starts polling that agent's service the first time. A change is
    /// matched against the last report at once and asks for a fresh one.
    pub(crate) fn observe_harness(
        self: &Arc<Self>,
        session: SessionId,
        pane: crate::harness::Pane,
    ) {
        let mut state = lock(&self.state);
        if state.harness_panes.get(&session) == Some(&pane) {
            drop(state);
        } else {
            state.harness_panes.insert(session, pane);
            state.harness_stale = true;
            self.wake.notify_all();
            self.reconcile_harness(state);
        }
        if cfg!(test) {
            return;
        }
        self.poller.call_once(|| {
            let activity = Arc::downgrade(self);
            if let Err(error) = std::thread::Builder::new()
                .name("muxy-harness".into())
                .spawn(move || crate::harness::poll(&activity))
            {
                log::warn!("agent tab polling did not start: {error}");
            }
        });
    }

    /// The terminals to report on, once `interval` passes or one changes.
    pub(crate) fn wait_for_harness(
        &self,
        interval: std::time::Duration,
    ) -> BTreeMap<SessionId, crate::harness::Pane> {
        let state = lock(&self.state);
        let (mut state, _) = self
            .wake
            .wait_timeout_while(state, interval, |state| !state.harness_stale)
            .unwrap_or_else(PoisonError::into_inner);
        state.harness_stale = false;
        state.harness_panes.clone()
    }

    /// Forgets what `session` showed when it stops running such an agent.
    pub(crate) fn leave_harness(&self, session: SessionId) {
        let mut state = lock(&self.state);
        if state.harness_panes.remove(&session).is_some() {
            self.reconcile_harness(state);
        }
    }

    /// Whether the agent in `session` reports work running in the background.
    pub(crate) fn harness_busy(&self, session: SessionId) -> bool {
        lock(&self.state).harness.busy.contains(&session)
    }

    pub(crate) fn set_harness_report(&self, report: crate::harness::Report) {
        let mut state = lock(&self.state);
        state.harness_report = report;
        self.reconcile_harness(state);
    }

    /// Matches the terminals to the last report, telling watchers when what
    /// the panel shows changed.
    fn reconcile_harness(&self, mut state: MutexGuard<'_, State>) {
        let reconciled = crate::harness::reconcile(&state.harness_panes, &state.harness_report);
        if state.harness == reconciled {
            return;
        }
        let shown =
            state.harness.titles != reconciled.titles || state.harness.tabs != reconciled.tabs;
        state.harness = reconciled;
        if shown {
            self.changed(state);
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
        self.changed(state);
    }

    pub(crate) fn clear_agent(&self, session: SessionId) {
        let mut state = lock(&self.state);
        let harness = state.harness_panes.remove(&session).is_some();
        let agent_removed = state.agents.remove(&session).is_some();
        if harness {
            state.harness = crate::harness::reconcile(&state.harness_panes, &state.harness_report);
        }
        if agent_removed || harness {
            self.changed(state);
        }
    }

    pub(crate) fn remove(&self, session: SessionId) {
        let mut state = lock(&self.state);
        let harness = state.harness_panes.remove(&session).is_some();
        if harness {
            state.harness = crate::harness::reconcile(&state.harness_panes, &state.harness_report);
        }
        let agent_removed = state.agents.remove(&session).is_some();
        let event_removed = state.retain_events(|event| event.session != session);
        if agent_removed || event_removed || harness {
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
    use muxy_protocol::{AgentProvider, ProjectId};

    fn agent(state: AgentState) -> AgentActivity {
        AgentActivity {
            session: std::num::NonZeroU64::MIN.into(),
            project: ProjectId::from_u128(1),
            provider: AgentProvider::Xal,
            state,
            title: None,
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
}
