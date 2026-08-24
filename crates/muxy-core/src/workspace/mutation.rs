use super::top_level::{extract_group, normalize_root};
use super::{
    AreaId, DropZone, Edge, SplitNode, Tab, TabArea, TabId, TopLevelNodeId, TopLevelTabNode,
    WorkspaceState,
};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseMode {
    Single,
    Others,
    ToLeft,
    ToRight,
}

impl WorkspaceState {
    pub fn new_top_level_tab(&mut self, mut tab: Tab) -> Option<TabId> {
        if self.tab(&tab.id).is_some() {
            return None;
        }
        let active_group_id = self.active_group_id().map(str::to_owned);
        tab.parent_id = None;
        let tab_id = tab.id.clone();
        let area_id = if let Some(area_id) = self
            .focused_area_id
            .clone()
            .filter(|area_id| self.area(area_id).is_some())
            .or_else(|| self.area_ids().first().cloned())
        {
            self.area_mut(&area_id)?
                .insert_tab(usize::MAX, tab, true)
                .then_some(area_id)?
        } else {
            let area = TabArea::from_tab(tab);
            let area_id = area.id.clone();
            self.root = Some(SplitNode::area(area));
            area_id
        };

        let order_index = if self.root_is_pinned(&tab_id) {
            self.top_level_order
                .iter()
                .take_while(|id| self.root_is_pinned(id))
                .count()
        } else {
            self.top_level_order.len()
        };
        self.top_level_order.insert(order_index, tab_id.clone());

        if self.top_level_root.is_none() {
            self.top_level_root = Some(TopLevelTabNode::group(
                vec![tab_id.clone()],
                Some(tab_id.clone()),
            ));
        } else {
            let group_id = active_group_id
                .filter(|id| self.top_level_root.as_ref().unwrap().contains_group(id))
                .unwrap_or_else(|| {
                    self.top_level_root
                        .as_ref()
                        .unwrap()
                        .first_group_id()
                        .to_owned()
                });
            let pinned = self.root_is_pinned(&tab_id);
            let pinned_ids = self.pinned_root_ids();
            let (tab_ids, active_tab_id) = self
                .top_level_root
                .as_mut()
                .unwrap()
                .group_parts_mut(&group_id)
                .unwrap();
            let index = partitioned_insertion_index(tab_ids, usize::MAX, pinned, &pinned_ids);
            tab_ids.insert(index, tab_id.clone());
            *active_tab_id = Some(tab_id.clone());
        }
        self.set_focus_id(&area_id);
        self.reconcile();
        Some(tab_id)
    }

    pub fn split_focused_area(&mut self, edge: Edge, mut tab: Tab) -> Option<AreaId> {
        if self.tab(&tab.id).is_some() {
            return None;
        }
        let focused_area_id = self.focused_area_id.clone()?;
        let root_tab_id = self.focused_root_tab_id()?.to_owned();
        tab.parent_id = Some(root_tab_id);
        let new_area = TabArea::from_tab(tab);
        let new_area_id = new_area.id.clone();
        if !self
            .root
            .as_mut()?
            .split_area(&focused_area_id, edge, new_area)
        {
            return None;
        }
        self.maximized_area_id = None;
        self.set_focus_id(&new_area_id);
        self.reconcile();
        Some(new_area_id)
    }

