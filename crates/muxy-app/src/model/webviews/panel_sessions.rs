use std::collections::HashMap;

use muxy_app_core::ProjectId;
use muxy_ui::panel::PanelPosition;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SavedPanel {
    pub owner: String,
    pub kind: String,
    pub instance: Option<String>,
    pub position: PanelPosition,
    pub data: Value,
}

#[derive(Default)]
pub(crate) struct PanelSessions {
    current: Option<ProjectId>,
    saved: HashMap<ProjectId, SavedPanel>,
    pub(crate) pending: Option<SavedPanel>,
}

impl PanelSessions {
    pub(crate) fn enter(&mut self, project: ProjectId) -> Option<ProjectId> {
        let previous = self.current.replace(project)?;
        (previous != project).then_some(previous)
    }

    pub(crate) fn save(&mut self, project: ProjectId, panel: SavedPanel) {
        self.saved.insert(project, panel);
    }

    pub(crate) fn take(&mut self, project: ProjectId) -> Option<SavedPanel> {
        self.saved.remove(&project)
    }

    pub(crate) fn keep(&mut self, panel: SavedPanel) {
        if let Some(project) = self.current {
            self.saved.insert(project, panel);
        }
    }

    pub(crate) fn retain_projects(&mut self, exists: impl Fn(ProjectId) -> bool) {
        self.saved.retain(|project, _| exists(*project));
    }

    pub(crate) fn retain_owners(&mut self, keep: impl Fn(&str) -> bool) {
        self.saved.retain(|_, panel| keep(&panel.owner));
    }
}
