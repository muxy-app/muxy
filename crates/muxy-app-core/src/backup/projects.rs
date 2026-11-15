use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use muxy_protocol::{ProjectDescriptor, ProjectMutation, ProjectPatch};
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

use super::{ImportReport, Result};
use crate::{
    AppState, Axis, Color, Layout, PROJECT_COLORS, Pane, PaneContent, PaneId, Project, ProjectId,
    ProjectStatus, ServerId, Tab, TabId, Workspace,
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

const LEGACY_HOME: ProjectId = ProjectId::from_u128(1);

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
    let mut ids = BTreeMap::from([(LEGACY_HOME, state.home().id)]);
    let mut logos = Logos::new();
    for value in values {
        let name = value["name"].as_str().unwrap_or("unknown");
        let Ok(id) = value["id"]
            .as_str()
            .unwrap_or_default()
            .parse::<ProjectId>()
        else {
            report
                .attention
                .push(format!("Project {name} (invalid ID)"));
            continue;
        };
        if id == LEGACY_HOME {
            continue;
        }
        if !value["remoteWorkspaceID"].is_null() || !value["remoteDeviceID"].is_null() {
            report.attention.push(format!("Remote project {name}"));
            continue;
        }
        if ids.contains_key(&id) {
            report
                .skipped
                .push(format!("Project {name} (duplicate ID)"));
            continue;
        }
        let project = match legacy_project(id, &value, assets, &mut logos, &mut report) {
            Ok(project) => project,
            Err(reason) => {
                report.attention.push(format!("Project {name} ({reason})"));
                continue;
            }
        };
        ids.insert(id, id);
        state.projects.push(project);
        import_project_settings(id, &value, settings);
        report.imported += 1;
    }
    import_worktrees(&mut state, assets, &mut ids, &mut report);
    for workspace in legacy_list(workspaces, "workspaces.json", &mut report) {
        if let Err(error) = import_layout_snapshot(&mut state, &workspace, &ids, &mut report) {
            report.attention.push(format!("Layout ({error})"));
        }
    }
    let mut groups = legacy_list(groups, "project-groups.json", &mut report);
    groups.sort_by_key(|group| group["sortOrder"].as_i64().unwrap_or(0));
    for group in groups {
        let name = group["name"].as_str().unwrap_or_default().trim();
        if group["type"].as_str().is_some_and(|kind| kind != "local") {
            report.attention.push(format!("Remote workspace {name}"));
            continue;
        }
        let Ok(id) = group["id"].as_str().unwrap_or_default().parse() else {
            report
                .attention
                .push(format!("Workspace {name} (invalid ID)"));
            continue;
        };
        if name.is_empty() {
            report.attention.push("Workspace without a name".into());
            continue;
        }
        state.workspaces.push(Workspace {
            id,
            name: name.into(),
            projects: group["projectIDs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|id| id.as_str()?.parse::<ProjectId>().ok())
                .filter_map(|id| ids.get(&id).copied())
                .filter(|id| *id != state.home().id)
                .collect(),
        });
        report.imported += 1;
    }
    state.ensure_home()?;
    state.validate()?;
    Ok((state, report))
}

fn legacy_list(source: Option<&[u8]>, file: &str, report: &mut ImportReport) -> Vec<Value> {
    source.map_or_else(Vec::new, |source| {
        serde_json::from_slice(source).unwrap_or_else(|_| {
            report.attention.push(format!("{file} (unreadable)"));
            Vec::new()
        })
    })
}

fn legacy_project(
    id: ProjectId,
    value: &Value,
    assets: &BTreeMap<String, Vec<u8>>,
    logos: &mut Logos,
    report: &mut ImportReport,
) -> Result<Project> {
    let name = value["name"].as_str().ok_or("missing name")?;
    let directory = PathBuf::from(value["path"].as_str().ok_or("missing folder")?);
    if !directory.is_dir() {
        return Err("folder not found".into());
    }
    let mut project = Project {
        id,
        home: false,
        name: name.into(),
        directory,
        created_at: None,
        icon: None,
        logo: None,
        color: value["iconColor"]
            .as_str()
            .and_then(legacy_color)
            .unwrap_or_default(),
        server_id: ServerId::local(),
        kind: None,
        parent_id: None,
        tabs: Vec::new(),
        groups: None,
        status: ProjectStatus::Available,
    };
    project
        .descriptor()
        .validate()
        .map_err(|_| "invalid name or folder")?;
    if let Some(icon) = value["icon"].as_str()
        && !keep_if_valid(&mut project, |project| {
            project.icon = Some(legacy_icon(icon));
        })
    {
        report.attention.push(format!("Icon of {name}"));
    }
    if let Some(logo) = value["logo"]
        .as_str()
        .and_then(|file| logos.shared(&format!("logos/{file}"), assets))
    {
        if !keep_if_valid(&mut project, |project| project.logo = Some(logo.clone())) {
            report.attention.push(format!("Logo of {name}"));
        } else if !logos.charge(logo.len()) {
            project.logo = None;
            report
                .attention
                .push(format!("Logo of {name} (the backup has too many logos)"));
        }
    }
    Ok(project)
}

const MAX_IMPORTED_LOGO_BYTES: usize = 16 * 1024 * 1024;

struct Logos {
    shared: BTreeMap<String, std::sync::Arc<[u8]>>,
    remaining: usize,
}

impl Logos {
    fn new() -> Self {
        Self {
            shared: BTreeMap::new(),
            remaining: MAX_IMPORTED_LOGO_BYTES,
        }
    }

    fn shared(
        &mut self,
        asset: &str,
        assets: &BTreeMap<String, Vec<u8>>,
    ) -> Option<std::sync::Arc<[u8]>> {
        if let Some(logo) = self.shared.get(asset) {
            return Some(logo.clone());
        }
        let logo: std::sync::Arc<[u8]> = assets.get(asset)?.as_slice().into();
        self.shared.insert(asset.to_owned(), logo.clone());
        Some(logo)
    }

    fn charge(&mut self, bytes: usize) -> bool {
        let Some(remaining) = self.remaining.checked_sub(bytes) else {
            return false;
        };
        self.remaining = remaining;
        true
    }
}

fn keep_if_valid(project: &mut Project, change: impl FnOnce(&mut Project)) -> bool {
    let mut candidate = project.clone();
    change(&mut candidate);
    let valid = candidate.descriptor().validate().is_ok();
    if valid {
        *project = candidate;
    }
    valid
}

fn legacy_icon(icon: &str) -> String {
    if icon.graphemes(true).count() == 1 || icon.starts_with("sf:") {
        icon.into()
    } else {
        format!("sf:{icon}")
    }
}

fn legacy_color(value: &str) -> Option<Color> {
    value.parse().ok().or_else(|| {
        PROJECT_COLORS
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(value))
            .and_then(|(_, hex)| hex.parse().ok())
    })
}

