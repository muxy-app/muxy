use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppLayout {
    #[default]
    ProjectFocused,
    TabFocused,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectOrder {
    #[default]
    Manual,
    Name,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarCollapsedStyle {
    #[default]
    Icons,
    Hidden,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Independent appearance preferences"
)]
pub struct Appearance {
    pub layout: AppLayout,
    pub dark_theme: String,
    pub light_theme: String,
    pub sidebar_expanded: bool,
    pub sidebar_vibrancy: bool,
    pub sidebar_vibrancy_level: u8,
    pub sidebar_expanded_width: Option<f32>,
    pub sidebar_collapsed_style: SidebarCollapsedStyle,
    pub status_bar_visible: bool,
    pub auto_expand_worktrees: bool,
    pub worktree_order_by_mru: bool,
    pub worktree_show_unread: bool,
    pub worktree_recent: Vec<crate::ProjectId>,
    pub hidden_worktrees: std::collections::BTreeSet<crate::ProjectId>,
    pub tab_focused_expanded: std::collections::BTreeMap<crate::ProjectId, bool>,
    #[serde(rename = "tab_focused_focus")]
    pub sidebar_focus: bool,
    #[serde(rename = "tab_focused_project_order")]
    pub sidebar_project_order: ProjectOrder,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            layout: AppLayout::default(),
            dark_theme: "Muxy".into(),
            light_theme: "Muxy Light".into(),
            sidebar_expanded: false,
            sidebar_vibrancy: true,
            sidebar_vibrancy_level: 50,
            sidebar_expanded_width: None,
            sidebar_collapsed_style: SidebarCollapsedStyle::default(),
            status_bar_visible: true,
            auto_expand_worktrees: false,
            worktree_order_by_mru: true,
            worktree_show_unread: true,
            worktree_recent: Vec::new(),
            hidden_worktrees: std::collections::BTreeSet::new(),
            tab_focused_expanded: std::collections::BTreeMap::new(),
            sidebar_focus: false,
            sidebar_project_order: ProjectOrder::default(),
        }
    }
}

impl Appearance {
    pub fn load(path: &Path) -> Result<Self> {
        let document = read_document(path)?;
        document.get("appearance").cloned().map_or_else(
            || Ok(Self::default()),
            |value| value.try_into().map_err(Into::into),
        )
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        save_section(path, "appearance", self)
    }

    pub fn save_changes(&self, previous: &Self, path: &Path) -> Result<Self> {
        let current = toml::Value::try_from(self)?;
        let previous_values = toml::Value::try_from(previous)?;
        let mut changes = toml::Table::new();
        if let Some(current) = current.as_table() {
            for (key, value) in current {
                if key != "tab_focused_expanded"
                    && key != "hidden_worktrees"
                    && previous_values.get(key) != Some(value)
                {
                    changes.insert(key.clone(), value.clone());
                }
            }
        }
        let mut document = read_document(path)?;
        let target = document.entry("appearance").or_insert(previous_values);
        let target = target
            .as_table_mut()
            .ok_or_else(|| io::Error::other("appearance must be a table"))?;
        target.extend(changes);
        let mut saved: Self = toml::Value::Table(target.clone()).try_into()?;
        for project in self.hidden_worktrees.difference(&previous.hidden_worktrees) {
            saved.hidden_worktrees.insert(*project);
        }
        for project in previous.hidden_worktrees.difference(&self.hidden_worktrees) {
            saved.hidden_worktrees.remove(project);
        }
        target.insert(
            "hidden_worktrees".into(),
            toml::Value::try_from(&saved.hidden_worktrees)?,
        );
        for (project, expanded) in &self.tab_focused_expanded {
            if previous.tab_focused_expanded.get(project) != Some(expanded) {
                saved.tab_focused_expanded.insert(*project, *expanded);
            }
        }
        for project in previous.tab_focused_expanded.keys() {
            if !self.tab_focused_expanded.contains_key(project) {
                saved.tab_focused_expanded.remove(project);
            }
        }
        target.insert(
            "tab_focused_expanded".into(),
            toml::Value::try_from(&saved.tab_focused_expanded)?,
        );
        write_document(path, &document)?;
        Ok(saved)
    }
}