    pub fn close_tab(&mut self, tab_id: &str, mode: CloseMode) -> Vec<TabId> {
        let Some(tab) = self.tab(tab_id).cloned() else {
            return Vec::new();
        };
        let Some(root_tab_id) = self.root_id_for_tab(tab_id).map(str::to_owned) else {
            return Vec::new();
        };
        if mode == CloseMode::Single {
            if tab.pinned {
                return Vec::new();
            }
            if tab.parent_id.is_some() {
                return self.remove_physical_tabs(HashSet::from([tab_id.to_owned()]));
            }
        }

        let Some(group_id) = self.group_id_for_root(&root_tab_id) else {
            return Vec::new();
        };
        let Some(group_tabs) = self
            .top_level_root
            .as_ref()
            .and_then(|root| root.group_tab_ids(&group_id))
            .map(<[TabId]>::to_vec)
        else {
            return Vec::new();
        };
        let Some(index) = group_tabs.iter().position(|id| id == &root_tab_id) else {
            return Vec::new();
        };
        let candidates: Vec<TabId> = match mode {
            CloseMode::Single => vec![root_tab_id],
            CloseMode::Others => group_tabs
                .iter()
                .filter(|id| *id != &root_tab_id)
                .cloned()
                .collect(),
            CloseMode::ToLeft => group_tabs[..index].to_vec(),
            CloseMode::ToRight => group_tabs[index + 1..].to_vec(),
        }
        .into_iter()
        .filter(|id| !self.root_is_pinned(id))
        .collect();
        self.close_root_tabs(&group_id, &group_tabs, candidates)
    }

    pub fn set_tab_pinned(&mut self, tab_id: &str, pinned: bool) -> bool {
        let Some(area_id) = self.area_containing_tab(tab_id).map(|area| area.id.clone()) else {
            return false;
        };
        if !self
            .area_mut(&area_id)
            .is_some_and(|area| area.set_pinned(tab_id, pinned))
        {
            return false;
        }
        if self.tab(tab_id).is_some_and(|tab| tab.parent_id.is_none()) {
            let pinned_ids = self.pinned_root_ids();
            self.top_level_order
                .sort_by_key(|id| !pinned_ids.contains(id));
            if let Some(root) = &mut self.top_level_root {
                root.sort_groups_by_order(&self.top_level_order);
            }
        }
        true
    }

    pub fn reorder_top_level_tab(&mut self, root_tab_id: &str, index: usize) -> bool {
        if self
            .tab(root_tab_id)
            .is_none_or(|tab| tab.parent_id.is_some())
        {
            return false;
        }
        let Some(group_id) = self.group_id_for_root(root_tab_id) else {
            return false;
        };
        let pinned = self.root_is_pinned(root_tab_id);
        let pinned_ids = self.pinned_root_ids();
        let Some((tab_ids, _)) = self
            .top_level_root
            .as_mut()
            .and_then(|root| root.group_parts_mut(&group_id))
        else {
            return false;
        };
        let group_members: HashSet<TabId> = tab_ids.iter().cloned().collect();
        if !move_partitioned(tab_ids, root_tab_id, index, pinned, &pinned_ids) {
            return false;
        }
        let reordered = tab_ids.clone();
        let slots: Vec<usize> = self
            .top_level_order
            .iter()
            .enumerate()
            .filter_map(|(index, id)| group_members.contains(id).then_some(index))
            .collect();
        for (slot, id) in slots.into_iter().zip(reordered) {
            self.top_level_order[slot] = id;
        }
        true
    }

    pub fn move_pane_center(&mut self, tab_id: &str, target_area_id: &str) -> bool {
        let Some(source_area_id) = self.area_containing_tab(tab_id).map(|area| area.id.clone())
        else {
            return false;
        };
        if source_area_id == target_area_id {
            return false;
        }
        let Some(root_tab_id) = self.root_id_for_tab(tab_id).map(str::to_owned) else {
            return false;
        };
        let Some(counterpart_id) = self
            .area(target_area_id)
            .and_then(|area| area.selected_for_root(&root_tab_id))
            .map(|tab| tab.id.clone())
        else {
            return false;
        };
        let snapshot = self.clone();
        let Some((source_index, source_was_active, dragged)) = self
            .area_mut(&source_area_id)
            .and_then(|area| area.extract_at(tab_id))
        else {
            return false;
        };
        let Some((target_index, _, counterpart)) = self
            .area_mut(target_area_id)
            .and_then(|area| area.extract_at(&counterpart_id))
        else {
            *self = snapshot;
            return false;
        };
        self.area_mut(&source_area_id).unwrap().insert_intact(
            source_index,
            counterpart,
            source_was_active,
        );
        self.area_mut(target_area_id)
            .unwrap()
            .insert_intact(target_index, dragged, true);
        self.set_focus_id(target_area_id);
        self.reconcile();
        true
    }

