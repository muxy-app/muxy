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

pub(crate) trait TabSource: Send {
    fn provider(&self) -> AgentProvider;

    fn report(&mut self, directories: &HashSet<String>) -> Option<Report>;

    fn shown_title<'a>(&self, title: &'a str) -> Option<(&'a str, bool)>;

    fn switch_input(&self, index: usize) -> Option<Vec<u8>>;

    fn resume_command(&self, id: &str) -> Option<String>;
}

const PROVIDERS: [AgentProvider; 1] = [AgentProvider::OpenCode];

pub(crate) fn sources() -> Vec<Box<dyn TabSource>> {
    vec![Box::new(opencode::OpenCode::default())]
}

pub(crate) fn supported(provider: AgentProvider) -> bool {
    PROVIDERS.contains(&provider)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Pane {
    pub(crate) provider: AgentProvider,
    pub(crate) project: ProjectId,
    pub(crate) directory: String,
    pub(crate) title: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Tab {
    pub(crate) id: String,
    pub(crate) title: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Report {
    pub(crate) tabs: HashMap<String, Vec<Tab>>,
    pub(crate) busy: HashSet<String>,
    pub(crate) viewed: HashMap<String, u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Reconciled {
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

pub(crate) struct Tracker {
    sources: Vec<Box<dyn TabSource>>,
    panes: BTreeMap<SessionId, Pane>,
    reports: HashMap<AgentProvider, Report>,
    reconciled: Reconciled,
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

    pub(crate) fn take_panes(&mut self) -> BTreeMap<SessionId, Pane> {
        self.stale = false;
        self.panes.clone()
    }

    pub(crate) fn observe(&mut self, session: SessionId, pane: Pane) -> bool {
        if self.panes.get(&session) == Some(&pane) {
            return false;
        }
        self.panes.insert(session, pane);
        self.stale = true;
        true
    }

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
