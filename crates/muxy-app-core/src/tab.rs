use serde::{Deserialize, Serialize};

use crate::{AppError, Color, Layout, Pane, PaneContent, PaneId, Project, TabId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabSide {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabCloseScope {
    Other,
    Left,
    Right,
}

impl Project {
    pub fn closable_tabs(&self, anchor: TabId, scope: TabCloseScope) -> Vec<TabId> {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == anchor) else {
            return Vec::new();
        };
        let group = self.group_tabs(anchor);
        self.tabs
            .iter()
            .enumerate()
            .filter(|(candidate, tab)| {
                !tab.pinned
                    && tab.window.is_none()
                    && group.contains(&tab.id)
                    && match scope {
                        TabCloseScope::Other => *candidate != index,
                        TabCloseScope::Left => *candidate < index,
                        TabCloseScope::Right => *candidate > index,
                    }
            })
            .map(|(_, tab)| tab.id)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StoredTab")]
pub struct Tab {
    pub id: TabId,
    pub custom_title: Option<String>,
    pub color: Option<Color>,
    pub pinned: bool,
    pub panes: Vec<Pane>,
    #[serde(skip)]
    pub(crate) legacy_active_pane: Option<PaneId>,
    pub layout: Layout,
    pub zoomed: Option<PaneId>,
    /// The window of its own the tab shows in, instead of the main window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<crate::TabWindow>,
}

#[derive(Deserialize)]
struct StoredTab {
    id: TabId,
    #[serde(default)]
    custom_title: Option<String>,
    #[serde(default)]
    color: Option<Color>,
    #[serde(default)]
    pinned: bool,
    panes: Vec<Pane>,
    #[serde(default)]
    active_pane: Option<PaneId>,
    #[serde(default)]
    layout: Option<Layout>,
    #[serde(default)]
    zoomed: Option<PaneId>,
    #[serde(default)]
    window: Option<crate::TabWindow>,
}

impl TryFrom<StoredTab> for Tab {
    type Error = AppError;

    fn try_from(stored: StoredTab) -> Result<Self, Self::Error> {
        let first = stored
            .panes
            .first()
            .ok_or_else(|| AppError::InvalidState("tab must contain a pane".into()))?;
        let mut tab = Self {
            id: stored.id,
            custom_title: stored.custom_title,
            color: stored.color,
            pinned: stored.pinned,
            layout: stored.layout.unwrap_or(Layout::Leaf(first.id)),
            panes: stored.panes,
            legacy_active_pane: stored.active_pane,
            zoomed: stored.zoomed,
            window: stored.window,
        };
        tab.sanitize_titles();
        tab.validate()?;
        Ok(tab)
    }
}

impl Tab {
    pub(crate) fn sanitize_titles(&mut self) {
        let mut terminal = false;
        for pane in &mut self.panes {
            if matches!(pane.content, PaneContent::Terminal { .. }) {
                pane.title = crate::title::sanitize_or_default(&pane.title);
                terminal = true;
            }
        }
        self.custom_title = self
            .custom_title
            .take()
            .map(|title| {
                if terminal {
                    crate::title::sanitize(&title)
                } else {
                    title.trim().to_owned()
                }
            })
            .filter(|title| !title.is_empty());
    }

    /// Whether a terminal in the tab started with the app's proxy.
    pub fn uses_proxy(&self) -> bool {
        self.panes
            .iter()
            .any(|pane| pane.proxy && matches!(pane.content, PaneContent::Terminal { .. }))
    }

    /// Whether the tab shows in a window of its own.
    pub fn detached(&self) -> bool {
        self.window.is_some()
    }

    pub fn visible_panes(&self) -> Vec<PaneId> {
        self.zoomed
            .map_or_else(|| self.layout.leaves(), |pane| vec![pane])
    }

    pub(crate) fn validate(&self) -> Result<(), AppError> {
        self.layout.validate()?;
        let leaves = self.layout.leaves();
        let ids: std::collections::HashSet<_> = self.panes.iter().map(|pane| pane.id).collect();
        let layout_ids: std::collections::HashSet<_> = leaves.iter().copied().collect();
        if ids.len() != self.panes.len() || layout_ids.len() != leaves.len() || ids != layout_ids {
            return Err(AppError::InvalidState(
                "layout leaves must match the tab's panes exactly".into(),
            ));
        }
        if self
            .legacy_active_pane
            .is_some_and(|pane| !ids.contains(&pane))
            || self.zoomed.is_some_and(|pane| !ids.contains(&pane))
            || self
                .window
                .as_ref()
                .and_then(crate::TabWindow::active_pane)
                .is_some_and(|pane| !ids.contains(&pane))
        {
            return Err(AppError::InvalidState(
                "active and zoomed panes must be layout leaves".into(),
            ));
        }
        Ok(())
    }

    pub fn title(&self, active: Option<PaneId>) -> &str {
        if let Some(title) = &self.custom_title {
            return title;
        }
        self.displayed_pane(active)
            .map_or("", |pane| pane.title.as_str())
    }

    pub fn displayed_pane(&self, active: Option<PaneId>) -> Option<&Pane> {
        self.panes
            .iter()
            .find(|pane| Some(pane.id) == active)
            .or_else(|| {
                let first = self.layout.leaves().first().copied();
                self.panes.iter().find(|pane| Some(pane.id) == first)
            })
    }

    pub(crate) fn terminal() -> Self {
        Self::with_content(PaneContent::Terminal { session: None }, "Terminal")
    }

    pub(crate) fn with_content(content: PaneContent, title: &str) -> Self {
        let pane = Pane {
            id: PaneId::new(),
            title: title.into(),
            content,
            proxy: false,
        };
        Self {
            id: TabId::new(),
            custom_title: None,
            color: None,
            pinned: false,
            legacy_active_pane: None,
            layout: Layout::Leaf(pane.id),
            zoomed: None,
            window: None,
            panes: vec![pane],
        }
    }
}