fn import_project_settings(id: ProjectId, value: &Value, settings: &mut crate::settings::Settings) {
    if let Some(prompt) = value["pullRequestPrompt"].as_str() {
        settings
            .ai
            .project_pr_prompts
            .entry(id.to_string())
            .or_insert_with(|| prompt.into());
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
        settings.worktrees.projects.entry(id).or_insert(location);
    }
}

fn import_worktrees(
    state: &mut AppState,
    assets: &BTreeMap<String, Vec<u8>>,
    ids: &mut BTreeMap<ProjectId, ProjectId>,
    report: &mut ImportReport,
) {
    for (file, bytes) in assets {
        let Some(old) = file
            .strip_prefix("worktrees/")
            .and_then(|name| name.strip_suffix(".json"))
            .and_then(|id| id.parse::<ProjectId>().ok())
        else {
            continue;
        };
        let Some(parent) = ids.get(&old).and_then(|id| state.project(*id)).cloned() else {
            continue;
        };
        let Ok(values) = serde_json::from_slice::<Vec<Value>>(bytes) else {
            report.attention.push(format!("{file} (unreadable)"));
            continue;
        };
        for value in values {
            let name = value["name"].as_str().unwrap_or("unknown");
            let Ok(id) = value["id"]
                .as_str()
                .unwrap_or_default()
                .parse::<ProjectId>()
            else {
                report
                    .attention
                    .push(format!("Worktree {name} (invalid ID)"));
                continue;
            };
            if value["isPrimary"].as_bool() == Some(true) {
                ids.insert(id, parent.id);
                continue;
            }
            if parent.home {
                report.attention.push(format!("Worktree {name} of Home"));
                continue;
            }
            if ids.contains_key(&id) {
                report
                    .skipped
                    .push(format!("Worktree {name} (duplicate ID)"));
                continue;
            }
            match legacy_worktree(id, &value, &parent) {
                Ok(worktree) => {
                    ids.insert(id, id);
                    state.projects.push(worktree);
                    report.imported += 1;
                }
                Err(reason) => report.attention.push(format!("Worktree {name} ({reason})")),
            }
        }
    }
}

fn legacy_worktree(id: ProjectId, value: &Value, parent: &Project) -> Result<Project> {
    let directory = PathBuf::from(value["path"].as_str().ok_or("missing folder")?);
    if !directory.join(".git").is_file() {
        return Err("worktree folder not found".into());
    }
    let worktree = Project {
        id,
        home: false,
        name: value["name"].as_str().ok_or("missing name")?.into(),
        directory,
        created_at: None,
        icon: None,
        logo: None,
        color: parent.color.clone(),
        server_id: ServerId::local(),
        kind: Some(crate::ProjectKind::Worktree),
        parent_id: Some(parent.id),
        tabs: Vec::new(),
        groups: None,
        status: ProjectStatus::Available,
    };
    worktree
        .descriptor()
        .validate()
        .map_err(|_| "invalid name or folder")?;
    Ok(worktree)
}

