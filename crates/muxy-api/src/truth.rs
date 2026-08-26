use crate::git::GitOptions;
use crate::{git, worktrees};
use muxy_core::store::Worktree;
use muxy_core::store::worktrees::WorktreeFile;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ProjectProbe {
    pub project_id: String,
    pub project_name: String,
    pub project_path: String,
    pub preferred_worktree_id: Option<String>,
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
                let candidate = worktrees::probe_from(
                    options,
                    worktrees_dir,
                    &project.project_id,
                    &project.project_name,
                    &project.project_path,
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
            ProjectTruth {
                project_id: project.project_id.clone(),
                generation: project.generation,
                request_id: project.request_id,
                is_git_repo,
                worktree_label: worktree_label.flatten(),
                worktrees: list,
                candidate,
            }
        })
        .collect()
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
            generation: 7,
            request_id: 11,
        }];

        let truth = refresh_truth_from(&options, &worktrees_dir, &projects);

        assert_eq!(truth.len(), 1);
        assert_eq!(truth[0].generation, 7);
        assert_eq!(truth[0].request_id, 11);
        assert!(truth[0].candidate.is_some());
        assert!(!worktrees_dir.exists());
    }
}
