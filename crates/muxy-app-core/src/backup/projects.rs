use std::collections::{BTreeMap, BTreeSet, HashSet};

use muxy_protocol::{ProjectDescriptor, ProjectMutation, ProjectPatch};

use super::Result;
use crate::{
    AppState, Layout, PaneContent, PaneId, Project, ProjectId, ServerId, TabId, Workspace,
};

impl AppState {
    #[must_use]
    pub fn configuration_backup(&self) -> Self {
        let mut state = self.clone();
        let remote: BTreeSet<_> = state
            .projects
            .iter()
            .map(|project| project.server_id)
            .filter(|server| !server.is_local())
            .collect();
        for server in remote {
            state.remove_remote_projects(server);
        }
        state.servers.clear();
        state.set_aside = None;
        state.startup_commands.clear();
        state.starting_directories.clear();
        state.starting_proxies.clear();
        state.quick_terminal = None;
        for tab in state
            .projects
            .iter_mut()
            .flat_map(|project| &mut project.tabs)
        {
            tab.window = None;
        }
        for project in &mut state.projects {
            project.sync_groups(state.window.selected_tab.get(&project.id).copied());
        }
        for pane in state
            .projects
            .iter_mut()
            .flat_map(|project| &mut project.tabs)
            .flat_map(|tab| &mut tab.panes)
        {
            if let PaneContent::Terminal { session } = &mut pane.content {
                *session = None;
            }
        }
        state
    }

    pub fn check_restore_directories(&self, current: &Self) -> Result<()> {
        let matched = self.match_restore_projects(current);
        for project in self.local_projects() {
            if !project.home && !matched.contains_key(&project.id) && !project.directory.is_dir() {
                return Err(format!("Project folder unavailable: {} ({}). Make this folder available before importing the backup.", project.name, project.directory.display()).into());
            }
        }
        Ok(())
    }

    fn local_projects(&self) -> impl Iterator<Item = &Project> {
        self.projects
            .iter()
            .filter(|project| project.server_id.is_local())
    }