    pub fn move_pane_to_edge(
        &mut self,
        tab_id: &str,
        target_area_id: &str,
        edge: Edge,
    ) -> Option<AreaId> {
        self.area(target_area_id)?;
        let snapshot = self.clone();
        let (source_area_id, tab) = self.root.as_mut()?.extract_tab(tab_id)?;
        self.root = self.root.take().and_then(SplitNode::pruned);
        if self.area(target_area_id).is_none() {
            *self = snapshot;
            return None;
        }
        let new_area = TabArea::from_tab(tab);
        let new_area_id = new_area.id.clone();
        if !self
            .root
            .as_mut()?
            .split_area(target_area_id, edge, new_area)
        {
            *self = snapshot;
            return None;
        }
        if self.maximized_area_id.as_deref() == Some(&source_area_id) {
            self.maximized_area_id = None;
        }
        self.set_focus_id(&new_area_id);
        self.reconcile();
        Some(new_area_id)
    }

    pub fn move_pane(
        &mut self,
        tab_id: &str,
        target_area_id: &str,
        zone: DropZone,
    ) -> Option<AreaId> {
        match zone {
            DropZone::Center => self
                .move_pane_center(tab_id, target_area_id)
                .then_some(target_area_id.to_owned()),
            DropZone::Left => self.move_pane_to_edge(tab_id, target_area_id, Edge::Left),
            DropZone::Right => self.move_pane_to_edge(tab_id, target_area_id, Edge::Right),
            DropZone::Top => self.move_pane_to_edge(tab_id, target_area_id, Edge::Top),
            DropZone::Bottom => self.move_pane_to_edge(tab_id, target_area_id, Edge::Bottom),
        }
    }

    pub fn dock_top_level_center(
        &mut self,
        root_tab_id: &str,
        target_group_id: &str,
        index: usize,
    ) -> bool {
        if self
            .tab(root_tab_id)
            .is_none_or(|tab| tab.parent_id.is_some())
            || self
                .top_level_root
                .as_ref()
                .is_none_or(|root| !root.contains_group(target_group_id))
        {
            return false;
        }
        let Some(source_group_id) = self.group_id_for_root(root_tab_id) else {
            return false;
        };
        if source_group_id == target_group_id {
            return false;
        }
        let snapshot = self.clone();
        if self
            .top_level_root
            .as_mut()
            .and_then(|root| root.remove_from_group(&source_group_id, root_tab_id))
            .is_none()
        {
            return false;
        }
        normalize_root(&mut self.top_level_root);
        let pinned = self.root_is_pinned(root_tab_id);
        let pinned_ids = self.pinned_root_ids();
        let Some((tab_ids, active_tab_id)) = self
            .top_level_root
            .as_mut()
            .and_then(|root| root.group_parts_mut(target_group_id))
        else {
            *self = snapshot;
            return false;
        };
        let index = partitioned_insertion_index(tab_ids, index, pinned, &pinned_ids);
        tab_ids.insert(index, root_tab_id.to_owned());
        *active_tab_id = Some(root_tab_id.to_owned());
        self.reconcile();
        self.select_root_tab(root_tab_id);
        true
    }

