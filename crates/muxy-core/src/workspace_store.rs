use crate::store;
use crate::workspace::{Axis, SplitNode, Tab, TabArea, TabKind, TopLevelTabNode, WorkspaceState};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct WorkspaceStore {
    path: PathBuf,
    states: Vec<WorkspaceState>,
    snapshots: Vec<RawSnapshot>,
    raw_tabs: HashMap<String, Map<String, Value>>,
    unparsed: Vec<Value>,
}

#[derive(Debug, Clone)]
struct RawSnapshot {
    key: WorkspaceKey,
    workspace: Map<String, Value>,
    tabs: HashMap<String, Map<String, Value>>,
    areas: HashMap<String, Map<String, Value>>,
    nodes: HashMap<String, Map<String, Value>>,
    first_area: Option<Map<String, Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceKey {
    project_id: String,
    worktree_id: Option<String>,
    path: Option<String>,
}

impl WorkspaceStore {
    pub fn load() -> Self {
        Self::load_from(crate::prefs::app_support_dir().join("workspaces.json"))
    }

    pub fn load_from(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut states = Vec::new();
        let mut snapshots = Vec::new();
        let mut raw_tabs = HashMap::new();
        let mut unparsed = Vec::new();
        if let Ok(contents) = std::fs::read(&path)
            && let Ok(Value::Array(workspaces)) = serde_json::from_slice::<Value>(&contents)
        {
            for workspace in workspaces {
                if let Some((state, snapshot)) = decode_workspace(workspace.clone()) {
                    for (id, tab) in &snapshot.tabs {
                        raw_tabs.entry(id.clone()).or_insert_with(|| tab.clone());
                    }
                    states.push(state);
                    snapshots.push(snapshot);
                } else {
                    unparsed.push(workspace);
                }
            }
        }
        Self {
            path,
            states,
            snapshots,
            raw_tabs,
            unparsed,
        }
    }

    pub fn states(&self) -> &[WorkspaceState] {
        &self.states
    }

    pub fn states_mut(&mut self) -> &mut [WorkspaceState] {
        &mut self.states
    }

    pub fn active(&self, project_id: &str, project_path: &str) -> Option<&WorkspaceState> {
        self.states
            .iter()
            .find(|state| {
                project_matches(state, project_id) && state_matches_path(state, project_path)
            })
            .or_else(|| self.project(project_id))
    }

    pub fn active_mut(
        &mut self,
        project_id: &str,
        project_path: &str,
    ) -> Option<&mut WorkspaceState> {
        let index = self
            .states
            .iter()
            .position(|state| {
                project_matches(state, project_id) && state_matches_path(state, project_path)
            })
            .or_else(|| self.project_index(project_id))?;
        self.states.get_mut(index)
    }

    pub fn project(&self, project_id: &str) -> Option<&WorkspaceState> {
        self.project_index(project_id)
            .and_then(|index| self.states.get(index))
    }

    pub fn worktree(&self, project_id: &str, worktree_id: &str) -> Option<&WorkspaceState> {
        self.states.iter().find(|state| {
            project_matches(state, project_id)
                && state
                    .worktree_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(worktree_id))
        })
    }