    fn match_restore_projects(&self, current: &Self) -> BTreeMap<ProjectId, ProjectId> {
        let mut ids = BTreeMap::new();
        let mut claimed = HashSet::new();
        for project in self.local_projects() {
            let existing = current.local_projects().find(|candidate| {
                if project.home {
                    candidate.home
                } else {
                    !candidate.home && candidate.id == project.id
                }
            });
            if let Some(existing) = existing {
                ids.insert(project.id, existing.id);
                claimed.insert(existing.id);
            }
        }
        for project in self.local_projects() {
            if ids.contains_key(&project.id) {
                continue;
            }
            if let Some(existing) = current.local_projects().find(|candidate| {
                !candidate.home
                    && muxy_core::paths::path_eq(&candidate.directory, &project.directory)
                    && !claimed.contains(&candidate.id)
            }) {
                ids.insert(project.id, existing.id);
                claimed.insert(existing.id);
            }
        }
        ids
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Restore project identities and layouts before queuing server mutations"
    )]
    pub fn restore_configuration(&self, current: &Self) -> Result<Self> {
        self.check_restore_directories(current)?;
        let mut restored = self.configuration_backup();
        let mut ids = self.match_restore_projects(current);
        for project in &restored.projects {
            ids.entry(project.id).or_insert_with(|| {
                if current.project(project.id).is_some() {
                    ProjectId::new()
                } else {
                    project.id
                }
            });
        }
        restored.window.current_project = ids[&restored.window.current_project];
        let selected = restored.window.selected_tab.clone();
        let active = restored.window.active_pane;
        restored.window.selected_tab.clear();
        restored.window.active_pane = None;
        restored.window.focus_history.clear();
        for project in &mut restored.projects {
            let selected_tab = selected.get(&project.id);
            project.id = ids[&project.id];
            project.parent_id = project.parent_id.map(|id| ids[&id]);
            if let Some(existing) = current.project(project.id) {
                project.directory.clone_from(&existing.directory);
                project.kind = existing.kind;
                project.parent_id = existing.parent_id;
            }
            let mut tabs = BTreeMap::new();
            for tab in &mut project.tabs {
                let selected = selected_tab == Some(&tab.id);
                let id = TabId::new();
                tabs.insert(tab.id, id);
                tab.id = id;
                if selected {
                    restored.window.selected_tab.insert(project.id, tab.id);
                }
                let mut panes = BTreeMap::new();
                for pane in &mut tab.panes {
                    let id = PaneId::new();
                    panes.insert(pane.id, id);
                    if active == Some(pane.id) {
                        restored.window.active_pane = Some(id);
                    }
                    pane.id = id;
                }
                remap_layout(&mut tab.layout, &panes);
                tab.zoomed = tab.zoomed.map(|id| panes[&id]);
            }
            if let Some(groups) = &mut project.groups {
                groups.remap_tabs(&tabs);
            }
            if let Some(tab) = project.tabs.first() {
                restored
                    .window
                    .selected_tab
                    .entry(project.id)
                    .or_insert(tab.id);
            }
        }
        for workspace in &mut restored.workspaces {
            workspace.projects = workspace
                .projects
                .iter()
                .filter_map(|id| ids.get(id).copied())
                .collect();
        }
        restored.servers.clone_from(&current.servers);
        restored.set_aside.clone_from(&current.set_aside);
        if let Some(local) = restored.servers.get_mut(&ServerId::local()) {
            local.pending_cancellations.clear();
            local.pending_discards.clear();
            local.close_operations.clear();
        }
        let imported: HashSet<_> = restored.projects.iter().map(|project| project.id).collect();
        for project in &current.projects {
            if !imported.contains(&project.id) {
                restored.projects.push(project.clone());
                if let Some(tab) = current.window.selected_tab.get(&project.id) {
                    restored.window.selected_tab.insert(project.id, *tab);
                }
            }
        }
        for workspace in &current.workspaces {
            if let Some(replaced) = restored
                .workspaces
                .iter_mut()
                .find(|candidate| candidate.id == workspace.id)
            {
                replaced
                    .projects
                    .extend(workspace.projects.iter().filter(|project| {
                        current
                            .project_server(**project)
                            .is_some_and(|server| !server.is_local())
                    }));
            } else {
                restored.workspaces.push(workspace.clone());
            }
        }
        let creates: Vec<_> = restored
            .projects
            .iter()
            .filter(|project| !project.home && current.project(project.id).is_none())
            .map(Project::descriptor)
            .collect();
        restored.queue_creates(&creates)?;
        let patches: Vec<_> = restored
            .projects
            .iter()
            .filter(|project| imported.contains(&project.id))
            .filter_map(|project| {
                current
                    .project(project.id)
                    .map(|previous| (project, previous))
            })
            .flat_map(|(project, previous)| {
                [
                    (project.name != previous.name)
                        .then(|| (project.id, ProjectPatch::Name(project.name.clone()))),
                    (project.icon != previous.icon)
                        .then(|| (project.id, ProjectPatch::Icon(project.icon.clone()))),
                    (project.color != previous.color)
                        .then(|| (project.id, ProjectPatch::Color(project.color.to_string()))),
                    (project.logo != previous.logo)
                        .then(|| (project.id, ProjectPatch::Logo(project.logo.clone()))),
                ]
                .into_iter()
                .flatten()
            })
            .collect();
        for (project, patch) in patches {
            restored.patch_project(project, patch)?;
        }
        restored.ensure_home()?;
        restored.focus_selected_tab();
        restored.retain_one_settings_pane()?;
        restored.remove_saved_devices_panes()?;
        restored.validate()?;
        Ok(restored)
    }

    pub fn merge_configuration(&self, current: &Self) -> Result<Self> {
        let imported = self.configuration_backup();
        let mut ids = imported.match_restore_projects(current);
        let mut merged = current.clone();
        let mut creates = Vec::new();
        for project in imported.local_projects() {
            if let Some(&existing) = ids.get(&project.id) {
                let existing = merged.project_mut(existing)?;
                if existing.tabs.is_empty() {
                    existing.tabs.clone_from(&project.tabs);
                }
                continue;
            }
            if !project.directory.is_dir() {
                continue;
            }
            let parent = match project.parent_id {
                None => None,
                Some(parent) => match ids.get(&parent).and_then(|id| merged.project(*id)) {
                    Some(parent)
                        if !parent.home && parent.kind.is_none() && parent.parent_id.is_none() =>
                    {
                        Some(parent.id)
                    }
                    _ => continue,
                },
            };
            let mut project = project.clone();
            project.parent_id = parent;
            let id = if merged.project(project.id).is_some() {
                ProjectId::new()
            } else {
                project.id
            };
            ids.insert(project.id, id);
            project.id = id;
            creates.push(project.descriptor());
            merged.projects.push(project);
        }
        merged.queue_creates(&creates)?;
        for workspace in &imported.workspaces {
            let members = workspace
                .projects
                .iter()
                .filter_map(|id| ids.get(id).copied());
            if let Some(existing) = merged.workspaces.iter_mut().find(|candidate| {
                candidate.id == workspace.id
                    || candidate.name.trim().to_lowercase() == workspace.name.trim().to_lowercase()
            }) {
                existing.projects.extend(members);
            } else {
                merged.workspaces.push(Workspace {
                    id: workspace.id,
                    name: workspace.name.clone(),
                    projects: members.collect(),
                });
            }
        }
        merged.retain_workspace_members();
        merged.ensure_home()?;
        merged.focus_selected_tab();
        merged.retain_one_settings_pane()?;
        merged.remove_saved_devices_panes()?;
        merged.validate()?;
        Ok(merged)
    }

    fn queue_creates(&mut self, projects: &[ProjectDescriptor]) -> Result<()> {
        for project in projects
            .iter()
            .filter(|project| project.parent_id.is_none())
            .chain(
                projects
                    .iter()
                    .filter(|project| project.parent_id.is_some()),
            )
        {
            self.queue_project(ServerId::local(), ProjectMutation::Create(project.clone()))?;
        }
        Ok(())
    }
}

