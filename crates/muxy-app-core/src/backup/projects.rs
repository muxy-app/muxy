use std::collections::{BTreeMap, BTreeSet, HashSet};

use muxy_protocol::{ProjectMutation, ProjectPatch};
use serde_json::Value;

use super::{ImportReport, Result};
use crate::{
    AppState, Axis, Layout, Pane, PaneContent, PaneId, Project, ProjectId, ProjectStatus, ServerId,
    Tab, TabId, Workspace,
};

impl AppState {
    /// Backups hold this computer's projects only, for now: exports leave
    /// remote projects out, and restoring never creates or replaces them.
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
        state.startup_commands.clear();
        state.starting_directories.clear();
        state.quick_terminal = None;
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
                    && candidate.directory == project.directory
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
            ids.entry(project.id).or_insert(project.id);
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
        for project in creates
            .iter()
            .filter(|project| project.parent_id.is_none())
            .chain(creates.iter().filter(|project| project.parent_id.is_some()))
        {
            restored.queue_project(ServerId::local(), ProjectMutation::Create(project.clone()))?;
        }
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
        restored.validate()?;
        Ok(restored)
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
    let mut settings: crate::settings::Settings = toml::from_str(source)?;
    let ids = imported.match_restore_projects(restored);
    let remap = |id: ProjectId| ids.get(&id).copied().unwrap_or(id);
    settings.worktrees.projects = settings
        .worktrees
        .projects
        .into_iter()
        .map(|(id, location)| (remap(id), location))
        .collect();
    settings.appearance.worktree_recent = settings
        .appearance
        .worktree_recent
        .into_iter()
        .map(remap)
        .collect();
    settings.appearance.hidden_worktrees = settings
        .appearance
        .hidden_worktrees
        .into_iter()
        .map(remap)
        .collect();
    settings.appearance.tab_focused_expanded = settings
        .appearance
        .tab_focused_expanded
        .into_iter()
        .map(|(id, expanded)| (remap(id), expanded))
        .collect();
    settings.ai.project_pr_prompts = settings
        .ai
        .project_pr_prompts
        .into_iter()
        .map(|(id, prompt)| {
            (
                id.parse::<ProjectId>()
                    .map_or(id, |id| remap(id).to_string()),
                prompt,
            )
        })
        .collect();
    super::settings_source(&settings)
}

