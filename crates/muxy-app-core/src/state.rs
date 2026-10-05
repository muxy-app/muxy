use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use muxy_protocol::SessionId;
use serde::{Deserialize, Serialize};

use crate::servers::ServerState;
use crate::{
    AppError, Branch, Color, Direction, PROJECT_COLORS, Pane, PaneContent, PaneId, Project,
    ProjectId, ProjectStatus, ServerId, Tab, TabId, WindowBounds, WindowState, Workspace,
};

/// The layout this build writes. Versions 1 and 2 kept one server's catalog
/// and pending work at the top level; version 3 keeps them per server.
pub(crate) const VERSION: u32 = 3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StoredState")]
pub struct AppState {
    pub(crate) servers: BTreeMap<ServerId, ServerState>,
    pub(crate) starting_directories: BTreeMap<PaneId, muxy_protocol::ServerPath>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) startup_commands: BTreeMap<PaneId, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) quick_terminal: Option<Pane>,
    pub(crate) version: u32,
    pub(crate) projects: Vec<Project>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) workspaces: Vec<Workspace>,
    pub(crate) window: WindowState,
}

#[derive(Deserialize)]
struct StoredState {
    #[serde(default)]
    servers: BTreeMap<ServerId, ServerState>,
    #[serde(default)]
    starting_directories: BTreeMap<PaneId, muxy_protocol::ServerPath>,
    #[serde(default)]
    startup_commands: BTreeMap<PaneId, String>,
    #[serde(default)]
    quick_terminal: Option<Pane>,
    version: u32,
    projects: Vec<Project>,
    #[serde(default)]
    workspaces: Vec<Workspace>,
    window: WindowState,
    /// Versions 1 and 2 kept the local server's state here, at the top level.
    #[serde(default)]
    catalog_server: Option<muxy_protocol::ServerIdentity>,
    #[serde(default)]
    catalog_revision: u64,
    #[serde(default)]
    project_intents: Vec<muxy_protocol::ProjectIntent>,
    #[serde(default)]
    pending_cancellations: Vec<muxy_protocol::OperationId>,
    #[serde(default)]
    pending_discards: Vec<SessionId>,
    #[serde(default)]
    close_operations: BTreeMap<SessionId, muxy_protocol::OperationId>,
}

impl TryFrom<StoredState> for AppState {
    type Error = AppError;

    fn try_from(stored: StoredState) -> Result<Self, Self::Error> {
        if !(1..=VERSION).contains(&stored.version) {
            return Err(AppError::UnsupportedVersion(stored.version));
        }
        let servers = if stored.version < 3 {
            BTreeMap::from([(
                ServerId::local(),
                ServerState {
                    identity: stored.catalog_server,
                    catalog_revision: stored.catalog_revision,
                    project_intents: stored.project_intents,
                    pending_cancellations: stored.pending_cancellations,
                    pending_discards: stored.pending_discards,
                    close_operations: stored.close_operations,
                },
            )])
        } else {
            stored.servers
        };
        let mut state = Self {
            servers,
            starting_directories: stored.starting_directories,
            startup_commands: stored.startup_commands,
            quick_terminal: stored.quick_terminal,
            version: VERSION,
            projects: stored.projects,
            workspaces: stored.workspaces,
            window: stored.window,
        };
        let previous_project = state.window.current_project;
        let previous_tab = state.window.selected_tab.get(&previous_project).copied();
        state.ensure_home()?;
        state.retain_workspace_members();
        if state.window.current_project != previous_project
            || state
                .window
                .selected_tab
                .get(&state.window.current_project)
                .copied()
                != previous_tab
        {
            state.window.active_pane = None;
        }
        let selected = state
            .window
            .selected_tab
            .get(&state.window.current_project)
            .copied();
        for tab in state
            .projects
            .iter_mut()
            .flat_map(|project| &mut project.tabs)
        {
            let legacy = tab.legacy_active_pane.take();
            if let Some(pane) = legacy
                && !state.window.focus_history.contains(&pane)
            {
                state.window.focus_history.push(pane);
            }
            if state.window.active_pane.is_none() && Some(tab.id) == selected {
                state.window.active_pane = legacy.or_else(|| tab.layout.leaves().first().copied());
            }
        }
        let panes: HashSet<_> = state
            .projects
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .map(|pane| pane.id)
            .collect();
        state
            .window
            .focus_history
            .retain(|pane| panes.contains(pane));
        state.validate()?;
        let legacy_settings: Vec<_> = state
            .projects
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .filter(|pane| pane.content == PaneContent::Settings)
            .map(|pane| pane.id)
            .collect();
        let restore_focus = state
            .window
            .active_pane
            .is_some_and(|pane| legacy_settings.contains(&pane));
        if restore_focus {
            state.window.active_pane = None;
        }
        for pane in legacy_settings {
            state.remove_pane(pane)?;
        }
        if restore_focus {
            state.focus_selected_tab();
        }
        state.validate()?;
        Ok(state)
    }
}

