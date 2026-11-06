//! Tabs and panes move between tabs, projects and windows. A tab can leave the
//! main window for a window of its own: it stays among its project's tabs, so
//! its panes, sessions and server keep working, but the main window no longer
//! lists it until it returns.

use muxy_protocol::{SessionId, SessionInfo};
use serde::{Deserialize, Serialize};

use crate::{
    AppError, AppState, Direction, Layout, PaneContent, PaneId, Project, ProjectId, ServerId, Tab,
    TabId, TabWindowId, WindowBounds,
};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabWindow {
    pub id: TabWindowId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<WindowBounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_pane: Option<PaneId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    focus_history: Vec<PaneId>,
}

impl TabWindow {
    fn new(bounds: Option<WindowBounds>, active_pane: Option<PaneId>) -> Self {
        Self {
            id: TabWindowId::new(),
            bounds,
            active_pane,
            focus_history: active_pane.into_iter().collect(),
        }
    }

    pub fn active_pane(&self) -> Option<PaneId> {
        self.active_pane
    }

    pub(crate) fn activate(&mut self, pane: Option<PaneId>) {
        self.active_pane = pane;
        if let Some(pane) = pane {
            self.focus_history.retain(|previous| *previous != pane);
            self.focus_history.push(pane);
        }
    }

    pub(crate) fn focus_history(&self) -> &[PaneId] {
        &self.focus_history
    }

    pub(crate) fn forget(&mut self, pane: PaneId) {
        self.focus_history.retain(|previous| *previous != pane);
    }

    pub(crate) fn replace(&mut self, pane: PaneId, new: PaneId) {
        for previous in &mut self.focus_history {
            if *previous == pane {
                *previous = new;
            }
        }
        if self.active_pane == Some(pane) {
            self.active_pane = Some(new);
        }
    }

    /// Keeps only what still names one of `layout`'s panes.
    pub(crate) fn retain_in(&mut self, layout: &Layout) {
        self.focus_history.retain(|pane| layout.contains(*pane));
        if self.active_pane.is_some_and(|pane| !layout.contains(pane)) {
            self.active_pane = None;
        }
        if self.active_pane.is_none() {
            self.active_pane = self
                .focus_history
                .last()
                .copied()
                .or_else(|| layout.leaves().first().copied());
        }
    }
}

/// A terminal whose tab or pane moved to another project. Its server must
/// hear of it, so the session keeps belonging to where it shows.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionMove {
    pub server: ServerId,
    pub session: SessionId,
    pub project: ProjectId,
}

/// Where a tab goes among a project's listed tabs: before `before`, or after
/// the last one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TabPlace {
    pub project: ProjectId,
    pub before: Option<TabId>,
}

impl TabPlace {
    pub fn end(project: ProjectId) -> Self {
        Self {
            project,
            before: None,
        }
    }
}

/// Where a pane from another tab lands in a tab's splits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneDrop {
    /// Takes `anchor`'s place, which takes the dropped pane's place.
    Swap(PaneId),
    /// Beside `anchor`, or the enclosing row or column `level` picks.
    Dock {
        anchor: PaneId,
        edge: Direction,
        level: usize,
    },
}

impl PaneDrop {
    pub fn anchor(self) -> PaneId {
        match self {
            Self::Swap(anchor) | Self::Dock { anchor, .. } => anchor,
        }
    }
}

impl Project {
    /// The tabs the main window lists: those not in a window of their own.
    pub fn listed_tabs(&self) -> impl Iterator<Item = &Tab> {
        self.tabs.iter().filter(|tab| !tab.detached())
    }
}

fn not_movable() -> AppError {
    AppError::InvalidState("tabs and panes move only between projects on the same computer".into())
}

impl AppState {
    /// Every tab in a window of its own, with its project.
    pub fn tab_windows(&self) -> impl Iterator<Item = (&Project, &Tab, &TabWindow)> {
        self.projects.iter().flat_map(|project| {
            project
                .tabs
                .iter()
                .filter_map(move |tab| Some((project, tab, tab.window.as_ref()?)))
        })
    }

