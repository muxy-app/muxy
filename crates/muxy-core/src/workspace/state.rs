use super::{
    AreaId, Rect, SplitNode, Tab, TabArea, TabId, TopLevelGroupId, TopLevelTabNode, VisibleArea,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub type ProjectId = String;
pub type WorktreeId = String;

pub const FOCUS_HISTORY_LIMIT: usize = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceState {
    pub project_id: ProjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_id: Option<WorktreeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<SplitNode>,
    #[serde(default)]
    pub top_level_order: Vec<TabId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_level_root: Option<TopLevelTabNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_area_id: Option<AreaId>,
    #[serde(default)]
    pub focus_history: Vec<AreaId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximized_area_id: Option<AreaId>,
}

impl WorkspaceState {
    pub fn new(project_id: impl Into<ProjectId>) -> Self {
        Self {
            project_id: project_id.into(),
            worktree_id: None,
            worktree_path: None,
            root: None,
            top_level_order: Vec::new(),
            top_level_root: None,
            focused_area_id: None,
            focus_history: Vec::new(),
            maximized_area_id: None,
        }
    }

    pub fn with_worktree(
        project_id: impl Into<ProjectId>,
        worktree_id: impl Into<WorktreeId>,
        worktree_path: impl Into<String>,
    ) -> Self {
        let mut state = Self::new(project_id);
        state.worktree_id = Some(worktree_id.into());
        state.worktree_path = Some(worktree_path.into());
        state
    }

    pub fn area(&self, area_id: &str) -> Option<&TabArea> {
        self.root.as_ref()?.area_by_id(area_id)
    }

    pub fn area_mut(&mut self, area_id: &str) -> Option<&mut TabArea> {
        self.root.as_mut()?.area_by_id_mut(area_id)
    }

    pub fn tab(&self, tab_id: &str) -> Option<&Tab> {
        self.root.as_ref()?.tab(tab_id)
    }

    pub fn tab_mut(&mut self, tab_id: &str) -> Option<&mut Tab> {
        self.root.as_mut()?.tab_mut(tab_id)
    }

    pub fn area_containing_tab(&self, tab_id: &str) -> Option<&TabArea> {
        self.root.as_ref()?.area_containing_tab(tab_id)
    }

    pub fn area_ids(&self) -> Vec<AreaId> {
        self.root
            .as_ref()
            .map(SplitNode::area_ids)
            .unwrap_or_default()
    }

    pub fn root_tab_ids(&self) -> Vec<TabId> {
        self.root
            .as_ref()
            .map(|root| {
                root.tabs()
                    .into_iter()
                    .filter(|tab| tab.parent_id.is_none())
                    .map(|tab| tab.id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn root_id_for_tab(&self, tab_id: &str) -> Option<&str> {
        let tab = self.tab(tab_id)?;
        let root_id = tab.root_id();
        self.tab(root_id)
            .filter(|root| root.parent_id.is_none())
            .map(|_| root_id)
    }

    pub fn focused_root_tab_id(&self) -> Option<&str> {
        let active_tab_id = self
            .area(self.focused_area_id.as_deref()?)?
            .active_tab_id
            .as_deref()?;
        self.root_id_for_tab(active_tab_id)
    }

    pub fn active_group_id(&self) -> Option<&str> {
        let top_level_root = self.top_level_root.as_ref()?;
        self.focused_root_tab_id()
            .and_then(|root_id| top_level_root.group_containing_tab(root_id))
            .or_else(|| Some(top_level_root.first_group_id()))
    }

    pub fn focus_area(&mut self, area_id: Option<&str>) -> bool {
        let Some(area_id) = area_id else {
            let changed =
                self.focused_area_id.take().is_some() || self.maximized_area_id.take().is_some();
            return changed;
        };
        let Some(root_tab_id) = self
            .area(area_id)
            .and_then(TabArea::active_tab)
            .and_then(|tab| self.root_id_for_tab(&tab.id))
            .map(str::to_owned)
        else {
            if self.area(area_id).is_none() {
                return false;
            }
            let changed = self.focused_area_id.as_deref() != Some(area_id);
            self.set_focus_id(area_id);
            return changed;
        };
        let changed = self.focused_area_id.as_deref() != Some(area_id);
        self.set_focus_id(area_id);
        if let Some(top_level_root) = &mut self.top_level_root {
            top_level_root.set_active_for_tab(&root_tab_id);
        }
        changed
    }

    pub fn select_tab(&mut self, area_id: &str, tab_id: &str) -> bool {
        if !self.area_mut(area_id).is_some_and(|area| {
            area.activate(tab_id) || area.active_tab_id.as_deref() == Some(tab_id)
        }) {
            return false;
        }
        self.focus_area(Some(area_id));
        true
    }

    pub fn select_root_tab(&mut self, root_tab_id: &str) -> bool {
        if self
            .tab(root_tab_id)
            .is_none_or(|tab| tab.parent_id.is_some())
        {
            return false;
        }
        if self.focused_root_tab_id() == Some(root_tab_id) {
            return false;
        }
        let Some(area_id) = self
            .area_containing_tab(root_tab_id)
            .map(|area| area.id.clone())
        else {
            return false;
        };
        let changed = self.focused_root_tab_id() != Some(root_tab_id)
            || self.focused_area_id.as_deref() != Some(&area_id);
        self.area_mut(&area_id).unwrap().activate(root_tab_id);
        self.focus_area(Some(&area_id));
        if self.maximized_area_id.as_deref().is_some_and(|maximized| {
            self.area(maximized)
                .and_then(|area| area.selected_for_root(root_tab_id))
                .is_none()
        }) {
            self.maximized_area_id = None;
        }
        changed
    }

    pub fn set_maximized_area(&mut self, area_id: Option<&str>) -> bool {
        let Some(area_id) = area_id else {
            return self.maximized_area_id.take().is_some();
        };
        if self.area(area_id).is_none() {
            return false;
        }
        self.focus_area(Some(area_id));
        let changed = self.maximized_area_id.as_deref() != Some(area_id);
        self.maximized_area_id = Some(area_id.to_owned());
        changed
    }

    pub fn toggle_maximized_area(&mut self, area_id: &str) -> bool {
        if self.maximized_area_id.as_deref() == Some(area_id) {
            self.set_maximized_area(None)
        } else {
            self.set_maximized_area(Some(area_id))
        }
    }

    pub fn visible_layout(&self, root_tab_id: &str) -> Option<SplitNode> {
        self.tab(root_tab_id)
            .filter(|tab| tab.parent_id.is_none())?;
        if let Some(maximized_area_id) = self.maximized_area_id.as_deref()
            && let Some(area) = self
                .area(maximized_area_id)
                .and_then(|area| area.visible_for_root(root_tab_id))
        {
            return Some(SplitNode::area(area));
        }
        self.root.as_ref()?.visible_layout(root_tab_id)
    }

    pub fn visible_area_tabs(&self) -> Vec<(AreaId, TabId)> {
        self.top_level_root
            .as_ref()
            .map(TopLevelTabNode::active_tab_ids)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|root_id| {
                let Some(layout) = self.visible_layout(&root_id) else {
                    return Vec::new();
                };
                layout
                    .area_ids()
                    .into_iter()
                    .filter_map(|area_id| {
                        layout
                            .area_by_id(&area_id)
                            .and_then(|area| area.active_tab_id.clone())
                            .map(|tab_id| (area_id, tab_id))
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn visible_areas(&self, root_tab_id: &str, frame: Rect) -> Vec<VisibleArea> {
        self.visible_layout(root_tab_id)
            .map(|layout| layout.visible_areas(frame))
            .unwrap_or_default()
    }

    pub fn reconcile(&mut self) {
        if let Some(root) = &mut self.root {
            root.reconcile();
            let mut seen = HashSet::new();
            root.for_each_area_mut(&mut |area| {
                area.tabs.retain(|tab| seen.insert(tab.id.clone()));
                area.reconcile();
            });
        }
        self.root = self.root.take().and_then(SplitNode::pruned);

        let roots_before_promotion: HashSet<TabId> = self
            .root
            .as_ref()
            .map(|root| {
                root.tabs()
                    .into_iter()
                    .filter(|tab| tab.parent_id.is_none())
                    .map(|tab| tab.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        if let Some(root) = &mut self.root {
            root.for_each_area_mut(&mut |area| {
                for tab in &mut area.tabs {
                    if tab.parent_id.as_ref().is_some_and(|parent_id| {
                        parent_id == &tab.id || !roots_before_promotion.contains(parent_id)
                    }) {
                        tab.parent_id = None;
                    }
                }
                area.reconcile();
            });
        }

        let physical_order = self.root_tab_ids();
        let valid_roots: HashSet<TabId> = physical_order.iter().cloned().collect();
        let mut seen_order = HashSet::new();
        self.top_level_order
            .retain(|tab_id| valid_roots.contains(tab_id) && seen_order.insert(tab_id.clone()));
        for tab_id in physical_order {
            if seen_order.insert(tab_id.clone()) {
                self.top_level_order.push(tab_id);
            }
        }

        if let Some(top_level_root) = &mut self.top_level_root {
            top_level_root.prune_assignments(&valid_roots, &mut HashSet::new());
        }
        self.top_level_root = self
            .top_level_root
            .take()
            .and_then(TopLevelTabNode::normalized);

        let assigned: HashSet<TabId> = self
            .top_level_root
            .as_ref()
            .map(|root| root.tab_ids().into_iter().collect())
            .unwrap_or_default();
        let missing: Vec<TabId> = self
            .top_level_order
            .iter()
            .filter(|tab_id| !assigned.contains(*tab_id))
            .cloned()
            .collect();
        if !missing.is_empty() {
            if let Some(top_level_root) = &mut self.top_level_root {
                let group_id = top_level_root.first_group_id().to_owned();
                let (tab_ids, active_tab_id) = top_level_root.group_parts_mut(&group_id).unwrap();
                tab_ids.extend(missing);
                if active_tab_id.is_none() {
                    *active_tab_id = tab_ids.first().cloned();
                }
            } else {
                self.top_level_root = Some(TopLevelTabNode::group(
                    missing.clone(),
                    missing.first().cloned(),
                ));
            }
        }
        if let Some(top_level_root) = &mut self.top_level_root {
            top_level_root.repair_active_ids();
        }

        let valid_areas: HashSet<AreaId> = self.area_ids().into_iter().collect();
        let mut history_seen = HashSet::new();
        self.focus_history.retain(|area_id| {
            valid_areas.contains(area_id) && history_seen.insert(area_id.clone())
        });
        self.focus_history.truncate(FOCUS_HISTORY_LIMIT);
        if self
            .focused_area_id
            .as_ref()
            .is_none_or(|area_id| !valid_areas.contains(area_id))
        {
            self.focused_area_id = self
                .focus_history
                .first()
                .cloned()
                .or_else(|| self.area_ids().first().cloned());
        }
        if let Some(focused_area_id) = self.focused_area_id.clone() {
            self.set_focus_id(&focused_area_id);
        }
        if self
            .maximized_area_id
            .as_ref()
            .is_some_and(|area_id| !valid_areas.contains(area_id))
        {
            self.maximized_area_id = None;
        }

        let focused_root_id = self.focused_root_tab_id().map(str::to_owned);
        if let (Some(top_level_root), Some(focused_root_id)) =
            (&mut self.top_level_root, focused_root_id)
        {
            top_level_root.set_active_for_tab(&focused_root_id);
        }
    }

    pub(crate) fn root_is_pinned(&self, root_tab_id: &str) -> bool {
        self.tab(root_tab_id)
            .filter(|tab| tab.parent_id.is_none())
            .is_some_and(|tab| tab.pinned)
    }

    pub(crate) fn group_id_for_root(&self, root_tab_id: &str) -> Option<TopLevelGroupId> {
        self.top_level_root
            .as_ref()?
            .group_containing_tab(root_tab_id)
            .map(str::to_owned)
    }

    pub(crate) fn set_focus_id(&mut self, area_id: &str) {
        self.focused_area_id = Some(area_id.to_owned());
        self.focus_history.retain(|id| id != area_id);
        self.focus_history.insert(0, area_id.to_owned());
        self.focus_history.truncate(FOCUS_HISTORY_LIMIT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{Axis, TabKind};

    fn tab(id: &str, parent_id: Option<&str>) -> Tab {
        let mut tab = Tab::new(TabKind::Terminal);
        tab.id = id.into();
        tab.parent_id = parent_id.map(str::to_owned);
        tab
    }

    fn area(id: &str, tabs: Vec<Tab>, active: &str) -> TabArea {
        TabArea {
            id: id.into(),
            tabs,
            active_tab_id: Some(active.into()),
            tab_history: Vec::new(),
        }
    }

    #[test]
    fn reconciliation_promotes_orphans_and_repairs_order_groups_and_focus() {
        let mut state = WorkspaceState::new("project");
        state.root = Some(SplitNode::split(
            Axis::Horizontal,
            0.5,
            SplitNode::area(area(
                "left",
                vec![tab("a", None), tab("orphan", Some("missing"))],
                "orphan",
            )),
            SplitNode::area(area("right", vec![tab("b", None)], "b")),
        ));
        state.top_level_order = vec!["b".into(), "b".into(), "missing".into()];
        state.top_level_root = Some(TopLevelTabNode::split(
            Axis::Vertical,
            2.0,
            TopLevelTabNode::group_with_id(
                "first",
                vec!["b".into(), "missing".into()],
                Some("missing".into()),
            ),
            TopLevelTabNode::group_with_id("duplicate", vec!["b".into()], Some("b".into())),
        ));
        state.focused_area_id = Some("left".into());
        state.reconcile();

        assert_eq!(state.tab("orphan").unwrap().parent_id, None);
        assert_eq!(state.top_level_order, ["b", "a", "orphan"]);
        assert_eq!(
            state.top_level_root.as_ref().unwrap().tab_ids(),
            ["b", "a", "orphan"]
        );
        assert!(
            !state
                .top_level_root
                .as_ref()
                .unwrap()
                .contains_group("duplicate")
        );
        assert_eq!(
            state
                .top_level_root
                .as_ref()
                .unwrap()
                .group_active_tab_id("first"),
            Some("orphan")
        );
    }

    #[test]
    fn focus_history_is_unique_mru_capped_at_twenty() {
        let mut state = WorkspaceState::new("project");
        for index in 0..25 {
            state.set_focus_id(&format!("area-{index}"));
        }
        assert_eq!(state.focus_history.len(), FOCUS_HISTORY_LIMIT);
        assert_eq!(state.focus_history.first().unwrap(), "area-24");
        assert_eq!(state.focus_history.last().unwrap(), "area-5");
    }

    #[test]
    fn reselecting_active_root_preserves_focused_child_area() {
        let mut state = WorkspaceState::new("project");
        state.root = Some(SplitNode::split(
            Axis::Horizontal,
            0.5,
            SplitNode::area(area("left", vec![tab("root", None)], "root")),
            SplitNode::area(area("right", vec![tab("child", Some("root"))], "child")),
        ));
        state.top_level_order = vec!["root".into()];
        state.top_level_root = Some(TopLevelTabNode::group(
            vec!["root".into()],
            Some("root".into()),
        ));
        state.focused_area_id = Some("right".into());
        state.reconcile();

        assert!(!state.select_root_tab("root"));
        assert_eq!(state.focused_area_id.as_deref(), Some("right"));
        assert_eq!(
            state.area("right").unwrap().active_tab_id.as_deref(),
            Some("child")
        );
    }

    #[test]
    fn maximized_layout_is_a_transient_projection() {
        let mut state = WorkspaceState::new("project");
        state.root = Some(SplitNode::split(
            Axis::Horizontal,
            0.5,
            SplitNode::area(area("left", vec![tab("root", None)], "root")),
            SplitNode::area(area("right", vec![tab("child", Some("root"))], "child")),
        ));
        state.top_level_order = vec!["root".into()];
        state.top_level_root = Some(TopLevelTabNode::group(
            vec!["root".into()],
            Some("root".into()),
        ));
        let physical = state.root.clone();
        state.maximized_area_id = Some("right".into());
        let visible = state.visible_layout("root").unwrap();
        assert_eq!(visible.area_ids(), ["right"]);
        assert_eq!(state.root, physical);
    }
}