    pub fn dock_top_level_edge(
        &mut self,
        root_tab_id: &str,
        target_group_id: &str,
        edge: Edge,
    ) -> Option<TopLevelNodeId> {
        if self
            .tab(root_tab_id)
            .is_none_or(|tab| tab.parent_id.is_some())
            || self
                .top_level_root
                .as_ref()
                .is_none_or(|root| !root.contains_group(target_group_id))
        {
            return None;
        }
        let source_group_id = self.group_id_for_root(root_tab_id)?;
        let source_count = self
            .top_level_root
            .as_ref()?
            .group_tab_ids(&source_group_id)?
            .len();
        if source_group_id == target_group_id && source_count == 1 {
            return None;
        }
        let snapshot = self.clone();
        let moving_group = if source_count == 1 {
            extract_group(&mut self.top_level_root, &source_group_id)?
        } else {
            self.top_level_root
                .as_mut()?
                .remove_from_group(&source_group_id, root_tab_id)?;
            normalize_root(&mut self.top_level_root);
            TopLevelTabNode::group(vec![root_tab_id.to_owned()], Some(root_tab_id.to_owned()))
        };
        let Some(split_id) = self
            .top_level_root
            .as_mut()
            .and_then(|root| root.wrap_group(target_group_id, moving_group, edge))
        else {
            *self = snapshot;
            return None;
        };
        self.reconcile();
        self.select_root_tab(root_tab_id);
        Some(split_id)
    }

    pub fn resize_split(&mut self, split_id: &str, ratio: f32) -> bool {
        self.root
            .as_mut()
            .is_some_and(|root| root.resize(split_id, ratio))
    }

    pub fn resize_top_level_split(&mut self, split_id: &str, ratio: f32) -> bool {
        self.top_level_root
            .as_mut()
            .is_some_and(|root| root.resize(split_id, ratio))
    }

