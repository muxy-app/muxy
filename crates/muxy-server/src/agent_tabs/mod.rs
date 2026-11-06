//! Agents that keep several sessions as tabs in one terminal, such as
//! `OpenCode`. Each one's [`TabSource`] says how its terminal title names the
//! session on screen, which tabs are open and working, and how a client
//! switches to or resumes a tab. From that the server tells clients which
//! session each terminal shows, which keep working in the background, and
//! which open tabs no terminal shows.
//!
//! Supporting another agent takes a `TabSource` in [`sources`] and its
//! provider in `PROVIDERS`; the protocol and clients need no change.

mod http;
mod opencode;
mod poll;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, HashMap, HashSet};

use muxy_protocol::{
    AgentProvider, AgentState, AgentTab, MAX_AGENT_TAB_HOSTS, MAX_AGENT_TAB_ID, MAX_AGENT_TABS,
    MAX_AGENT_TITLE, ProjectId, SessionId,
};

pub(crate) use poll::poll;

/// What one agent with session tabs reports, and how it's driven.
pub(crate) trait TabSource: Send {
    fn provider(&self) -> AgentProvider;

    /// The tabs open in `directories` and the sessions with work running,
    /// or `None` while the agent's service can't be reached.
    fn report(&mut self, directories: &HashSet<String>) -> Option<Report>;

    /// The session title the agent put in a terminal title, and whether it
    /// shortened it with a trailing ellipsis.
    fn shown_title<'a>(&self, title: &'a str) -> Option<(&'a str, bool)>;

    /// Input that makes the agent show the tab at `index` in its tab bar.
    fn switch_input(&self, index: usize) -> Option<Vec<u8>>;

    /// A command line that opens session `id` in a new terminal.
    fn resume_command(&self, id: &str) -> Option<String>;
}

/// The agents the server reads tabs from, in the order of [`sources`].
const PROVIDERS: [AgentProvider; 1] = [AgentProvider::OpenCode];

/// A source for each of [`PROVIDERS`].
pub(crate) fn sources() -> Vec<Box<dyn TabSource>> {
    vec![Box::new(opencode::OpenCode::default())]
}

/// Whether the server reads tabs from `provider`.
pub(crate) fn supported(provider: AgentProvider) -> bool {
    PROVIDERS.contains(&provider)
}

/// A terminal running such an agent, as its session owner last saw it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Pane {
    pub(crate) provider: AgentProvider,
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

/// What an agent's service reported.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Report {
    /// Open tabs in order, by working directory.
    pub(crate) tabs: HashMap<String, Vec<Tab>>,
    /// Sessions with work running, themselves or through a subagent.
    pub(crate) busy: HashSet<String>,
    /// When each session was last shown, in the agent's own clock, so
    /// terminals whose titles match several tabs show the latest of them.
    pub(crate) viewed: HashMap<String, u64>,
}

/// Where the reports put each terminal and every open tab.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Reconciled {
    /// The ID and title of the tab each terminal shows.
    pub(crate) shown: BTreeMap<SessionId, (String, String)>,
    pub(crate) busy: HashSet<SessionId>,
    pub(crate) tabs: Vec<AgentTab>,
}

pub(crate) fn same_directory(left: &str, right: &str) -> bool {
    fn trim(path: &str) -> &str {
        if path.len() > 1 {
            path.strip_suffix('/').unwrap_or(path)
        } else {
            path
        }
    }
    trim(left) == trim(right)
}

/// Matches each terminal to the tab whose title it shows, then lists every
/// tab of each directory with the terminal showing it and all those that
/// have it open. A terminal keeps the tab it showed before while that tab
/// still matches, so tabs with the same title don't trade terminals.
pub(crate) fn reconcile(
    sources: &[Box<dyn TabSource>],
    panes: &BTreeMap<SessionId, Pane>,
    reports: &HashMap<AgentProvider, Report>,
    previous: &Reconciled,
) -> Reconciled {
    let mut reconciled = Reconciled::default();
    for source in sources {
        let provider = source.provider();
        let Some(report) = reports.get(&provider) else {
            continue;
        };
        let panes: BTreeMap<_, _> = panes
            .iter()
            .filter(|(_, pane)| pane.provider == provider)
            .collect();
        reconcile_source(source.as_ref(), &panes, report, previous, &mut reconciled);
    }
    reconciled
}

