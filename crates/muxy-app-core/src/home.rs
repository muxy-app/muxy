use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crate::{AppError, AppState, Color, Project, ProjectId, ProjectStatus, ServerId, WindowState};

impl AppState {
    pub fn bootstrap() -> Result<Self, AppError> {
        let home = new_home(home_directory()?);
        let window = WindowState {
            active_pane: None,
            focus_history: Vec::new(),
            current_project: home.id,
            selected_tab: HashMap::new(),
            bounds: None,
            workspace: None,
        };
        Ok(Self {
            servers: BTreeMap::new(),
            starting_directories: BTreeMap::new(),
            startup_commands: BTreeMap::new(),
            quick_terminal: None,
            version: crate::state::VERSION,
            projects: vec![home],
            workspaces: Vec::new(),
            window,
        })
    }

    /// The local Home goes first, in the OS home directory. A remote server's
    /// Home comes from its catalog and goes first among its projects.
    pub(crate) fn ensure_home(&mut self) -> Result<(), AppError> {
        let directory = home_directory()?;
        let index = self
            .projects
            .iter()
            .position(|project| project.home && project.server_id.is_local())
            .or_else(|| {
                self.projects.iter().position(|project| {
                    project.name == "Home"
                        && project.server_id.is_local()
                        && project.kind.is_none()
                        && project.parent_id.is_none()
                })
            });
        if let Some(index) = index {
            let mut home = self.projects.remove(index);
            home.home = true;
            home.directory = directory;
            self.projects.insert(0, home);
        } else {
            self.projects.insert(0, new_home(directory));
        }
        let remote_homes: Vec<_> = self
            .projects
            .iter()
            .filter(|project| project.home && !project.server_id.is_local())
            .map(|project| project.id)
            .collect();
        for home in remote_homes {
            if let Some(index) = self.projects.iter().position(|project| project.id == home) {
                self.place_home(index);
            }
        }
        self.refresh_project_statuses();
        if self.project(self.window.current_project).is_none() {
            self.window.current_project = self.home().id;
        }
        self.window.selected_tab.retain(|project, tab| {
            self.projects
                .iter()
                .find(|candidate| candidate.id == *project)
                .is_some_and(|project| project.tabs.iter().any(|candidate| candidate.id == *tab))
        });
        for project in &self.projects {
            if let Some(tab) = project.tabs.first() {
                self.window.selected_tab.entry(project.id).or_insert(tab.id);
            }
        }
        for project in self
            .projects
            .iter()
            .map(|project| project.id)
            .collect::<Vec<_>>()
        {
            self.sync_groups(project);
        }
        Ok(())
    }

    /// Moves the Home at `index` in front of its server's other projects, and
    /// the local Home in front of all. A Home already there stays put.
    pub(crate) fn place_home(&mut self, index: usize) {
        let server = self.projects[index].server_id;
        let first = if server.is_local() {
            0
        } else {
            self.projects
                .iter()
                .position(|project| project.server_id == server)
                .unwrap_or(index)
        };
        if first < index {
            let home = self.projects.remove(index);
            self.projects.insert(first, home);
        }
    }
}

fn home_directory() -> Result<PathBuf, AppError> {
    std::env::home_dir()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or(AppError::HomeDirectoryUnavailable)
}

fn new_home(directory: PathBuf) -> Project {
    Project {
        id: ProjectId::new(),
        home: true,
        name: "Home".into(),
        icon: None,
        logo: None,
        color: Color::default(),
        server_id: ServerId::local(),
        directory,
        kind: None,
        parent_id: None,
        tabs: Vec::new(),
        groups: None,
        status: ProjectStatus::Available,
    }
}
