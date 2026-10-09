use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use muxy_protocol::ProjectId;
use serde::{Deserialize, Serialize};

use super::{Error, Result, Settings};

pub const DEFAULT_WORKTREE_FOLDER: &str = "~/.muxy/worktrees";

pub const SUGGESTED_WORKTREE_TEMPLATE: &str = "../{base-dir}.{branch}";

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorktreeLocation {
    pub path_template: String,
    pub parent_path: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorktreeSettings {
    pub default_location: WorktreeLocation,
    pub projects: BTreeMap<ProjectId, WorktreeLocation>,
}

impl WorktreeLocation {
    pub fn validate(&self) -> Result<()> {
        let template = self.path_template.trim();
        if template.is_empty() {
            return Ok(());
        }
        if !template.contains("{branch}") {
            return Err(Error::new(
                "worktrees",
                "Path template must include {branch}.",
            ));
        }
        let resolve = |branch| {
            normalize(&Path::new("/muxy/project").join(template.replace("{branch}", branch)))
        };
        if resolve("muxy-validation-a") == resolve("muxy-validation-b") {
            return Err(Error::new(
                "worktrees",
                "Path template must keep {branch} in the resolved path.",
            ));
        }
        Ok(())
    }

    pub fn is_default(&self) -> bool {
        self.path_template.trim().is_empty() && self.parent_path.trim().is_empty()
    }
}

impl WorktreeSettings {
    /// Where a new worktree goes. `~` means `home`, the Home of the project's
    /// computer; `None` is this computer's `$HOME`.
    pub fn directory(
        &self,
        project: &crate::Project,
        location: &WorktreeLocation,
        name: &str,
        branch: &str,
        home: Option<&Path>,
    ) -> Result<PathBuf> {
        self.directory_in(
            &project.name,
            &project.directory,
            location,
            name,
            branch,
            home,
        )
    }

    /// [`Self::directory`] for the project named `project_name` in
    /// `project_directory`.
    pub fn directory_in(
        &self,
        project_name: &str,
        project_directory: &Path,
        location: &WorktreeLocation,
        name: &str,
        branch: &str,
        home: Option<&Path>,
    ) -> Result<PathBuf> {
        let inherited = location.is_default();
        let location = if inherited {
            &self.default_location
        } else {
            location
        };
        location.validate()?;
        let slug = sanitized_component(name, "name");
        if !location.path_template.trim().is_empty() {
            let base = project_directory
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let template = location
                .path_template
                .trim()
                .replace(
                    "{project-name}",
                    &sanitized_component(project_name, "project"),
                )
                .replace("{base-dir}", &sanitized_component(&base, "project"))
                .replace("{branch}", &sanitized_component(branch, "branch"));
            return resolve(project_directory, &template, home);
        }
        let folder = match location.parent_path.trim() {
            "" => DEFAULT_WORKTREE_FOLDER,
            folder => folder,
        };
        let mut parent = resolve(project_directory, folder, home)?;
        if inherited {
            parent.push(sanitized_component(project_name, "project"));
        }
        Ok(parent.join(slug))
    }
}

pub fn sanitized_component(value: &str, fallback: &str) -> String {
    let value: String = value
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    let value = value
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if matches!(value.as_str(), "" | "." | "..") {
        fallback.into()
    } else {
        value
    }
}

fn resolve(project: &Path, value: &str, home: Option<&Path>) -> Result<PathBuf> {
    let path = if value == "~" || value.starts_with("~/") {
        let home = match home {
            Some(home) => home.to_path_buf(),
            None => PathBuf::from(
                std::env::var_os("HOME")
                    .ok_or_else(|| Error::new("worktrees", "HOME is not set"))?,
            ),
        };
        home.join(value.strip_prefix("~/").unwrap_or(""))
    } else {
        project.join(value)
    };
    Ok(normalize(&path))
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => (),
            _ => result.push(component),
        }
    }
    result
}

impl Settings {
    pub fn save_worktrees(&self, path: &Path) -> Result<()> {
        self.worktrees.default_location.validate()?;
        for location in self.worktrees.projects.values() {
            location.validate()?;
        }
        super::appearance::save_section(path, "worktrees", &self.worktrees)
            .map_err(|error| Error::new("worktrees", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worktree_templates_require_a_branch_that_survives_normalization() {
        for template in ["../fixed", "../{branch}/../fixed", "  "] {
            let location = WorktreeLocation {
                path_template: template.into(),
                ..Default::default()
            };
            assert_eq!(location.validate().is_ok(), template.trim().is_empty());
        }
        assert!(
            WorktreeLocation {
                path_template: SUGGESTED_WORKTREE_TEMPLATE.into(),
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
        assert_eq!(sanitized_component("feature/a b", "name"), "feature-a-b");
        assert_eq!(sanitized_component("..", "name"), "name");
    }

    #[test]
    fn worktree_location_precedence_and_saved_preferences() -> Result<()> {
        let mut state = crate::AppState::bootstrap().map_err(|e| Error::new("test", e))?;
        let id = state
            .add_project(crate::ServerId::local(), std::env::temp_dir())
            .map_err(|e| Error::new("test", e))?;
        let mut project = state
            .project(id)
            .ok_or_else(|| Error::new("test", "project"))?
            .clone();
        project.directory = "/code/repo".into();
        project.name = "My Project".into();
        let home = std::env::home_dir().ok_or_else(|| Error::new("test", "home directory"))?;
        let mut settings = Settings::default();
        let default = WorktreeLocation::default();
        assert_eq!(
            settings
                .worktrees
                .directory(&project, &default, "Feature A", "feature/a", None)?,
            home.join(".muxy/worktrees/My-Project/Feature-A")
        );
        settings.worktrees.default_location.parent_path = "  ".into();
        assert_eq!(
            settings
                .worktrees
                .directory(&project, &default, "Feature A", "feature/a", None)?,
            home.join(".muxy/worktrees/My-Project/Feature-A")
        );
        settings.worktrees.default_location.parent_path = "/trees".into();
        assert_eq!(
            settings
                .worktrees
                .directory(&project, &default, "Feature A", "feature/a", None)?,
            Path::new("/trees/My-Project/Feature-A")
        );
        settings.worktrees.default_location.parent_path = "~/trees".into();
        assert_eq!(
            settings.worktrees.directory(
                &project,
                &default,
                "Feature A",
                "feature/a",
                Some(Path::new("/home/dev"))
            )?,
            Path::new("/home/dev/trees/My-Project/Feature-A")
        );
        settings.worktrees.default_location.parent_path = "/trees".into();
        let folder = WorktreeLocation {
            parent_path: "../trees".into(),
            ..Default::default()
        };
        assert_eq!(
            settings
                .worktrees
                .directory(&project, &folder, "Feature A", "feature/a", None)?,
            Path::new("/code/trees/Feature-A")
        );
        let template = WorktreeLocation {
            path_template: "../{base-dir}.{branch}/{project-name}".into(),
            ..Default::default()
        };
        assert_eq!(
            settings
                .worktrees
                .directory(&project, &template, "ignored", "feature/a", None)?,
            Path::new("/code/repo.feature-a/My-Project")
        );
        settings.worktrees.projects.insert(id, template);
        let dir = tempfile::tempdir().map_err(|e| Error::new("test", e))?;
        let path = dir.path().join("settings.toml");
        settings.save_worktrees(&path)?;
        assert_eq!(Settings::load(&path)?.worktrees, settings.worktrees);
        Ok(())
    }
}
