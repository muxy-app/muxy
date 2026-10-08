//! Tab groups show several of a project's tabs side by side, each group with
//! its own selected tab. A project without groups shows one tab at a time.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    AppError, AppState, Branch, Direction, GroupId, Layout, Project, ProjectId, Tab, TabId,
};

/// Always two or more groups. Every tab of the project is in exactly one, in
/// the project's tab order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabGroups {
    layout: Layout<GroupId>,
    groups: Vec<TabGroup>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabGroup {
    id: GroupId,
    tabs: Vec<TabId>,
    selected: TabId,
}

impl TabGroup {
    pub fn id(&self) -> GroupId {
        self.id
    }

    pub fn tabs(&self) -> &[TabId] {
        &self.tabs
    }

    pub fn selected(&self) -> TabId {
        self.selected
    }
}

impl TabGroups {
    pub fn layout(&self) -> &Layout<GroupId> {
        &self.layout
    }

    pub fn groups(&self) -> &[TabGroup] {
        &self.groups
    }

    pub fn group(&self, id: GroupId) -> Option<&TabGroup> {
        self.groups.iter().find(|group| group.id == id)
    }

    pub fn group_of(&self, tab: TabId) -> Option<&TabGroup> {
        self.groups.iter().find(|group| group.tabs.contains(&tab))
    }

    fn single(tabs: Vec<TabId>, selected: TabId) -> Self {
        let group = TabGroup {
            id: GroupId::new(),
            tabs,
            selected,
        };
        Self {
            layout: Layout::Leaf(group.id),
            groups: vec![group],
        }
    }

    fn group_mut(&mut self, id: GroupId) -> Option<&mut TabGroup> {
        self.groups.iter_mut().find(|group| group.id == id)
    }

    /// Adds `tab` to the group holding `near`, or to the first group.
    fn insert(&mut self, tab: TabId, near: Option<TabId>) {
        let index = near
            .and_then(|near| {
                self.groups
                    .iter()
                    .position(|group| group.tabs.contains(&near))
            })
            .unwrap_or(0);
        if let Some(group) = self.groups.get_mut(index) {
            group.tabs.push(tab);
        }
    }

    /// Takes `tab` out of its group. The group then selects the tab after it,
    /// or the one before, and disappears once empty.
    pub(crate) fn remove(&mut self, tab: TabId) {
        let Some(index) = self
            .groups
            .iter()
            .position(|group| group.tabs.contains(&tab))
        else {
            return;
        };
        let group = &mut self.groups[index];
        if group.selected == tab
            && let Some(next) = neighbor_in(&group.tabs, tab)
        {
            group.selected = next;
        }
        group.tabs.retain(|candidate| *candidate != tab);
        if group.tabs.is_empty() {
            let id = group.id;
            self.groups.remove(index);
            self.layout.remove(id);
        }
    }

    /// The group beside `id`, preferring the right, then left, below, above.
    fn neighbor(&self, id: GroupId) -> Option<&TabGroup> {
        [
            Direction::Right,
            Direction::Left,
            Direction::Down,
            Direction::Up,
        ]
        .into_iter()
        .find_map(|direction| self.layout.neighbor(id, direction))
        .and_then(|neighbor| self.group(neighbor))
        .or_else(|| self.groups.iter().find(|group| group.id != id))
    }

    pub(crate) fn remap_tabs(&mut self, tabs: &BTreeMap<TabId, TabId>) {
        for group in &mut self.groups {
            for tab in &mut group.tabs {
                *tab = tabs.get(tab).copied().unwrap_or(*tab);
            }
            group.selected = tabs.get(&group.selected).copied().unwrap_or(group.selected);
        }
    }

    pub(crate) fn validate(&self, order: &[TabId], focused: Option<TabId>) -> Result<(), AppError> {
        let invalid = |reason: &str| Err(AppError::InvalidState(format!("tab groups: {reason}")));
        self.layout.validate()?;
        let leaves = self.layout.leaves();
        let ids: HashSet<_> = self.groups.iter().map(|group| group.id).collect();
        if self.groups.len() < 2
            || ids.len() != self.groups.len()
            || leaves.len() != ids.len()
            || leaves.iter().any(|leaf| !ids.contains(leaf))
        {
            return invalid("layout must hold every group once, and at least two");
        }
        let grouped: Vec<_> = self
            .groups
            .iter()
            .flat_map(|group| group.tabs.iter().copied())
            .collect();
        let mut sorted = grouped.clone();
        sorted.sort_by_key(|tab| order.iter().position(|candidate| candidate == tab));
        if sorted.len() != order.len() || sorted != order {
            return invalid("every tab must be in exactly one group");
        }
        for group in &self.groups {
            if !group.tabs.contains(&group.selected)
                || !group
                    .tabs
                    .is_sorted_by_key(|tab| order.iter().position(|id| id == tab))
            {
                return invalid("groups keep tab order and select one of their tabs");
            }
        }
        if let Some(focused) = focused
            && self
                .group_of(focused)
                .is_some_and(|group| group.selected != focused)
        {
            return invalid("the focused group must select the window's tab");
        }
        Ok(())
    }
}

