use serde::{Deserialize, Serialize};

use crate::{ProjectId, SessionId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AgentProvider {
    Claude,
    Codex,
    OpenCode,
    Cursor,
    Copilot,
    Droid,
    Pi,
    Grok,
    Kiro,
    Xal,
    Antigravity,
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
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    #[default]
    Unknown,
    Idle,
    Working,
    Blocked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentActivity {
    pub session: SessionId,
    pub project: ProjectId,
    pub provider: AgentProvider,
    pub state: AgentState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ActivityKind {
    Attention,
    Completed,
}

impl ActivityKind {
    pub fn description(self) -> &'static str {
        match self {
            Self::Attention => "Needs your attention",
            Self::Completed => "Finished working",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivityEvent {
    pub id: u64,
    pub session: SessionId,
    pub project: ProjectId,
    pub provider: AgentProvider,
    pub timestamp: u64,
    pub kind: ActivityKind,
    pub read: bool,
}

pub const ACTIVITY_HISTORY_LIMIT: usize = 200;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivitySnapshot {
    pub revision: u64,
    pub agents: Vec<AgentActivity>,
    pub events: Vec<ActivityEvent>,
}