    pub fn worktree_mut(
        &mut self,
        project_id: &str,
        worktree_id: &str,
    ) -> Option<&mut WorkspaceState> {
        self.states.iter_mut().find(|state| {
            project_matches(state, project_id)
                && state
                    .worktree_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(worktree_id))
        })
    }

    pub fn ensure_project(
        &mut self,
        project_id: impl Into<String>,
        project_path: impl Into<String>,
    ) -> &mut WorkspaceState {
        let project_id = project_id.into();
        let project_path = project_path.into();
        if let Some(index) = self.states.iter().position(|state| {
            project_matches(state, &project_id) && state_matches_path(state, &project_path)
        }) {
            return &mut self.states[index];
        }
        if let Some(index) = self
            .states
            .iter()
            .position(|state| project_matches(state, &project_id) && state.worktree_id.is_none())
        {
            return &mut self.states[index];
        }
        let mut state = WorkspaceState::new(project_id);
        let mut tab = Tab::new(TabKind::Terminal);
        tab.project_path = Some(project_path);
        state.new_top_level_tab(tab);
        self.states.push(state);
        self.states.last_mut().unwrap()
    }

    pub fn ensure_worktree(
        &mut self,
        project_id: &str,
        worktree_id: &str,
        worktree_path: &str,
    ) -> &mut WorkspaceState {
        if let Some(index) = self.states.iter().position(|state| {
            project_matches(state, project_id)
                && state.worktree_id.as_deref() == Some(worktree_id)
                && state_matches_path(state, worktree_path)
        }) {
            return &mut self.states[index];
        }
        if let Some(index) = self.states.iter().position(|state| {
            project_matches(state, project_id)
                && state.worktree_id.is_none()
                && state_matches_path(state, worktree_path)
        }) {
            let previous = WorkspaceKey::from_state(&self.states[index]);
            self.states[index].worktree_id = Some(worktree_id.to_owned());
            self.states[index].worktree_path = Some(worktree_path.to_owned());
            let updated = WorkspaceKey::from_state(&self.states[index]);
            if let Some(snapshot) = self
                .snapshots
                .iter_mut()
                .find(|snapshot| snapshot.key == previous)
            {
                snapshot.key = updated;
            }
            return &mut self.states[index];
        }
        let mut state = WorkspaceState::with_worktree(project_id, worktree_id, worktree_path);
        let mut tab = Tab::new(TabKind::Terminal);
        tab.project_path = Some(worktree_path.to_owned());
        state.new_top_level_tab(tab);
        self.states.push(state);
        self.states.last_mut().unwrap()
    }

    pub fn remove_workspace(&mut self, project_id: &str, project_path: &str) -> bool {
        let Some(index) = self.states.iter().position(|state| {
            project_matches(state, project_id) && state_matches_path(state, project_path)
        }) else {
            return false;
        };
        let removed = self.states.remove(index);
        let key = WorkspaceKey::from_state(&removed);
        self.snapshots.retain(|snapshot| snapshot.key != key);
        true
    }

    pub fn remove_worktree(
        &mut self,
        project_id: &str,
        worktree_id: &str,
    ) -> Option<WorkspaceState> {
        let index = self.states.iter().position(|state| {
            project_matches(state, project_id)
                && state
                    .worktree_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(worktree_id))
        })?;
        let removed = self.states.remove(index);
        self.snapshots.retain(|snapshot| {
            !snapshot.key.project_id.eq_ignore_ascii_case(project_id)
                || snapshot
                    .key
                    .worktree_id
                    .as_deref()
                    .is_none_or(|id| !id.eq_ignore_ascii_case(worktree_id))
        });
        Some(removed)
    }

    pub fn has_project(&self, project_id: &str) -> bool {
        self.states
            .iter()
            .any(|state| project_matches(state, project_id))
    }

    pub fn remove_project(&mut self, project_id: &str) -> bool {
        let previous_len = self.states.len();
        self.states
            .retain(|state| !state.project_id.eq_ignore_ascii_case(project_id));
        self.snapshots
            .retain(|snapshot| !snapshot.key.project_id.eq_ignore_ascii_case(project_id));
        self.unparsed.retain(|snapshot| {
            snapshot
                .get("projectID")
                .and_then(Value::as_str)
                .is_none_or(|id| !id.eq_ignore_ascii_case(project_id))
        });
        self.states.len() != previous_len
    }

    pub fn save(&self) -> io::Result<()> {
        let mut workspaces: Vec<Value> = self
            .states
            .iter()
            .map(|state| encode_workspace(state, self.snapshot_for(state), &self.raw_tabs))
            .collect();
        workspaces.extend(self.unparsed.iter().cloned());
        let contents = serde_json::to_vec_pretty(&workspaces)?;
        store::write_atomic(&self.path, &contents)
    }

    fn project_index(&self, project_id: &str) -> Option<usize> {
        self.states
            .iter()
            .position(|state| project_matches(state, project_id) && state.worktree_id.is_none())
            .or_else(|| {
                self.states
                    .iter()
                    .position(|state| project_matches(state, project_id))
            })
    }

    fn snapshot_for(&self, state: &WorkspaceState) -> Option<&RawSnapshot> {
        let key = WorkspaceKey::from_state(state);
        self.snapshots
            .iter()
            .find(|snapshot| snapshot.key == key)
            .or_else(|| {
                self.snapshots.iter().find(|snapshot| {
                    snapshot.key.project_id == key.project_id
                        && snapshot.key.worktree_id == key.worktree_id
                })
            })
    }
}

impl WorkspaceKey {
    fn from_state(state: &WorkspaceState) -> Self {
        Self {
            project_id: state.project_id.to_ascii_lowercase(),
            worktree_id: state.worktree_id.clone(),
            path: state.worktree_path.clone(),
        }
    }
}

impl RawSnapshot {
    fn new(key: WorkspaceKey, workspace: Map<String, Value>) -> Self {
        Self {
            key,
            workspace,
            tabs: HashMap::new(),
            areas: HashMap::new(),
            nodes: HashMap::new(),
            first_area: None,
        }
    }
}

fn project_matches(state: &WorkspaceState, project_id: &str) -> bool {
    state.project_id.eq_ignore_ascii_case(project_id)
}

fn state_matches_path(state: &WorkspaceState, project_path: &str) -> bool {
    state.worktree_path.as_deref() == Some(project_path)
        || state.root.as_ref().is_some_and(|root| {
            root.tabs()
                .iter()
                .any(|tab| tab.project_path.as_deref() == Some(project_path))
        })
}

