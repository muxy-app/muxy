use crate::prefs::app_support_dir;
use crate::store::persistence;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    #[default]
    Muxy,
    External,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Worktree {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default)]
    pub source: Source,
    pub is_primary: bool,
    #[serde(default = "crate::store::reference_now")]
    pub created_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<f64>,
}

pub fn worktrees_dir() -> PathBuf {
    app_support_dir().join("worktrees")
}

pub fn file_path(dir: &Path, project_id: &str) -> PathBuf {
    dir.join(format!("{project_id}.json"))
}

pub fn load_from(dir: &Path, project_id: &str) -> Vec<Worktree> {
    let Ok(contents) = std::fs::read_to_string(file_path(dir, project_id)) else {
        return Vec::new();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn save_to(dir: &Path, project_id: &str, worktrees: &[Worktree]) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)?;
    let contents = serde_json::to_vec_pretty(worktrees)?;
    persistence::write_atomic(&file_path(dir, project_id), &contents)
}

pub fn remove(project_id: &str) {
    let _ = std::fs::remove_file(file_path(&worktrees_dir(), project_id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn worktree(id: &str, name: &str, path: &str, source: Source, is_primary: bool) -> Worktree {
        Worktree {
            id: id.to_owned(),
            name: name.to_owned(),
            path: path.to_owned(),
            branch: None,
            source,
            is_primary,
            created_at: 1.0,
            last_active_at: None,
        }
    }

    #[test]
    fn decodes_a_file_missing_source_and_created_at_without_losing_the_id() {
        let raw = r#"[{"id":"STABLE-ID","name":"repo","path":"/repo","isPrimary":true}]"#;

        let decoded: Vec<Worktree> = serde_json::from_str(raw).expect("decodes");

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].id, "STABLE-ID");
        assert_eq!(decoded[0].source, Source::Muxy);
        assert!(decoded[0].created_at > 0.0);
    }

    #[test]
    fn round_trips_a_persisted_worktree_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let list = vec![
            worktree("PRIMARY-ID", "repo", "/repo", Source::Muxy, true),
            worktree("OTHER-ID", "feature", "/repo-wt", Source::External, false),
        ];

        save_to(temp.path(), "PROJECT-ID", &list).expect("saves");
        let loaded = load_from(temp.path(), "PROJECT-ID");

        let original: Value = serde_json::to_value(&list).expect("encodes");
        let reloaded: Value = serde_json::to_value(&loaded).expect("re-encodes");
        assert_eq!(original, reloaded);
    }
}
