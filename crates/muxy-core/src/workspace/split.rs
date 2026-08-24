use super::{AreaId, Axis, Edge, Rect, Tab, TabArea, TabId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub type SplitId = String;

pub const MIN_SPLIT_RATIO: f32 = 0.15;
pub const MAX_SPLIT_RATIO: f32 = 0.85;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SplitNode {
    Area {
        area: TabArea,
    },
    Split {
        id: SplitId,
        axis: Axis,
        ratio: f32,
        first: Box<SplitNode>,
        second: Box<SplitNode>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisibleArea {
    pub area_id: AreaId,
    pub active_tab_id: Option<TabId>,
    pub frame: Rect,
}

impl SplitNode {
    pub fn area(area: TabArea) -> Self {
        Self::Area { area }
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

    pub fn area_by_id(&self, area_id: &str) -> Option<&TabArea> {
        match self {
            Self::Area { area } if area.id == area_id => Some(area),
            Self::Area { .. } => None,
            Self::Split { first, second, .. } => first
                .area_by_id(area_id)
                .or_else(|| second.area_by_id(area_id)),
        }
    }

    pub fn area_by_id_mut(&mut self, area_id: &str) -> Option<&mut TabArea> {
        match self {
            Self::Area { area } if area.id == area_id => Some(area),
            Self::Area { .. } => None,
            Self::Split { first, second, .. } => {
                if first.contains_area(area_id) {
                    first.area_by_id_mut(area_id)
                } else {
                    second.area_by_id_mut(area_id)
                }
            }
        }
    }

    pub fn area_containing_tab(&self, tab_id: &str) -> Option<&TabArea> {
        match self {
            Self::Area { area } if area.contains(tab_id) => Some(area),
            Self::Area { .. } => None,
            Self::Split { first, second, .. } => first
                .area_containing_tab(tab_id)
                .or_else(|| second.area_containing_tab(tab_id)),
        }
    }

    pub fn tab(&self, tab_id: &str) -> Option<&Tab> {
        self.area_containing_tab(tab_id)?.tab(tab_id)
    }

    pub fn tab_mut(&mut self, tab_id: &str) -> Option<&mut Tab> {
        match self {
            Self::Area { area } => area.tab_mut(tab_id),
            Self::Split { first, second, .. } => {
                if first.area_containing_tab(tab_id).is_some() {
                    first.tab_mut(tab_id)
                } else {
                    second.tab_mut(tab_id)
                }
            }
        }
    }

    pub fn contains_area(&self, area_id: &str) -> bool {
        self.area_by_id(area_id).is_some()
    }

    pub fn contains_split(&self, split_id: &str) -> bool {
        match self {
            Self::Area { .. } => false,
            Self::Split {
                id, first, second, ..
            } => {
                id == split_id || first.contains_split(split_id) || second.contains_split(split_id)
            }
        }
    }

    pub fn area_ids(&self) -> Vec<AreaId> {
        let mut area_ids = Vec::new();
        self.collect_area_ids(&mut area_ids);
        area_ids
    }

    pub fn tabs(&self) -> Vec<&Tab> {
        let mut tabs = Vec::new();
        self.collect_tabs(&mut tabs);
        tabs
    }

    pub fn first_area_id(&self) -> &str {
        match self {
            Self::Area { area } => &area.id,
            Self::Split { first, .. } => first.first_area_id(),
        }
    }

    pub fn split_area(&mut self, area_id: &str, edge: Edge, new_area: TabArea) -> bool {
        match self {
            Self::Area { area } if area.id == area_id => {
                let existing = Self::area(area.clone());
                let inserted = Self::area(new_area);
                let (first, second) = if edge.is_before() {
                    (inserted, existing)
                } else {
                    (existing, inserted)
                };
                *self = Self::split(edge.axis(), 0.5, first, second);
                true
            }
            Self::Area { .. } => false,
            Self::Split { first, second, .. } => {
                if first.contains_area(area_id) {
                    first.split_area(area_id, edge, new_area)
                } else {
                    second.split_area(area_id, edge, new_area)
                }
            }
        }
    }

    pub fn resize(&mut self, split_id: &str, ratio: f32) -> bool {
        match self {
            Self::Area { .. } => false,
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

    pub fn visible_layout(&self, root_tab_id: &str) -> Option<Self> {
        match self {
            Self::Area { area } => area.visible_for_root(root_tab_id).map(Self::area),
            Self::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => match (
                first.visible_layout(root_tab_id),
                second.visible_layout(root_tab_id),
            ) {
                (Some(first), Some(second)) => Some(Self::Split {
                    id: id.clone(),
                    axis: *axis,
                    ratio: clamp_ratio(*ratio),
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(node), None) | (None, Some(node)) => Some(node),
                (None, None) => None,
            },
        }
    }

    pub fn visible_areas(&self, frame: Rect) -> Vec<VisibleArea> {
        let mut areas = Vec::new();
        self.collect_visible_areas(frame, &mut areas);
        areas
    }

    pub(crate) fn extract_tab(&mut self, tab_id: &str) -> Option<(AreaId, Tab)> {
        match self {
            Self::Area { area } => {
                let area_id = area.id.clone();
                area.extract(tab_id).map(|tab| (area_id, tab))
            }
            Self::Split { first, second, .. } => {
                if first.area_containing_tab(tab_id).is_some() {
                    first.extract_tab(tab_id)
                } else {
                    second.extract_tab(tab_id)
                }
            }
        }
    }

    pub(crate) fn remove_tab_ids(&mut self, tab_ids: &HashSet<TabId>) -> Vec<Tab> {
        let mut removed = Vec::new();
        self.for_each_area_mut(&mut |area| removed.extend(area.remove_ids(tab_ids)));
        removed
    }

    pub(crate) fn for_each_area_mut(&mut self, operation: &mut impl FnMut(&mut TabArea)) {
        match self {
            Self::Area { area } => operation(area),
            Self::Split { first, second, .. } => {
                first.for_each_area_mut(operation);
                second.for_each_area_mut(operation);
            }
        }
    }

    pub(crate) fn pruned(self) -> Option<Self> {
        match self {
            Self::Area { area } if area.tabs.is_empty() => None,
            Self::Area { area } => Some(Self::area(area)),
            Self::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => match (first.pruned(), second.pruned()) {
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

    pub(crate) fn reconcile(&mut self) {
        match self {
            Self::Area { area } => area.reconcile(),
            Self::Split {
                ratio,
                first,
                second,
                ..
            } => {
                *ratio = clamp_ratio(*ratio);
                first.reconcile();
                second.reconcile();
            }
        }
    }

    fn collect_area_ids(&self, area_ids: &mut Vec<AreaId>) {
        match self {
            Self::Area { area } => area_ids.push(area.id.clone()),
            Self::Split { first, second, .. } => {
                first.collect_area_ids(area_ids);
                second.collect_area_ids(area_ids);
            }
        }
    }

    fn collect_tabs<'a>(&'a self, tabs: &mut Vec<&'a Tab>) {
        match self {
            Self::Area { area } => tabs.extend(&area.tabs),
            Self::Split { first, second, .. } => {
                first.collect_tabs(tabs);
                second.collect_tabs(tabs);
            }
        }
    }

    fn collect_visible_areas(&self, frame: Rect, areas: &mut Vec<VisibleArea>) {
        match self {
            Self::Area { area } => areas.push(VisibleArea {
                area_id: area.id.clone(),
                active_tab_id: area.active_tab_id.clone(),
                frame,
            }),
            Self::Split {
                axis,
                ratio,
                first,
                second,
                ..
            } => {
                let (first_frame, second_frame) = frame.split(*axis, clamp_ratio(*ratio));
                first.collect_visible_areas(first_frame, areas);
                second.collect_visible_areas(second_frame, areas);
            }
        }
    }
}

pub fn clamp_ratio(ratio: f32) -> f32 {
    if ratio.is_finite() {
        ratio.clamp(MIN_SPLIT_RATIO, MAX_SPLIT_RATIO)
    } else {
        0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::TabKind;

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
    fn visible_layout_filters_shared_areas_and_prunes_unrelated_branches() {
        let root = SplitNode::split(
            Axis::Horizontal,
            0.5,
            SplitNode::area(area(
                "left",
                vec![tab("root-a", None), tab("root-b", None)],
                "root-b",
            )),
            SplitNode::split(
                Axis::Vertical,
                0.5,
                SplitNode::area(area(
                    "child",
                    vec![
                        tab("child-a", Some("root-a")),
                        tab("child-b", Some("root-b")),
                    ],
                    "child-a",
                )),
                SplitNode::area(area(
                    "only-b",
                    vec![tab("other-b", Some("root-b"))],
                    "other-b",
                )),
            ),
        );

        let visible = root.visible_layout("root-a").unwrap();
        assert_eq!(visible.area_ids(), ["left", "child"]);
        assert_eq!(visible.area_by_id("left").unwrap().tabs.len(), 1);
        assert_eq!(
            visible.area_by_id("left").unwrap().active_tab_id.as_deref(),
            Some("root-a")
        );
        assert_eq!(
            visible
                .area_by_id("child")
                .unwrap()
                .active_tab_id
                .as_deref(),
            Some("child-a")
        );
        assert!(visible.area_by_id("only-b").is_none());
    }

    #[test]
    fn split_ratios_clamp_to_fifteen_and_eighty_five_percent() {
        let mut root = SplitNode::split(
            Axis::Horizontal,
            0.0,
            SplitNode::area(area("a", vec![tab("a", None)], "a")),
            SplitNode::area(area("b", vec![tab("b", None)], "b")),
        );
        let split_id = match &root {
            SplitNode::Split { id, ratio, .. } => {
                assert_eq!(*ratio, MIN_SPLIT_RATIO);
                id.clone()
            }
            _ => unreachable!(),
        };
        assert!(root.resize(&split_id, 2.0));
        assert!(matches!(root, SplitNode::Split { ratio, .. } if ratio == MAX_SPLIT_RATIO));
        assert!(root.resize(&split_id, f32::NAN));
        assert!(matches!(root, SplitNode::Split { ratio, .. } if ratio == 0.5));
    }
}