fn import_layout_snapshot(
    state: &mut AppState,
    workspace: &Value,
    ids: &BTreeMap<ProjectId, ProjectId>,
    report: &mut ImportReport,
) -> Result<()> {
    let old: ProjectId = workspace["projectID"]
        .as_str()
        .ok_or("Missing project ID")?
        .parse()?;
    let Some(&parent) = ids.get(&old) else {
        return Ok(());
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
            return Ok(());
        };
        project
    };
    let mut roots = Vec::new();
    collect_roots(&workspace["root"], &mut roots, report, 0)?;
    let mut order = HashMap::new();
    for (position, id) in workspace["topLevelTabOrder"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        order.entry(id.to_string()).or_insert(position);
    }
    roots.sort_by_key(|tab| {
        order
            .get(&tab["id"].to_string())
            .copied()
            .unwrap_or(usize::MAX)
    });
    let mut areas = HashMap::new();
    index_areas(&workspace["root"], &mut areas);
    let mut budget = MAX_LAYOUT_VISITS;
    let mut tabs = Vec::new();
    for root in roots {
        let mut tab = Tab::terminal();
        tab.panes.clear();
        let lookup = Lookup {
            areas: &areas,
            root: root["id"].to_string(),
        };
        let Some(layout) =
            import_layout(&workspace["root"], &lookup, &mut tab.panes, 0, &mut budget)?
        else {
            continue;
        };
        tab.layout = layout;
        tab.custom_title = root["customTitle"].as_str().map(Into::into);
        tab.sanitize_titles();
        tab.color = root["colorID"].as_str().and_then(legacy_color);
        tab.pinned = root["isPinned"].as_bool().unwrap_or(false);
        tabs.push(tab);
    }
    report.imported += tabs.len();
    let project = state.project_mut(project)?;
    project.tabs.extend(tabs);
    project.tabs.sort_by_key(|tab| !tab.pinned);
    Ok(())
}

fn is_terminal(tab: &Value) -> bool {
    !matches!(tab["kind"].as_str(), Some("extensionWebView" | "browser"))
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
                if !is_terminal(tab) {
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

const MAX_LAYOUT_VISITS: usize = 1 << 20;

type Areas<'a> = HashMap<*const Value, HashMap<String, &'a Value>>;

fn index_areas<'a>(node: &'a Value, areas: &mut Areas<'a>) {
    if node["type"].as_str() == Some("tabArea") {
        let mut first = HashMap::new();
        for tab in node["tabArea"]["tabs"].as_array().into_iter().flatten() {
            if is_terminal(tab) {
                first.entry(tab["id"].to_string()).or_insert(tab);
                first.entry(tab["parentTabID"].to_string()).or_insert(tab);
            }
        }
        areas.insert(std::ptr::from_ref(node), first);
    } else if node["type"].as_str() == Some("split") {
        index_areas(&node["split"]["first"], areas);
        index_areas(&node["split"]["second"], areas);
    }
}

struct Lookup<'a> {
    areas: &'a Areas<'a>,
    root: String,
}

fn import_layout(
    node: &Value,
    lookup: &Lookup<'_>,
    panes: &mut Vec<Pane>,
    depth: usize,
    budget: &mut usize,
) -> Result<Option<Layout>> {
    if depth > 32 || panes.len() >= 256 || *budget == 0 {
        return Err("1.x layout is too large".into());
    }
    *budget -= 1;
    if node["type"].as_str() == Some("tabArea") {
        let tab = lookup
            .areas
            .get(&std::ptr::from_ref(node))
            .and_then(|area| area.get(&lookup.root));
        return Ok(tab.map(|tab| {
            let id = PaneId::new();
            panes.push(Pane {
                id,
                title: tab["paneTitle"].as_str().unwrap_or("Terminal").into(),
                content: PaneContent::Terminal { session: None },
                proxy: false,
            });
            Layout::Leaf(id)
        }));
    }
    let branch = &node["split"];
    let first = import_layout(&branch["first"], lookup, panes, depth + 1, budget)?;
    let second = import_layout(&branch["second"], lookup, panes, depth + 1, budget)?;
    Ok(match (first, second) {
        (Some(first), Some(second)) => Some(Layout::Split {
            axis: if branch["direction"].as_str() == Some("vertical") {
                Axis::Vertical
            } else {
                Axis::Horizontal
            },
            ratio: serde_json::from_value::<f32>(branch["ratio"].clone())
                .unwrap_or(0.5)
                .clamp(0.15, 0.85),
            first: Box::new(first),
            second: Box::new(second),
        }),
        (first, second) => first.or(second),
    })
}
