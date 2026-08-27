use crate::prefs::{app_support_dir, home_dir};
use serde::{Deserialize, Serialize};

pub const HOME_PROJECT_ID: &str = "00000000-0000-0000-0000-000000000001";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default)]
    pub created_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_worktree_path_template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_worktree_parent_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pull_request_prompt: Option<String>,
    #[serde(default)]
    pub worktrees_enabled: bool,
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(
        rename = "remoteWorkspaceID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub remote_workspace_id: Option<String>,
    #[serde(
        rename = "remoteDeviceID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub remote_device_id: Option<String>,
    #[serde(skip)]
    pub is_git_repo: bool,
    #[serde(skip)]
    pub worktree_label: Option<String>,
}

impl Project {
    pub fn new(name: String, path: String, sort_order: i64) -> Self {
        Self {
            id: crate::store::new_uuid(),
            name,
            path,
            sort_order,
            created_at: crate::store::reference_now(),
            last_active_at: None,
            icon: None,
            logo: None,
            icon_color: None,
            preferred_worktree_path_template: None,
            preferred_worktree_parent_path: None,
            pull_request_prompt: None,
            worktrees_enabled: false,
            is_pinned: false,
            remote_workspace_id: None,
            remote_device_id: None,
            is_git_repo: false,
            worktree_label: None,
        }
    }

    pub fn is_home(&self) -> bool {
        self.id.eq_ignore_ascii_case(HOME_PROJECT_ID)
    }

    pub fn is_remote(&self) -> bool {
        self.remote_workspace_id.is_some() || self.remote_device_id.is_some()
    }

    pub fn display_letter(&self) -> String {
        self.name
            .chars()
            .next()
            .map(|letter| letter.to_uppercase().to_string())
            .unwrap_or_default()
    }

    pub fn has_worktree_ui(&self) -> bool {
        self.worktrees_enabled && self.is_git_repo
    }

    pub fn can_remove_worktree(&self, worktree: &super::worktrees::Worktree) -> bool {
        !self.is_home() && !self.is_remote() && self.is_git_repo && !worktree.is_primary
    }

    pub fn last_active(&self) -> f64 {
        self.last_active_at.unwrap_or(0.0)
    }
}

pub fn home_project() -> Project {
    Project {
        id: HOME_PROJECT_ID.into(),
        name: "Home".into(),
        path: home_dir().to_string_lossy().into_owned(),
        sort_order: i64::MIN,
        created_at: 0.0,
        last_active_at: None,
        icon: Some("house.fill".into()),
        logo: None,
        icon_color: None,
        preferred_worktree_path_template: None,
        preferred_worktree_parent_path: None,
        pull_request_prompt: None,
        worktrees_enabled: false,
        is_pinned: false,
        remote_workspace_id: None,
        remote_device_id: None,
        is_git_repo: false,
        worktree_label: None,
    }
}

pub fn projects_path() -> std::path::PathBuf {
    app_support_dir().join("projects.json")
}

pub fn load_projects() -> Vec<Project> {
    let Ok(contents) = std::fs::read_to_string(projects_path()) else {
        return Vec::new();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn round_trips_stored_projects() {
        let Ok(contents) = std::fs::read_to_string(projects_path()) else {
            return;
        };
        let original: Value = serde_json::from_str(&contents).expect("valid json");
        let projects: Vec<Project> = serde_json::from_str(&contents).expect("decodes");
        let encoded = serde_json::to_value(&projects).expect("encodes");
        assert_eq!(original, encoded);
    }
}
