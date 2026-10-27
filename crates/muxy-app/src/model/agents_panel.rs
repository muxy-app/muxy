use gpui::{AppContext, Context, FocusHandle, Pixels, Point, Window, px};
use muxy_app_core::settings::{AgentsPanelDock, AgentsPanelGrouping};
use muxy_app_core::{PaneContent, PaneId, ProjectId, ServerId, TabId};
use muxy_protocol::{ActivityKind, AgentProvider, AgentState, SessionId};
use muxy_ui::panel::{PanelId, PanelMode, PanelPlacement, PanelPosition, PanelResizeState};

use super::AppModel;
use crate::views::overlays::Overlay;

pub(crate) const PANEL: &str = "builtin:agents";
pub(crate) const MIN_WIDTH: f32 = 220.0;
pub(crate) const MIN_HEIGHT: f32 = 140.0;
const DEFAULT_FRAME: [f32; 4] = [80.0, 80.0, 320.0, 360.0];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum AgentStatus {
    Blocked,
    Working,
    Completed,
    Idle,
}

impl AgentStatus {
    pub(crate) const ALL: [Self; 4] = [Self::Blocked, Self::Working, Self::Completed, Self::Idle];
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
    Worktree(ProjectId),
}

impl std::fmt::Display for EntryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent(server, session) => write!(f, "agent-{server}-{}", session.get()),
            Self::Worktree(project) => write!(f, "worktree-{project}"),
        }
    }
}

impl Entry {
    pub(crate) fn key(&self) -> EntryKey {
        match self {
            Self::Agent(agent) => EntryKey::Agent(agent.server, agent.session),
            Self::Worktree { project, .. } => EntryKey::Worktree(*project),
        }
    }

    pub(crate) fn status(&self) -> AgentStatus {
        match self {
            Self::Agent(agent) => agent.status,
            Self::Worktree { counts, .. } => AgentStatus::ALL
                .into_iter()
                .zip(counts)
                .find(|(_, count)| **count > 0)
                .map_or(AgentStatus::Idle, |(status, _)| status),
        }
    }
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
        }
    }
}

impl AppModel {
    pub(crate) fn running_agents(&self) -> Vec<RunningAgent> {
        type Index<'a> = (
            std::collections::HashMap<SessionId, &'a muxy_protocol::AgentActivity>,
            std::collections::HashSet<SessionId>,
            &'a std::collections::HashMap<SessionId, u64>,
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
                            (agents, finished, &activity.finished)
                        });
                    servers.push((project.server_id, index));
                    servers.len() - 1
                });
            let Some((by_session, finished, finished_at)) = &servers[position].1 else {
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
                    });
                }
            }
        }
        agents.sort_by_key(|agent| agent.status);
        agents
    }

    pub(crate) fn agents_panel_entries(&self) -> Vec<Entry> {
        self.agents_panel_entries_from(self.running_agents())
    }

    pub(crate) fn agents_panel_entries_from(&self, agents: Vec<RunningAgent>) -> Vec<Entry> {
        if self.appearance.agents_panel_grouping == AgentsPanelGrouping::Agents {
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
        let mut entries: Vec<_> = entries
            .into_iter()
            .map(|(project, counts)| Entry::Worktree { project, counts })
            .collect();
        entries.sort_by_key(|entry| match entry {
            Entry::Worktree { counts, .. } => (entry.status(), std::cmp::Reverse(*counts)),
            Entry::Agent(_) => (entry.status(), std::cmp::Reverse([0; 4])),
        });
        entries
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
        if self.close_prompt.is_some() {
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
                    self.overlay = Some(Overlay::Agents);
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
        let project = self.state.current_project();
        let tab = self.active_tab();
        let session = self.active_pane().and_then(|pane| self.pane_session(pane));
        entries
            .iter()
            .find(|entry| match entry {
                Entry::Agent(agent) => {
                    agent.server == project.server_id && Some(agent.session) == session
                }
                Entry::Worktree { project: id, .. } => *id == project.id,
            })
            .or_else(|| {
                let tab = project.tabs.iter().find(|entry| Some(entry.id) == tab)?;
                entries.iter().find(|entry| match entry {
                    Entry::Agent(agent) => {
                        agent.server == project.server_id
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
        let headings = 1 + entries[..=next]
            .windows(2)
            .filter(|pair| pair[0].status() != pair[1].status())
            .count();
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
        if matches!(self.overlay, Some(Overlay::Agents)) {
            self.dismiss_overlay(cx);
        }
        match entry {
            Entry::Agent(agent) => self.focus_agent(agent.project, agent.tab, agent.pane, cx),
            Entry::Worktree { project, .. } => self.select_project(project, cx),
        }
        self.focus_active(window, cx);
        cx.notify();
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
        self.agents_panel.focus.focus(window);
        self.focus_requested = false;
    }

    pub(crate) fn agents_panel_frame(&self, viewport: gpui::Size<Pixels>) -> [f32; 4] {
        let scale = f32::from(self.metrics.scaled(1.0));
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
        let scale = f32::from(self.metrics.scaled(1.0));
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
        let scale = f32::from(self.metrics.scaled(1.0));
        let max = (f32::from(viewport.height) / scale * 0.7).max(MIN_HEIGHT);
        height.clamp(MIN_HEIGHT, max)
    }

    pub(crate) fn begin_agents_panel_docked_drag(
        &mut self,
        pointer: Pixels,
        height: Pixels,
        cx: &mut Context<Self>,
    ) {
        let scale = f32::from(self.metrics.scaled(1.0));
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
