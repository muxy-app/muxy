//! Presentation derived from server activity; no independent history or read state.
use muxy_protocol::{ActivityKind, ActivitySnapshot, AgentState, SessionId};

pub fn effective_progress(
    agent: Option<AgentState>,
    progress: Option<muxy_protocol::TerminalProgress>,
) -> Option<muxy_protocol::TerminalProgress> {
    match agent {
        None => progress,
        Some(AgentState::Working) => Some(progress.unwrap_or(muxy_protocol::TerminalProgress {
            state: muxy_protocol::ProgressState::Indeterminate,
            percent: None,
        })),
        Some(
            AgentState::Idle
            | AgentState::Blocked
            | AgentState::Unknown
            | AgentState::Unrecognized(_),
        ) => None,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum ActivityIndicator {
    #[default]
    None,
    Idle,
    Completed,
    Working,
    Blocked,
}

pub fn indicator(
    snapshot: &ActivitySnapshot,
    includes: impl Fn(SessionId) -> bool,
) -> ActivityIndicator {
    let live = snapshot
        .agents
        .iter()
        .filter(|a| includes(a.session))
        .map(|a| match a.state {
            AgentState::Unknown | AgentState::Idle | AgentState::Unrecognized(_) => {
                ActivityIndicator::Idle
            }
            AgentState::Working => ActivityIndicator::Working,
            AgentState::Blocked => ActivityIndicator::Blocked,
        });
    let unread = snapshot
        .events
        .iter()
        .filter(|event| !event.read && includes(event.session))
        .map(|event| match event.kind {
            ActivityKind::Attention | ActivityKind::Completed | ActivityKind::Unrecognized(_) => {
                ActivityIndicator::Completed
            }
        });
    live.chain(unread).max().unwrap_or_default()
}

pub fn new_notifications(
    previous: Option<&ActivitySnapshot>,
    current: &ActivitySnapshot,
    focused: Option<SessionId>,
) -> Vec<u64> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    let latest = previous
        .events
        .iter()
        .map(|event| event.id)
        .max()
        .unwrap_or(0);
    current
        .events
        .iter()
        .filter(|event| event.id > latest && !event.read && Some(event.session) != focused)
        .map(|event| event.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_protocol::{ActivityEvent, AgentActivity, AgentProvider, ProjectId};

    #[test]
    fn rollups_prioritize_live_blockers_and_shared_read_state() {
        let session: SessionId = std::num::NonZeroU64::MIN.into();
        let mut snapshot = ActivitySnapshot {
            revision: 1,
            agents: vec![AgentActivity {
                session,
                project: ProjectId::from_u128(1),
                provider: AgentProvider::Codex,
                state: AgentState::Working,
            }],
            events: vec![ActivityEvent {
                id: 1,
                session,
                project: ProjectId::from_u128(1),
                provider: AgentProvider::Codex,
                kind: ActivityKind::Completed,
                read: false,
                timestamp: 0,
            }],
        };
        assert_eq!(indicator(&snapshot, |_| true), ActivityIndicator::Working);
        snapshot.agents[0].state = AgentState::Blocked;
        snapshot.events[0].read = true;
        assert_eq!(indicator(&snapshot, |_| true), ActivityIndicator::Blocked);
        snapshot.agents[0].state = AgentState::Idle;
        assert_eq!(indicator(&snapshot, |_| true), ActivityIndicator::Idle);
        snapshot.events[0].read = false;
        assert_eq!(indicator(&snapshot, |_| true), ActivityIndicator::Completed);
        assert_eq!(indicator(&snapshot, |_| false), ActivityIndicator::None);
    }

    #[test]
    fn reconnects_and_focused_sessions_do_not_replay_os_notifications() {
        let session: SessionId = std::num::NonZeroU64::MIN.into();
        let mut snapshot = ActivitySnapshot {
            events: vec![ActivityEvent {
                id: 42,
                session,
                project: ProjectId::from_u128(1),
                provider: AgentProvider::Codex,
                kind: ActivityKind::Attention,
                read: false,
                timestamp: 0,
            }],
            ..ActivitySnapshot::default()
        };
        assert!(new_notifications(None, &snapshot, None).is_empty());
        let previous = snapshot.clone();
        snapshot.events[0].id = 43;
        assert_eq!(
            new_notifications(Some(&previous), &snapshot, None),
            vec![43]
        );
        assert!(new_notifications(Some(&previous), &snapshot, Some(session)).is_empty());
        snapshot.events[0].read = true;
        assert!(new_notifications(Some(&previous), &snapshot, None).is_empty());
    }
}

#[cfg(test)]
mod progress_tests {
    use super::*;

    #[test]
    fn idle_agents_suppress_stale_progress_while_ordinary_commands_keep_it() {
        let progress = muxy_protocol::TerminalProgress {
            state: muxy_protocol::ProgressState::Indeterminate,
            percent: None,
        };
        assert_eq!(effective_progress(None, Some(progress)), Some(progress));
        assert_eq!(
            effective_progress(Some(AgentState::Working), None),
            Some(progress)
        );
        for state in [AgentState::Idle, AgentState::Blocked, AgentState::Unknown] {
            assert_eq!(effective_progress(Some(state), Some(progress)), None);
        }
    }
}