    pub fn tab_window(&self, id: TabWindowId) -> Option<(&Project, &Tab, &TabWindow)> {
        self.tab_windows().find(|(_, _, window)| window.id == id)
    }

    /// The window of its own that shows `pane`, if any.
    pub fn pane_window(&self, pane: PaneId) -> Option<TabWindowId> {
        self.tab_windows()
            .find(|(_, tab, _)| tab.layout.contains(pane))
            .map(|(_, _, window)| window.id)
    }

    /// Whether `tab` and `pane` may move into `project`: only within one
    /// computer, since a terminal can't change servers.
    pub fn can_move_into(&self, source: ProjectId, project: ProjectId) -> bool {
        match (self.project(source), self.project(project)) {
            (Some(source), Some(project)) => source.server_id == project.server_id,
            _ => false,
        }
    }

    fn tab_project(&self, tab: TabId) -> Result<ProjectId, AppError> {
        self.projects
            .iter()
            .find(|project| project.tabs.iter().any(|candidate| candidate.id == tab))
            .map(|project| project.id)
            .ok_or_else(|| AppError::InvalidState("unknown tab".into()))
    }

    fn pane_project_id(&self, pane: PaneId) -> Result<(ProjectId, TabId), AppError> {
        self.projects
            .iter()
            .find_map(|project| {
                project
                    .tabs
                    .iter()
                    .find(|tab| tab.layout.contains(pane))
                    .map(|tab| (project.id, tab.id))
            })
            .ok_or(AppError::UnknownPane(pane))
    }

    fn check_move(&self, source: ProjectId, target: ProjectId) -> Result<(), AppError> {
        let from = self
            .project(source)
            .ok_or(AppError::UnknownProject(source))?;
        let to = self
            .project(target)
            .ok_or(AppError::UnknownProject(target))?;
        from.require_available()?;
        to.require_available()?;
        if from.server_id == to.server_id {
            Ok(())
        } else {
            Err(not_movable())
        }
    }

    /// Refuses a place before a tab the project doesn't list.
    fn check_place(&self, place: TabPlace) -> Result<(), AppError> {
        if place.before.is_some_and(|before| {
            self.project(place.project).is_none_or(|project| {
                !project
                    .listed_tabs()
                    .any(|candidate| candidate.id == before)
            })
        }) {
            return Err(AppError::InvalidState("unknown destination tab".into()));
        }
        Ok(())
    }

    /// Moves `tab` into a window of its own. The main window shows what it
    /// would after closing the tab.
    pub fn move_tab_to_window(
        &mut self,
        tab: TabId,
        bounds: Option<WindowBounds>,
    ) -> Result<TabWindowId, AppError> {
        if let Some(bounds) = bounds {
            bounds.validate()?;
        }
        let project = self.tab_project(tab)?;
        self.project_mut(project)?.require_available()?;
        let shown = self
            .project(project)
            .and_then(|target| target.tabs.iter().find(|candidate| candidate.id == tab))
            .and_then(|target| self.shown_pane(target));
        self.unlist_tab(project, tab)?;
        let target = self.tab_mut(tab)?;
        if let Some(window) = &target.window {
            return Ok(window.id);
        }
        let active = shown
            .filter(|pane| target.layout.contains(*pane))
            .or_else(|| target.layout.leaves().first().copied());
        let window = TabWindow::new(bounds, active);
        let id = window.id;
        let panes = target.layout.leaves();
        target.window = Some(window);
        self.window
            .focus_history
            .retain(|pane| !panes.contains(pane));
        self.sync_groups(project);
        Ok(id)
    }

    /// Moves `pane` into a window of its own, as a new tab of its project.
    pub fn move_pane_to_window(
        &mut self,
        pane: PaneId,
        bounds: Option<WindowBounds>,
    ) -> Result<TabWindowId, AppError> {
        if let Some(bounds) = bounds {
            bounds.validate()?;
        }
        let (project, tab) = self.pane_project_id(pane)?;
        if self.tab_mut(tab)?.panes.len() == 1 {
            return self.move_tab_to_window(tab, bounds);
        }
        let window = TabWindow::new(bounds, Some(pane));
        let id = window.id;
        self.split_off(pane, TabPlace::end(project), Some(window))?;
        Ok(id)
    }