impl AppState {
    pub fn quick_terminal(&self) -> Option<&Pane> {
        self.quick_terminal.as_ref()
    }

    pub fn ensure_quick_terminal(&mut self) -> PaneId {
        self.quick_terminal
            .get_or_insert_with(|| Pane {
                id: PaneId::new(),
                title: "Quick Terminal".into(),
                content: PaneContent::Terminal { session: None },
            })
            .id
    }

    pub fn close_quick_terminal(&mut self) {
        if let Some(pane) = &self.quick_terminal {
            self.cancel_pending_creation(pane.id);
        }
        if let Some(Pane {
            content: PaneContent::Terminal {
                session: Some(session),
            },
            ..
        }) = self.quick_terminal.take()
        {
            self.discard_unreferenced(ServerId::local(), session);
        }
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    pub fn window(&self) -> &WindowState {
        &self.window
    }

    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|project| project.id == id)
    }

    pub fn home(&self) -> &Project {
        &self.projects[0]
    }

    pub fn current_project(&self) -> &Project {
        self.project(self.window.current_project)
            .unwrap_or_else(|| self.home())
    }

    pub fn refresh_project_statuses(&mut self) {
        for project in &mut self.projects {
            project.refresh_status();
        }
    }

    /// Another computer's server says whether the project's folder is there.
    /// This computer's projects are checked on disk instead.
    pub fn set_remote_project_status(
        &mut self,
        project: ProjectId,
        status: ProjectStatus,
    ) -> Result<(), AppError> {
        let project = self.project_mut(project)?;
        if project.server_id.is_local() {
            return Err(AppError::InvalidState(
                "this computer's projects are checked on disk".into(),
            ));
        }
        project.status = status;
        Ok(())
    }

    /// An offline server's projects are available: nothing says otherwise.
    pub fn forget_remote_statuses(&mut self, server: ServerId) {
        for project in &mut self.projects {
            if project.server_id == server && !server.is_local() {
                project.status = ProjectStatus::Available;
            }
        }
    }

    pub fn select_project(&mut self, id: ProjectId) -> Result<(), AppError> {
        self.project_mut(id)?.require_available()?;
        self.window.current_project = id;
        self.focus_selected_tab();
        Ok(())
    }

    /// A remote server checks the folder itself when it creates the project.
    pub fn add_project(
        &mut self,
        server: ServerId,
        directory: PathBuf,
    ) -> Result<ProjectId, AppError> {
        if server.is_local() && !(directory.is_absolute() && directory.is_dir()) {
            return Err(AppError::InvalidState(
                "project path must be an existing absolute directory".into(),
            ));
        }
        if !directory.is_absolute() {
            return Err(AppError::InvalidState(
                "project path must be absolute".into(),
            ));
        }
        if self.server_home(server).is_none() {
            return Err(AppError::InvalidState(
                "connect to the server before adding projects to it".into(),
            ));
        }
        let id = ProjectId::new();
        let name = directory.file_name().map_or_else(
            || directory.to_string_lossy().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let color = PROJECT_COLORS[(self.projects.len() - 1) % PROJECT_COLORS.len()]
            .1
            .parse()?;
        let project = Project {
            id,
            home: false,
            name,
            icon: None,
            logo: None,
            color,
            server_id: server,
            directory,
            kind: None,
            parent_id: None,
            tabs: Vec::new(),
            status: ProjectStatus::Available,
        };
        self.queue_project(
            server,
            muxy_protocol::ProjectMutation::Create(project.descriptor()),
        )?;
        self.projects.push(project);
        self.join_active_workspace(id);
        self.window.current_project = id;
        self.window.active_pane = None;
        Ok(id)
    }

    pub fn rename_project(&mut self, id: ProjectId, name: &str) -> Result<(), AppError> {
        let project = self.project_mut(id)?;
        project.require_available()?;
        if name.trim().is_empty() {
            return Err(AppError::InvalidState(
                "project name cannot be empty".into(),
            ));
        }
        self.patch_project(id, muxy_protocol::ProjectPatch::Name(name.trim().into()))?;
        name.trim().clone_into(&mut self.project_mut(id)?.name);
        Ok(())
    }

    pub fn set_project_icon(
        &mut self,
        id: ProjectId,
        icon: Option<String>,
    ) -> Result<(), AppError> {
        let project = self.project_mut(id)?;
        project.require_available()?;
        self.patch_project(id, muxy_protocol::ProjectPatch::Icon(icon.clone()))?;
        self.project_mut(id)?.icon = icon;
        Ok(())
    }

    pub fn set_project_logo(
        &mut self,
        id: ProjectId,
        logo: Option<std::sync::Arc<[u8]>>,
    ) -> Result<(), AppError> {
        self.project_mut(id)?.require_available()?;
        self.patch_project(id, muxy_protocol::ProjectPatch::Logo(logo.clone()))?;
        self.project_mut(id)?.logo = logo;
        Ok(())
    }

    pub fn set_project_color(&mut self, id: ProjectId, color: Color) -> Result<(), AppError> {
        let project = self.project_mut(id)?;
        project.require_available()?;
        let patch = muxy_protocol::ProjectPatch::Color(color.to_string());
        self.patch_project(id, patch)?;
        self.project_mut(id)?.color = color;
        Ok(())
    }

    pub fn move_project(&mut self, id: ProjectId, to: usize) -> Result<(), AppError> {
        let project = self.project_mut(id)?;
        project.require_available()?;
        let (home, server) = (project.home, project.server_id);
        let server_home = self
            .projects
            .iter()
            .position(|candidate| candidate.home && candidate.server_id == server);
        if home || to >= self.projects.len() || server_home.is_none_or(|index| to <= index) {
            return Err(AppError::InvalidState(
                "Home stays first; project destination must be in range".into(),
            ));
        }
        let from = self
            .projects
            .iter()
            .position(|project| project.id == id)
            .ok_or(AppError::UnknownProject(id))?;
        let project = self.projects.remove(from);
        self.projects.insert(to, project);
        Ok(())
    }

    pub fn remove_project(&mut self, id: ProjectId) -> Result<Vec<SessionId>, AppError> {
        self.remove_project_with_mutation(id, muxy_protocol::ProjectMutation::Delete(id))
    }

    pub(crate) fn remove_project_with_mutation(
        &mut self,
        id: ProjectId,
        mutation: muxy_protocol::ProjectMutation,
    ) -> Result<Vec<SessionId>, AppError> {
        let index = self
            .projects
            .iter()
            .position(|project| project.id == id)
            .ok_or(AppError::UnknownProject(id))?;
        if self.projects[index].home {
            return Err(AppError::InvalidState("Home cannot be removed".into()));
        }
        let server = self.projects[index].server_id;
        self.queue_project(server, mutation)?;
        let project = self.projects.remove(index);
        self.retain_workspace_members();
        self.window.selected_tab.remove(&id);
        self.window
            .focus_history
            .retain(|pane| !project.tabs.iter().any(|tab| tab.layout.contains(*pane)));
        if self.window.current_project == id {
            self.window.current_project = self.home().id;
            self.focus_selected_tab();
        }
        let mut sessions = Vec::new();
        for pane in project.tabs.into_iter().flat_map(|tab| tab.panes) {
            self.startup_commands.remove(&pane.id);
            if let PaneContent::Terminal {
                session: Some(session),
            } = pane.content
                && !sessions.contains(&session)
            {
                sessions.push(session);
            }
        }
        Ok(sessions)
    }

    pub fn open_terminal_tab(&mut self, project: ProjectId) -> Result<TabId, AppError> {
        self.project_mut(project)?.require_available()?;
        let tab = Tab::terminal();
        let id = tab.id;
        self.window.activate(tab.layout.leaves().first().copied());
        self.project_mut(project)?.tabs.push(tab);
        self.window.selected_tab.insert(project, id);
        self.window.current_project = project;
        Ok(id)
    }

    pub fn open_terminal_tab_adjacent(
        &mut self,
        project: ProjectId,
        anchor: TabId,
        side: crate::TabSide,
    ) -> Result<TabId, AppError> {
        let target = self
            .project(project)
            .ok_or(AppError::UnknownProject(project))?;
        target.require_available()?;
        let index =
            target
                .tabs
                .iter()
                .position(|tab| tab.id == anchor)
                .ok_or(AppError::UnknownTab {
                    project,
                    tab: anchor,
                })?;
        let boundary = target.tabs.iter().take_while(|tab| tab.pinned).count();
        let index = (index + usize::from(side == crate::TabSide::Right)).max(boundary);
        let id = self.open_terminal_tab(project)?;
        let tabs = &mut self.project_mut(project)?.tabs;
        let tab = tabs
            .pop()
            .ok_or(AppError::UnknownTab { project, tab: id })?;
        tabs.insert(index, tab);
        Ok(id)
    }

    pub fn set_tab_title(&mut self, tab: TabId, title: Option<String>) -> Result<(), AppError> {
        self.tab_mut(tab)?.custom_title = title
            .map(|title| title.trim().to_owned())
            .filter(|title| !title.is_empty());
        Ok(())
    }

    pub fn set_tab_color(&mut self, tab: TabId, color: Option<Color>) -> Result<(), AppError> {
        self.tab_mut(tab)?.color = color;
        Ok(())
    }

    pub fn toggle_tab_pin(&mut self, tab: TabId) -> Result<(), AppError> {
        let target = self.tab_mut(tab)?;
        target.pinned = !target.pinned;
        let project = self
            .projects
            .iter_mut()
            .find(|project| project.tabs.iter().any(|item| item.id == tab))
            .ok_or_else(|| AppError::InvalidState("unknown tab".into()))?;
        let index =
            project
                .tabs
                .iter()
                .position(|item| item.id == tab)
                .ok_or(AppError::UnknownTab {
                    project: project.id,
                    tab,
                })?;
        let tab = project.tabs.remove(index);
        let boundary = project.tabs.iter().take_while(|tab| tab.pinned).count();
        project.tabs.insert(boundary, tab);
        Ok(())
    }

    pub fn close_tab(&mut self, project: ProjectId, tab: TabId) -> Result<(), AppError> {
        let panes: Vec<_> = self
            .project(project)
            .ok_or(AppError::UnknownProject(project))?
            .tabs
            .iter()
            .find(|candidate| candidate.id == tab)
            .ok_or(AppError::UnknownTab { project, tab })?
            .panes
            .iter()
            .map(|pane| pane.id)
            .collect();
        for pane in panes {
            self.cancel_pending_creation(pane);
        }
        let active = self.window.active_pane;
        let tabs = &mut self.project_mut(project)?.tabs;
        let index = tabs
            .iter()
            .position(|candidate| candidate.id == tab)
            .ok_or(AppError::UnknownTab { project, tab })?;
        let removed_active = active.is_some_and(|pane| tabs[index].layout.contains(pane));
        let removed = tabs.remove(index);
        let next_index = index.min(tabs.len().saturating_sub(1));
        let next = tabs.get_mut(next_index);
        let next_pane = next
            .as_ref()
            .and_then(|tab| tab.layout.leaves().first().copied());
        let next = next.map(|tab| {
            if removed_active {
                tab.zoomed = None;
            }
            tab.id
        });
        self.window
            .focus_history
            .retain(|pane| !removed.layout.contains(*pane));
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

    pub fn close_pane(&mut self, pane: PaneId) -> Result<(), AppError> {
        self.pane_tab_mut(pane)?;
        self.remove_pane(pane)
    }

    pub fn detach_pane(&mut self, pane: PaneId) -> Result<(), AppError> {
        self.pane_tab_mut(pane)?;
        self.starting_directories.remove(&pane);
        self.remove_pane(pane)
    }

    pub fn close_session_panes(
        &mut self,
        server: ServerId,
        session: SessionId,
    ) -> Result<(), AppError> {
        let panes: Vec<_> = self.projects.iter().filter(|project| project.server_id == server)
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .filter(|pane| matches!(pane.content, PaneContent::Terminal { session: Some(id) } if id == session))
            .map(|pane| pane.id).collect();
        for pane in panes {
            self.remove_pane(pane)?;
        }
        self.discard_unreferenced(server, session);
        Ok(())
    }

    pub fn close_session_pane(&mut self, pane: PaneId) -> Result<(), AppError> {
        let server = self.pane_server(pane).ok_or(AppError::UnknownPane(pane))?;
        let content = self.pane_mut(pane)?.content.clone();
        self.remove_pane(pane)?;
        if let PaneContent::Terminal {
            session: Some(session),
        } = content
        {
            self.discard_unreferenced(server, session);
        }
        Ok(())
    }

    /// Closes `server`'s terminal panes, and the Quick Terminal for this
    /// computer's server.
    pub fn clear_terminal_panes(&mut self, server: ServerId) -> Result<(), AppError> {
        if server.is_local() {
            self.close_quick_terminal();
        }
        let terminals: Vec<_> = self
            .projects
            .iter()
            .filter(|project| project.server_id == server)
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .filter(|pane| matches!(pane.content, PaneContent::Terminal { .. }))
            .map(|pane| pane.id)
            .collect();
        for pane in terminals {
            self.remove_pane(pane)?;
        }
        Ok(())
    }

    fn remove_pane(&mut self, pane: PaneId) -> Result<(), AppError> {
        let (project, tab) = self
            .projects
            .iter()
            .find_map(|project| {
                project.tabs.iter().find_map(|tab| {
                    tab.panes
                        .iter()
                        .any(|candidate| candidate.id == pane)
                        .then_some((project.id, tab.id))
                })
            })
            .ok_or(AppError::UnknownPane(pane))?;
        self.cancel_pending_creation(pane);
        let target = self
            .project_mut(project)?
            .tabs
            .iter_mut()
            .find(|item| item.id == tab)
            .ok_or(AppError::UnknownTab { project, tab })?;
        if target.panes.len() == 1 {
            return self.close_tab(project, tab);
        }
        let neighbor = [
            Direction::Right,
            Direction::Left,
            Direction::Down,
            Direction::Up,
        ]
        .into_iter()
        .find_map(|direction| target.layout.neighbor(pane, direction));
        target.layout.remove(pane);
        target.panes.retain(|candidate| candidate.id != pane);
        if target.zoomed == Some(pane) {
            target.zoomed = None;
        }
        let fallback = neighbor.or_else(|| target.layout.leaves().first().copied());
        self.window
            .focus_history
            .retain(|previous| *previous != pane);
        if self.window.active_pane == Some(pane) {
            self.window.activate(fallback);
        }
        Ok(())
    }

    pub fn split_pane(&mut self, pane: PaneId, edge: Direction) -> Result<PaneId, AppError> {
        let tab = self.pane_tab_mut(pane)?;
        let new = Pane {
            id: PaneId::new(),
            title: "Terminal".into(),
            content: PaneContent::Terminal { session: None },
        };
        tab.layout.split(pane, new.id, edge);
        let id = new.id;
        tab.zoomed = None;
        tab.panes.push(new);
        self.focus_pane(id)?;
        Ok(id)
    }

    pub fn move_pane(
        &mut self,
        pane: PaneId,
        target: PaneId,
        edge: Option<Direction>,
    ) -> Result<(), AppError> {
        let tab = self.pane_tab_mut(pane)?;
        let layout = tab
            .layout
            .moved(pane, target, edge)
            .ok_or_else(|| AppError::InvalidState("invalid pane drop".into()))?;
        tab.layout = layout;
        tab.zoomed = None;
        self.focus_pane(pane)
    }

    pub fn dock_pane(
        &mut self,
        pane: PaneId,
        anchor: PaneId,
        edge: Direction,
        level: usize,
    ) -> Result<(), AppError> {
        let tab = self.pane_tab_mut(pane)?;
        let layout = tab
            .layout
            .docked(pane, anchor, edge, level)
            .ok_or_else(|| AppError::InvalidState("invalid pane dock".into()))?;
        tab.layout = layout;
        tab.zoomed = None;
        self.focus_pane(pane)
    }

    pub fn focus_pane(&mut self, pane: PaneId) -> Result<(), AppError> {
        let tab = self.pane_tab_mut(pane)?;
        if tab.zoomed.is_some() {
            tab.zoomed = Some(pane);
        }
        let tab = tab.id;
        let project = self
            .projects
            .iter()
            .find(|project| project.tabs.iter().any(|item| item.id == tab))
            .ok_or(AppError::UnknownPane(pane))?
            .id;
        self.window.current_project = project;
        self.window.selected_tab.insert(project, tab);
        self.window.activate(Some(pane));
        Ok(())
    }

    pub fn toggle_zoom(&mut self, pane: PaneId) -> Result<(), AppError> {
        self.focus_pane(pane)?;
        let tab = self.pane_tab_mut(pane)?;
        tab.zoomed = if tab.zoomed == Some(pane) {
            None
        } else {
            Some(pane)
        };
        Ok(())
    }

    pub fn neighbor(&self, pane: PaneId, direction: Direction) -> Option<PaneId> {
        self.projects
            .iter()
            .flat_map(|project| &project.tabs)
            .find(|tab| tab.layout.contains(pane))?
            .layout
            .neighbor(pane, direction)
    }

    pub fn set_ratio(&mut self, tab: TabId, path: &[Branch], ratio: f32) -> Result<(), AppError> {
        self.tab_mut(tab)?.layout.set_ratio(path, ratio)
    }

    fn tab_mut(&mut self, id: TabId) -> Result<&mut Tab, AppError> {
        let project = self
            .projects
            .iter_mut()
            .find(|project| project.tabs.iter().any(|tab| tab.id == id))
            .ok_or_else(|| AppError::InvalidState("unknown tab".into()))?;
        project.require_available()?;
        project
            .tabs
            .iter_mut()
            .find(|tab| tab.id == id)
            .ok_or_else(|| AppError::InvalidState("unknown tab".into()))
    }

    fn pane_tab_mut(&mut self, id: PaneId) -> Result<&mut Tab, AppError> {
        let project = self
            .projects
            .iter_mut()
            .find(|project| project.tabs.iter().any(|tab| tab.layout.contains(id)))
            .ok_or(AppError::UnknownPane(id))?;
        project.require_available()?;
        project
            .tabs
            .iter_mut()
            .find(|tab| tab.layout.contains(id))
            .ok_or(AppError::UnknownPane(id))
    }

    pub fn select_tab(&mut self, project: ProjectId, tab: TabId) -> Result<(), AppError> {
        self.project_mut(project)?.require_available()?;
        if !self
            .project_mut(project)?
            .tabs
            .iter()
            .any(|item| item.id == tab)
        {
            return Err(AppError::UnknownTab { project, tab });
        }
        self.window.selected_tab.insert(project, tab);
        self.window.current_project = project;
        self.focus_selected_tab();
        Ok(())
    }

    pub(crate) fn focus_selected_tab(&mut self) {
        let selected = self.window.selected_tab.get(&self.window.current_project);
        let tab = self
            .current_project()
            .tabs
            .iter()
            .find(|tab| Some(&tab.id) == selected);
        let pane = tab.and_then(|tab| {
            self.window
                .active_pane
                .filter(|pane| tab.layout.contains(*pane))
                .or(tab.zoomed)
                .or_else(|| {
                    self.window
                        .focus_history
                        .iter()
                        .rev()
                        .copied()
                        .find(|pane| tab.layout.contains(*pane))
                })
                .or_else(|| tab.layout.leaves().first().copied())
        });
        self.window.activate(pane);
    }

    pub fn move_tab(&mut self, project: ProjectId, from: usize, to: usize) -> Result<(), AppError> {
        self.project_mut(project)?.require_available()?;
        let tabs = &mut self.project_mut(project)?.tabs;
        for index in [from, to] {
            if index >= tabs.len() {
                return Err(AppError::InvalidTabIndex {
                    index,
                    len: tabs.len(),
                });
            }
        }
        let boundary = tabs.iter().take_while(|tab| tab.pinned).count();
        let to = if tabs[from].pinned {
            to.min(boundary.saturating_sub(1))
        } else {
            to.max(boundary)
        };
        let tab = tabs.remove(from);
        tabs.insert(to, tab);
        Ok(())
    }

    pub fn move_visible_tab(
        &mut self,
        project: ProjectId,
        from: TabId,
        to: TabId,
        visible: &[TabId],
    ) -> Result<(), AppError> {
        let project = self.project_mut(project)?;
        project.require_available()?;
        let tabs = &mut project.tabs;
        let Some(pinned) = tabs.iter().find(|tab| tab.id == from).map(|tab| tab.pinned) else {
            return Ok(());
        };
        let indices: Vec<_> = tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.pinned == pinned && visible.contains(&tab.id))
            .map(|(index, _)| index)
            .collect();
        let positions = (
            indices.iter().position(|index| tabs[*index].id == from),
            indices.iter().position(|index| tabs[*index].id == to),
        );
        let (Some(from), Some(to)) = positions else {
            return Ok(());
        };
        if from < to {
            for pair in indices[from..=to].windows(2) {
                tabs.swap(pair[0], pair[1]);
            }
        } else {
            for pair in indices[to..=from].windows(2).rev() {
                tabs.swap(pair[0], pair[1]);
            }
        }
        Ok(())
    }

    pub fn set_pane_session(
        &mut self,
        pane: PaneId,
        session: Option<SessionId>,
    ) -> Result<(), AppError> {
        match &mut self.pane_mut(pane)?.content {
            PaneContent::Terminal { session: current } => {
                *current = session;
                Ok(())
            }
            PaneContent::Settings | PaneContent::Webview(_) => Err(AppError::NotTerminal(pane)),
        }
    }

    pub fn set_pane_title(
        &mut self,
        pane: PaneId,
        title: impl Into<String>,
    ) -> Result<(), AppError> {
        self.pane_mut(pane)?.title = title.into();
        Ok(())
    }

    pub fn set_window_bounds(&mut self, bounds: Option<WindowBounds>) -> Result<(), AppError> {
        if let Some(bounds) = bounds {
            bounds.validate()?;
        }
        self.window.bounds = bounds;
        Ok(())
    }

    pub(crate) fn project_mut(&mut self, id: ProjectId) -> Result<&mut Project, AppError> {
        self.projects
            .iter_mut()
            .find(|project| project.id == id)
            .ok_or(AppError::UnknownProject(id))
    }

    pub(crate) fn pane_mut(&mut self, id: PaneId) -> Result<&mut Pane, AppError> {
        self.projects
            .iter_mut()
            .flat_map(|project| &mut project.tabs)
            .flat_map(|tab| &mut tab.panes)
            .chain(self.quick_terminal.iter_mut())
            .find(|pane| pane.id == id)
            .ok_or(AppError::UnknownPane(id))
    }

    pub(crate) fn validate(&self) -> Result<(), AppError> {
        let home_first = || AppError::InvalidState("exactly one Home project must be first".into());
        if self
            .projects
            .first()
            .is_none_or(|project| !project.home || !project.server_id.is_local())
        {
            return Err(home_first());
        }
        let mut projects = HashSet::new();
        let mut servers = HashSet::new();
        for project in &self.projects {
            if !projects.insert(project.id) {
                return Err(AppError::InvalidState(format!(
                    "duplicate project ID {}",
                    project.id
                )));
            }
            let first_of_its_server = servers.insert(project.server_id);
            if first_of_its_server != project.home {
                return Err(home_first());
            }
            project
                .descriptor()
                .validate()
                .map_err(|_| AppError::InvalidState("invalid project metadata".into()))?;
            if let Some(parent) = project.parent_id
                && self.project(parent).is_none_or(|parent| {
                    parent.home || parent.kind.is_some() || parent.parent_id.is_some()
                })
            {
                return Err(AppError::InvalidState("invalid project parent".into()));
            }
        }
        let mut tabs = HashSet::new();
        let mut panes = HashSet::new();
        for tab in self.projects.iter().flat_map(|project| &project.tabs) {
            if !tabs.insert(tab.id) {
                return Err(AppError::InvalidState(format!(
                    "duplicate tab ID {}",
                    tab.id
                )));
            }
            tab.validate()?;
            for pane in &tab.panes {
                if !panes.insert(pane.id) {
                    return Err(AppError::InvalidState(format!(
                        "duplicate pane ID {}",
                        pane.id
                    )));
                }
            }
        }

        if let Some(pane) = &self.quick_terminal
            && (!panes.insert(pane.id) || !matches!(pane.content, PaneContent::Terminal { .. }))
        {
            return Err(AppError::InvalidState("invalid Quick Terminal pane".into()));
        }
        if let Some(active) = self.window.active_pane {
            let selected = self.window.selected_tab.get(&self.window.current_project);
            if !self
                .current_project()
                .tabs
                .iter()
                .any(|tab| Some(&tab.id) == selected && tab.layout.contains(active))
            {
                return Err(AppError::InvalidState(
                    "window active pane must belong to the selected tab".into(),
                ));
            }
        }
        if let Some(bounds) = self.window.bounds {
            bounds.validate()?;
        }
        Ok(())
    }
}