#[allow(
    clippy::too_many_lines,
    reason = "Convert related legacy projects, layouts, and workspace memberships together"
)]
pub fn import_projects(
    projects: &[u8],
    workspaces: Option<&[u8]>,
    groups: Option<&[u8]>,
    assets: &BTreeMap<String, Vec<u8>>,
    settings: &mut crate::settings::Settings,
) -> Result<(AppState, ImportReport)> {
    let mut values: Vec<Value> = serde_json::from_slice(projects)?;
    values.sort_by_key(|value| value["sortOrder"].as_i64().unwrap_or(0));
    let mut state = AppState::bootstrap()?;
    let mut report = ImportReport::default();
    let mut ids = BTreeMap::new();
    for value in values {
        let id: ProjectId = value["id"].as_str().ok_or("Missing project ID")?.parse()?;
        if !value["remoteWorkspaceID"].is_null() || !value["remoteDeviceID"].is_null() {
            report.skipped.push(format!(
                "Remote project {}",
                value["name"].as_str().unwrap_or("unknown")
            ));
            continue;
        }
        if id == ProjectId::from_u128(1) {
            ids.insert(id, state.home().id);
            continue;
        }
        if ids.insert(id, id).is_some() {
            return Err("Duplicate project ID in 1.x configuration".into());
        }
        state.projects.push(Project {
            id,
            home: false,
            name: value["name"].as_str().ok_or("Missing project name")?.into(),
            directory: value["path"].as_str().ok_or("Missing project path")?.into(),
            icon: value["icon"].as_str().map(Into::into),
            logo: value["logo"]
                .as_str()
                .and_then(|name| assets.get(&format!("logos/{name}")))
                .map(|bytes| bytes.clone().into()),
            color: value["iconColor"]
                .as_str()
                .and_then(|value| value.parse().ok())
                .unwrap_or_default(),
            server_id: ServerId::local(),
            kind: None,
            parent_id: None,
            tabs: Vec::new(),
            groups: None,
            status: ProjectStatus::Available,
        });
        if let Some(prompt) = value["pullRequestPrompt"].as_str() {
            settings
                .ai
                .project_pr_prompts
                .insert(id.to_string(), prompt.into());
        }
        let location = crate::settings::WorktreeLocation {
            path_template: value["preferredWorktreePathTemplate"]
                .as_str()
                .unwrap_or_default()
                .into(),
            parent_path: value["preferredWorktreeParentPath"]
                .as_str()
                .unwrap_or_default()
                .into(),
        };
        if !location.is_default() && location.validate().is_ok() {
            settings.worktrees.projects.insert(id, location);
        }
        report.imported += 1;
    }
    import_worktrees(&mut state, assets, &mut ids, &mut report)?;
    if let Some(source) = workspaces {
        for workspace in serde_json::from_slice::<Vec<Value>>(source)? {
            let old: ProjectId = workspace["projectID"]
                .as_str()
                .ok_or("Missing project ID")?
                .parse()?;
            let Some(&parent) = ids.get(&old) else {
                continue;
            };
            let project = if workspace["worktreeID"].is_null() {
                parent
            } else {
                let worktree: ProjectId = workspace["worktreeID"]
                    .as_str()
                    .ok_or("Missing worktree ID")?
                    .parse()?;
                let Some(&project) = ids.get(&worktree) else {
                    report
                        .skipped
                        .push("Layout for an unavailable worktree".into());
                    continue;
                };
                project
            };
            let mut roots = Vec::new();
            collect_roots(&workspace["root"], &mut roots, &mut report, 0)?;
            let order = workspace["topLevelTabOrder"].as_array();
            roots.sort_by_key(|tab| {
                order
                    .and_then(|order| order.iter().position(|id| id == &tab["id"]))
                    .unwrap_or(usize::MAX)
            });
            for root in roots {
                let mut tab = Tab::terminal();
                tab.panes.clear();
                let Some(layout) =
                    import_layout(&workspace["root"], &root["id"], &mut tab.panes, 0)?
                else {
                    continue;
                };
                tab.layout = layout;
                tab.custom_title = root["customTitle"].as_str().map(Into::into);
                tab.pinned = root["isPinned"].as_bool().unwrap_or(false);
                state.project_mut(project)?.tabs.push(tab);
                report.imported += 1;
            }
            state
                .project_mut(project)?
                .tabs
                .sort_by_key(|tab| !tab.pinned);
        }
    }
    if let Some(source) = groups {
        for group in serde_json::from_slice::<Vec<Value>>(source)? {
            if group["type"].as_str().is_some_and(|kind| kind != "local") {
                continue;
            }
            state.workspaces.push(Workspace {
                id: group["id"]
                    .as_str()
                    .ok_or("Missing workspace ID")?
                    .parse()?,
                name: group["name"]
                    .as_str()
                    .ok_or("Missing workspace name")?
                    .into(),
                projects: group["projectIDs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|id| id.as_str()?.parse::<ProjectId>().ok())
                    .filter_map(|id| ids.get(&id).copied())
                    .filter(|id| *id != state.home().id)
                    .collect(),
            });
        }
    }
    state.ensure_home()?;
    state.validate()?;
    Ok((state, report))
}

fn import_worktrees(
    state: &mut AppState,
    assets: &BTreeMap<String, Vec<u8>>,
    ids: &mut BTreeMap<ProjectId, ProjectId>,
    report: &mut ImportReport,
) -> Result<()> {
    for (name, bytes) in assets {
        let Some(id) = name
            .strip_prefix("worktrees/")
            .and_then(|name| name.strip_suffix(".json"))
        else {
            continue;
        };
        let old: ProjectId = id.parse()?;
        let Some(&parent) = ids.get(&old) else {
            continue;
        };
        for value in serde_json::from_slice::<Vec<Value>>(bytes)? {
            let id: ProjectId = value["id"].as_str().ok_or("Missing worktree ID")?.parse()?;
            if value["isPrimary"].as_bool() == Some(true) {
                ids.insert(id, parent);
                continue;
            }
            if ids.insert(id, id).is_some() {
                return Err("Duplicate worktree ID".into());
            }
            state.projects.push(Project {
                id,
                home: false,
                name: value["name"]
                    .as_str()
                    .ok_or("Missing worktree name")?
                    .into(),
                directory: value["path"]
                    .as_str()
                    .ok_or("Missing worktree path")?
                    .into(),
                icon: None,
                logo: None,
                color: crate::Color::default(),
                server_id: ServerId::local(),
                kind: Some(crate::ProjectKind::Worktree),
                parent_id: Some(parent),
                tabs: Vec::new(),
                groups: None,
                status: ProjectStatus::Available,
            });
            report.imported += 1;
        }
    }
    Ok(())
}

fn collect_roots(
    node: &Value,
    roots: &mut Vec<Value>,
    report: &mut ImportReport,
    depth: usize,
) -> Result<()> {
    if depth > 32 {
        return Err("1.x layout exceeds 32 levels".into());
    }
    match node["type"].as_str() {
        Some("tabArea") => {
            for tab in node["tabArea"]["tabs"].as_array().ok_or("Missing tabs")? {
                if tab["kind"].as_str() != Some("terminal") {
                    report.skipped.push(format!(
                        "Unsupported {} tab",
                        tab["kind"].as_str().unwrap_or("unknown")
                    ));
                } else if tab["parentTabID"].is_null() {
                    roots.push(tab.clone());
                }
            }
        }
        Some("split") => {
            collect_roots(&node["split"]["first"], roots, report, depth + 1)?;
            collect_roots(&node["split"]["second"], roots, report, depth + 1)?;
        }
        _ => return Err("Invalid 1.x layout node".into()),
    }
    Ok(())
}

fn import_layout(
    node: &Value,
    root: &Value,
    panes: &mut Vec<Pane>,
    depth: usize,
) -> Result<Option<Layout>> {
    if depth > 32 || panes.len() >= 256 {
        return Err("1.x layout is too large".into());
    }
    if node["type"].as_str() == Some("tabArea") {
        let tab = node["tabArea"]["tabs"].as_array().and_then(|tabs| {
            tabs.iter().find(|tab| {
                tab["kind"].as_str() == Some("terminal")
                    && (&tab["id"] == root || &tab["parentTabID"] == root)
            })
        });
        return Ok(tab.map(|tab| {
            let id = PaneId::new();
            panes.push(Pane {
                id,
                title: tab["paneTitle"].as_str().unwrap_or("Terminal").into(),
                content: PaneContent::Terminal { session: None },
            });
            Layout::Leaf(id)
        }));
    }
    let branch = &node["split"];
    let first = import_layout(&branch["first"], root, panes, depth + 1)?;
    let second = import_layout(&branch["second"], root, panes, depth + 1)?;
    Ok(match (first, second) {
        (Some(first), Some(second)) => Some(Layout::Split {
            axis: if branch["direction"].as_str() == Some("vertical") {
                Axis::Vertical
            } else {
                Axis::Horizontal
            },
            ratio: serde_json::from_value::<f32>(branch["ratio"].clone())?.clamp(0.1, 0.9),
            first: Box::new(first),
            second: Box::new(second),
        }),
        (first, second) => first.or(second),
    })
}
