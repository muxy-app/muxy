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
