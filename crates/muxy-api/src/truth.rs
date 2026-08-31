use crate::git::GitOptions;
use crate::{git, worktrees};
use muxy_core::store::Worktree;
use muxy_core::store::worktrees::WorktreeFile;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OwnerExistence {
    Present,
    Missing,
    Unknown,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OwnerExistenceFact {
    pub project_id: String,
    pub worktree_id: Option<String>,
    pub path: String,
    pub generation: u64,
    pub request_id: u64,
    pub existence: OwnerExistence,
}

#[derive(Debug, Clone)]
pub struct ProjectProbe {
    pub project_id: String,
    pub project_name: String,
    pub project_path: String,
    pub preferred_worktree_id: Option<String>,
    pub current_worktrees: Vec<Worktree>,
    pub generation: u64,
    pub request_id: u64,
}

#[derive(Debug, Clone)]
pub struct ProjectTruth {
    pub project_id: String,
    pub generation: u64,
    pub request_id: u64,
    pub is_git_repo: bool,
    pub worktree_label: Option<String>,
    pub worktrees: Option<Vec<Worktree>>,
    pub candidate: Option<worktrees::RefreshCandidate>,
    pub owner_existence: Vec<OwnerExistenceFact>,
}

pub fn refresh_truth(options: &GitOptions, projects: &[ProjectProbe]) -> Vec<ProjectTruth> {
    refresh_truth_from(
        options,
        &muxy_core::store::worktrees::worktrees_dir(),
        projects,
    )
}

pub fn refresh_truth_from(
    options: &GitOptions,
    worktrees_dir: &Path,
    projects: &[ProjectProbe],
) -> Vec<ProjectTruth> {
    projects
        .iter()
        .map(|project| {
            let is_git_repo = git::is_git_repo(options, Path::new(&project.project_path));
            let (list, candidate) = if is_git_repo {
                let candidate = worktrees::probe_from_current(
                    options,
                    worktrees_dir,
                    &project.project_id,
                    &project.project_name,
                    &project.project_path,
                    &project.current_worktrees,
                );
                (
                    candidate.worktrees().map(<[Worktree]>::to_vec),
                    Some(candidate),
                )
            } else {
                let list = match muxy_core::store::worktrees::load_file_from(
                    worktrees_dir,
                    &project.project_id,
                ) {
                    Ok(WorktreeFile::Loaded(worktrees)) => Some(worktrees),
                    Ok(WorktreeFile::Missing | WorktreeFile::Invalid) | Err(_) => None,
                };
                (list, None)
            };
            let worktree_label = is_git_repo.then(|| {
                list.as_deref().and_then(|list| {
                    worktrees::label(list, project.preferred_worktree_id.as_deref())
                })
            });
            let mut owner_existence = Vec::with_capacity(project.current_worktrees.len() + 1);
            owner_existence.push(owner_fact(project, None, project.project_path.clone()));
            owner_existence.extend(project.current_worktrees.iter().map(|worktree| {
                owner_fact(project, Some(worktree.id.clone()), worktree.path.clone())
            }));
            ProjectTruth {
                project_id: project.project_id.clone(),
                generation: project.generation,
                request_id: project.request_id,
                is_git_repo,
                worktree_label: worktree_label.flatten(),
                worktrees: list,
                candidate,
                owner_existence,
            }
        })
        .collect()
}

fn owner_fact(
    project: &ProjectProbe,
    worktree_id: Option<String>,
    path: String,
) -> OwnerExistenceFact {
    OwnerExistenceFact {
        project_id: project.project_id.clone(),
        worktree_id,
        existence: probe_owner_path(Path::new(&path)),
        path,
        generation: project.generation,
        request_id: project.request_id,
    }
}

pub fn probe_owner_path(path: &Path) -> OwnerExistence {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => OwnerExistence::Present,
        Ok(_) => OwnerExistence::Missing,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            OwnerExistence::Missing
        }
        Err(_) => OwnerExistence::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn truth_refresh_returns_candidates_without_writing_tracking_state() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "-q"])
                .arg(&repo)
                .status()
                .unwrap()
                .success()
        );
        let worktrees_dir = temp.path().join("worktrees");
        let options = GitOptions {
            executable: PathBuf::from("git"),
            environment: HashMap::new(),
        };
        let projects = [ProjectProbe {
            project_id: "PROJECT".to_owned(),
            project_name: "Repo".to_owned(),
            project_path: repo.to_string_lossy().into_owned(),
            preferred_worktree_id: None,
            current_worktrees: Vec::new(),
            generation: 7,
            request_id: 11,
        }];

        let truth = refresh_truth_from(&options, &worktrees_dir, &projects);

        assert_eq!(truth.len(), 1);
        assert_eq!(truth[0].generation, 7);
        assert_eq!(truth[0].request_id, 11);
        assert!(truth[0].candidate.is_some());
        assert_eq!(truth[0].owner_existence.len(), 1);
        assert_eq!(
            truth[0].owner_existence[0].existence,
            OwnerExistence::Present
        );
        assert!(!worktrees_dir.exists());
    }

    #[cfg(unix)]
    #[test]
    fn truth_owner_existence_follows_directory_symlinks_and_classifies_missing_paths() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let present = temp.path().join("present");
        let link = temp.path().join("link");
        std::fs::create_dir(&present).unwrap();
        symlink(&present, &link).unwrap();

        assert_eq!(probe_owner_path(&link), OwnerExistence::Present);
        assert_eq!(
            probe_owner_path(&temp.path().join("missing")),
            OwnerExistence::Missing
        );
        let file = temp.path().join("file");
        std::fs::write(&file, "value").unwrap();
        assert_eq!(probe_owner_path(&file), OwnerExistence::Missing);
        assert_eq!(
            probe_owner_path(&file.join("child")),
            OwnerExistence::Missing
        );
    }

    #[cfg(unix)]
    #[test]
    fn truth_owner_existence_treats_permission_errors_and_symlink_loops_as_unknown() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        symlink(&second, &first).unwrap();
        symlink(&first, &second).unwrap();
        assert_eq!(probe_owner_path(&first), OwnerExistence::Unknown);

        let restricted = temp.path().join("restricted");
        std::fs::create_dir(&restricted).unwrap();
        std::fs::set_permissions(&restricted, std::fs::Permissions::from_mode(0o000)).unwrap();
        let existence = probe_owner_path(&restricted.join("child"));
        std::fs::set_permissions(&restricted, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(existence, OwnerExistence::Unknown);
    }
}