/// The tab after `tab` in `tabs`, or the one before.
fn neighbor_in(tabs: &[TabId], tab: TabId) -> Option<TabId> {
    let index = tabs.iter().position(|candidate| *candidate == tab)?;
    tabs.get(index + 1)
        .or_else(|| index.checked_sub(1).and_then(|index| tabs.get(index)))
        .copied()
}

impl Project {
    pub fn groups(&self) -> Option<&TabGroups> {
        self.groups.as_ref()
    }

    /// The tabs in `tab`'s group, in order: every tab when there are no groups.
    pub fn group_tabs(&self, tab: TabId) -> Vec<TabId> {
        match self.groups.as_ref().and_then(|groups| groups.group_of(tab)) {
            Some(group) => group.tabs.clone(),
            None => self.tabs.iter().map(|tab| tab.id).collect(),
        }
    }

    /// The tab shown once `tab` closes: its neighbor in the group, or the
    /// selected tab of the neighboring group when `tab` is the last.
    pub(crate) fn tab_after_close(&self, tab: TabId) -> Option<TabId> {
        let tabs = self.group_tabs(tab);
        neighbor_in(&tabs, tab).or_else(|| {
            let groups = self.groups.as_ref()?;
            groups
                .neighbor(groups.group_of(tab)?.id)
                .map(|group| group.selected)
        })
    }

    /// Repairs groups after tabs were added, removed, reordered, or selected:
    /// tabs follow the project's order, unknown tabs go to `focused`'s group,
    /// empty groups disappear, and `focused` is its group's selected tab.
    pub(crate) fn sync_groups(&mut self, focused: Option<TabId>) {
        let Some(groups) = &mut self.groups else {
            return;
        };
        let order: Vec<_> = self.tabs.iter().map(|tab| tab.id).collect();
        let leaves: HashSet<_> = groups.layout.leaves().into_iter().collect();
        let mut seen = HashSet::new();
        let mut orphans = Vec::new();
        for group in &mut groups.groups {
            group
                .tabs
                .retain(|tab| order.contains(tab) && seen.insert(*tab));
            if !leaves.contains(&group.id) {
                orphans.append(&mut group.tabs);
            }
        }
        let known: HashSet<_> = groups.groups.iter().map(|group| group.id).collect();
        for leaf in leaves.iter().filter(|leaf| !known.contains(leaf)) {
            groups.layout.remove(*leaf);
        }
        orphans.extend(order.iter().filter(|tab| !seen.contains(*tab)));
        let target = focused
            .and_then(|tab| groups.group_of(tab).map(|group| group.id))
            .filter(|id| leaves.contains(id))
            .or_else(|| groups.layout.leaves().first().copied());
        if let Some(group) = target.and_then(|id| groups.group_mut(id)) {
            group.tabs.append(&mut orphans);
        }
        let empty: Vec<_> = groups
            .groups
            .iter()
            .filter(|group| group.tabs.is_empty() || !leaves.contains(&group.id))
            .map(|group| group.id)
            .collect();
        for id in empty {
            groups.groups.retain(|group| group.id != id);
            groups.layout.remove(id);
        }
        for group in &mut groups.groups {
            group
                .tabs
                .sort_by_key(|tab| order.iter().position(|id| id == tab));
            if !group.tabs.contains(&group.selected)
                && let Some(first) = group.tabs.first()
            {
                group.selected = *first;
            }
            if let Some(focused) = focused
                && group.tabs.contains(&focused)
            {
                group.selected = focused;
            }
        }
        if groups.validate(&order, focused).is_err() {
            self.groups = None;
        }
    }
}

