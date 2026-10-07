//! Extension panels belong to the project they were opened in, as on main:
//! each root project remembers its open panel for this app session, and
//! switching projects closes it and reopens the new project's.

use std::collections::HashMap;

use muxy_app_core::ProjectId;
use muxy_ui::panel::PanelPosition;
use serde_json::Value;

/// A panel to reopen when its project returns.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SavedPanel {
    pub owner: String,
    pub kind: String,
    pub position: PanelPosition,
    pub data: Value,
}

#[derive(Default)]
pub(crate) struct PanelSessions {
    /// The root project whose panel is showing.
    current: Option<ProjectId>,
    saved: HashMap<ProjectId, SavedPanel>,
    /// A reopened panel waiting for the next frame, which has the window its
    /// page needs.
    pub(crate) pending: Option<SavedPanel>,
}

impl PanelSessions {
    /// Makes `project` current, returning the project it replaces. The first
    /// project and a return to the same one replace nothing.
    pub(crate) fn enter(&mut self, project: ProjectId) -> Option<ProjectId> {
        let previous = self.current.replace(project)?;
        (previous != project).then_some(previous)
    }

    /// Remembers the panel that was open when `project` was left.
    pub(crate) fn save(&mut self, project: ProjectId, panel: SavedPanel) {
        self.saved.insert(project, panel);
    }

    /// Hands back `project`'s panel to reopen, forgetting it.
    pub(crate) fn take(&mut self, project: ProjectId) -> Option<SavedPanel> {
        self.saved.remove(&project)
    }

    /// Keeps a panel that couldn't reopen yet for the current project's next visit.
    pub(crate) fn keep(&mut self, panel: SavedPanel) {
        if let Some(project) = self.current {
            self.saved.insert(project, panel);
        }
    }

    /// Forgets projects that no longer exist.
    pub(crate) fn retain_projects(&mut self, exists: impl Fn(ProjectId) -> bool) {
        self.saved.retain(|project, _| exists(*project));
    }

    /// Forgets the panels of extensions that stopped.
    pub(crate) fn retain_owners(&mut self, keep: impl Fn(&str) -> bool) {
        self.saved.retain(|_, panel| keep(&panel.owner));
    }
}
