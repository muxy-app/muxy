use super::{Axis, Edge, TabId, clamp_ratio};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub type TopLevelNodeId = String;
pub type TopLevelGroupId = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TopLevelTabNode {
    Group {
        #[serde(skip, default = "crate::store::new_uuid")]
        id: TopLevelGroupId,
        #[serde(default)]
        tab_ids: Vec<TabId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active_tab_id: Option<TabId>,
    },
    Split {
        #[serde(skip, default = "crate::store::new_uuid")]
        id: TopLevelNodeId,
        axis: Axis,
        ratio: f32,
        first: Box<TopLevelTabNode>,
        second: Box<TopLevelTabNode>,
    },
}

impl TopLevelTabNode {
    pub fn group(tab_ids: Vec<TabId>, active_tab_id: Option<TabId>) -> Self {
        Self::group_with_id(crate::store::new_uuid(), tab_ids, active_tab_id)
    }

    pub fn group_with_id(
        id: impl Into<TopLevelGroupId>,
        tab_ids: Vec<TabId>,
        active_tab_id: Option<TabId>,
    ) -> Self {
        Self::Group {
            id: id.into(),
            tab_ids,
            active_tab_id,
        }
    }

    pub fn split(axis: Axis, ratio: f32, first: Self, second: Self) -> Self {
        Self::Split {
            id: crate::store::new_uuid(),
            axis,
            ratio: clamp_ratio(ratio),
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Group { id, .. } | Self::Split { id, .. } => id,
        }
    }

    pub fn active_tab_ids(&self) -> Vec<TabId> {
        match self {
            Self::Group { active_tab_id, .. } => active_tab_id.iter().cloned().collect(),
            Self::Split { first, second, .. } => {
                let mut ids = first.active_tab_ids();
                ids.extend(second.active_tab_ids());
                ids
            }
        }
    }

    pub fn tab_ids(&self) -> Vec<TabId> {
        let mut tab_ids = Vec::new();
        self.collect_tab_ids(&mut tab_ids);
        tab_ids
    }

    pub fn first_group_id(&self) -> &str {
        match self {
            Self::Group { id, .. } => id,
            Self::Split { first, .. } => first.first_group_id(),
        }
    }

    pub fn contains_group(&self, group_id: &str) -> bool {
        match self {
            Self::Group { id, .. } => id == group_id,
            Self::Split { first, second, .. } => {
                first.contains_group(group_id) || second.contains_group(group_id)
            }
        }
    }

    pub fn group_containing_tab(&self, tab_id: &str) -> Option<&str> {
        match self {
            Self::Group { id, tab_ids, .. } if tab_ids.iter().any(|id| id == tab_id) => Some(id),
            Self::Group { .. } => None,
            Self::Split { first, second, .. } => first
                .group_containing_tab(tab_id)
                .or_else(|| second.group_containing_tab(tab_id)),
        }
    }

    pub fn group_tab_ids(&self, group_id: &str) -> Option<&[TabId]> {
        match self {
            Self::Group { id, tab_ids, .. } if id == group_id => Some(tab_ids),
            Self::Group { .. } => None,
            Self::Split { first, second, .. } => first
                .group_tab_ids(group_id)
                .or_else(|| second.group_tab_ids(group_id)),
        }
    }

    pub fn group_active_tab_id(&self, group_id: &str) -> Option<&str> {
        match self {
            Self::Group {
                id, active_tab_id, ..
            } if id == group_id => active_tab_id.as_deref(),
            Self::Group { .. } => None,
            Self::Split { first, second, .. } => first
                .group_active_tab_id(group_id)
                .or_else(|| second.group_active_tab_id(group_id)),
        }
    }

    pub fn resize(&mut self, split_id: &str, ratio: f32) -> bool {
        match self {
            Self::Group { .. } => false,
            Self::Split {
                id,
                ratio: current,
                first,
                second,
                ..
            } => {
                if id == split_id {
                    let ratio = clamp_ratio(ratio);
                    let changed = *current != ratio;
                    *current = ratio;
                    changed
                } else {
                    first.resize(split_id, ratio) || second.resize(split_id, ratio)
                }
            }
        }
    }

    pub(crate) fn group_parts_mut(
        &mut self,
        group_id: &str,
    ) -> Option<(&mut Vec<TabId>, &mut Option<TabId>)> {
        match self {
            Self::Group {
                id,
                tab_ids,
                active_tab_id,
            } if id == group_id => Some((tab_ids, active_tab_id)),
            Self::Group { .. } => None,
            Self::Split { first, second, .. } => {
                if first.contains_group(group_id) {
                    first.group_parts_mut(group_id)
                } else {
                    second.group_parts_mut(group_id)
                }
            }
        }
    }

