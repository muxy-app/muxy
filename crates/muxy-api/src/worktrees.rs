use crate::git;
use crate::git::GitOptions;
use muxy_core::store::worktrees::{
    Source, Worktree, WorktreeFile, load_file_from, load_or_create_primary_from, primary, save_to,
    worktrees_dir,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum RefreshOutcome {
    Updated(Vec<Worktree>),
    Preserved(Vec<Worktree>, String),
    Unavailable(String),
}

#[derive(Debug, Clone)]
struct Record {
    path: String,
    branch: Option<String>,
    is_bare: bool,
    is_prunable: bool,
}

fn parse_porcelain(raw: &str) -> Vec<Record> {
    let mut records = Vec::new();
    let mut current: Option<Record> = None;
    for line in raw.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            records.extend(current.take());
            continue;
        }
        if let Some(path) = line.strip_prefix("worktree ") {
            records.extend(current.take());
            current = Some(Record {
                path: path.to_owned(),
                branch: None,
                is_bare: false,
                is_prunable: false,
            });
            continue;
        }
        let Some(record) = current.as_mut() else {
            continue;
        };
        if let Some(branch) = line.strip_prefix("branch ") {
            record.branch = Some(
                branch
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch)
                    .to_owned(),
            );
        } else if line == "bare" {
            record.is_bare = true;
        } else if line == "prunable" || line.starts_with("prunable ") {
            record.is_prunable = true;
        }
    }
    records.extend(current);
    records
}

fn is_externally_managed(worktree: &Worktree) -> bool {
    !worktree.is_primary && worktree.source == Source::External
}

