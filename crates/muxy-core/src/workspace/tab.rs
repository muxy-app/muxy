use muxy_proto::session::SessionId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

pub type TabId = String;
pub type AreaId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TabKind {
    Terminal,
    Browser,
    ExtensionWebView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: TabId,
    pub kind: TabKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<TabId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_icon: Option<String>,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub static_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_web_view_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_data: Option<Value>,
}

impl Tab {
    pub fn new(kind: TabKind) -> Self {
        Self {
            id: crate::store::new_uuid(),
            kind,
            session_id: None,
            parent_id: None,
            project_path: None,
            custom_title: None,
            color_id: None,
            custom_icon: None,
            pinned: false,
            pane_title: None,
            static_title: None,
            browser_url: None,
            browser_profile: None,
            extension_id: None,
            extension_web_view_id: None,
            extension_data: None,
        }
    }

    pub fn with_static_title(kind: TabKind, title: impl Into<String>) -> Self {
        let mut tab = Self::new(kind);
        tab.static_title = Some(title.into());
        tab
    }

    pub fn child(kind: TabKind, parent_id: impl Into<TabId>) -> Self {
        let mut tab = Self::new(kind);
        tab.parent_id = Some(parent_id.into());
        tab
    }

    pub fn root_id(&self) -> &str {
        self.parent_id.as_deref().unwrap_or(&self.id)
    }

    pub fn belongs_to(&self, root_tab_id: &str) -> bool {
        self.id == root_tab_id || self.parent_id.as_deref() == Some(root_tab_id)
    }

    pub fn title(&self) -> &str {
        if let Some(title) = self
            .custom_title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
        {
            return title;
        }
        match self.kind {
            TabKind::Terminal => self
                .pane_title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
                .or_else(|| {
                    self.static_title
                        .as_deref()
                        .filter(|title| !title.trim().is_empty())
                })
                .unwrap_or("Terminal"),
            TabKind::Browser => self
                .static_title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
                .or_else(|| self.browser_host())
                .unwrap_or("New Tab"),
            TabKind::ExtensionWebView => self
                .static_title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
                .or_else(|| {
                    self.extension_id
                        .as_deref()
                        .filter(|title| !title.trim().is_empty())
                })
                .unwrap_or("Extension"),
        }
    }