    /// Brings a tab window's tab back to the main window, at `place`, and
    /// shows it there.
    pub fn return_tab(
        &mut self,
        window: TabWindowId,
        place: TabPlace,
    ) -> Result<Vec<SessionMove>, AppError> {
        let tab = self
            .tab_window(window)
            .map(|(_, tab, _)| tab.id)
            .ok_or_else(|| AppError::InvalidState("unknown tab window".into()))?;
        self.move_tab_to(tab, place)
    }

    /// Moves `tab` among `place.project`'s listed tabs and shows it there.
    /// A tab in a window of its own leaves that window.
    pub fn move_tab_to(
        &mut self,
        tab: TabId,
        place: TabPlace,
    ) -> Result<Vec<SessionMove>, AppError> {
        if place.before == Some(tab) {
            return Err(AppError::InvalidState(
                "a tab can't move before itself".into(),
            ));
        }
        let source = self.tab_project(tab)?;
        self.check_move(source, place.project)?;
        self.check_place(place)?;
        self.unlist_tab(source, tab)?;
        let from = self.project_mut(source)?;
        let index = from
            .tabs
            .iter()
            .position(|candidate| candidate.id == tab)
            .ok_or(AppError::UnknownTab {
                project: source,
                tab,
            })?;
        let mut moved = from.tabs.remove(index);
        self.sync_groups(source);
        let shown = moved
            .window
            .take()
            .and_then(|window| window.active_pane())
            .filter(|pane| moved.layout.contains(*pane));
        let shown = shown
            .or_else(|| {
                self.window
                    .focus_history
                    .iter()
                    .rev()
                    .copied()
                    .find(|pane| moved.layout.contains(*pane))
            })
            .or(moved.zoomed)
            .or_else(|| moved.layout.leaves().first().copied());
        let sessions = self.session_moves(&moved, source, place.project);
        self.list_tab(moved, place, shown)?;
        Ok(sessions)
    }

    /// Moves `pane` out of its tab into a new tab at `place`, and shows it.
    pub fn move_pane_to_new_tab(
        &mut self,
        pane: PaneId,
        place: TabPlace,
    ) -> Result<(TabId, Vec<SessionMove>), AppError> {
        let (source, tab) = self.pane_project_id(pane)?;
        self.check_move(source, place.project)?;
        self.check_place(place)?;
        if self.tab_mut(tab)?.panes.len() == 1 {
            let sessions = self.move_tab_to(tab, place)?;
            return Ok((tab, sessions));
        }
        self.split_off(pane, place, None)
    }