    pub(crate) fn set_active_for_tab(&mut self, tab_id: &str) -> bool {
        match self {
            Self::Group {
                tab_ids,
                active_tab_id,
                ..
            } if tab_ids.iter().any(|id| id == tab_id) => {
                let changed = active_tab_id.as_deref() != Some(tab_id);
                *active_tab_id = Some(tab_id.to_owned());
                changed
            }
            Self::Group { .. } => false,
            Self::Split { first, second, .. } => {
                if first.group_containing_tab(tab_id).is_some() {
                    first.set_active_for_tab(tab_id)
                } else {
                    second.set_active_for_tab(tab_id)
                }
            }
        }
    }

    pub(crate) fn remove_from_group(
        &mut self,
        group_id: &str,
        tab_id: &str,
    ) -> Option<Option<TabId>> {
        let (tab_ids, active_tab_id) = self.group_parts_mut(group_id)?;
        let index = tab_ids.iter().position(|id| id == tab_id)?;
        tab_ids.remove(index);
        let fallback = tab_ids
            .get(index)
            .or_else(|| index.checked_sub(1).and_then(|index| tab_ids.get(index)))
            .cloned();
        if active_tab_id.as_deref() == Some(tab_id) {
            *active_tab_id = fallback.clone();
        }
        Some(fallback)
    }

    pub(crate) fn remove_tab_everywhere(&mut self, tab_id: &str) -> Option<TabId> {
        match self {
            Self::Group {
                tab_ids,
                active_tab_id,
                ..
            } => {
                let mut first_fallback = None;
                while let Some(index) = tab_ids.iter().position(|id| id == tab_id) {
                    tab_ids.remove(index);
                    let fallback = tab_ids
                        .get(index)
                        .or_else(|| index.checked_sub(1).and_then(|index| tab_ids.get(index)))
                        .cloned();
                    if active_tab_id.as_deref() == Some(tab_id) {
                        *active_tab_id = fallback.clone();
                    }
                    first_fallback = first_fallback.or(fallback);
                }
                first_fallback
            }
            Self::Split { first, second, .. } => first
                .remove_tab_everywhere(tab_id)
                .or_else(|| second.remove_tab_everywhere(tab_id)),
        }
    }

    pub(crate) fn prune_assignments(
        &mut self,
        valid_tab_ids: &HashSet<TabId>,
        seen: &mut HashSet<TabId>,
    ) {
        match self {
            Self::Group { tab_ids, .. } => {
                tab_ids
                    .retain(|tab_id| valid_tab_ids.contains(tab_id) && seen.insert(tab_id.clone()));
            }
            Self::Split { first, second, .. } => {
                first.prune_assignments(valid_tab_ids, seen);
                second.prune_assignments(valid_tab_ids, seen);
            }
        }
    }

    pub(crate) fn sort_groups_by_order(&mut self, order: &[TabId]) {
        let positions: std::collections::HashMap<&str, usize> = order
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect();
        match self {
            Self::Group { tab_ids, .. } => {
                tab_ids.sort_by_key(|id| positions.get(id.as_str()).copied().unwrap_or(usize::MAX));
            }
            Self::Split { first, second, .. } => {
                first.sort_groups_by_order(order);
                second.sort_groups_by_order(order);
            }
        }
    }

    pub(crate) fn repair_active_ids(&mut self) {
        match self {
            Self::Group {
                tab_ids,
                active_tab_id,
                ..
            } => {
                if active_tab_id
                    .as_ref()
                    .is_none_or(|active| !tab_ids.contains(active))
                {
                    *active_tab_id = tab_ids.first().cloned();
                }
            }
            Self::Split { first, second, .. } => {
                first.repair_active_ids();
                second.repair_active_ids();
            }
        }
    }