    fn browser_host(&self) -> Option<&str> {
        let url = self.browser_url.as_deref()?.trim();
        if url.is_empty() {
            return None;
        }
        let without_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
        without_scheme
            .split(['/', '?', '#'])
            .next()
            .filter(|host| !host.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabArea {
    pub id: AreaId,
    #[serde(default)]
    pub tabs: Vec<Tab>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tab_id: Option<TabId>,
    #[serde(skip)]
    pub tab_history: Vec<TabId>,
}

impl Default for TabArea {
    fn default() -> Self {
        Self::new()
    }
}

impl TabArea {
    pub fn new() -> Self {
        Self::with_id(crate::store::new_uuid())
    }

    pub fn with_id(id: impl Into<AreaId>) -> Self {
        Self {
            id: id.into(),
            tabs: Vec::new(),
            active_tab_id: None,
            tab_history: Vec::new(),
        }
    }

    pub fn from_tab(tab: Tab) -> Self {
        let mut area = Self::new();
        area.active_tab_id = Some(tab.id.clone());
        area.tabs.push(tab);
        area
    }

    pub fn tab(&self, tab_id: &str) -> Option<&Tab> {
        self.tabs.iter().find(|tab| tab.id == tab_id)
    }

    pub fn tab_mut(&mut self, tab_id: &str) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|tab| tab.id == tab_id)
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.tab(self.active_tab_id.as_deref()?)
    }

    pub fn contains(&self, tab_id: &str) -> bool {
        self.tabs.iter().any(|tab| tab.id == tab_id)
    }

    pub fn position(&self, tab_id: &str) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == tab_id)
    }

    pub fn pinned_count(&self) -> usize {
        self.tabs.iter().take_while(|tab| tab.pinned).count()
    }

    pub fn insert_tab(&mut self, index: usize, tab: Tab, activate: bool) -> bool {
        if self.contains(&tab.id) {
            return false;
        }
        let index = self.partitioned_index(index, tab.pinned);
        let tab_id = tab.id.clone();
        self.tabs.insert(index, tab);
        if activate || self.active_tab_id.is_none() {
            self.activate(&tab_id);
        }
        true
    }

    pub fn activate(&mut self, tab_id: &str) -> bool {
        if !self.contains(tab_id) || self.active_tab_id.as_deref() == Some(tab_id) {
            return false;
        }
        if let Some(previous) = self.active_tab_id.take() {
            self.tab_history.retain(|id| id != &previous);
            self.tab_history.insert(0, previous);
        }
        self.tab_history.retain(|id| id != tab_id);
        self.active_tab_id = Some(tab_id.to_owned());
        true
    }

    pub fn selected_for_root(&self, root_tab_id: &str) -> Option<&Tab> {
        self.active_tab()
            .filter(|tab| tab.belongs_to(root_tab_id))
            .or_else(|| self.tabs.iter().find(|tab| tab.id == root_tab_id))
            .or_else(|| {
                self.tabs
                    .iter()
                    .find(|tab| tab.parent_id.as_deref() == Some(root_tab_id))
            })
    }

    pub fn visible_for_root(&self, root_tab_id: &str) -> Option<Self> {
        let selected_tab_id = self.selected_for_root(root_tab_id)?.id.clone();
        let tabs = self
            .tabs
            .iter()
            .filter(|tab| tab.belongs_to(root_tab_id))
            .cloned()
            .collect();
        Some(Self {
            id: self.id.clone(),
            tabs,
            active_tab_id: Some(selected_tab_id),
            tab_history: Vec::new(),
        })
    }

    pub fn reorder(&mut self, tab_id: &str, index: usize) -> bool {
        let Some(current_index) = self.position(tab_id) else {
            return false;
        };
        let tab = self.tabs.remove(current_index);
        let target_index = self.partitioned_index(index, tab.pinned);
        self.tabs.insert(target_index, tab);
        current_index != target_index
    }

    pub fn set_pinned(&mut self, tab_id: &str, pinned: bool) -> bool {
        let Some(index) = self.position(tab_id) else {
            return false;
        };
        if self.tabs[index].pinned == pinned {
            return false;
        }
        let mut tab = self.tabs.remove(index);
        tab.pinned = pinned;
        let index = self.pinned_count();
        self.tabs.insert(index, tab);
        true
    }

    pub fn extract(&mut self, tab_id: &str) -> Option<Tab> {
        let index = self.position(tab_id)?;
        let tab = self.tabs.remove(index);
        self.repair_active(index);
        Some(tab)
    }

    pub(crate) fn extract_at(&mut self, tab_id: &str) -> Option<(usize, bool, Tab)> {
        let index = self.position(tab_id)?;
        let was_active = self.active_tab_id.as_deref() == Some(tab_id);
        let tab = self.tabs.remove(index);
        self.repair_active(index);
        Some((index, was_active, tab))
    }

    pub(crate) fn insert_intact(&mut self, index: usize, tab: Tab, activate: bool) {
        let tab_id = tab.id.clone();
        self.tabs.insert(index.min(self.tabs.len()), tab);
        if activate || self.active_tab_id.is_none() {
            self.active_tab_id = Some(tab_id);
        }
    }

    pub(crate) fn remove_ids(&mut self, tab_ids: &HashSet<TabId>) -> Vec<Tab> {
        let preferred_index = self
            .active_tab_id
            .as_deref()
            .and_then(|id| self.position(id))
            .unwrap_or_default();
        let mut removed = Vec::new();
        self.tabs.retain(|tab| {
            if tab_ids.contains(&tab.id) {
                removed.push(tab.clone());
                false
            } else {
                true
            }
        });
        self.repair_active(preferred_index);
        removed
    }

    pub(crate) fn reconcile(&mut self) {
        let mut seen = HashSet::new();
        self.tabs.retain(|tab| seen.insert(tab.id.clone()));
        self.repair_active(0);
    }

    fn partitioned_index(&self, index: usize, pinned: bool) -> usize {
        let pinned_count = self.pinned_count();
        if pinned {
            index.min(pinned_count)
        } else {
            index.clamp(pinned_count, self.tabs.len())
        }
    }

    fn repair_active(&mut self, _preferred_index: usize) {
        let valid: HashSet<&str> = self.tabs.iter().map(|tab| tab.id.as_str()).collect();
        self.tab_history.retain(|id| valid.contains(id.as_str()));
        if self
            .active_tab_id
            .as_deref()
            .is_some_and(|id| valid.contains(id))
        {
            return;
        }
        self.active_tab_id = self
            .tab_history
            .first()
            .cloned()
            .or_else(|| self.tabs.last().map(|tab| tab.id.clone()));
        if let Some(active) = &self.active_tab_id {
            self.tab_history.retain(|id| id != active);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tab(id: &str, kind: TabKind) -> Tab {
        let mut tab = Tab::new(kind);
        tab.id = id.into();
        tab
    }

    #[test]
    fn serializes_swift_tab_kind_names_and_all_payload_fields() {
        let mut extension = tab("extension", TabKind::ExtensionWebView);
        extension.project_path = Some("/project".into());
        extension.custom_title = Some("Custom".into());
        extension.color_id = Some("blue".into());
        extension.custom_icon = Some("sparkles".into());
        extension.pinned = true;
        extension.pane_title = Some("Pane".into());
        extension.static_title = Some("Static".into());
        extension.browser_url = Some("https://muxy.dev".into());
        extension.browser_profile = Some("profile".into());
        extension.extension_id = Some("extension-id".into());
        extension.extension_web_view_id = Some("view-id".into());
        extension.extension_data = Some(json!({"key": "value"}));

        let value = serde_json::to_value(&extension).unwrap();
        assert_eq!(value["kind"], "extensionWebView");
        assert_eq!(value["projectPath"], "/project");
        assert_eq!(value["browserProfile"], "profile");
        assert_eq!(value["extensionWebViewId"], "view-id");
        assert_eq!(serde_json::to_value(TabKind::Terminal).unwrap(), "terminal");
        assert_eq!(serde_json::to_value(TabKind::Browser).unwrap(), "browser");
        assert_eq!(serde_json::from_value::<Tab>(value).unwrap(), extension);
    }

    #[test]
    fn title_uses_custom_pane_static_and_kind_fallbacks() {
        let mut terminal = tab("terminal", TabKind::Terminal);
        assert_eq!(terminal.title(), "Terminal");
        terminal.static_title = Some("Static".into());
        assert_eq!(terminal.title(), "Static");
        terminal.pane_title = Some("Pane".into());
        assert_eq!(terminal.title(), "Pane");
        terminal.custom_title = Some("Custom".into());
        assert_eq!(terminal.title(), "Custom");

        let mut browser = tab("browser", TabKind::Browser);
        browser.browser_url = Some("https://muxy.dev".into());
        assert_eq!(browser.title(), "muxy.dev");
    }

    #[test]
    fn removing_active_tab_prefers_local_mru_then_last() {
        let mut area = TabArea::with_id("area");
        area.insert_tab(0, tab("first", TabKind::Terminal), true);
        area.insert_tab(1, tab("second", TabKind::Terminal), true);
        area.insert_tab(2, tab("third", TabKind::Terminal), true);
        area.activate("first");
        area.activate("third");

        area.extract("third");
        assert_eq!(area.active_tab_id.as_deref(), Some("first"));
        area.extract("first");
        assert_eq!(area.active_tab_id.as_deref(), Some("second"));
    }

    #[test]
    fn visible_area_prefers_active_then_root_then_first_child() {
        let root = tab("root", TabKind::Terminal);
        let mut first = tab("first", TabKind::Terminal);
        first.parent_id = Some("root".into());
        let mut second = tab("second", TabKind::Terminal);
        second.parent_id = Some("root".into());
        let unrelated = tab("other", TabKind::Browser);
        let mut area = TabArea::with_id("area");
        area.tabs = vec![first, unrelated, root, second];
        area.active_tab_id = Some("second".into());
        assert_eq!(area.selected_for_root("root").unwrap().id, "second");

        area.active_tab_id = Some("other".into());
        assert_eq!(area.selected_for_root("root").unwrap().id, "root");
        area.tabs.retain(|tab| tab.id != "root");
        assert_eq!(area.selected_for_root("root").unwrap().id, "first");

        let visible = area.visible_for_root("root").unwrap();
        assert_eq!(visible.tabs.len(), 2);
        assert_eq!(visible.active_tab_id.as_deref(), Some("first"));
    }
}
