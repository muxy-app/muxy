use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

use crate::backup::{ImportReport, Result};
use crate::{
    AppState, Axis, Color, Layout, PROJECT_COLORS, Pane, PaneContent, PaneId, Project, ProjectId,
    ProjectStatus, ServerId, Tab, Workspace,
};

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