fn decode_workspace(value: Value) -> Option<(WorkspaceState, RawSnapshot)> {
    let workspace = value.as_object()?.clone();
    let project_id = string_field(&workspace, "projectID")?;
    if project_id.trim().is_empty() {
        return None;
    }
    let worktree_id = optional_string_field(&workspace, "worktreeID");
    let worktree_path = optional_string_field(&workspace, "worktreePath");
    let key = WorkspaceKey {
        project_id: project_id.to_ascii_lowercase(),
        worktree_id: worktree_id.clone(),
        path: worktree_path.clone(),
    };
    let mut snapshot = RawSnapshot::new(key, workspace.clone());
    let root_value = workspace.get("root")?;
    collect_raw_root(root_value, &mut snapshot);
    let root = decode_split_node(root_value)?;
    record_physical_nodes(root_value, &root, &mut snapshot.nodes);
    let top_level_order = string_array_field(&workspace, "topLevelTabOrder");
    let focused_area_id = optional_string_field(&workspace, "focusedAreaID");
    let decoded_layout = workspace
        .get("topLevelTabLayout")
        .and_then(decode_top_level_node);
    if let (Some(raw_layout), Some(layout)) =
        (workspace.get("topLevelTabLayout"), decoded_layout.as_ref())
    {
        record_top_level_nodes(raw_layout, layout, &mut snapshot.nodes);
    }

    let mut state = WorkspaceState::new(project_id);
    state.worktree_id = worktree_id;
    state.worktree_path = worktree_path;
    state.root = Some(root);
    state.top_level_order = top_level_order;
    state.focused_area_id = focused_area_id;
    state.top_level_root = decoded_layout.or_else(|| {
        let tab_ids = if state.top_level_order.is_empty() {
            state.root_tab_ids()
        } else {
            state.top_level_order.clone()
        };
        let active_tab_id = tab_ids.first().cloned();
        Some(TopLevelTabNode::group(tab_ids, active_tab_id))
    });
    state.reconcile();
    Some((state, snapshot))
}

fn decode_split_node(value: &Value) -> Option<SplitNode> {
    let node = value.as_object()?;
    match string_field(node, "type")?.as_str() {
        "tabArea" => {
            let stored_area = node.get("tabArea")?.as_object()?;
            let id = string_field(stored_area, "id")?;
            let project_path = string_field(stored_area, "projectPath")?;
            let stored_tabs = stored_area.get("tabs")?.as_array()?;
            let tabs = stored_tabs
                .iter()
                .filter_map(|tab| decode_tab(tab, &project_path))
                .collect::<Vec<_>>();
            let active_tab_id = stored_area
                .get("activeTabIndex")
                .and_then(Value::as_u64)
                .and_then(|index| stored_tabs.get(index as usize))
                .and_then(Value::as_object)
                .and_then(|tab| tab.get("id"))
                .and_then(Value::as_str)
                .filter(|id| tabs.iter().any(|tab| tab.id == *id))
                .map(str::to_owned);
            Some(SplitNode::area(TabArea {
                id,
                tabs,
                active_tab_id,
                tab_history: Vec::new(),
            }))
        }
        "split" => {
            let split = node.get("split")?.as_object()?;
            let axis = decode_axis(split.get("direction")?)?;
            let ratio = split.get("ratio")?.as_f64()? as f32;
            let first = decode_split_node(split.get("first")?)?;
            let second = decode_split_node(split.get("second")?)?;
            Some(SplitNode::split(axis, ratio, first, second))
        }
        _ => None,
    }
}

fn decode_tab(value: &Value, area_project_path: &str) -> Option<Tab> {
    let stored = value.as_object()?;
    let id = string_field(stored, "id")?;
    if id.is_empty() {
        return None;
    }
    let extension_id = optional_string_field(stored, "extensionID");
    let extension_tab_type_id = optional_string_field(stored, "extensionTabTypeID");
    let kind = match optional_string_field(stored, "kind").as_deref() {
        Some("browser") => TabKind::Browser,
        Some("extensionWebView" | "extension")
            if extension_id.as_deref().is_some_and(|id| !id.is_empty())
                && extension_tab_type_id
                    .as_deref()
                    .is_some_and(|id| !id.is_empty()) =>
        {
            TabKind::ExtensionWebView
        }
        _ => TabKind::Terminal,
    };
    Some(Tab {
        id,
        kind,
        parent_id: optional_string_field(stored, "parentTabID"),
        project_path: optional_string_field(stored, "projectPath")
            .or_else(|| Some(area_project_path.to_owned())),
        custom_title: optional_string_field(stored, "customTitle"),
        color_id: optional_string_field(stored, "colorID"),
        custom_icon: optional_string_field(stored, "customIcon"),
        pinned: stored
            .get("isPinned")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        pane_title: optional_string_field(stored, "paneTitle"),
        static_title: None,
        browser_url: optional_string_field(stored, "browserURL"),
        browser_profile: optional_string_field(stored, "browserProfileID"),
        extension_id,
        extension_web_view_id: extension_tab_type_id,
        extension_data: stored
            .get("extensionTabData")
            .filter(|value| !value.is_null())
            .cloned(),
    })
}

fn decode_top_level_node(value: &Value) -> Option<TopLevelTabNode> {
    let node = value.as_object()?;
    match string_field(node, "type")?.as_str() {
        "group" => {
            let group = node.get("group")?.as_object()?;
            let tab_ids = string_array_field(group, "tabIDs");
            let active_tab_id = optional_string_field(group, "activeTabID");
            Some(TopLevelTabNode::group(tab_ids, active_tab_id))
        }
        "split" => {
            let split = node.get("split")?.as_object()?;
            let axis = decode_axis(split.get("direction")?)?;
            let ratio = split.get("ratio")?.as_f64()? as f32;
            let first = decode_top_level_node(split.get("first")?)?;
            let second = decode_top_level_node(split.get("second")?)?;
            Some(TopLevelTabNode::split(axis, ratio, first, second))
        }
        _ => None,
    }
}

fn decode_axis(value: &Value) -> Option<Axis> {
    match value.as_str()? {
        "horizontal" => Some(Axis::Horizontal),
        "vertical" => Some(Axis::Vertical),
        _ => None,
    }
}

