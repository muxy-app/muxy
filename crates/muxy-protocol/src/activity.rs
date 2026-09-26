use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::wire::cbor::open_enum;

use crate::{ProjectId, SessionId};

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum AgentProvider {
        Claude = 0,
        Codex = 1,
        OpenCode = 2,
        Cursor = 3,
        Copilot = 4,
        Droid = 5,
        Pi = 6,
        Grok = 7,
        Kiro = 8,
        Xal = 9,
        Antigravity = 10,
    }
}

impl AgentProvider {
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::OpenCode => "OpenCode",
            Self::Cursor => "Cursor",
            Self::Copilot => "Copilot",
            Self::Droid => "Droid",
            Self::Pi => "Pi",
            Self::Grok => "Grok",
            Self::Kiro => "Kiro",
            Self::Xal => "Xal",
            Self::Antigravity => "Antigravity",
            Self::Unrecognized(_) => "Agent",
        }
    }
}

open_enum! {
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum AgentState {
        #[default]
        Unknown = 0,
        Idle = 1,
        Working = 2,
        Blocked = 3,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct AgentActivity {
    #[n(0)]
    pub session: SessionId,
    #[n(1)]
    pub project: ProjectId,
    #[n(2)]
    pub provider: AgentProvider,
    #[n(3)]
    pub state: AgentState,
}

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum ActivityKind {
        Attention = 0,
        Completed = 1,
    }
}

impl ActivityKind {
    pub fn description(self) -> &'static str {
        match self {
            Self::Attention => "Needs your attention",
            Self::Completed => "Finished working",
            Self::Unrecognized(_) => "Has an update",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ActivityEvent {
    #[n(0)]
    pub id: u64,
    #[n(1)]
    pub session: SessionId,
    #[n(2)]
    pub project: ProjectId,
    #[n(3)]
    pub provider: AgentProvider,
    #[n(4)]
    pub timestamp: u64,
    #[n(5)]
    pub kind: ActivityKind,
    #[n(6)]
    pub read: bool,
}

pub const ACTIVITY_HISTORY_LIMIT: usize = 200;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ActivitySnapshot {
    #[n(0)]
    pub revision: u64,
    #[n(1)]
    pub agents: Vec<AgentActivity>,
    #[n(2)]
    pub events: Vec<ActivityEvent>,
}
