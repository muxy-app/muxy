use super::document::{edit_document, read_document, save_section};
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppLayout {
    #[default]
    ProjectFocused,
    TabFocused,
    AgentsFocused,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOrder {
    #[default]
    Manual,
    Name,
    NameDescending,
    RecentlyActive,
    DateAdded,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarCollapsedStyle {
    #[default]
    Icons,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceSize {
    #[default]
    Regular,
    Large,
    ExtraLarge,
    Huge,
}

impl InterfaceSize {
    pub const fn multiplier(self) -> f32 {
        match self {
            Self::Regular => 1.0,
            Self::Large => 1.12,
            Self::ExtraLarge => 1.24,
            Self::Huge => 1.40,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentsPanelGrouping {
    #[default]
    Agents,
    Worktrees,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentsPanelIcon {
    #[default]
    Agent,
    Project,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentsPanelDensity {
    #[default]
    Normal,
    Compact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionsLayout {
    Grid,
    List,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentsPanelDock {
    #[default]
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSessionsPosition {
    #[default]
    Right,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent appearance preferences"
)]
pub struct Appearance {
    pub layout: AppLayout,
    pub dark_theme: String,
    pub light_theme: String,
    pub interface_size: InterfaceSize,
    pub sidebar_expanded: bool,
    pub sidebar_vibrancy: bool,
    pub sidebar_vibrancy_level: u8,
    pub sidebar_expanded_width: Option<f32>,
    pub sidebar_collapsed_style: SidebarCollapsedStyle,
    pub sidebar_auto_hide: bool,
    pub status_bar_visible: bool,
    pub rail_enabled: bool,
    pub rail_expanded: bool,
    pub pane_headers: bool,
    pub open_in_app_button: bool,
    pub tips_visible: bool,
    pub onboarding_completed: bool,
    pub auto_expand_worktrees: bool,
    pub worktree_show_unread: bool,
    pub worktree_recent_order: bool,
    pub worktree_recent: Vec<crate::ProjectId>,
    pub project_recent: Vec<crate::ProjectId>,
    pub hidden_worktrees: std::collections::BTreeSet<crate::ProjectId>,
    pub tab_focused_expanded: std::collections::BTreeMap<crate::ProjectId, bool>,
    pub sidebar_servers_expanded: std::collections::BTreeMap<crate::ServerId, bool>,
    pub sidebar_remote_servers: bool,
    pub remember_projects_without_tabs: bool,
    pub projects_without_tabs_shown: bool,
    #[serde(rename = "tab_focused_focus")]
    pub sidebar_focus: bool,
    #[serde(rename = "tab_focused_project_order")]
    pub sidebar_project_order: ProjectOrder,
    pub extension_sidebar: String,
    pub language: String,
    pub agents_panel_visible: bool,
    pub agents_panel_detached: bool,
    pub agents_panel_dock: AgentsPanelDock,
    pub agents_panel_right_width: Option<f32>,
    pub agents_panel_frame: Option<[f32; 4]>,
    pub agents_panel_docked_height: Option<f32>,
    pub agents_panel_grouping: AgentsPanelGrouping,
    pub agents_panel_grouped: bool,
    pub agents_panel_icon: AgentsPanelIcon,
    pub agents_panel_density: AgentsPanelDensity,
    pub agents_panel_project_colors: bool,
    pub agents_panel_idle_time: bool,
    pub agent_sessions_visible: bool,
    pub agent_sessions_position: AgentSessionsPosition,
    pub agent_sessions_pinned: bool,
    pub agent_sessions_detached: bool,
    pub agent_sessions_frame: Option<[f32; 4]>,
    pub agent_sessions_width: Option<f32>,
    pub agent_sessions_height: Option<f32>,
    pub agent_sessions_providers: Vec<muxy_protocol::AgentProvider>,
    pub extensions_installed_layout: ExtensionsLayout,
    pub extensions_browse_layout: ExtensionsLayout,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            layout: AppLayout::default(),
            dark_theme: "Muxy".into(),
            light_theme: "Muxy Light".into(),
            interface_size: InterfaceSize::default(),
            sidebar_expanded: false,
            sidebar_vibrancy: true,
            sidebar_vibrancy_level: 50,
            sidebar_expanded_width: None,
            sidebar_collapsed_style: SidebarCollapsedStyle::default(),
            sidebar_auto_hide: false,
            status_bar_visible: true,
            rail_enabled: true,
            rail_expanded: false,
            pane_headers: true,
            open_in_app_button: true,
            tips_visible: true,
            onboarding_completed: false,
            auto_expand_worktrees: false,
            worktree_show_unread: true,
            worktree_recent_order: true,
            worktree_recent: Vec::new(),
            project_recent: Vec::new(),
            hidden_worktrees: std::collections::BTreeSet::new(),
            tab_focused_expanded: std::collections::BTreeMap::new(),
            sidebar_servers_expanded: std::collections::BTreeMap::new(),
            sidebar_remote_servers: true,
            remember_projects_without_tabs: false,
            projects_without_tabs_shown: false,
            sidebar_focus: false,
            sidebar_project_order: ProjectOrder::default(),
            extension_sidebar: String::new(),
            language: String::new(),
            agents_panel_visible: true,
            agents_panel_detached: false,
            agents_panel_dock: AgentsPanelDock::default(),
            agents_panel_right_width: None,
            agents_panel_frame: None,
            agents_panel_docked_height: None,
            agents_panel_grouping: AgentsPanelGrouping::default(),
            agents_panel_grouped: true,
            agents_panel_icon: AgentsPanelIcon::default(),
            agents_panel_density: AgentsPanelDensity::default(),
            agents_panel_project_colors: false,
            agents_panel_idle_time: false,
            agent_sessions_visible: false,
            agent_sessions_position: AgentSessionsPosition::default(),
            agent_sessions_pinned: true,
            agent_sessions_detached: false,
            agent_sessions_frame: None,
            agent_sessions_width: None,
            agent_sessions_height: None,
            agent_sessions_providers: crate::agent_sessions::PROVIDERS
                .into_iter()
                .filter(|provider| crate::agent_sessions::supported(*provider))
                .collect(),
            extensions_installed_layout: ExtensionsLayout::List,
            extensions_browse_layout: ExtensionsLayout::Grid,
        }
    }
}

impl Appearance {
    pub fn agents_panel_colors_apply(&self) -> bool {
        self.agents_panel_icon == AgentsPanelIcon::Agent
            && self.agents_panel_density == AgentsPanelDensity::Normal
    }

    pub fn server_expanded(&self, server: crate::ServerId) -> bool {
        self.sidebar_servers_expanded
            .get(&server)
            .copied()
            .unwrap_or_else(|| server.is_local())
    }

    pub fn theme(&self, dark: bool) -> &str {
        if dark {
            &self.dark_theme
        } else {
            &self.light_theme
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let document = read_document(path)?;
        document.get("appearance").cloned().map_or_else(
            || Ok(Self::default()),
            |value| value.try_into().map_err(Into::into),
        )
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        save_section(path, "appearance", self)
    }

    pub fn save_changes(&self, previous: &Self, path: &Path) -> Result<Self> {
        let current = toml::Value::try_from(self)?;
        let previous_values = toml::Value::try_from(previous)?;
        let mut changes = toml::Table::new();
        if let Some(current) = current.as_table() {
            for (key, value) in current {
                if key != "tab_focused_expanded"
                    && key != "sidebar_servers_expanded"
                    && key != "hidden_worktrees"
                    && previous_values.get(key) != Some(value)
                {
                    changes.insert(key.clone(), value.clone());
                }
            }
        }
        edit_document(path, |document| {
            let target = document.entry("appearance").or_insert(previous_values);
            let target = target
                .as_table_mut()
                .ok_or_else(|| io::Error::other("appearance must be a table"))?;
            target.extend(changes);
            let mut saved: Self = toml::Value::Table(target.clone()).try_into()?;
            for project in self.hidden_worktrees.difference(&previous.hidden_worktrees) {
                saved.hidden_worktrees.insert(*project);
            }
            for project in previous.hidden_worktrees.difference(&self.hidden_worktrees) {
                saved.hidden_worktrees.remove(project);
            }
            target.insert(
                "hidden_worktrees".into(),
                toml::Value::try_from(&saved.hidden_worktrees)?,
            );
            for (project, expanded) in &self.tab_focused_expanded {
                if previous.tab_focused_expanded.get(project) != Some(expanded) {
                    saved.tab_focused_expanded.insert(*project, *expanded);
                }
            }
            for project in previous.tab_focused_expanded.keys() {
                if !self.tab_focused_expanded.contains_key(project) {
                    saved.tab_focused_expanded.remove(project);
                }
            }
            target.insert(
                "tab_focused_expanded".into(),
                toml::Value::try_from(&saved.tab_focused_expanded)?,
            );
            for (server, expanded) in &self.sidebar_servers_expanded {
                if previous.sidebar_servers_expanded.get(server) != Some(expanded) {
                    saved.sidebar_servers_expanded.insert(*server, *expanded);
                }
            }
            for server in previous.sidebar_servers_expanded.keys() {
                if !self.sidebar_servers_expanded.contains_key(server) {
                    saved.sidebar_servers_expanded.remove(server);
                }
            }
            target.insert(
                "sidebar_servers_expanded".into(),
                toml::Value::try_from(&saved.sidebar_servers_expanded)?,
            );
            Ok(saved)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn worktree_visibility_defaults_and_independent_saved_choices() -> Result<()> {
        let first = crate::ProjectId::new();
        let second = crate::ProjectId::new();
        let directory = std::env::temp_dir().join(format!("muxy-worktree-visibility-{first}"));
        let path = directory.join("settings.toml");
        let initial = Appearance::load(&path)?;
        assert!(initial.hidden_worktrees.is_empty());
        initial.save(&path)?;
        let mut one = initial.clone();
        one.hidden_worktrees.insert(first);
        one.save_changes(&initial, &path)?;
        let mut two = initial.clone();
        two.hidden_worktrees.insert(second);
        let saved = two.save_changes(&initial, &path)?;
        assert_eq!(saved.hidden_worktrees, [first, second].into());
        let previous_one = one.clone();
        one.hidden_worktrees.remove(&first);
        let saved = one.save_changes(&previous_one, &path)?;
        assert_eq!(saved.hidden_worktrees, [second].into());
        assert_eq!(Appearance::load(&path)?, saved);
        fs::remove_dir_all(directory)?;
        Ok(())
    }
}