pub(crate) fn save_section(path: &Path, section: &str, values: &impl Serialize) -> Result<()> {
    let mut document = read_document(path)?;
    let values = toml::Value::try_from(values)?;
    let values = values
        .as_table()
        .ok_or_else(|| io::Error::other(format!("{section} must be a table")))?;
    let target = document
        .entry(section)
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    target
        .as_table_mut()
        .ok_or_else(|| io::Error::other(format!("{section} must be a table")))?
        .extend(values.clone());
    write_document(path, &document)
}

pub(crate) fn replace_section(path: &Path, section: &str, values: &impl Serialize) -> Result<()> {
    let mut document = read_document(path)?;
    document.insert(section.into(), toml::Value::try_from(values)?);
    write_document(path, &document)
}

fn write_document(path: &Path, document: &toml::Table) -> Result<()> {
    atomic_write(path, &toml::to_string_pretty(document)?)
}

pub(crate) fn atomic_write(path: &Path, source: &str) -> Result<()> {
    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("settings path has no parent"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".settings-{}-{}.tmp",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| -> io::Result<()> {
        file.write_all(source.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(Into::into)
}

fn read_document(path: &Path) -> Result<toml::Table> {
    match fs::read_to_string(path) {
        Ok(source) => source.parse().map_err(Into::into),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(toml::Table::new()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_visibility_defaults_and_independent_saved_choices() -> Result<()> {
        let first = crate::ProjectId::new();
        let second = crate::ProjectId::new();
        let directory = std::env::temp_dir().join(format!("muxy-worktree-visibility-{first}"));
        let path = directory.join("settings.toml");
        let initial = Appearance::load(&path)?;
        assert!(initial.hidden_worktrees.is_empty());
        initial.save(&path)?;
        let mut one = initial.clone();
        one.hidden_worktrees.insert(first);
        one.save_changes(&initial, &path)?;
        let mut two = initial.clone();
        two.hidden_worktrees.insert(second);
        let saved = two.save_changes(&initial, &path)?;
        assert_eq!(saved.hidden_worktrees, [first, second].into());
        let previous_one = one.clone();
        one.hidden_worktrees.remove(&first);
        let saved = one.save_changes(&previous_one, &path)?;
        assert_eq!(saved.hidden_worktrees, [second].into());
        assert_eq!(Appearance::load(&path)?, saved);
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn independent_project_expansion_changes_preserve_the_latest_saved_choices() -> Result<()> {
        let first = crate::ProjectId::new();
        let second = crate::ProjectId::new();
        let directory = std::env::temp_dir().join(format!("muxy-appearance-projects-{first}"));
        let path = directory.join("settings.toml");
        let initial = Appearance {
            tab_focused_expanded: [(first, true), (second, true)].into(),
            ..Appearance::default()
        };
        initial.save(&path)?;
        let mut one = initial.clone();
        one.tab_focused_expanded.insert(first, false);
        one.save_changes(&initial, &path)?;
        let mut two = initial.clone();
        two.tab_focused_expanded.insert(second, false);
        let saved = two.save_changes(&initial, &path)?;
        assert_eq!(
            saved.tab_focused_expanded,
            [(first, false), (second, false)].into()
        );
        assert_eq!(Appearance::load(&path)?, saved);
        fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn updating_appearance_preserves_other_settings() -> Result<()> {
        let directory =
            std::env::temp_dir().join(format!("muxy-appearance-{}", std::process::id()));
        fs::create_dir_all(&directory)?;
        let path = directory.join("settings.toml");
        fs::write(
            &path,
            "[terminal]\nfont_size = 17\n[appearance]\nlight_theme = 'Solarized Light'\n",
        )?;
        let mut appearance = Appearance::load(&path)?;
        appearance.dark_theme = "Dracula".into();
        appearance.save(&path)?;
        assert_eq!(Appearance::load(&path)?, appearance);
        assert_eq!(
            read_document(&path)?["terminal"]["font_size"].as_integer(),
            Some(17)
        );
        fs::remove_dir_all(directory)?;
        Ok(())
    }
}
