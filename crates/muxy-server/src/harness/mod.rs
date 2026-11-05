//! What agents with their own session tabs, such as `OpenCode`, report beyond
//! the screen: which of their sessions each terminal shows, which sessions
//! work in the background, and which open tabs no terminal shows.

mod http;
mod opencode;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};

use muxy_protocol::{AgentProvider, AgentState, AgentTab, ProjectId, SessionId};

pub(crate) use opencode::poll;

/// A terminal running `OpenCode`, as its session owner last saw it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Pane {
    pub(crate) project: ProjectId,
    /// The agent's working directory, which scopes its tabs.
    pub(crate) directory: String,
    /// The terminal title the agent set.
    pub(crate) title: String,
}

/// One open tab in an agent's tab bar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Tab {
    pub(crate) id: String,
    pub(crate) title: String,
}

/// What the agent's service reported.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Report {
    /// Open tabs in order, by working directory.
    pub(crate) tabs: HashMap<String, Vec<Tab>>,
    /// Sessions with work running, themselves or through a subagent.
    pub(crate) busy: HashSet<String>,
}

/// Where the report puts each terminal and every tab none of them shows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Reconciled {
    pub(crate) titles: BTreeMap<SessionId, String>,
    pub(crate) busy: HashSet<SessionId>,
    pub(crate) tabs: Vec<AgentTab>,
}

/// The session title in an `OpenCode` terminal title, `OC | <title>`, and
/// whether `OpenCode` shortened it with a trailing ellipsis.
fn shown_title(title: &str) -> Option<(&str, bool)> {
    let title = title.trim().strip_prefix("OC | ")?;
    Some(match title.strip_suffix('…') {
        Some(prefix) => (prefix, true),
        None => (title, false),
    })
}

fn same_directory(left: &str, right: &str) -> bool {
    let trim = |path: &str| path.len() > 1 && path.ends_with('/');
    let left = if trim(left) {
        &left[..left.len() - 1]
    } else {
        left
    };
    let right = if trim(right) {
        &right[..right.len() - 1]
    } else {
        right
    };
    left == right
}

/// Matches each terminal to the tab whose title it shows, then lists every
/// tab of each directory with the terminal showing it and all those that
/// have it open.
pub(crate) fn reconcile(panes: &BTreeMap<SessionId, Pane>, report: &Report) -> Reconciled {
    let mut reconciled = Reconciled::default();
    let mut shown: HashMap<&str, SessionId> = HashMap::new();
    let mut directories: Vec<(&str, &[Tab], Vec<SessionId>, ProjectId)> = Vec::new();
    for (&session, pane) in panes {
        let Some((directory, tabs)) = report
            .tabs
            .iter()
            .find(|(directory, _)| same_directory(directory, &pane.directory))
        else {
            continue;
        };
        match directories
            .iter_mut()
            .find(|(known, ..)| *known == directory)
        {
            Some((_, _, hosts, _)) => hosts.push(session),
            None => directories.push((directory, tabs, vec![session], pane.project)),
        }
        let Some((title, shortened)) = shown_title(&pane.title) else {
            continue;
        };
        let tab = tabs.iter().find(|tab| {
            !shown.contains_key(tab.id.as_str())
                && if shortened {
                    tab.title.starts_with(title)
                } else {
                    tab.title == title
                }
        });
        if let Some(tab) = tab {
            shown.insert(&tab.id, session);
            reconciled.titles.insert(session, tab.title.clone());
            if report.busy.contains(&tab.id) {
                reconciled.busy.insert(session);
            }
        }
    }
    for (_, tabs, hosts, project) in directories {
        for (index, tab) in tabs.iter().enumerate() {
            reconciled.tabs.push(AgentTab {
                id: tab.id.clone(),
                project,
                provider: AgentProvider::OpenCode,
                title: tab.title.clone(),
                state: if report.busy.contains(&tab.id) {
                    AgentState::Working
                } else {
                    AgentState::Idle
                },
                shown_by: shown.get(tab.id.as_str()).copied(),
                index: u16::try_from(index).ok(),
                hosts: hosts.clone(),
            });
        }
    }
    reconciled
}