fn collect_raw_root(value: &Value, snapshot: &mut RawSnapshot) {
    let Some(node) = value.as_object() else {
        return;
    };
    match node.get("type").and_then(Value::as_str) {
        Some("tabArea") => {
            let Some(area) = node.get("tabArea").and_then(Value::as_object) else {
                return;
            };
            if snapshot.first_area.is_none() {
                snapshot.first_area = Some(area.clone());
            }
            if let Some(id) = area.get("id").and_then(Value::as_str) {
                snapshot
                    .areas
                    .entry(id.to_owned())
                    .or_insert_with(|| area.clone());
            }
            if let Some(tabs) = area.get("tabs").and_then(Value::as_array) {
                for tab in tabs {
                    let Some(stored) = tab.as_object() else {
                        continue;
                    };
                    let Some(id) = stored.get("id").and_then(Value::as_str) else {
                        continue;
                    };
                    snapshot
                        .tabs
                        .entry(id.to_owned())
                        .or_insert_with(|| stored.clone());
                }
            }
        }
        Some("split") => {
            let Some(split) = node.get("split").and_then(Value::as_object) else {
                return;
            };
            if let Some(first) = split.get("first") {
                collect_raw_root(first, snapshot);
            }
            if let Some(second) = split.get("second") {
                collect_raw_root(second, snapshot);
            }
        }
        _ => {}
    }
}

fn record_physical_nodes(
    raw: &Value,
    node: &SplitNode,
    nodes: &mut HashMap<String, Map<String, Value>>,
) {
    let Some(raw_node) = raw.as_object() else {
        return;
    };
    match node {
        SplitNode::Area { area } => {
            nodes.insert(format!("physical-area:{}", area.id), raw_node.clone());
        }
        SplitNode::Split {
            id, first, second, ..
        } => {
            nodes.insert(format!("physical-split:{id}"), raw_node.clone());
            let Some(raw_split) = raw_node.get("split").and_then(Value::as_object) else {
                return;
            };
            if let Some(raw_first) = raw_split.get("first") {
                record_physical_nodes(raw_first, first, nodes);
            }
            if let Some(raw_second) = raw_split.get("second") {
                record_physical_nodes(raw_second, second, nodes);
            }
        }
    }
}

fn record_top_level_nodes(
    raw: &Value,
    node: &TopLevelTabNode,
    nodes: &mut HashMap<String, Map<String, Value>>,
) {
    let Some(raw_node) = raw.as_object() else {
        return;
    };
    match node {
        TopLevelTabNode::Group { id, .. } => {
            nodes.insert(format!("top-group:{id}"), raw_node.clone());
        }
        TopLevelTabNode::Split {
            id, first, second, ..
        } => {
            nodes.insert(format!("top-split:{id}"), raw_node.clone());
            let Some(raw_split) = raw_node.get("split").and_then(Value::as_object) else {
                return;
            };
            if let Some(raw_first) = raw_split.get("first") {
                record_top_level_nodes(raw_first, first, nodes);
            }
            if let Some(raw_second) = raw_split.get("second") {
                record_top_level_nodes(raw_second, second, nodes);
            }
        }
    }
}