    fn close_root_tabs(
        &mut self,
        group_id: &str,
        original_group_tabs: &[TabId],
        closing: Vec<TabId>,
    ) -> Vec<TabId> {
        if closing.is_empty() {
            return Vec::new();
        }
        let closing_set: HashSet<TabId> = closing.iter().cloned().collect();
        let focused_was_closed = self
            .focused_root_tab_id()
            .is_some_and(|id| closing_set.contains(id));
        let active_before = self
            .top_level_root
            .as_ref()
            .and_then(|root| root.group_active_tab_id(group_id))
            .map(str::to_owned);
        let fallback_index = active_before
            .as_ref()
            .filter(|id| closing_set.contains(*id))
            .and_then(|id| {
                original_group_tabs
                    .iter()
                    .position(|candidate| candidate == id)
            })
            .or_else(|| {
                original_group_tabs
                    .iter()
                    .position(|id| closing_set.contains(id))
            });
        let remaining: Vec<TabId> = original_group_tabs
            .iter()
            .filter(|id| !closing_set.contains(*id))
            .cloned()
            .collect();
        let fallback = fallback_index.and_then(|index| {
            remaining
                .get(index)
                .or_else(|| index.checked_sub(1).and_then(|index| remaining.get(index)))
                .cloned()
        });

        if let Some(top_level_root) = &mut self.top_level_root {
            for root_tab_id in &closing {
                top_level_root.remove_tab_everywhere(root_tab_id);
            }
            if let Some(fallback) = fallback.as_deref() {
                top_level_root.set_active_for_tab(fallback);
            }
        }
        normalize_root(&mut self.top_level_root);
        self.top_level_order
            .retain(|tab_id| !closing_set.contains(tab_id));
        let physical_ids: HashSet<TabId> = self
            .root
            .as_ref()
            .map(|root| {
                root.tabs()
                    .into_iter()
                    .filter(|tab| {
                        closing_set.contains(&tab.id)
                            || tab
                                .parent_id
                                .as_ref()
                                .is_some_and(|parent_id| closing_set.contains(parent_id))
                    })
                    .map(|tab| tab.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        if focused_was_closed {
            self.focused_area_id = None;
        }
        let removed = self.remove_physical_tabs(physical_ids);
        if focused_was_closed && let Some(fallback) = fallback {
            self.select_root_tab(&fallback);
        }
        removed
    }

    fn remove_physical_tabs(&mut self, tab_ids: HashSet<TabId>) -> Vec<TabId> {
        if tab_ids.is_empty() {
            return Vec::new();
        }
        let removed = self
            .root
            .as_mut()
            .map(|root| root.remove_tab_ids(&tab_ids))
            .unwrap_or_default();
        self.root = self.root.take().and_then(SplitNode::pruned);
        self.reconcile();
        removed.into_iter().map(|tab| tab.id).collect()
    }

    fn pinned_root_ids(&self) -> HashSet<TabId> {
        self.root_tab_ids()
            .into_iter()
            .filter(|tab_id| self.root_is_pinned(tab_id))
            .collect()
    }
}

fn partitioned_insertion_index(
    tab_ids: &[TabId],
    index: usize,
    pinned: bool,
    pinned_ids: &HashSet<TabId>,
) -> usize {
    let pinned_count = tab_ids.iter().filter(|id| pinned_ids.contains(*id)).count();
    if pinned {
        index.min(pinned_count)
    } else {
        index.clamp(pinned_count, tab_ids.len())
    }
}

fn move_partitioned(
    tab_ids: &mut Vec<TabId>,
    tab_id: &str,
    index: usize,
    pinned: bool,
    pinned_ids: &HashSet<TabId>,
) -> bool {
    let Some(current_index) = tab_ids.iter().position(|id| id == tab_id) else {
        return false;
    };
    let tab_id = tab_ids.remove(current_index);
    let target_index = partitioned_insertion_index(tab_ids, index, pinned, pinned_ids);
    tab_ids.insert(target_index, tab_id);
    current_index != target_index
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{Axis, TabKind};

    fn tab(id: &str) -> Tab {
        let mut tab = Tab::with_static_title(TabKind::Terminal, id);
        tab.id = id.into();
        tab
    }

    fn group_id(state: &WorkspaceState, root_tab_id: &str) -> String {
        state.group_id_for_root(root_tab_id).unwrap()
    }

    #[test]
    fn new_roots_share_focused_area_and_splits_create_direct_children() {
        let mut state = WorkspaceState::new("project");
        state.new_top_level_tab(tab("a")).unwrap();
        let first_area = state.focused_area_id.clone().unwrap();
        state.new_top_level_tab(tab("b")).unwrap();
        assert_eq!(state.area(&first_area).unwrap().tabs.len(), 2);
        assert_eq!(state.top_level_order, ["a", "b"]);
        assert_eq!(state.top_level_root.as_ref().unwrap().tab_ids(), ["a", "b"]);

        let child_area = state
            .split_focused_area(Edge::Right, tab("child-b"))
            .unwrap();
        assert_eq!(
            state.tab("child-b").unwrap().parent_id.as_deref(),
            Some("b")
        );
        assert_eq!(state.focused_area_id.as_deref(), Some(child_area.as_str()));
        assert!(matches!(state.root, Some(SplitNode::Split { .. })));
    }

    #[test]
    fn selecting_and_closing_root_uses_actual_area_and_same_group_fallback() {
        let mut state = WorkspaceState::new("project");
        state.new_top_level_tab(tab("a")).unwrap();
        state.new_top_level_tab(tab("b")).unwrap();
        let b_child_area = state
            .split_focused_area(Edge::Right, tab("b-child"))
            .unwrap();
        state.new_top_level_tab(tab("c")).unwrap();
        let c_area = state.area_containing_tab("c").unwrap().id.clone();
        assert_eq!(c_area, b_child_area);

        assert!(state.select_root_tab("a"));
        assert_eq!(
            state
                .area(state.focused_area_id.as_deref().unwrap())
                .unwrap()
                .active_tab_id
                .as_deref(),
            Some("a")
        );
        state.select_root_tab("b");
        let removed = state.close_tab("b", CloseMode::Single);
        assert!(removed.contains(&"b".into()));
        assert!(removed.contains(&"b-child".into()));
        assert!(state.tab("b").is_none());
        assert_eq!(state.focused_root_tab_id(), Some("c"));
    }

    #[test]
    fn pinned_partition_and_bulk_close_keep_pinned_roots() {
        let mut state = WorkspaceState::new("project");
        for id in ["a", "b", "c"] {
            state.new_top_level_tab(tab(id)).unwrap();
        }
        assert!(state.set_tab_pinned("b", true));
        assert_eq!(state.top_level_order, ["b", "a", "c"]);
        assert!(state.close_tab("b", CloseMode::Single).is_empty());
        state.close_tab("a", CloseMode::Others);
        assert!(state.tab("b").is_some());
        assert!(state.tab("a").is_some());
        assert!(state.tab("c").is_none());
    }

    #[test]
    fn group_reorder_rewrites_only_that_groups_global_slots() {
        let mut state = WorkspaceState::new("project");
        for id in ["a", "b", "c", "d"] {
            state.new_top_level_tab(tab(id)).unwrap();
        }
        let first_group = group_id(&state, "a");
        state
            .dock_top_level_edge("c", &first_group, Edge::Right)
            .unwrap();
        let second_group = group_id(&state, "c");
        assert!(state.dock_top_level_center("d", &second_group, 0));
        assert_eq!(state.top_level_order, ["a", "b", "c", "d"]);

        assert!(state.reorder_top_level_tab("d", 1));
        assert_eq!(
            state
                .top_level_root
                .as_ref()
                .unwrap()
                .group_tab_ids(&second_group)
                .unwrap(),
            ["c", "d"]
        );
        assert_eq!(state.top_level_order, ["a", "b", "c", "d"]);

        assert!(state.set_tab_pinned("d", true));
        assert_eq!(state.top_level_order, ["d", "a", "b", "c"]);
        assert_eq!(
            state
                .top_level_root
                .as_ref()
                .unwrap()
                .group_tab_ids(&second_group)
                .unwrap(),
            ["d", "c"]
        );
    }

    #[test]
    fn center_move_swaps_family_tabs_and_edge_move_preserves_tab() {
        let mut state = WorkspaceState::new("project");
        state.new_top_level_tab(tab("root")).unwrap();
        let left = state.focused_area_id.clone().unwrap();
        let right = state.split_focused_area(Edge::Right, tab("child")).unwrap();
        let root_before = state.tab("root").unwrap().clone();
        assert!(state.move_pane_center("root", &right));
        assert!(state.area(&right).unwrap().contains("root"));
        assert!(state.area(&left).unwrap().contains("child"));
        assert_eq!(state.tab("root"), Some(&root_before));

        let child_before = state.tab("child").unwrap().clone();
        let new_area = state
            .move_pane_to_edge("child", &right, Edge::Bottom)
            .unwrap();
        assert_eq!(state.tab("child"), Some(&child_before));
        assert!(state.area(&new_area).unwrap().contains("child"));
        assert!(state.area(&left).is_none());
    }

    #[test]
    fn center_and_edge_docking_collapse_sources_and_preserve_singleton_identity() {
        let mut state = WorkspaceState::new("project");
        for id in ["a", "b", "c"] {
            state.new_top_level_tab(tab(id)).unwrap();
        }
        let group = group_id(&state, "a");
        assert!(!state.dock_top_level_center("b", &group, 3));
        assert_eq!(
            state
                .top_level_root
                .as_ref()
                .unwrap()
                .group_tab_ids(&group)
                .unwrap(),
            ["a", "b", "c"]
        );
        state.dock_top_level_edge("c", &group, Edge::Right).unwrap();
        let singleton_group = group_id(&state, "c");
        assert_ne!(singleton_group, group);
        assert!(state.dock_top_level_center("b", &singleton_group, 0));
        assert_eq!(
            state
                .top_level_root
                .as_ref()
                .unwrap()
                .group_tab_ids(&singleton_group)
                .unwrap(),
            ["b", "c"]
        );

        let source_group = group_id(&state, "a");
        state
            .dock_top_level_edge("a", &singleton_group, Edge::Left)
            .unwrap();
        assert_eq!(group_id(&state, "a"), source_group);
        assert!(
            state
                .top_level_root
                .as_ref()
                .unwrap()
                .contains_group(&source_group)
        );
        assert!(matches!(
            state.top_level_root,
            Some(TopLevelTabNode::Split {
                axis: Axis::Horizontal,
                ..
            })
        ));
    }
}