fn default_name(record: &Record) -> String {
    if let Some(branch) = record.branch.as_deref() {
        let trimmed = branch.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    Path::new(&record.path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| record.path.clone())
}

pub fn load_or_create_primary(
    project_id: &str,
    project_name: &str,
    project_path: &str,
) -> Option<Vec<Worktree>> {
    load_or_create_primary_from(&worktrees_dir(), project_id, project_name, project_path)
        .ok()
        .flatten()
}

fn reconcile(
    persisted: Vec<Worktree>,
    records: &[Record],
    project_name: &str,
    project_path: &str,
) -> Vec<Worktree> {
    let records: Vec<&Record> = records
        .iter()
        .filter(|record| !record.is_bare && !record.is_prunable)
        .collect();

    let mut list = persisted;
    match list.iter().position(|worktree| worktree.is_primary) {
        Some(index) => {
            list[index].path = project_path.to_owned();
            list[index].name = project_name.to_owned();
        }
        None => list.insert(0, primary(project_name, project_path)),
    }

    let mut existing_by_key: HashMap<PathBuf, (String, bool)> = HashMap::new();
    for worktree in &list {
        let key = git::canonical_path(Path::new(&worktree.path));
        match existing_by_key.get(&key) {
            Some((_, existing_is_primary)) => {
                if worktree.is_primary && !existing_is_primary {
                    existing_by_key.insert(key, (worktree.id.clone(), true));
                }
            }
            None => {
                existing_by_key.insert(key, (worktree.id.clone(), worktree.is_primary));
            }
        }
    }

    let project_key = git::canonical_path(Path::new(project_path));
    let record_keys: HashSet<PathBuf> = records
        .iter()
        .map(|record| git::canonical_path(Path::new(&record.path)))
        .collect();

    for record in &records {
        let record_key = git::canonical_path(Path::new(&record.path));
        if record_key == project_key {
            if let Some(index) = list.iter().position(|worktree| worktree.is_primary) {
                list[index].branch = record.branch.clone();
            }
            continue;
        }
        let existing_index = existing_by_key
            .get(&record_key)
            .and_then(|(id, _)| list.iter().position(|worktree| &worktree.id == id));
        if let Some(index) = existing_index {
            if list[index].is_primary {
                list[index].name = project_name.to_owned();
                list[index].path = project_path.to_owned();
            } else if record.branch.is_some()
                && list[index].branch.as_deref() == Some(list[index].name.as_str())
            {
                list[index].name = default_name(record);
            }
            list[index].branch = record.branch.clone();
            continue;
        }
        list.push(Worktree {
            id: muxy_core::store::new_uuid(),
            name: default_name(record),
            path: record.path.clone(),
            branch: record.branch.clone(),
            source: Source::External,
            is_primary: false,
            created_at: muxy_core::store::reference_now(),
            last_active_at: None,
        });
    }

    list.retain(|worktree| {
        !is_externally_managed(worktree)
            || record_keys.contains(&git::canonical_path(Path::new(&worktree.path)))
    });

    sort_primary_first(collapse_duplicate_paths(list))
}

fn collapse_duplicate_paths(list: Vec<Worktree>) -> Vec<Worktree> {
    let mut index_by_key: HashMap<PathBuf, usize> = HashMap::new();
    let mut result: Vec<Worktree> = Vec::new();
    for worktree in list {
        if worktree.is_primary {
            result.push(worktree);
            continue;
        }
        let key = git::canonical_path(Path::new(&worktree.path));
        match index_by_key.get(&key) {
            Some(&existing) => {
                if is_externally_managed(&result[existing]) && !is_externally_managed(&worktree) {
                    result[existing] = worktree;
                }
            }
            None => {
                index_by_key.insert(key, result.len());
                result.push(worktree);
            }
        }
    }
    result
}

fn sort_primary_first(list: Vec<Worktree>) -> Vec<Worktree> {
    let (primary, mut others): (Vec<Worktree>, Vec<Worktree>) =
        list.into_iter().partition(|worktree| worktree.is_primary);
    others.sort_by(|left, right| left.created_at.total_cmp(&right.created_at));
    primary.into_iter().chain(others).collect()
}

pub fn refresh(
    options: &GitOptions,
    project_id: &str,
    project_name: &str,
    project_path: &str,
) -> RefreshOutcome {
    let dir = worktrees_dir();
    let persisted = match load_file_from(&dir, project_id) {
        Ok(WorktreeFile::Missing) => {
            let worktrees = vec![primary(project_name, project_path)];
            if let Err(error) = save_to(&dir, project_id, &worktrees) {
                return RefreshOutcome::Unavailable(error.to_string());
            }
            worktrees
        }
        Ok(WorktreeFile::Loaded(worktrees))
            if worktrees.iter().any(|worktree| worktree.is_primary) =>
        {
            worktrees
        }
        Ok(WorktreeFile::Loaded(worktrees)) => return RefreshOutcome::Updated(worktrees),
        Ok(WorktreeFile::Invalid) => {
            return RefreshOutcome::Unavailable("worktree file is malformed".to_owned());
        }
        Err(error) => return RefreshOutcome::Unavailable(error.to_string()),
    };
    let raw = match git::run_git(
        options,
        Path::new(project_path),
        &["worktree", "list", "--porcelain"],
    ) {
        Ok(raw) => raw,
        Err(error) => {
            log::warn!("worktree list failed for {project_path}: {error}");
            return RefreshOutcome::Preserved(persisted, error.to_string());
        }
    };
    let reconciled = reconcile(
        persisted.clone(),
        &parse_porcelain(&raw),
        project_name,
        project_path,
    );
    if let Err(error) = save_to(&dir, project_id, &reconciled) {
        return RefreshOutcome::Preserved(persisted, error.to_string());
    }
    RefreshOutcome::Updated(reconciled)
}

pub fn label(worktrees: &[Worktree], preferred_id: Option<&str>) -> Option<String> {
    let preferred = preferred_id
        .and_then(|id| {
            worktrees
                .iter()
                .find(|worktree| worktree.id.eq_ignore_ascii_case(id))
        })
        .or_else(|| worktrees.iter().find(|worktree| worktree.is_primary))
        .or_else(|| worktrees.first())?;
    Some(if preferred.is_primary {
        "primary".to_owned()
    } else {
        preferred.name.clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(path: &str, branch: Option<&str>) -> Record {
        Record {
            path: path.to_owned(),
            branch: branch.map(str::to_owned),
            is_bare: false,
            is_prunable: false,
        }
    }

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
    fn parses_porcelain_skipping_head_and_detached_lines() {
        let raw = "worktree /repo\nHEAD abc123\nbranch refs/heads/main\n\nworktree /repo/wt\nHEAD def456\ndetached\nprunable gitdir file points to non-existent location\n\nworktree /bare\nbare\n";

        let records = parse_porcelain(raw);

        assert_eq!(records.len(), 3);
        assert_eq!(records[0].path, "/repo");
        assert_eq!(records[0].branch.as_deref(), Some("main"));
        assert!(!records[0].is_prunable);
        assert_eq!(records[1].path, "/repo/wt");
        assert!(records[1].is_prunable);
        assert!(records[2].is_bare);
    }

    #[test]
    fn reconcile_keeps_the_persisted_id_for_a_path_git_still_reports() {
        let persisted = vec![
            worktree("PRIMARY-ID", "repo", "/repo", Source::Muxy, true),
            worktree("KEEP-ID", "feature", "/repo-wt", Source::External, false),
        ];

        let result = reconcile(
            persisted,
            &[
                record("/repo", Some("main")),
                record("/repo-wt", Some("feature")),
            ],
            "repo",
            "/repo",
        );

        let kept = result
            .iter()
            .find(|worktree| worktree.path == "/repo-wt")
            .expect("entry survives");
        assert_eq!(kept.id, "KEEP-ID");
    }

    #[test]
    fn reconcile_drops_a_vanished_external_but_keeps_a_vanished_muxy_entry() {
        let persisted = vec![
            worktree("PRIMARY-ID", "repo", "/repo", Source::Muxy, true),
            worktree(
                "EXTERNAL-ID",
                "gone",
                "/gone-external",
                Source::External,
                false,
            ),
            worktree("MUXY-ID", "kept", "/gone-muxy", Source::Muxy, false),
        ];

        let result = reconcile(persisted, &[record("/repo", Some("main"))], "repo", "/repo");

        assert!(!result.iter().any(|worktree| worktree.id == "EXTERNAL-ID"));
        assert!(result.iter().any(|worktree| worktree.id == "MUXY-ID"));
    }

    #[test]
    fn reconcile_renames_an_adopted_entry_still_named_after_its_branch() {
        let mut tracking = worktree(
            "TRACK-ID",
            "old-branch",
            "/repo-wt",
            Source::External,
            false,
        );
        tracking.branch = Some("old-branch".to_owned());
        let mut pinned = worktree(
            "PINNED-ID",
            "custom",
            "/repo-other",
            Source::External,
            false,
        );
        pinned.branch = Some("old-branch".to_owned());
        let persisted = vec![
            worktree("PRIMARY-ID", "repo", "/repo", Source::Muxy, true),
            tracking,
            pinned,
        ];

        let result = reconcile(
            persisted,
            &[
                record("/repo", Some("main")),
                record("/repo-wt", Some("new-branch")),
                record("/repo-other", Some("new-branch")),
            ],
            "repo",
            "/repo",
        );

        let renamed = result
            .iter()
            .find(|worktree| worktree.id == "TRACK-ID")
            .expect("tracking entry");
        assert_eq!(renamed.name, "new-branch");
        let untouched = result
            .iter()
            .find(|worktree| worktree.id == "PINNED-ID")
            .expect("pinned entry");
        assert_eq!(untouched.name, "custom");
    }

    #[test]
    fn reconcile_collapses_duplicate_paths_keeping_the_non_external_entry() {
        let persisted = vec![
            worktree("PRIMARY-ID", "repo", "/repo", Source::Muxy, true),
            worktree("EXTERNAL-ID", "dup", "/repo-wt", Source::External, false),
            worktree("MUXY-ID", "dup", "/repo-wt", Source::Muxy, false),
        ];

        let result = reconcile(
            persisted,
            &[
                record("/repo", Some("main")),
                record("/repo-wt", Some("dup")),
            ],
            "repo",
            "/repo",
        );

        let matching: Vec<&Worktree> = result
            .iter()
            .filter(|worktree| worktree.path == "/repo-wt")
            .collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].id, "MUXY-ID");
    }

    #[test]
    fn reconcile_mints_a_primary_when_nothing_is_persisted() {
        let result = reconcile(
            Vec::new(),
            &[record("/repo", Some("main"))],
            "repo",
            "/repo",
        );

        assert_eq!(result.len(), 1);
        assert!(result[0].is_primary);
        assert_eq!(result[0].name, "repo");
        assert_eq!(result[0].path, "/repo");
        assert_eq!(result[0].branch.as_deref(), Some("main"));
        assert!(!result[0].id.is_empty());
    }

    #[test]
    fn label_prefers_the_named_worktree_then_the_primary() {
        let list = vec![
            worktree("PRIMARY-ID", "repo", "/repo", Source::Muxy, true),
            worktree("OTHER-ID", "feature", "/repo-wt", Source::External, false),
        ];

        assert_eq!(label(&list, Some("OTHER-ID")).as_deref(), Some("feature"));
        assert_eq!(label(&list, None).as_deref(), Some("primary"));
        assert_eq!(label(&list, Some("MISSING")).as_deref(), Some("primary"));
        assert_eq!(label(&[], None), None);
    }
}
