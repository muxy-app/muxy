use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
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
    tabs: crate::agent_tabs::Tracker,
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
    notifier: Arc<Notifier>,
    /// Whether the agent tab poller is running.
    polling: AtomicBool,
    /// Wakes the poller when terminals change.
    wake: Condvar,
}

impl Activity {
    pub(crate) fn new(notifier: Arc<Notifier>) -> Self {
        Self {
            notifier,
            ..Self::default()
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
            polling: AtomicBool::new(false),
            wake: Condvar::new(),
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
        let tabs = state.tabs.reconciled();
        ActivitySnapshot {
            revision: state.revision,
            agents: state
                .agents
                .values()
                .map(|agent| AgentActivity {
                    title: tabs
                        .shown
                        .get(&agent.session)
                        .map(|(_, title)| title.clone()),
                    ..agent.clone()
                })
                .collect(),
            events: state.events.clone(),
            tabs: tabs.tabs.clone(),
        }
    }

    /// Records what a terminal running an agent with tabs shows, matches it
    /// against the last reports at once, and starts the poller if it isn't
    /// running.
    pub(crate) fn observe_agent_tabs(
        self: &Arc<Self>,
        session: SessionId,
        pane: crate::agent_tabs::Pane,
    ) {
        let mut state = lock(&self.state);
        if state.tabs.observe(session, pane) {
            self.wake.notify_all();
            self.reconcile_agent_tabs(state);
        } else {
            drop(state);
        }
        if cfg!(test) || self.polling.swap(true, Ordering::AcqRel) {
            return;
        }
        let activity = Arc::downgrade(self);
        if let Err(error) = std::thread::Builder::new()
            .name("muxy-agent-tabs".into())
            .spawn(move || crate::agent_tabs::poll(&activity))
        {
            log::warn!("agent tab polling did not start: {error}");
            self.polling.store(false, Ordering::Release);
        }
    }

    /// The terminals to report on, once `interval` passes or one changes.
    pub(crate) fn wait_for_agent_tabs(
        &self,
        interval: std::time::Duration,
    ) -> BTreeMap<SessionId, crate::agent_tabs::Pane> {
        let state = lock(&self.state);
        let (mut state, _) = self
            .wake
            .wait_timeout_while(state, interval, |state| !state.tabs.stale())
            .unwrap_or_else(PoisonError::into_inner);
        state.tabs.take_panes()
    }

    /// Forgets what `session` showed when it stops running such an agent.
    pub(crate) fn leave_agent_tabs(&self, session: SessionId) {
        let mut state = lock(&self.state);
        if state.tabs.forget(session) {
            self.reconcile_agent_tabs(state);
        }
    }

    /// Whether the agent in `session` reports work running that its screen
    /// may not show.
    pub(crate) fn agent_tab_busy(&self, session: SessionId) -> bool {
        lock(&self.state).tabs.reconciled().busy.contains(&session)
    }

    pub(crate) fn set_agent_tabs_report(
        &self,
        provider: muxy_protocol::AgentProvider,
        report: crate::agent_tabs::Report,
    ) {
        let mut state = lock(&self.state);
        state.tabs.set_report(provider, report);
        self.reconcile_agent_tabs(state);
    }

    pub(crate) fn clear_agent_tabs_busy(&self, provider: muxy_protocol::AgentProvider) {
        let mut state = lock(&self.state);
        state.tabs.clear_busy(provider);
        self.reconcile_agent_tabs(state);
    }

    fn reconcile_agent_tabs(&self, mut state: MutexGuard<'_, State>) {
        if state.tabs.reconcile() {
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
        let tabs = state.tabs.forget(session) && state.tabs.reconcile();
        if state.agents.remove(&session).is_some() || tabs {
            self.changed(state);
        }
    }

    pub(crate) fn remove(&self, session: SessionId) {
        let mut state = lock(&self.state);
        let tabs = state.tabs.forget(session) && state.tabs.reconcile();
        let agent_removed = state.agents.remove(&session).is_some();
        let event_removed = state.retain_events(|event| event.session != session);
        if agent_removed || event_removed || tabs {
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