    /// Moves `pane` into the tab showing `drop`'s anchor, beside or in place
    /// of it, and focuses it.
    pub fn move_pane_to(
        &mut self,
        pane: PaneId,
        drop: PaneDrop,
    ) -> Result<Vec<SessionMove>, AppError> {
        let anchor = drop.anchor();
        let (source, from) = self.pane_project_id(pane)?;
        let (target, into) = self.pane_project_id(anchor)?;
        if from == into {
            match drop {
                PaneDrop::Swap(anchor) => self.move_pane(pane, anchor, None)?,
                PaneDrop::Dock {
                    anchor,
                    edge,
                    level,
                } => self.dock_pane(pane, anchor, edge, level)?,
            }
            return Ok(Vec::new());
        }
        self.check_move(source, target)?;
        let destination = self.tab_mut(into)?;
        let layout = match drop {
            PaneDrop::Swap(anchor) => {
                let mut layout = destination.layout.clone();
                layout.replace(anchor, pane);
                layout
            }
            PaneDrop::Dock {
                anchor,
                edge,
                level,
            } => destination
                .layout
                .inserted(pane, anchor, edge, level)
                .ok_or_else(|| AppError::InvalidState("invalid pane drop".into()))?,
        };
        let origin = self.tab_mut(from)?;
        let index = origin
            .panes
            .iter()
            .position(|candidate| candidate.id == pane)
            .ok_or(AppError::UnknownPane(pane))?;
        let swapped = match drop {
            PaneDrop::Swap(anchor) => Some(anchor),
            PaneDrop::Dock { .. } => None,
        };
        let moved = origin.panes[index].clone();
        let mut sessions = Vec::new();
        if let Some(anchor) = swapped {
            let destination = self.tab_mut(into)?;
            let returning = destination
                .panes
                .iter()
                .position(|candidate| candidate.id == anchor)
                .map(|index| destination.panes.remove(index))
                .ok_or(AppError::UnknownPane(anchor))?;
            destination.zoomed = None;
            if let Some(window) = &mut destination.window {
                window.forget(anchor);
            }
            sessions.extend(self.pane_session_move(&returning, target, source));
            let origin = self.tab_mut(from)?;
            origin.layout.replace(pane, anchor);
            origin.panes[index] = returning;
            if origin.zoomed == Some(pane) {
                origin.zoomed = Some(anchor);
            }
            if let Some(window) = &mut origin.window {
                window.replace(pane, anchor);
            }
            self.window.focus_history.retain(|id| *id != anchor);
            if self.window.active_pane == Some(pane) {
                self.window.active_pane = Some(anchor);
            }
        } else {
            self.extract_pane(pane)?;
        }
        self.window.focus_history.retain(|id| *id != pane);
        sessions.extend(self.pane_session_move(&moved, source, target));
        let destination = self.tab_mut(into)?;
        destination.layout = layout;
        destination.zoomed = None;
        destination.panes.push(moved);
        if let Some(window) = &mut destination.window {
            window.activate(Some(pane));
        } else {
            self.window.current_project = target;
            self.window.selected_tab.insert(target, into);
            self.window.activate(Some(pane));
            self.sync_groups(target);
        }
        Ok(sessions)
    }

    /// Remembers that servers were asked to make `moves`, until they answer.
    pub fn record_session_moves(&mut self, moves: &[SessionMove]) {
        for moved in moves {
            self.session_moves.retain(|pending| {
                (pending.server, pending.session) != (moved.server, moved.session)
            });
            self.session_moves.push(*moved);
        }
    }

    /// `server` answered the move of `session`, either way.
    pub fn settle_session_move(&mut self, server: ServerId, session: SessionId) {
        self.session_moves
            .retain(|pending| (pending.server, pending.session) != (server, session));
    }

    /// Forgets the moves `server`'s `sessions` show it made: those it lists
    /// under the project they were moving to.
    pub fn settle_session_moves(&mut self, server: ServerId, sessions: &[SessionInfo]) {
        self.session_moves.retain(|pending| {
            pending.server != server
                || !sessions.iter().any(|session| {
                    session.id == pending.session && session.project == pending.project
                })
        });
    }

    /// The move of `session` the app is still waiting on `server` for.
    pub fn pending_session_move(
        &self,
        server: ServerId,
        session: SessionId,
    ) -> Option<SessionMove> {
        self.session_moves
            .iter()
            .copied()
            .find(|pending| (pending.server, pending.session) == (server, session))
    }

    /// Records where a tab window sits on screen.
    pub fn set_tab_window_bounds(
        &mut self,
        window: TabWindowId,
        bounds: Option<WindowBounds>,
    ) -> Result<(), AppError> {
        if let Some(bounds) = bounds {
            bounds.validate()?;
        }
        let tab = self
            .tab_window(window)
            .map(|(_, tab, _)| tab.id)
            .ok_or_else(|| AppError::InvalidState("unknown tab window".into()))?;
        if let Some(window) = &mut self.tab_mut(tab)?.window {
            window.bounds = bounds;
        }
        Ok(())
    }