fn remap_layout(layout: &mut Layout, panes: &BTreeMap<PaneId, PaneId>) {
    match layout {
        Layout::Leaf(id) => *id = panes[id],
        Layout::Split { first, second, .. } => {
            remap_layout(first, panes);
            remap_layout(second, panes);
        }
    }
}

pub fn remap_settings(source: &str, imported: &AppState, restored: &AppState) -> Result<String> {
    super::settings_source(&remap_project_settings(
        toml::from_str(source)?,
        imported,
        restored,
    ))
}

#[must_use]
pub fn remap_project_settings(
    mut settings: crate::settings::Settings,
    imported: &AppState,
    restored: &AppState,
) -> crate::settings::Settings {
    let ids = imported.match_restore_projects(restored);
    let remap = |id: ProjectId| ids.get(&id).copied().unwrap_or(id);
    settings.worktrees.projects = remap_keys(settings.worktrees.projects, |id| remap(*id));
    settings.appearance.worktree_recent = settings
        .appearance
        .worktree_recent
        .into_iter()
        .map(remap)
        .collect();
    settings.appearance.project_recent = settings
        .appearance
        .project_recent
        .into_iter()
        .map(remap)
        .collect();
    settings.appearance.hidden_worktrees = settings
        .appearance
        .hidden_worktrees
        .into_iter()
        .map(remap)
        .collect();
    settings.appearance.tab_focused_expanded =
        remap_keys(settings.appearance.tab_focused_expanded, |id| remap(*id));
    settings.ai.project_pr_prompts = remap_keys(settings.ai.project_pr_prompts, |id| {
        id.parse::<ProjectId>()
            .map_or_else(|_| id.clone(), |id| remap(id).to_string())
    });
    settings
}

fn remap_keys<K: Ord, V>(entries: BTreeMap<K, V>, remap: impl Fn(&K) -> K) -> BTreeMap<K, V> {
    let (kept, moved): (Vec<_>, Vec<_>) =
        entries.into_iter().partition(|(key, _)| remap(key) == *key);
    moved
        .into_iter()
        .map(|(key, value)| (remap(&key), value))
        .chain(kept)
        .collect()
}
