mod files;
mod parse;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use muxy_protocol::AgentProvider;

pub use files::{Catalog, Listing, Roots};

pub const PROVIDERS: [AgentProvider; 13] = [
    AgentProvider::Claude,
    AgentProvider::Codex,
    AgentProvider::Copilot,
    AgentProvider::Droid,
    AgentProvider::Pi,
    AgentProvider::OhMyPi,
    AgentProvider::Xal,
    AgentProvider::OpenCode,
    AgentProvider::Cursor,
    AgentProvider::Grok,
    AgentProvider::Kiro,
    AgentProvider::Antigravity,
    AgentProvider::Hermes,
];

pub fn supported(provider: AgentProvider) -> bool {
    matches!(
        provider,
        AgentProvider::Claude
            | AgentProvider::Codex
            | AgentProvider::Copilot
            | AgentProvider::Droid
            | AgentProvider::Pi
            | AgentProvider::OhMyPi
            | AgentProvider::Xal
            | AgentProvider::Cursor
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub provider: AgentProvider,
    pub id: String,
    pub title: String,
    pub directory: PathBuf,
    pub path: PathBuf,
    pub modified: SystemTime,
}

impl Session {
    pub fn matches(&self, query: &str) -> bool {
        query.split_whitespace().all(|word| {
            let word = word.to_lowercase();
            self.title.to_lowercase().contains(&word)
                || self.id.to_lowercase().contains(&word)
                || self.provider.name().to_lowercase().contains(&word)
        })
    }

    pub fn resume_command(&self) -> Option<String> {
        if !valid_id(&self.id) {
            return None;
        }
        let command = match self.provider {
            AgentProvider::Claude => "claude --resume",
            AgentProvider::Codex => "codex resume",
            AgentProvider::Copilot => "copilot --resume",
            AgentProvider::Droid => "droid --resume",
            AgentProvider::Pi => "pi --session",
            AgentProvider::OhMyPi => "omp --resume",
            AgentProvider::Xal => "xal resume",
            AgentProvider::Cursor => "cursor-agent --resume",
            _ => return None,
        };
        Some(format!("{command} {}", self.id))
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 200
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

fn normalized(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests;