    /// Takes `pane` out of its tab into a new tab: in `window`, or else
    /// listed at `place`.
    fn split_off(
        &mut self,
        pane: PaneId,
        place: TabPlace,
        window: Option<TabWindow>,
    ) -> Result<(TabId, Vec<SessionMove>), AppError> {
        let (source, tab) = self.pane_project_id(pane)?;
        let moved = self
            .tab_mut(tab)?
            .panes
            .iter()
            .find(|candidate| candidate.id == pane)
            .cloned()
            .ok_or(AppError::UnknownPane(pane))?;
        self.extract_pane(pane)?;
        self.window.focus_history.retain(|id| *id != pane);
        let sessions = self
            .pane_session_move(&moved, source, place.project)
            .into_iter()
            .collect();
        let new = Tab {
            id: TabId::new(),
            custom_title: None,
            color: None,
            pinned: false,
            legacy_active_pane: None,
            layout: Layout::Leaf(pane),
            zoomed: None,
            window,
            panes: vec![moved],
        };
        let id = new.id;
        if new.window.is_some() {
            self.project_mut(place.project)?.tabs.push(new);
            self.sync_groups(place.project);
        } else {
            self.list_tab(new, place, Some(pane))?;
        }
        Ok((id, sessions))
    }

    /// Takes `tab` out of the main window, as if it closed: the window
    /// selects and focuses what it would then. A tab in its own window isn't
    /// listed, so nothing changes.
    pub(crate) fn unlist_tab(&mut self, project: ProjectId, tab: TabId) -> Result<(), AppError> {
        let active = self.window.active_pane;
        let history = self.window.focus_history.clone();
        let target = self.project_mut(project)?;
        let listed = target
            .tabs
            .iter()
            .find(|candidate| candidate.id == tab)
            .ok_or(AppError::UnknownTab { project, tab })?;
        if listed.detached() {
            return Ok(());
        }
        let removed_active = active.is_some_and(|pane| listed.layout.contains(pane));
        let next = target.tab_after_close(tab);
        let grouped = target.groups.is_some();
        let next = next.and_then(|next| target.tabs.iter_mut().find(|tab| tab.id == next));
        let next_pane = next.as_ref().and_then(|next| {
            let shown = |pane: &&PaneId| next.layout.contains(**pane);
            let previous = history.iter().rev().find(shown);
            grouped
                .then(|| next.zoomed.or(previous.copied()))
                .flatten()
                .or_else(|| next.layout.leaves().first().copied())
        });
        let next = next.map(|next| {
            if removed_active && !grouped {
                next.zoomed = None;
            }
            next.id
        });
        if let Some(groups) = &mut target.groups {
            groups.remove(tab);
        }
        if self.window.selected_tab.get(&project) == Some(&tab) {
            if let Some(next) = next {
                self.window.selected_tab.insert(project, next);
            } else {
                self.window.selected_tab.remove(&project);
            }
        }
        if removed_active {
            self.window.activate(next_pane);
        }
        Ok(())
    }

    /// Lists `tab` among `place.project`'s tabs, then shows and focuses it
    /// in the main window.
    fn list_tab(
        &mut self,
        mut tab: Tab,
        place: TabPlace,
        shown: Option<PaneId>,
    ) -> Result<(), AppError> {
        let id = tab.id;
        tab.window = None;
        let near = self.window.selected_tab.get(&place.project).copied();
        let target = self.project_mut(place.project)?;
        let boundary = target.tabs.iter().take_while(|tab| tab.pinned).count();
        let index = place
            .before
            .and_then(|before| target.tabs.iter().position(|tab| tab.id == before))
            .unwrap_or(target.tabs.len());
        let index = if tab.pinned {
            index.min(boundary)
        } else {
            index.max(boundary)
        };
        let group_near = place.before.or(near);
        if let Some(groups) = &mut target.groups {
            groups.insert(id, group_near);
        }
        target.tabs.insert(index, tab);
        self.window.current_project = place.project;
        self.window.selected_tab.insert(place.project, id);
        self.window.activate(shown);
        self.sync_groups(place.project);
        Ok(())
    }

    fn session_moves(&self, tab: &Tab, source: ProjectId, target: ProjectId) -> Vec<SessionMove> {
        tab.panes
            .iter()
            .filter_map(|pane| self.pane_session_move(pane, source, target))
            .collect()
    }

    fn pane_session_move(
        &self,
        pane: &crate::Pane,
        source: ProjectId,
        target: ProjectId,
    ) -> Option<SessionMove> {
        if source == target {
            return None;
        }
        let PaneContent::Terminal {
            session: Some(session),
        } = pane.content
        else {
            return None;
        };
        Some(SessionMove {
            server: self.project(target)?.server_id,
            session,
            project: target,
        })
    }
}