fn encode_workspace(
    state: &WorkspaceState,
    snapshot: Option<&RawSnapshot>,
    raw_tabs: &HashMap<String, Map<String, Value>>,
) -> Value {
    let mut workspace = snapshot
        .map(|snapshot| snapshot.workspace.clone())
        .unwrap_or_default();
    workspace.insert("projectID".into(), Value::String(state.project_id.clone()));
    set_optional_string(&mut workspace, "worktreeID", state.worktree_id.as_ref());
    set_optional_string(&mut workspace, "worktreePath", state.worktree_path.as_ref());
    set_optional_string(
        &mut workspace,
        "focusedAreaID",
        state.focused_area_id.as_ref(),
    );
    if state.top_level_order.is_empty() {
        workspace.remove("topLevelTabOrder");
    } else {
        workspace.insert(
            "topLevelTabOrder".into(),
            Value::Array(
                state
                    .top_level_order
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
    }
    if let Some(layout) = &state.top_level_root {
        workspace.insert(
            "topLevelTabLayout".into(),
            encode_top_level_node(layout, snapshot),
        );
    } else {
        workspace.remove("topLevelTabLayout");
    }
    let root = state
        .root
        .as_ref()
        .map(|root| encode_split_node(root, snapshot, raw_tabs))
        .unwrap_or_else(|| encode_empty_root(state, snapshot));
    workspace.insert("root".into(), root);
    Value::Object(workspace)
}

fn encode_split_node(
    node: &SplitNode,
    snapshot: Option<&RawSnapshot>,
    raw_tabs: &HashMap<String, Map<String, Value>>,
) -> Value {
    match node {
        SplitNode::Area { area } => {
            let raw_area = snapshot.and_then(|snapshot| snapshot.areas.get(&area.id));
            let mut stored_area = raw_area.cloned().unwrap_or_default();
            let project_path = raw_area
                .and_then(|area| area.get("projectPath"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    area.active_tab()
                        .and_then(|tab| tab.project_path.clone())
                        .or_else(|| area.tabs.iter().find_map(|tab| tab.project_path.clone()))
                })
                .unwrap_or_default();
            stored_area.insert("id".into(), Value::String(area.id.clone()));
            stored_area.insert("projectPath".into(), Value::String(project_path.clone()));
            stored_area.insert(
                "tabs".into(),
                Value::Array(
                    area.tabs
                        .iter()
                        .map(|tab| encode_tab(tab, &project_path, raw_tabs))
                        .collect(),
                ),
            );
            let active_index = area
                .active_tab_id
                .as_deref()
                .and_then(|active| area.tabs.iter().position(|tab| tab.id == active))
                .unwrap_or_default();
            stored_area.insert("activeTabIndex".into(), Value::from(active_index as u64));
            wrapped_node_from_raw(
                snapshot,
                &format!("physical-area:{}", area.id),
                "tabArea",
                "tabArea",
                Value::Object(stored_area),
            )
        }
        SplitNode::Split {
            id,
            axis,
            ratio,
            first,
            second,
        } => {
            let node_key = format!("physical-split:{id}");
            let mut split = snapshot
                .and_then(|snapshot| snapshot.nodes.get(&node_key))
                .and_then(|node| node.get("split"))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            split.insert("direction".into(), Value::String(encode_axis(*axis).into()));
            split.insert("ratio".into(), ratio_value(*ratio));
            split.insert("first".into(), encode_split_node(first, snapshot, raw_tabs));
            split.insert(
                "second".into(),
                encode_split_node(second, snapshot, raw_tabs),
            );
            wrapped_node_from_raw(snapshot, &node_key, "split", "split", Value::Object(split))
        }
    }
}

fn encode_empty_root(state: &WorkspaceState, snapshot: Option<&RawSnapshot>) -> Value {
    let raw_area = snapshot.and_then(|snapshot| snapshot.first_area.as_ref());
    let mut area = raw_area.cloned().unwrap_or_default();
    let id = raw_area
        .and_then(|area| area.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(store::new_uuid);
    let project_path = raw_area
        .and_then(|area| area.get("projectPath"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| state.worktree_path.clone())
        .unwrap_or_default();
    area.insert("id".into(), Value::String(id.clone()));
    area.insert("projectPath".into(), Value::String(project_path));
    area.insert("tabs".into(), Value::Array(Vec::new()));
    area.remove("activeTabIndex");
    wrapped_node_from_raw(
        snapshot,
        &format!("physical-area:{id}"),
        "tabArea",
        "tabArea",
        Value::Object(area),
    )
}

fn encode_tab(
    tab: &Tab,
    area_project_path: &str,
    raw_tabs: &HashMap<String, Map<String, Value>>,
) -> Value {
    let raw = raw_tabs.get(&tab.id);
    let mut stored = raw.cloned().unwrap_or_default();
    stored.insert(
        "kind".into(),
        Value::String(
            match tab.kind {
                TabKind::Terminal => "terminal",
                TabKind::Browser => "browser",
                TabKind::ExtensionWebView => "extensionWebView",
            }
            .into(),
        ),
    );
    stored.insert("id".into(), Value::String(tab.id.clone()));
    set_optional_string(&mut stored, "parentTabID", tab.parent_id.as_ref());
    set_optional_string(&mut stored, "customTitle", tab.custom_title.as_ref());
    set_optional_string(&mut stored, "colorID", tab.color_id.as_ref());
    set_optional_string(&mut stored, "customIcon", tab.custom_icon.as_ref());
    stored.insert("isPinned".into(), Value::Bool(tab.pinned));
    stored.insert(
        "projectPath".into(),
        Value::String(
            tab.project_path
                .clone()
                .unwrap_or_else(|| area_project_path.to_owned()),
        ),
    );
    set_optional_string(&mut stored, "paneTitle", tab.pane_title.as_ref());
    set_optional_string(&mut stored, "extensionID", tab.extension_id.as_ref());
    set_optional_string(
        &mut stored,
        "extensionTabTypeID",
        tab.extension_web_view_id.as_ref(),
    );
    if let Some(data) = &tab.extension_data {
        stored.insert("extensionTabData".into(), data.clone());
    } else {
        stored.remove("extensionTabData");
    }
    set_optional_string(&mut stored, "browserURL", tab.browser_url.as_ref());
    set_optional_string(
        &mut stored,
        "browserProfileID",
        tab.browser_profile.as_ref(),
    );
    Value::Object(stored)
}

fn encode_top_level_node(node: &TopLevelTabNode, snapshot: Option<&RawSnapshot>) -> Value {
    match node {
        TopLevelTabNode::Group {
            id,
            tab_ids,
            active_tab_id,
        } => {
            let node_key = format!("top-group:{id}");
            let mut group = snapshot
                .and_then(|snapshot| snapshot.nodes.get(&node_key))
                .and_then(|node| node.get("group"))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            group.insert(
                "tabIDs".into(),
                Value::Array(tab_ids.iter().cloned().map(Value::String).collect()),
            );
            set_optional_string(&mut group, "activeTabID", active_tab_id.as_ref());
            wrapped_node_from_raw(snapshot, &node_key, "group", "group", Value::Object(group))
        }
        TopLevelTabNode::Split {
            id,
            axis,
            ratio,
            first,
            second,
        } => {
            let node_key = format!("top-split:{id}");
            let mut split = snapshot
                .and_then(|snapshot| snapshot.nodes.get(&node_key))
                .and_then(|node| node.get("split"))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            split.insert("direction".into(), Value::String(encode_axis(*axis).into()));
            split.insert("ratio".into(), ratio_value(*ratio));
            split.insert("first".into(), encode_top_level_node(first, snapshot));
            split.insert("second".into(), encode_top_level_node(second, snapshot));
            wrapped_node_from_raw(snapshot, &node_key, "split", "split", Value::Object(split))
        }
    }
}

fn wrapped_node_from_raw(
    snapshot: Option<&RawSnapshot>,
    node_key: &str,
    kind: &str,
    payload_key: &str,
    payload: Value,
) -> Value {
    let mut node = snapshot
        .and_then(|snapshot| snapshot.nodes.get(node_key))
        .cloned()
        .unwrap_or_default();
    node.insert("type".into(), Value::String(kind.into()));
    node.insert(payload_key.into(), payload);
    Value::Object(node)
}

fn encode_axis(axis: Axis) -> &'static str {
    match axis {
        Axis::Horizontal => "horizontal",
        Axis::Vertical => "vertical",
    }
}

fn ratio_value(ratio: f32) -> Value {
    if ratio.is_finite() {
        Value::from(ratio as f64)
    } else {
        Value::from(0.5)
    }
}

fn string_field(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key)?.as_str().map(str::to_owned)
}

fn optional_string_field(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn string_array_field(object: &Map<String, Value>, key: &str) -> Vec<String> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn set_optional_string(object: &mut Map<String, Value>, key: &str, value: Option<&String>) {
    if let Some(value) = value {
        object.insert(key.into(), Value::String(value.clone()));
    } else {
        object.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

    struct TempFile {
        directory: PathBuf,
        path: PathBuf,
    }

    impl TempFile {
        fn new() -> Self {
            let sequence = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let directory = std::env::temp_dir().join(format!(
                "muxy-workspace-store-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join("workspaces.json");
            Self { directory, path }
        }

        fn write(&self, value: &Value) {
            std::fs::write(&self.path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
        }

        fn read(&self) -> Value {
            serde_json::from_slice(&std::fs::read(&self.path).unwrap()).unwrap()
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    fn terminal_tab(id: &str) -> Value {
        json!({
            "kind": "terminal",
            "id": id,
            "isPinned": false,
            "paneTitle": "shell",
            "paneUsesDefaultTitle": true,
            "paneID": "pane-id",
            "paneSessionID": "session-id",
            "filePath": "/tmp/file",
            "currentWorkingDirectory": "/project"
        })
    }

    fn area(id: &str, path: &str, tabs: Vec<Value>, active: usize) -> Value {
        json!({
            "type": "tabArea",
            "tabArea": {
                "id": id,
                "projectPath": path,
                "tabs": tabs,
                "activeTabIndex": active
            }
        })
    }

    fn exact_fixture() -> Value {
        json!([{
            "projectID": "PROJECT-ID",
            "worktreeID": "WORKTREE-ID",
            "worktreePath": "/project-worktree",
            "focusedAreaID": "missing-area",
            "topLevelTabOrder": ["terminal", "browser"],
            "topLevelTabLayout": {
                "type": "split",
                "layoutNodeMeta": true,
                "split": {
                    "direction": "horizontal",
                    "ratio": 0.4,
                    "layoutSplitMeta": true,
                    "first": {
                        "type": "group",
                        "group": {
                            "tabIDs": ["terminal"],
                            "activeTabID": "terminal",
                            "groupMeta": true
                        }
                    },
                    "second": {
                        "type": "group",
                        "group": {
                            "tabIDs": ["browser"],
                            "activeTabID": "browser"
                        }
                    }
                }
            },
            "root": {
                "type": "split",
                "rootNodeMeta": true,
                "split": {
                    "direction": "vertical",
                    "ratio": 0.6,
                    "rootSplitMeta": true,
                    "first": area("left", "/project-worktree", vec![terminal_tab("terminal")], 0),
                    "second": area("right", "/project-worktree", vec![json!({
                        "kind": "browser",
                        "id": "browser",
                        "customTitle": "Docs",
                        "colorID": "blue",
                        "customIcon": "book",
                        "isPinned": true,
                        "projectPath": "/other",
                        "browserURL": "https://muxy.dev",
                        "browserProfileID": "profile"
                    })], 0)
                }
            },
            "unrelated": {"kept": true}
        }])
    }

    #[test]
    fn loads_exact_swift_fixture_and_reconciles_focus_and_layout() {
        let file = TempFile::new();
        file.write(&exact_fixture());
        let store = WorkspaceStore::load_from(&file.path);
        assert_eq!(store.states().len(), 1);
        let state = store.active("project-id", "/project-worktree").unwrap();
        assert_eq!(state.worktree_id.as_deref(), Some("WORKTREE-ID"));
        assert_eq!(state.worktree_path.as_deref(), Some("/project-worktree"));
        assert_eq!(state.focused_area_id.as_deref(), Some("left"));
        assert_eq!(state.tab("terminal").unwrap().kind, TabKind::Terminal);
        let browser = state.tab("browser").unwrap();
        assert_eq!(browser.kind, TabKind::Browser);
        assert_eq!(browser.parent_id, None);
        assert_eq!(browser.browser_profile.as_deref(), Some("profile"));
        assert!(browser.pinned);
        assert!(matches!(
            state.top_level_root.as_ref(),
            Some(TopLevelTabNode::Split {
                axis: Axis::Horizontal,
                ..
            })
        ));
    }

    #[test]
    fn saves_swift_wrappers_without_runtime_node_ids() {
        let file = TempFile::new();
        file.write(&exact_fixture());
        let mut store = WorkspaceStore::load_from(&file.path);
        let mut created = Tab::new(TabKind::Terminal);
        created.id = "rust-created".into();
        created.project_path = Some("/project-worktree".into());
        store
            .active_mut("PROJECT-ID", "/project-worktree")
            .unwrap()
            .new_top_level_tab(created);
        store.save().unwrap();
        let saved = file.read();
        let workspace = &saved[0];
        assert_eq!(workspace["root"]["type"], "split");
        assert_eq!(workspace["root"]["split"]["direction"], "vertical");
        assert_eq!(workspace["root"]["split"]["first"]["tabArea"]["id"], "left");
        assert!(workspace["root"].get("id").is_none());
        assert!(workspace["root"]["split"].get("axis").is_none());
        assert_eq!(workspace["root"]["rootNodeMeta"], true);
        assert_eq!(workspace["root"]["split"]["rootSplitMeta"], true);
        let created = workspace["root"]["split"]["first"]["tabArea"]["tabs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tab| tab["id"] == "rust-created")
            .unwrap();
        assert_eq!(created["projectPath"], "/project-worktree");
        assert_eq!(workspace["topLevelTabLayout"]["type"], "split");
        assert_eq!(workspace["topLevelTabLayout"]["layoutNodeMeta"], true);
        assert_eq!(
            workspace["topLevelTabLayout"]["split"]["layoutSplitMeta"],
            true
        );
        assert_eq!(
            workspace["topLevelTabLayout"]["split"]["first"]["group"]["groupMeta"],
            true
        );
        assert_eq!(
            workspace["topLevelTabLayout"]["split"]["first"]["type"],
            "group"
        );
        assert!(
            workspace["topLevelTabLayout"]["split"]["first"]
                .get("id")
                .is_none()
        );
        assert_eq!(workspace["unrelated"]["kept"], true);
    }

    #[test]
    fn preserves_engine_and_unknown_tab_fields_by_id() {
        let file = TempFile::new();
        let fixture = json!([{
            "projectID": "project",
            "root": area("area", "/project", vec![json!({
                "kind": "terminal",
                "id": "terminal",
                "isPinned": false,
                "paneUsesDefaultTitle": false,
                "paneID": "old-pane",
                "paneSessionID": "old-session",
                "filePath": "/old/file",
                "currentWorkingDirectory": "/old/cwd",
                "engineVersion": 7
            })], 0)
        }]);
        file.write(&fixture);
        let mut store = WorkspaceStore::load_from(&file.path);
        store.states_mut()[0]
            .tab_mut("terminal")
            .unwrap()
            .custom_title = Some("Changed".into());
        store.save().unwrap();
        let tab = &file.read()[0]["root"]["tabArea"]["tabs"][0];
        assert_eq!(tab["customTitle"], "Changed");
        assert_eq!(tab["paneUsesDefaultTitle"], false);
        assert_eq!(tab["paneID"], "old-pane");
        assert_eq!(tab["paneSessionID"], "old-session");
        assert_eq!(tab["filePath"], "/old/file");
        assert_eq!(tab["currentWorkingDirectory"], "/old/cwd");
        assert_eq!(tab["engineVersion"], 7);
    }

    #[test]
    fn switching_to_the_primary_worktree_adopts_the_projects_existing_state() {
        let file = TempFile::new();
        file.write(&json!([{
            "projectID": "project",
            "root": area(
                "area",
                "/project",
                vec![terminal_tab("one"), terminal_tab("two"), terminal_tab("three")],
                0
            ),
            "topLevelTabOrder": ["one", "two", "three"]
        }]));
        let mut store = WorkspaceStore::load_from(&file.path);
        assert_eq!(store.states().len(), 1);
        assert_eq!(store.states()[0].worktree_id, None);

        let state = store.ensure_worktree("project", "PRIMARY-ID", "/project");
        assert_eq!(state.worktree_id.as_deref(), Some("PRIMARY-ID"));
        assert_eq!(state.worktree_path.as_deref(), Some("/project"));
        assert_eq!(state.top_level_order.len(), 3);
        assert_eq!(store.states().len(), 1);

        store.ensure_worktree("project", "PRIMARY-ID", "/project");
        assert_eq!(store.states().len(), 1);

        store.ensure_worktree("project", "OTHER-ID", "/project-other");
        assert_eq!(store.states().len(), 2);

        store.save().unwrap();
        assert_eq!(file.read()[0]["worktreeID"], "PRIMARY-ID");
        assert_eq!(file.read()[0]["worktreePath"], "/project");
    }

    #[test]
    fn adopting_a_worktree_keeps_the_workspaces_unknown_keys() {
        let file = TempFile::new();
        file.write(&json!([{
            "projectID": "project",
            "root": area("area", "/project", vec![terminal_tab("one")], 0),
            "unrelated": {"kept": true}
        }]));
        let mut store = WorkspaceStore::load_from(&file.path);
        store.ensure_worktree("project", "PRIMARY-ID", "/project");
        store.save().unwrap();
        assert_eq!(file.read()[0]["unrelated"]["kept"], true);
        assert_eq!(file.read()[0]["worktreeID"], "PRIMARY-ID");
    }

    #[test]
    fn removing_one_worktree_preserves_sibling_workspaces() {
        let file = TempFile::new();
        let fixture = json!([
            {
                "projectID": "project",
                "worktreeID": "one",
                "worktreePath": "/one",
                "root": area("one-area", "/one", vec![terminal_tab("one-tab")], 0)
            },
            {
                "projectID": "project",
                "worktreeID": "two",
                "worktreePath": "/two",
                "root": area("two-area", "/two", vec![terminal_tab("two-tab")], 0)
            }
        ]);
        file.write(&fixture);
        let mut store = WorkspaceStore::load_from(&file.path);

        assert!(store.remove_workspace("PROJECT", "/one"));
        assert!(store.has_project("project"));
        assert!(store.active("project", "/two").is_some());
        store.save().unwrap();
        assert_eq!(file.read().as_array().unwrap().len(), 1);
    }

    #[test]
    fn exact_worktree_apis_use_ids_instead_of_stale_or_duplicate_paths() {
        let file = TempFile::new();
        file.write(&json!([
            {
                "projectID": "project",
                "worktreeID": "one",
                "worktreePath": "/shared",
                "root": area("one-area", "/stale-one", vec![terminal_tab("one-tab")], 0),
                "rawOnly": "must-not-return"
            },
            {
                "projectID": "project",
                "worktreeID": "two",
                "worktreePath": "/shared",
                "root": area("two-area", "/stale-two", vec![terminal_tab("two-tab")], 0)
            }
        ]));
        let mut store = WorkspaceStore::load_from(&file.path);

        assert_eq!(
            store
                .worktree("PROJECT", "TWO")
                .unwrap()
                .focused_area_id
                .as_deref(),
            Some("two-area")
        );
        store.worktree_mut("project", "one").unwrap().worktree_path = Some("/updated".into());
        let removed = store.remove_worktree("project", "one").unwrap();
        assert_eq!(removed.worktree_id.as_deref(), Some("one"));
        assert!(store.worktree("project", "one").is_none());
        store.ensure_worktree("project", "one", "/replacement");
        store.save().unwrap();
        let saved = file.read();
        assert_eq!(saved.as_array().unwrap().len(), 2);
        let replacement = saved
            .as_array()
            .unwrap()
            .iter()
            .find(|workspace| workspace["worktreeID"] == "one")
            .unwrap();
        assert!(replacement.get("rawOnly").is_none());
    }

    #[test]
    fn invalid_extension_and_unknown_or_missing_kinds_restore_as_terminal() {
        let file = TempFile::new();
        let fixture = json!([{
            "projectID": "project",
            "root": area("area", "/project", vec![
                json!({
                    "kind": "extensionWebView",
                    "id": "invalid-extension",
                    "extensionID": "extension",
                    "isPinned": false
                }),
                json!({"kind": "future", "id": "future", "isPinned": false}),
                json!({"id": "missing", "isPinned": false}),
                json!({
                    "kind": "extensionWebView",
                    "id": "valid-extension",
                    "extensionID": "extension",
                    "extensionTabTypeID": "view",
                    "extensionTabData": {"value": 1},
                    "isPinned": false
                })
            ], 3)
        }]);
        file.write(&fixture);
        let store = WorkspaceStore::load_from(&file.path);
        let state = &store.states()[0];
        assert_eq!(
            state.tab("invalid-extension").unwrap().kind,
            TabKind::Terminal
        );
        assert_eq!(state.tab("future").unwrap().kind, TabKind::Terminal);
        assert_eq!(state.tab("missing").unwrap().kind, TabKind::Terminal);
        assert_eq!(
            state.tab("valid-extension").unwrap().kind,
            TabKind::ExtensionWebView
        );
    }

    #[test]
    fn isolates_malformed_entries_and_builds_a_group_when_layout_is_missing() {
        let file = TempFile::new();
        let fixture = json!([
            null,
            {"projectID": "missing-root"},
            {"projectID": "bad-root", "root": {"type": "future"}},
            {
                "projectID": "valid",
                "root": area("area", "/valid", vec![terminal_tab("terminal")], 99)
            }
        ]);
        file.write(&fixture);
        let mut store = WorkspaceStore::load_from(&file.path);
        assert_eq!(store.states().len(), 1);
        store.ensure_project("VALID", "/valid");
        assert_eq!(store.states().len(), 1);
        let state = store.project("VALID").unwrap();
        assert_eq!(state.focused_area_id.as_deref(), Some("area"));
        assert_eq!(
            state.area("area").unwrap().active_tab_id.as_deref(),
            Some("terminal")
        );
        assert!(matches!(
            state.top_level_root,
            Some(TopLevelTabNode::Group { .. })
        ));
        store.save().unwrap();
        let saved: Value = serde_json::from_slice(&std::fs::read(&file.path).unwrap()).unwrap();
        assert_eq!(saved.as_array().unwrap().len(), 4);
    }
}