fn reconcile_source(
    source: &dyn TabSource,
    panes: &BTreeMap<&SessionId, &Pane>,
    report: &Report,
    previous: &Reconciled,
    reconciled: &mut Reconciled,
) {
    let mut directories: Vec<(&str, &[Tab], Vec<SessionId>, ProjectId)> = Vec::new();
    let mut wanted: Vec<(SessionId, &[Tab], &str, bool)> = Vec::new();
    for (&&session, pane) in panes {
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
        if let Some((title, shortened)) = source.shown_title(&pane.title) {
            wanted.push((session, tabs, title, shortened));
        }
    }
    let matches = |tab: &Tab, title: &str, shortened: bool| {
        if shortened {
            tab.title.starts_with(title)
        } else {
            tab.title == title
        }
    };
    let viewed = |tab: &Tab| report.viewed.get(&tab.id).copied().unwrap_or(0);
    let kept: HashMap<SessionId, &str> = wanted
        .iter()
        .filter_map(|(session, tabs, title, shortened)| {
            let (id, _) = previous.shown.get(session)?;
            tabs.iter()
                .find(|tab| tab.id == *id && matches(tab, title, *shortened))
                .map(|tab| (*session, tab.id.as_str()))
        })
        .collect();
    let mut shown: HashMap<&str, SessionId> = HashMap::new();
    for (session, tabs, title, shortened) in wanted {
        let held_elsewhere = |tab: &Tab| {
            kept.iter()
                .any(|(other, id)| *other != session && *id == tab.id)
        };
        let newest = tabs
            .iter()
            .filter(|tab| {
                matches(tab, title, shortened)
                    && !shown.contains_key(tab.id.as_str())
                    && !held_elsewhere(tab)
            })
            .min_by_key(|tab| std::cmp::Reverse(viewed(tab)));
        let kept = kept
            .get(&session)
            .and_then(|id| tabs.iter().find(|tab| tab.id == *id))
            .filter(|tab| !shown.contains_key(tab.id.as_str()));
        let tab = match (kept, newest) {
            (Some(kept), Some(newest)) if viewed(newest) > viewed(kept) => Some(newest),
            (Some(kept), _) => Some(kept),
            (None, newest) => newest,
        };
        if let Some(tab) = tab {
            claim(&mut shown, reconciled, report, session, tab);
        }
    }
    for (_, tabs, mut hosts, project) in directories {
        hosts.truncate(MAX_AGENT_TAB_HOSTS);
        for (index, tab) in tabs.iter().enumerate() {
            if reconciled.tabs.len() >= MAX_AGENT_TABS {
                return;
            }
            if tab.id.len() > MAX_AGENT_TAB_ID {
                continue;
            }
            reconciled.tabs.push(AgentTab {
                id: tab.id.clone(),
                project,
                provider: source.provider(),
                title: clip(&tab.title),
                state: if report.busy.contains(&tab.id) {
                    AgentState::Working
                } else {
                    AgentState::Idle
                },
                shown_by: shown.get(tab.id.as_str()).copied(),
                index: u16::try_from(index).ok(),
                hosts: hosts.clone(),
                switch: source.switch_input(index),
                resume: source.resume_command(&tab.id),
            });
        }
    }
}

/// `title` cut at a character boundary to fit `MAX_AGENT_TITLE`.
fn clip(title: &str) -> String {
    let mut end = title.len().min(MAX_AGENT_TITLE);
    while !title.is_char_boundary(end) {
        end -= 1;
    }
    title[..end].to_owned()
}

fn claim<'a>(
    shown: &mut HashMap<&'a str, SessionId>,
    reconciled: &mut Reconciled,
    report: &Report,
    session: SessionId,
    tab: &'a Tab,
) {
    shown.insert(&tab.id, session);
    reconciled
        .shown
        .insert(session, (tab.id.clone(), clip(&tab.title)));
    if report.busy.contains(&tab.id) {
        reconciled.busy.insert(session);
    }
}

/// The terminals running agents with tabs, their last reports, and where
/// those put each terminal and tab.
pub(crate) struct Tracker {
    sources: Vec<Box<dyn TabSource>>,
    panes: BTreeMap<SessionId, Pane>,
    reports: HashMap<AgentProvider, Report>,
    reconciled: Reconciled,
    /// The terminals changed since the poller last read them.
    stale: bool,
}

impl Default for Tracker {
    fn default() -> Self {
        Self {
            sources: sources(),
            panes: BTreeMap::new(),
            reports: HashMap::new(),
            reconciled: Reconciled::default(),
            stale: false,
        }
    }
}

impl std::fmt::Debug for Tracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tracker")
            .field("panes", &self.panes)
            .field("reconciled", &self.reconciled)
            .finish_non_exhaustive()
    }
}

impl Tracker {
    pub(crate) fn reconciled(&self) -> &Reconciled {
        &self.reconciled
    }

    pub(crate) fn stale(&self) -> bool {
        self.stale
    }

    /// The terminals to report on, clearing the change since the last read.
    pub(crate) fn take_panes(&mut self) -> BTreeMap<SessionId, Pane> {
        self.stale = false;
        self.panes.clone()
    }

    /// Records what `session` shows; returns whether that changed.
    pub(crate) fn observe(&mut self, session: SessionId, pane: Pane) -> bool {
        if self.panes.get(&session) == Some(&pane) {
            return false;
        }
        self.panes.insert(session, pane);
        self.stale = true;
        true
    }

    /// Forgets `session`; returns whether it was known.
    pub(crate) fn forget(&mut self, session: SessionId) -> bool {
        let known = self.panes.remove(&session).is_some();
        self.stale |= known;
        known
    }

    pub(crate) fn set_report(&mut self, provider: AgentProvider, report: Report) {
        self.reports.insert(provider, report);
    }

    pub(crate) fn clear_busy(&mut self, provider: AgentProvider) {
        if let Some(report) = self.reports.get_mut(&provider) {
            report.busy.clear();
        }
    }

    /// Matches the terminals to the last reports; returns whether what
    /// clients see changed.
    pub(crate) fn reconcile(&mut self) -> bool {
        let next = reconcile(&self.sources, &self.panes, &self.reports, &self.reconciled);
        if next == self.reconciled {
            return false;
        }
        let shown = next.shown != self.reconciled.shown || next.tabs != self.reconciled.tabs;
        self.reconciled = next;
        shown
    }
}
