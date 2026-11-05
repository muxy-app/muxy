use gpui::{AppContext, Context, FocusHandle, Pixels, Point, Window, px};
use muxy_app_core::settings::{AgentsPanelDensity, AgentsPanelDock, AgentsPanelGrouping};
use muxy_app_core::{PaneContent, PaneId, ProjectId, ServerId, TabId};
use muxy_protocol::{ActivityKind, AgentProvider, AgentState, SessionId};
use muxy_ui::panel::{PanelId, PanelMode, PanelPlacement, PanelPosition, PanelResizeState};

use super::AppModel;
use crate::views::overlays::Overlay;

pub(crate) const PANEL: &str = "builtin:agents";
pub(crate) const MIN_WIDTH: f32 = 220.0;
pub(crate) const MIN_HEIGHT: f32 = 140.0;
const DEFAULT_FRAME: [f32; 4] = [80.0, 80.0, 320.0, 360.0];
/// How long a background tab stays the active row while its host terminal
/// switches to it, in case the switch never shows.
const SWITCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum AgentStatus {
    Blocked,
    Working,
    Completed,
    Idle,
}

impl AgentStatus {
    pub(crate) const ALL: [Self; 4] = [Self::Blocked, Self::Working, Self::Completed, Self::Idle];

    /// How early a row sorts. Completed and idle agents rank together, so
    /// opening a finished agent, which reads its finish, doesn't move it.
    pub(crate) fn urgency(self) -> u8 {
        match self {
            Self::Blocked => 0,
            Self::Working => 1,
            Self::Completed | Self::Idle => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RunningAgent {
    pub(crate) server: ServerId,
    pub(crate) session: SessionId,
    pub(crate) project: ProjectId,
    pub(crate) tab: TabId,
    pub(crate) pane: PaneId,
    pub(crate) provider: AgentProvider,
    pub(crate) status: AgentStatus,
    pub(crate) finished_at: Option<u64>,
    /// Set for a tab the agent in `session` keeps open but doesn't show.
    pub(crate) hidden: Option<HiddenTab>,
    /// Where the row goes among rows of the same project and status: its
    /// terminal's place, then its tab's place in the agent's tab bar, so
    /// switching tabs doesn't reorder them.
    pub(crate) order: (usize, u16),
}

/// A tab inside an agent's own tab bar, behind the one its terminal shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HiddenTab {
    /// A hash of the agent's ID for the session, which the snapshot keeps.
    pub(crate) key: u64,
    /// Its 0-based position in the agent's tab bar.
    pub(crate) index: Option<u16>,
}

pub(crate) fn hidden_key(id: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut hasher);
    hasher.finish()
}

/// Places each pane's row at the tab it shows, then adds rows for the tabs
/// no pane here shows.
fn add_agent_tabs(
    agents: &mut Vec<RunningAgent>,
    servers: &[(ServerId, &[muxy_protocol::AgentTab])],
) {
    for (server, tabs) in servers {
        for tab in *tabs {
            if let (Some(session), Some(position)) = (tab.shown_by, tab.index)
                && let Some(agent) = agents
                    .iter_mut()
                    .find(|agent| agent.server == *server && agent.session == session)
            {
                agent.order.1 = position;
            }
        }
    }
    let shown = agents.clone();
    for (server, tabs) in servers {
        agents.extend(
            tabs.iter()
                .filter_map(|tab| hidden_tab(tab, *server, &shown)),
        );
    }
}

/// A row for `tab` under the first of its hosts this window shows, unless
/// a pane here already shows it. A tab shown only by a terminal no pane here
/// shows, such as one in a closed tab, moves under another host at once.
fn hidden_tab(
    tab: &muxy_protocol::AgentTab,
    server: ServerId,
    shown: &[RunningAgent],
) -> Option<RunningAgent> {
    let on_screen = |session: SessionId| {
        shown
            .iter()
            .find(|agent| agent.server == server && agent.session == session)
    };
    if tab
        .shown_by
        .is_some_and(|session| on_screen(session).is_some())
    {
        return None;
    }
    let host = tab.hosts.iter().find_map(|session| on_screen(*session))?;
    Some(RunningAgent {
        provider: tab.provider,
        status: if tab.state == AgentState::Working {
            AgentStatus::Working
        } else {
            AgentStatus::Idle
        },
        finished_at: None,
        hidden: Some(HiddenTab {
            key: hidden_key(&tab.id),
            index: tab.index,
        }),
        order: (host.order.0, tab.index.unwrap_or(u16::MAX)),
        ..*host
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Entry {
    Agent(RunningAgent),
    Worktree {
        project: ProjectId,
        counts: [usize; 4],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EntryKey {
    Agent(ServerId, SessionId),
    Hidden(ServerId, u64),
    Worktree(ProjectId),
}

impl std::fmt::Display for EntryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent(server, session) => write!(f, "agent-{server}-{}", session.get()),
            Self::Hidden(server, key) => write!(f, "tab-{server}-{key:x}"),
            Self::Worktree(project) => write!(f, "worktree-{project}"),
        }
    }
}

impl RunningAgent {
    pub(crate) fn key(&self) -> EntryKey {
        match self.hidden {
            Some(hidden) => EntryKey::Hidden(self.server, hidden.key),
            None => EntryKey::Agent(self.server, self.session),
        }
    }
}

impl Entry {
    pub(crate) fn key(&self) -> EntryKey {
        match self {
            Self::Agent(agent) => agent.key(),
            Self::Worktree { project, .. } => EntryKey::Worktree(*project),
        }
    }
}

/// Whether the agent at `index` is the first listed for its project, so a
/// normal-density panel puts a project heading above it. Project rows have
/// no headings.
pub(crate) fn starts_project_group(entries: &[Entry], index: usize) -> bool {
    let Entry::Agent(agent) = &entries[index] else {
        return false;
    };
    index == 0
        || !matches!(&entries[index - 1], Entry::Agent(previous) if previous.project == agent.project)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Placement {
    Docked,
    Right,
    Popup,
    Floating,
}

#[derive(Clone, Copy)]
pub(crate) enum Drag {
    Move {
        pointer: Point<Pixels>,
        origin: [f32; 2],
    },
    Resize {
        pointer: Point<Pixels>,
        size: [f32; 2],
        axes: [bool; 2],
    },
    Docked {
        pointer: Pixels,
        height: f32,
    },
    Scroll {
        pointer: Pixels,
        offset: Pixels,
        ratio: f32,
    },
}

pub(crate) struct AgentsPanel {
    pub(crate) focus: FocusHandle,
    pub(crate) selected: Option<EntryKey>,
    pub(crate) drag: Option<Drag>,
    pub(crate) scroll: gpui::ScrollHandle,
    pub(crate) resize: PanelResizeState,
    pub(crate) docked_height: std::rc::Rc<std::cell::Cell<Pixels>>,
    pub(crate) owner: gpui::Entity<()>,
    pub(crate) clock: Option<gpui::Task<()>>,
    /// Keys that switch a host terminal to a background tab once it attaches,
    /// after opening the tab from another project.
    pub(crate) pending_switch: Option<(PaneId, [u8; 2])>,
    /// The background tab a host terminal is switching to, which stays the
    /// active row until the terminal shows it, rather than the tab it leaves.
    pub(crate) switching: Option<(EntryKey, std::time::Instant)>,
}

impl AgentsPanel {
    pub(crate) fn new(cx: &mut Context<AppModel>) -> Self {
        Self {
            focus: cx.focus_handle(),
            selected: None,
            drag: None,
            scroll: gpui::ScrollHandle::new(),
            resize: PanelResizeState::default(),
            docked_height: std::rc::Rc::default(),
            owner: cx.new(|_| ()),
            clock: None,
            pending_switch: None,
            switching: None,
        }
    }
}

impl AppModel {
    pub(crate) fn running_agents(&self) -> Vec<RunningAgent> {
        type Index<'a> = (
            std::collections::HashMap<SessionId, &'a muxy_protocol::AgentActivity>,
            std::collections::HashSet<SessionId>,
            &'a std::collections::HashMap<SessionId, u64>,
            &'a [muxy_protocol::AgentTab],
        );
        let mut servers: Vec<(ServerId, Option<Index<'_>>)> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut agents: Vec<RunningAgent> = Vec::new();
        for project in self.state.projects() {
            let position = servers
                .iter()
                .position(|(server, _)| *server == project.server_id)
                .unwrap_or_else(|| {
                    let index = self
                        .activity(project.server_id)
                        .filter(|activity| !activity.snapshot.agents.is_empty())
                        .map(|activity| {
                            let snapshot = &activity.snapshot;
                            let agents = snapshot
                                .agents
                                .iter()
                                .rev()
                                .map(|agent| (agent.session, agent))
                                .collect();
                            let finished = snapshot
                                .events
                                .iter()
                                .filter(|event| {
                                    event.kind == ActivityKind::Completed && !event.read
                                })
                                .map(|event| event.session)
                                .collect();
                            (
                                agents,
                                finished,
                                &activity.finished,
                                snapshot.tabs.as_slice(),
                            )
                        });
                    servers.push((project.server_id, index));
                    servers.len() - 1
                });
            let Some((by_session, finished, finished_at, _)) = &servers[position].1 else {
                continue;
            };
            for tab in &project.tabs {
                for pane in &tab.panes {
                    let PaneContent::Terminal {
                        session: Some(session),
                    } = pane.content
                    else {
                        continue;
                    };
                    let Some(agent) = by_session.get(&session) else {
                        continue;
                    };
                    if !seen.insert((project.server_id, session)) {
                        continue;
                    }
                    let status = match agent.state {
                        AgentState::Blocked => AgentStatus::Blocked,
                        AgentState::Working => AgentStatus::Working,
                        _ if finished.contains(&session) => AgentStatus::Completed,
                        _ => AgentStatus::Idle,
                    };
                    let finished_at = finished_at
                        .get(&session)
                        .copied()
                        .filter(|_| matches!(status, AgentStatus::Completed | AgentStatus::Idle));
                    agents.push(RunningAgent {
                        server: project.server_id,
                        session,
                        project: project.id,
                        tab: tab.id,
                        pane: pane.id,
                        provider: agent.provider,
                        status,
                        finished_at,
                        hidden: None,
                        order: (agents.len(), 0),
                    });
                }
            }
        }
        let tabs: Vec<_> = servers
            .iter()
            .filter_map(|(server, index)| index.as_ref().map(|(_, _, _, tabs)| (*server, *tabs)))
            .collect();
        add_agent_tabs(&mut agents, &tabs);
        agents.sort_by_key(|agent| (agent.status.urgency(), agent.order));
        agents
    }

    pub(crate) fn agents_panel_entries(&self) -> Vec<Entry> {
        self.agents_panel_entries_from(self.running_agents())
    }

    /// What a row calls an agent's work: the name the user gave its tab,
    /// else the session title the agent reports, else its terminal title
    /// without the `OC | ` prefix `OpenCode` adds, else the agent's name.
    pub(crate) fn agent_title(&self, agent: &RunningAgent) -> String {
        let snapshot = self
            .activity(agent.server)
            .map(|activity| &activity.snapshot);
        let usable = |title: &str| Some(title.trim().to_owned()).filter(|title| !title.is_empty());
        if let Some(hidden) = agent.hidden {
            return snapshot
                .and_then(|snapshot| {
                    snapshot
                        .tabs
                        .iter()
                        .find(|tab| hidden_key(&tab.id) == hidden.key)
                })
                .and_then(|tab| usable(&tab.title))
                .unwrap_or_else(|| agent.provider.name().to_owned());
        }
        let tab = self
            .state
            .project(agent.project)
            .and_then(|project| project.tabs.iter().find(|tab| tab.id == agent.tab));
        tab.and_then(|tab| tab.custom_title.as_deref().and_then(usable))
            .or_else(|| {
                snapshot
                    .and_then(|snapshot| {
                        snapshot
                            .agents
                            .iter()
                            .find(|entry| entry.session == agent.session)
                    })
                    .and_then(|entry| entry.title.as_deref().and_then(usable))
            })
            .or_else(|| {
                let title = tab?.title(Some(agent.pane)).trim();
                usable(title.strip_prefix("OC | ").unwrap_or(title))
            })
            .unwrap_or_else(|| agent.provider.name().to_owned())
    }

    /// Agents grouped by project in sidebar order, most urgent first within a
    /// project; or one row per project running agents.
    pub(crate) fn agents_panel_entries_from(&self, mut agents: Vec<RunningAgent>) -> Vec<Entry> {
        let order = self.agents_project_order();
        let rank = |project: &ProjectId| order.get(project).copied().unwrap_or(usize::MAX);
        if self.appearance.agents_panel_grouping == AgentsPanelGrouping::Agents {
            agents.sort_by_key(|agent| (rank(&agent.project), agent.status.urgency(), agent.order));
            return agents.into_iter().map(Entry::Agent).collect();
        }
        let mut entries: Vec<(ProjectId, [usize; 4])> = Vec::new();
        for agent in agents {
            let index = AgentStatus::ALL
                .iter()
                .position(|status| *status == agent.status)
                .unwrap_or(3);
            if let Some((_, counts)) = entries.iter_mut().find(|(id, _)| *id == agent.project) {
                counts[index] += 1;
            } else {
                let mut counts = [0; 4];
                counts[index] = 1;
                entries.push((agent.project, counts));
            }
        }
        entries.sort_by_key(|(project, _)| rank(project));
        entries
            .into_iter()
            .map(|(project, counts)| Entry::Worktree { project, counts })
            .collect()
    }

    /// Each project's position in the sidebar, with worktrees after their project.
    fn agents_project_order(&self) -> std::collections::HashMap<ProjectId, usize> {
        let mut parents: Vec<_> = self
            .state
            .projects()
            .iter()
            .filter(|project| project.parent_id.is_none())
            .collect();
        self.sort_parents(&mut parents);
        parents
            .into_iter()
            .flat_map(|parent| std::iter::once(parent).chain(self.worktree_children(parent.id)))
            .enumerate()
            .map(|(index, project)| (project.id, index))
            .collect()
    }

    pub(crate) fn agents_panel_placement(&self) -> Placement {
        if self.appearance.agents_panel_detached {
            Placement::Floating
        } else if self.appearance.agents_panel_dock == AgentsPanelDock::Right {
            Placement::Right
        } else if self.appearance.sidebar_expanded && !self.extension_sidebar_active() {
            Placement::Docked
        } else {
            Placement::Popup
        }
    }

    pub(crate) fn agents_panel_shown(&self) -> bool {
        match self.agents_panel_placement() {
            Placement::Popup => matches!(self.overlay, Some(Overlay::Agents)),
            Placement::Right => {
                self.appearance.agents_panel_visible
                    && self.panels.placement(&PanelId::new(PANEL)).is_some()
            }
            Placement::Docked | Placement::Floating => self.appearance.agents_panel_visible,
        }
    }

    pub(crate) fn toggle_agents_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialogs.is_open() {
            return;
        }
        let focused = self.agents_panel.focus.contains_focused(window, cx);
        if self.agents_panel_shown() && focused {
            self.hide_agents_panel(window, cx);
            return;
        }
        if !self.appearance.agents_panel_visible {
            self.appearance.agents_panel_visible = true;
            self.save_appearance(cx);
        }
        self.sync_agents_panel(cx);
        match self.agents_panel_placement() {
            Placement::Popup => {
                if !matches!(self.overlay, Some(Overlay::Agents)) {
                    self.dismiss_overlay(cx);
                    self.open_overlay(Overlay::Agents, cx);
                }
            }
            Placement::Docked | Placement::Right | Placement::Floating => {
                if matches!(self.overlay, Some(Overlay::Agents)) {
                    self.dismiss_overlay(cx);
                }
            }
        }
        self.focus_agents_panel(window);
        cx.notify();
    }

    pub(crate) fn hide_agents_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::Agents)) {
            self.dismiss_overlay(cx);
        }
        if self.appearance.agents_panel_visible {
            self.appearance.agents_panel_visible = false;
            self.save_appearance(cx);
        }
        self.agents_panel.drag = None;
        self.sync_agents_panel(cx);
        self.focus_active(window, cx);
        cx.notify();
    }

