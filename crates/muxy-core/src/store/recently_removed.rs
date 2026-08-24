use super::persistence::write_json;
use super::projects::Project;
use crate::prefs::app_support_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const LIMIT: usize = 10;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentlyRemovedProject {
    pub project: Project,
    pub removed_at: f64,
}

fn path() -> PathBuf {
    app_support_dir().join("recently-removed-projects.json")
}

pub fn load() -> Vec<RecentlyRemovedProject> {
    load_from(&path())
}

fn load_from(file: &std::path::Path) -> Vec<RecentlyRemovedProject> {
    let Ok(contents) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn take(id: &str) -> Option<Project> {
    take_from(&path(), id)
}

fn take_from(file: &std::path::Path, id: &str) -> Option<Project> {
    let mut entries = load_from(file);
    let index = entries
        .iter()
        .position(|entry| entry.project.id.eq_ignore_ascii_case(id))?;
    let entry = entries.remove(index);
    write_json(file, &entries).ok()?;
    Some(entry.project)
}

pub fn forget(project: &Project) -> std::io::Result<()> {
    forget_from(&path(), project)
}

fn forget_from(file: &std::path::Path, project: &Project) -> std::io::Result<()> {
    let mut entries = load_from(file);
    let previous_len = entries.len();
    entries.retain(|entry| !matches(&entry.project, project));
    if entries.len() == previous_len {
        return Ok(());
    }
    write_json(file, &entries)
}

pub fn record(project: &Project) -> std::io::Result<()> {
    let mut entries = load();
    entries.retain(|entry| !matches(&entry.project, project));
    entries.insert(
        0,
        RecentlyRemovedProject {
            project: project.clone(),
            removed_at: super::reference_now(),
        },
    );
    entries.truncate(LIMIT);
    write_json(&path(), &entries)
}

fn matches(left: &Project, right: &Project) -> bool {
    left.id == right.id || left.path == right.path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_returns_the_entry_and_rewrites_the_file() {
        let dir = std::env::temp_dir().join("muxy-recently-removed-take");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("recently-removed-projects.json");

        let first = Project::new("First".to_owned(), "/tmp/first".to_owned(), 0);
        let second = Project::new("Second".to_owned(), "/tmp/second".to_owned(), 1);
        let entries = vec![
            RecentlyRemovedProject {
                project: first.clone(),
                removed_at: 1.0,
            },
            RecentlyRemovedProject {
                project: second.clone(),
                removed_at: 2.0,
            },
        ];
        write_json(&file, &entries).expect("write fixture");

        let taken = take_from(&file, &first.id).expect("takes the first entry");
        assert_eq!(taken.id, first.id);
        assert_eq!(taken.path, "/tmp/first");

        let remaining = load_from(&file);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].project.id, second.id);
        assert!(take_from(&file, &first.id).is_none());
    }

    #[test]
    fn forget_removes_an_entry_with_the_same_path() {
        let dir = std::env::temp_dir().join("muxy-recently-removed-forget");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("recently-removed-projects.json");

        let removed = Project::new("Removed".to_owned(), "/tmp/shared".to_owned(), 0);
        let kept = Project::new("Kept".to_owned(), "/tmp/kept".to_owned(), 1);
        let entries = vec![
            RecentlyRemovedProject {
                project: removed,
                removed_at: 1.0,
            },
            RecentlyRemovedProject {
                project: kept.clone(),
                removed_at: 2.0,
            },
        ];
        write_json(&file, &entries).expect("write fixture");

        let replacement = Project::new("Replacement".to_owned(), "/tmp/shared".to_owned(), 2);
        forget_from(&file, &replacement).expect("forgets matching path");

        let remaining = load_from(&file);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].project.id, kept.id);
    }
}