impl AppState {
    /// The tab each group shows, in layout order: just the selected tab when
    /// there are no groups.
    pub fn visible_tabs(&self, project: ProjectId) -> Vec<TabId> {
        let Some(project) = self.project(project) else {
            return Vec::new();
        };
        match &project.groups {
            Some(groups) => groups
                .layout
                .leaves()
                .into_iter()
                .filter_map(|id| groups.group(id).map(|group| group.selected))
                .collect(),
            None => self
                .window
                .selected_tab
                .get(&project.id)
                .copied()
                .into_iter()
                .collect(),
        }
    }

    /// Moves `tab` into a new group on the `edge` side of the group showing
    /// `beside`, and focuses it.
    pub fn split_tab(
        &mut self,
        tab: TabId,
        beside: TabId,
        edge: Direction,
    ) -> Result<(), AppError> {
        let project = self.shared_project(tab, beside)?;
        let focused = self.window.selected_tab.get(&project).copied();
        let target = self.project_mut(project)?;
        let order: Vec<_> = target.tabs.iter().map(|tab| tab.id).collect();
        let groups = target
            .groups
            .get_or_insert_with(|| TabGroups::single(order, focused.unwrap_or(beside)));
        let (Some(source), Some(anchor)) = (groups.group_of(tab), groups.group_of(beside)) else {
            return Err(AppError::InvalidState("unknown tab group".into()));
        };
        if source.id == anchor.id && source.tabs.len() == 1 {
            target.sync_groups(focused);
            return Err(AppError::InvalidState(
                "a group's only tab can't split off".into(),
            ));
        }
        let anchor = anchor.id;
        groups.remove(tab);
        let group = TabGroup {
            id: GroupId::new(),
            tabs: vec![tab],
            selected: tab,
        };
        groups.layout.split(anchor, group.id, edge);
        groups.groups.push(group);
        self.select_tab(project, tab)
    }

    /// Moves `tab` into the group showing `into`, before `before` or after the
    /// group's last tab, and focuses it.
    pub fn move_tab_to_group(
        &mut self,
        tab: TabId,
        into: TabId,
        before: Option<TabId>,
    ) -> Result<(), AppError> {
        let project = self.shared_project(tab, into)?;
        let target = self.project_mut(project)?;
        let anchor = before.unwrap_or(into);
        if let Some(groups) = &mut target.groups
            && groups.group_of(tab).map(|group| group.id)
                != groups.group_of(into).map(|group| group.id)
        {
            groups.remove(tab);
            groups.insert(tab, Some(into));
        }
        let position = |id: TabId| target.tabs.iter().position(|candidate| candidate.id == id);
        let anchor = match before {
            Some(_) => position(anchor),
            None => target
                .group_tabs(into)
                .into_iter()
                .rfind(|candidate| *candidate != tab)
                .and_then(position)
                .map(|index| index + 1),
        };
        if let (Some(from), Some(to)) = (position(tab), anchor) {
            let to = if from < to { to - 1 } else { to };
            self.move_tab(project, from, to)?;
        }
        self.select_tab(project, tab)
    }

    pub fn set_group_ratio(
        &mut self,
        project: ProjectId,
        path: &[Branch],
        ratio: f32,
    ) -> Result<(), AppError> {
        let target = self.project_mut(project)?;
        target.require_available()?;
        target
            .groups
            .as_mut()
            .ok_or_else(|| AppError::InvalidState("project has no tab groups".into()))?
            .layout
            .set_ratio(path, ratio)
    }

    pub(crate) fn sync_groups(&mut self, project: ProjectId) {
        let focused = self.window.selected_tab.get(&project).copied();
        if let Some(project) = self
            .projects
            .iter_mut()
            .find(|candidate| candidate.id == project)
        {
            project.sync_groups(focused);
        }
    }

    /// Pushes `tab` into `project`, in `near`'s group.
    pub(crate) fn add_tab(
        &mut self,
        project: ProjectId,
        tab: Tab,
        near: Option<TabId>,
    ) -> Result<(), AppError> {
        let target = self.project_mut(project)?;
        if let Some(groups) = &mut target.groups {
            groups.insert(tab.id, near);
        }
        target.tabs.push(tab);
        Ok(())
    }

    fn shared_project(&self, tab: TabId, other: TabId) -> Result<ProjectId, AppError> {
        let project = self
            .projects
            .iter()
            .find(|project| project.tabs.iter().any(|candidate| candidate.id == tab))
            .ok_or_else(|| AppError::InvalidState("unknown tab".into()))?;
        project.require_available()?;
        if !project.tabs.iter().any(|candidate| candidate.id == other) {
            return Err(AppError::InvalidState(
                "tabs must belong to the same project".into(),
            ));
        }
        Ok(project.id)
    }
}
