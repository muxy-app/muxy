use crate::{
    AppError, AppState, PaneContent, PaneId, Project, ProjectId, ProjectStatus, Tab, TabId,
};

impl AppState {
    pub(crate) fn retain_one_settings_pane(&mut self) -> Result<(), AppError> {
        let panes: Vec<_> = self
            .panes()
            .filter(|placed| placed.pane.content == PaneContent::Settings)
            .map(|placed| placed.pane.id)
            .collect();
        let keep = self
            .window
            .active_pane
            .filter(|pane| panes.contains(pane))
            .or_else(|| panes.first().copied());
        for pane in panes {
            if Some(pane) != keep {
                self.remove_pane(pane)?;
            }
        }
        Ok(())
    }

    pub fn settings_tab(&self) -> Option<(ProjectId, TabId)> {
        self.find_settings_tab(|project| project.status() == ProjectStatus::Available)
    }

    fn find_settings_tab(&self, within: impl Fn(&Project) -> bool) -> Option<(ProjectId, TabId)> {
        self.projects
            .iter()
            .filter(|project| within(project))
            .find_map(|project| {
                project
                    .tabs
                    .iter()
                    .find(|tab| {
                        tab.panes
                            .iter()
                            .any(|pane| pane.content == PaneContent::Settings)
                    })
                    .map(|tab| (project.id, tab.id))
            })
    }

    pub fn is_settings_pane(&self, pane: PaneId) -> bool {
        self.projects
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .any(|candidate| candidate.id == pane && candidate.content == PaneContent::Settings)
    }

    pub fn show_settings_tab(
        &mut self,
        project: ProjectId,
        title: &str,
    ) -> Result<TabId, AppError> {
        let server = self.default_home().server_id;
        let open = self.settings_tab().filter(|(project, tab)| {
            self.project_server(*project) == Some(server)
                || self
                    .tab_windows()
                    .any(|(_, detached, _)| detached.id == *tab)
        });
        let (project, tab) = if let Some(open) = open {
            for pane in &mut self.tab_mut(open.1)?.panes {
                if pane.content == PaneContent::Settings {
                    title.clone_into(&mut pane.title);
                }
            }
            open
        } else {
            let stale: Vec<_> = self
                .projects
                .iter()
                .filter(|project| {
                    project.status() != ProjectStatus::Available || project.server_id != server
                })
                .flat_map(|project| &project.tabs)
                .flat_map(|tab| &tab.panes)
                .filter(|pane| pane.content == PaneContent::Settings)
                .map(|pane| pane.id)
                .collect();
            for pane in stale {
                self.remove_pane(pane)?;
            }
            let here = self.project(project).is_some_and(|project| {
                project.server_id == server && project.status() == ProjectStatus::Available
            });
            let project = if here {
                project
            } else {
                self.default_home().id
            };
            let tab = Tab::with_content(PaneContent::Settings, title);
            let id = tab.id;
            let near = self.window.selected_tab.get(&project).copied();
            self.add_tab(project, tab, near)?;
            (project, id)
        };
        if self.tab_mut(tab)?.window.is_none() {
            self.select_project(project)?;
            self.select_tab(project, tab)?;
        }
        if let Some(pane) = self
            .tab_mut(tab)?
            .panes
            .iter()
            .find(|pane| pane.content == PaneContent::Settings)
            .map(|pane| pane.id)
        {
            self.focus_pane(pane)?;
        }
        Ok(tab)
    }
}