    pub(crate) fn normalized(self) -> Option<Self> {
        match self {
            Self::Group { tab_ids, .. } if tab_ids.is_empty() => None,
            Self::Group { .. } => Some(self),
            Self::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => match (first.normalized(), second.normalized()) {
                (Some(first), Some(second)) => Some(Self::Split {
                    id,
                    axis,
                    ratio: clamp_ratio(ratio),
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(node), None) | (None, Some(node)) => Some(node),
                (None, None) => None,
            },
        }
    }

    pub(crate) fn wrap_group(
        &mut self,
        target_group_id: &str,
        moving_group: Self,
        edge: Edge,
    ) -> Option<TopLevelNodeId> {
        match self {
            Self::Group { id, .. } if id == target_group_id => {
                let placeholder = Self::group(Vec::new(), None);
                let target = std::mem::replace(self, placeholder);
                let split = if edge.is_before() {
                    Self::split(edge.axis(), 0.5, moving_group, target)
                } else {
                    Self::split(edge.axis(), 0.5, target, moving_group)
                };
                let split_id = split.id().to_owned();
                *self = split;
                Some(split_id)
            }
            Self::Group { .. } => None,
            Self::Split { first, second, .. } => {
                if first.contains_group(target_group_id) {
                    first.wrap_group(target_group_id, moving_group, edge)
                } else {
                    second.wrap_group(target_group_id, moving_group, edge)
                }
            }
        }
    }

    fn collect_tab_ids(&self, tab_ids: &mut Vec<TabId>) {
        match self {
            Self::Group {
                tab_ids: group_tabs,
                ..
            } => tab_ids.extend(group_tabs.iter().cloned()),
            Self::Split { first, second, .. } => {
                first.collect_tab_ids(tab_ids);
                second.collect_tab_ids(tab_ids);
            }
        }
    }
}

pub(crate) fn normalize_root(root: &mut Option<TopLevelTabNode>) {
    *root = root.take().and_then(TopLevelTabNode::normalized);
}

pub(crate) fn extract_group(
    root: &mut Option<TopLevelTabNode>,
    group_id: &str,
) -> Option<TopLevelTabNode> {
    let node = root.take()?;
    let (remaining, extracted) = extract_group_from_node(node, group_id);
    *root = remaining.and_then(TopLevelTabNode::normalized);
    extracted
}

fn extract_group_from_node(
    node: TopLevelTabNode,
    group_id: &str,
) -> (Option<TopLevelTabNode>, Option<TopLevelTabNode>) {
    match node {
        TopLevelTabNode::Group { ref id, .. } if id == group_id => (None, Some(node)),
        TopLevelTabNode::Group { .. } => (Some(node), None),
        TopLevelTabNode::Split {
            id,
            axis,
            ratio,
            first,
            second,
        } => {
            let (first, extracted) = extract_group_from_node(*first, group_id);
            if extracted.is_some() {
                let remaining = match first {
                    Some(first) => TopLevelTabNode::Split {
                        id,
                        axis,
                        ratio,
                        first: Box::new(first),
                        second,
                    }
                    .normalized(),
                    None => Some(*second),
                };
                return (remaining, extracted);
            }
            let first = first.expect("untouched top-level branch");
            let (second, extracted) = extract_group_from_node(*second, group_id);
            let remaining = match second {
                Some(second) => TopLevelTabNode::Split {
                    id,
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }
                .normalized(),
                None => Some(first),
            };
            (remaining, extracted)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_only_group_and_split_contract_without_runtime_ids() {
        let node = TopLevelTabNode::split(
            Axis::Horizontal,
            0.5,
            TopLevelTabNode::group_with_id("left", vec!["a".into()], Some("a".into())),
            TopLevelTabNode::group_with_id("right", vec!["b".into()], Some("b".into())),
        );
        let value = serde_json::to_value(node).unwrap();
        assert_eq!(value["type"], "split");
        assert_eq!(value["first"]["type"], "group");
        assert!(value.get("id").is_none());
        assert!(value["first"].get("id").is_none());
    }

    #[test]
    fn prunes_assignments_depth_first_and_collapses_empty_groups() {
        let mut node = TopLevelTabNode::split(
            Axis::Horizontal,
            0.5,
            TopLevelTabNode::group_with_id(
                "left",
                vec!["a".into(), "duplicate".into(), "missing".into()],
                Some("missing".into()),
            ),
            TopLevelTabNode::group_with_id(
                "right",
                vec!["duplicate".into()],
                Some("duplicate".into()),
            ),
        );
        let valid = HashSet::from(["a".to_owned(), "duplicate".to_owned()]);
        node.prune_assignments(&valid, &mut HashSet::new());
        node.repair_active_ids();
        let node = node.normalized().unwrap();
        assert_eq!(node.tab_ids(), ["a", "duplicate"]);
        assert_eq!(node.group_active_tab_id("left"), Some("a"));
        assert!(!node.contains_group("right"));
    }

    #[test]
    fn removing_active_tab_uses_same_group_index_fallback() {
        let mut node = TopLevelTabNode::group_with_id(
            "group",
            vec!["a".into(), "b".into(), "c".into()],
            Some("b".into()),
        );
        assert_eq!(node.remove_from_group("group", "b"), Some(Some("c".into())));
        assert_eq!(node.group_active_tab_id("group"), Some("c"));
        assert_eq!(node.remove_from_group("group", "c"), Some(Some("a".into())));
        assert_eq!(node.group_active_tab_id("group"), Some("a"));
    }
}