    pub(crate) fn close_agents_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.overlay, Some(Overlay::Agents)) {
            self.dismiss_overlay(cx);
        }
        self.focus_active(window, cx);
        cx.notify();
    }

    pub(crate) fn active_agents_entry(&self, entries: &[Entry]) -> Option<EntryKey> {
        if let Some((key, since)) = self.agents_panel.switching
            && since.elapsed() < SWITCH_TIMEOUT
            && entries.iter().any(|entry| {
                matches!(entry, Entry::Agent(agent)
                    if agent.key() == key && self.active_pane() == Some(agent.pane))
            })
        {
            return Some(key);
        }
        let project = self.state.current_project();
        let tab = self.active_tab();
        let session = self
            .active_pane()
            .and_then(|pane| self.state.pane_session(pane));
        entries
            .iter()
            .find(|entry| match entry {
                Entry::Agent(agent) => {
                    agent.hidden.is_none() && session == Some((agent.server, agent.session))
                }
                Entry::Worktree { project: id, .. } => *id == project.id,
            })
            .or_else(|| {
                let tab = project.tabs.iter().find(|entry| Some(entry.id) == tab)?;
                entries.iter().find(|entry| match entry {
                    Entry::Agent(agent) => {
                        agent.hidden.is_none()
                            && agent.server == project.server_id
                            && tab.panes.iter().any(|pane| {
                                matches!(pane.content, PaneContent::Terminal { session: Some(session) } if session == agent.session)
                            })
                    }
                    Entry::Worktree { .. } => false,
                })
            })
            .map(Entry::key)
    }

    pub(crate) fn selected_agents_entry(&self, entries: &[Entry]) -> Option<EntryKey> {
        self.agents_panel
            .selected
            .filter(|key| entries.iter().any(|entry| entry.key() == *key))
            .or_else(|| self.active_agents_entry(entries))
            .or_else(|| entries.first().map(Entry::key))
    }

    pub(crate) fn move_agents_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let entries = self.agents_panel_entries();
        if entries.is_empty() {
            self.agents_panel.selected = None;
            return;
        }
        let current = self
            .selected_agents_entry(&entries)
            .and_then(|key| entries.iter().position(|entry| entry.key() == key))
            .unwrap_or(0);
        let next = current.saturating_add_signed(delta).min(entries.len() - 1);
        self.agents_panel.selected = Some(entries[next].key());
        let headings = if self.appearance.agents_panel_density == AgentsPanelDensity::Normal {
            (0..=next)
                .filter(|index| starts_project_group(&entries, *index))
                .count()
        } else {
            0
        };
        self.agents_panel.scroll.scroll_to_item(next + headings);
        cx.notify();
    }

    pub(crate) fn open_agents_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.selected_agents_entry(&self.agents_panel_entries()) else {
            return;
        };
        self.open_agents_entry(key, window, cx);
    }

    pub(crate) fn open_agents_entry(
        &mut self,
        key: EntryKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self
            .agents_panel_entries()
            .into_iter()
            .find(|entry| entry.key() == key)
        else {
            return;
        };
        self.agents_panel.selected = None;
        self.agents_panel.switching = None;
        if matches!(self.overlay, Some(Overlay::Agents)) {
            self.dismiss_overlay(cx);
        }
        match entry {
            Entry::Agent(agent) => match agent.hidden {
                Some(hidden) => self.open_hidden_agent_tab(agent, hidden, cx),
                None => self.focus_agent(agent.project, agent.tab, agent.pane, cx),
            },
            Entry::Worktree { project, .. } => self.select_project(project, cx),
        }
        self.focus_active(window, cx);
        cx.notify();
    }

    /// Shows a tab the agent keeps behind the one on screen. Its host
    /// terminal switches to it with the agent's own shortcut when one reaches
    /// it, `<leader>1`…`<leader>0` with the default `ctrl+x` leader of `OpenCode`;
    /// otherwise a new terminal tab resumes it.
    fn open_hidden_agent_tab(
        &mut self,
        agent: RunningAgent,
        hidden: HiddenTab,
        cx: &mut Context<Self>,
    ) {
        let Some((id, title)) = self
            .activity(agent.server)
            .and_then(|activity| {
                activity
                    .snapshot
                    .tabs
                    .iter()
                    .find(|tab| hidden_key(&tab.id) == hidden.key)
            })
            .map(|tab| (tab.id.clone(), tab.title.clone()))
        else {
            return;
        };
        let shortcut = hidden
            .index
            .filter(|index| *index < 10)
            .map(|index| [0x18, b'0' + u8::try_from((index + 1) % 10).unwrap_or(0)]);
        let channel = self
            .terminal(&agent.pane)
            .and_then(|pane| pane.view.read(cx).channel());
        if let Some(keys) = shortcut
            && agent.provider == AgentProvider::OpenCode
        {
            self.focus_agent(agent.project, agent.tab, agent.pane, cx);
            self.agents_panel.switching = Some((agent.key(), std::time::Instant::now()));
            cx.spawn(async move |model, cx| {
                cx.background_executor().timer(SWITCH_TIMEOUT).await;
                let _ = model.update(cx, |_, cx| cx.notify());
            })
            .detach();
            match channel {
                Some(channel) => {
                    self.send(
                        agent.server,
                        crate::boot::Work::Input(channel, keys.to_vec()),
                        cx,
                    );
                }
                None => self.agents_panel.pending_switch = Some((agent.pane, keys)),
            }
            return;
        }
        if agent.provider != AgentProvider::OpenCode
            || !muxy_app_core::agent_sessions::valid_id(&id)
        {
            return;
        }
        let command = format!("opencode -s {id}");
        let opened = self.open_terminal(
            super::tabs::OpenTerminal {
                project: agent.project,
                title: Some(&title),
                command: Some(&command),
                directory: None,
            },
            cx,
        );
        if let Err(error) = opened {
            self.fail(error, cx);
        }
    }

    pub(crate) fn focus_agent(
        &mut self,
        project: ProjectId,
        tab: TabId,
        pane: PaneId,
        cx: &mut Context<Self>,
    ) {
        let present = self.state.project(project).is_some_and(|project| {
            project
                .tabs
                .iter()
                .any(|entry| entry.id == tab && entry.panes.iter().any(|entry| entry.id == pane))
        });
        if !present {
            return;
        }
        self.select_project(project, cx);
        self.select_tab(tab, cx);
        self.focus_pane(pane, cx);
    }

    pub(crate) fn sync_agents_panel(&mut self, cx: &mut Context<Self>) {
        let placement = self.agents_panel_placement();
        if (placement != Placement::Popup || !self.appearance.agents_panel_visible)
            && matches!(self.overlay, Some(Overlay::Agents))
        {
            self.dismiss_overlay(cx);
        }
        let id = PanelId::new(PANEL);
        if self.appearance.agents_panel_visible && placement == Placement::Right {
            if self.panels.placement(&id).is_none() {
                self.place_panel(Self::agents_right_placement(), cx);
            }
        } else {
            self.panels.remove(&id);
            self.agents_panel.resize.end();
        }
        self.sync_agents_panel_clock(cx);
    }

    fn sync_agents_panel_clock(&mut self, cx: &mut Context<Self>) {
        if !(self.appearance.agents_panel_visible && self.appearance.agents_panel_idle_time) {
            self.agents_panel.clock = None;
            return;
        }
        if self.agents_panel.clock.is_some() {
            return;
        }
        self.agents_panel.clock = Some(cx.spawn(async move |model, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(60))
                    .await;
                let tick = model.update(cx, |model, cx| {
                    if model
                        .running_agents()
                        .iter()
                        .any(|agent| agent.finished_at.is_some())
                    {
                        cx.notify();
                    }
                });
                if tick.is_err() {
                    break;
                }
            }
        }));
    }

    pub(crate) fn agents_right_placement() -> PanelPlacement {
        PanelPlacement::new(PANEL, PanelPosition::Right, PanelMode::Pinned)
    }

    pub(crate) fn set_agents_panel_dock(
        &mut self,
        dock: AgentsPanelDock,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.appearance.agents_panel_dock = dock;
        self.set_agents_panel_detached(false, window, cx);
    }

    pub(crate) fn set_agents_panel_detached(
        &mut self,
        detached: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.overlay, Some(Overlay::Agents)) {
            self.dismiss_overlay(cx);
        }
        self.appearance.agents_panel_detached = detached;
        self.appearance.agents_panel_visible = true;
        if detached && self.appearance.agents_panel_frame.is_none() {
            self.appearance.agents_panel_frame = Some(DEFAULT_FRAME);
        }
        self.agents_panel.drag = None;
        self.agents_panel.resize.end();
        self.save_appearance(cx);
        self.sync_agents_panel(cx);
        if self.agents_panel_placement() == Placement::Popup {
            self.focus_active(window, cx);
        } else {
            self.focus_agents_panel(window);
        }
        cx.notify();
    }

    pub(crate) fn focus_agents_panel(&mut self, window: &mut Window) {
        self.agents_panel.selected = None;
        self.focus_owner.place(&self.agents_panel.focus, window);
    }

    pub(crate) fn agents_panel_frame(&self, viewport: gpui::Size<Pixels>) -> [f32; 4] {
        let scale = self.metrics.multiplier();
        let [x, y, width, height] = self
            .appearance
            .agents_panel_frame
            .filter(|frame| frame.iter().all(|value| value.is_finite()))
            .unwrap_or(DEFAULT_FRAME);
        let max_width = (f32::from(viewport.width) / scale).max(MIN_WIDTH);
        let max_height = (f32::from(viewport.height) / scale).max(MIN_HEIGHT);
        let width = width.clamp(MIN_WIDTH, max_width);
        let height = height.clamp(MIN_HEIGHT, max_height);
        [
            x.clamp(0.0, max_width - width),
            y.clamp(0.0, max_height - height),
            width,
            height,
        ]
    }

    pub(crate) fn begin_agents_panel_drag(
        &mut self,
        resize: Option<[bool; 2]>,
        pointer: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let [x, y, width, height] = self.agents_panel_frame(window.viewport_size());
        self.agents_panel.drag = Some(match resize {
            Some(axes) => Drag::Resize {
                pointer,
                size: [width, height],
                axes,
            },
            None => Drag::Move {
                pointer,
                origin: [x, y],
            },
        });
        cx.notify();
    }

    pub(crate) fn move_agents_panel_drag(
        &mut self,
        pointer: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.agents_panel.drag else {
            return;
        };
        let scale = self.metrics.multiplier();
        match drag {
            Drag::Docked {
                pointer: from,
                height,
            } => {
                let height = height - f32::from(pointer.y - from) / scale;
                self.appearance.agents_panel_docked_height =
                    Some(self.clamp_docked_height(height, window.viewport_size()));
                cx.notify();
                return;
            }
            Drag::Scroll {
                pointer: from,
                offset,
                ratio,
            } => {
                let scroll = &self.agents_panel.scroll;
                let max = scroll.max_offset().height;
                let target = (offset + (pointer.y - from) * ratio).clamp(px(0.0), max);
                scroll.set_offset(gpui::point(scroll.offset().x, -target));
                cx.notify();
                return;
            }
            Drag::Move { .. } | Drag::Resize { .. } => {}
        }
        let [x, y, width, height] = self.agents_panel_frame(window.viewport_size());
        let offset = |from: Point<Pixels>| {
            [
                f32::from(pointer.x - from.x) / scale,
                f32::from(pointer.y - from.y) / scale,
            ]
        };
        let frame = match drag {
            Drag::Move { pointer, origin } => {
                let [dx, dy] = offset(pointer);
                [origin[0] + dx, origin[1] + dy, width, height]
            }
            Drag::Resize {
                pointer,
                size,
                axes,
            } => {
                let [dx, dy] = offset(pointer);
                [
                    x,
                    y,
                    if axes[0] { size[0] + dx } else { width },
                    if axes[1] { size[1] + dy } else { height },
                ]
            }
            Drag::Docked { .. } | Drag::Scroll { .. } => return,
        };
        self.appearance.agents_panel_frame = Some(frame);
        let clamped = self.agents_panel_frame(window.viewport_size());
        self.appearance.agents_panel_frame = Some(clamped);
        cx.notify();
    }

    pub(crate) fn finish_agents_panel_drag(&mut self, cx: &mut Context<Self>) {
        if self.agents_panel.drag.take().is_none() {
            return;
        }
        if self.appearance.agents_panel_frame != self.settings.appearance.agents_panel_frame
            || self.appearance.agents_panel_docked_height
                != self.settings.appearance.agents_panel_docked_height
        {
            self.save_appearance(cx);
        }
        cx.notify();
    }

    pub(crate) fn clamp_docked_height(&self, height: f32, viewport: gpui::Size<Pixels>) -> f32 {
        let scale = self.metrics.multiplier();
        let max = (f32::from(viewport.height) / scale * 0.7).max(MIN_HEIGHT);
        height.clamp(MIN_HEIGHT, max)
    }

    pub(crate) fn begin_agents_panel_docked_drag(
        &mut self,
        pointer: Pixels,
        height: Pixels,
        cx: &mut Context<Self>,
    ) {
        let scale = self.metrics.multiplier();
        self.agents_panel.drag = Some(Drag::Docked {
            pointer,
            height: f32::from(height) / scale,
        });
        cx.notify();
    }

    pub(crate) fn begin_agents_panel_scroll_drag(
        &mut self,
        pointer: Pixels,
        ratio: f32,
        cx: &mut Context<Self>,
    ) {
        self.agents_panel.drag = Some(Drag::Scroll {
            pointer,
            offset: -self.agents_panel.scroll.offset().y,
            ratio,
        });
        cx.notify();
    }
}
