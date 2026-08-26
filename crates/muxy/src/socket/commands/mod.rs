pub mod panes;
pub mod projects;
pub mod tabs;
pub mod target;
pub mod workspaces;

pub struct CommandResult {
    pub reply: String,
    pub changed: bool,
}

impl CommandResult {
    pub fn reply(reply: impl Into<String>) -> Self {
        Self {
            reply: reply.into(),
            changed: false,
        }
    }

    pub fn changed(reply: impl Into<String>) -> Self {
        Self {
            reply: reply.into(),
            changed: true,
        }
    }
}

#[cfg(test)]
pub fn test_state(directory: &std::path::Path) -> crate::state::AppState {
    let prefs = muxy_core::prefs::Prefs::default();
    crate::state::AppState {
        metrics: muxy_ui::theme::Metrics::new(prefs.scale.multiplier()),
        theme: crate::themes::load("Muxy", "Muxy"),
        workspace: muxy_core::store::Workspace::for_tests(Vec::new()),
        tab_workspaces: muxy_core::workspace_store::WorkspaceStore::load_from(
            directory.join("workspaces.json"),
        ),
        shortcuts: muxy_core::shortcuts::ShortcutMap::load(),
        command_shortcuts: muxy_core::store::CommandShortcuts::default(),
        worktrees: std::collections::HashMap::new(),
        socket_ingress: crate::socket::ingress::IngressQueues::default(),
        active_project_id: None,
        ide_name: None,
        appearance: muxy_ui::theme::Appearance::Dark,
        prefs,
    }
}
