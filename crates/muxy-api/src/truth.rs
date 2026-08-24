use crate::git::GitOptions;
use crate::{git, worktrees};
use muxy_core::store::Worktree;
use std::path::Path;

pub type ProjectProbe = (String, String, String, Option<String>);

#[derive(Debug, Clone)]
pub struct ProjectTruth {
    pub project_id: String,
    pub is_git_repo: bool,
    pub worktree_label: Option<String>,
    pub worktrees: Vec<Worktree>,
}

pub fn refresh_truth(options: &GitOptions, projects: &[ProjectProbe]) -> Vec<ProjectTruth> {
    projects
        .iter()
        .map(|(id, name, path, preferred_worktree_id)| {
            let is_git_repo = git::is_git_repo(options, Path::new(path));
            let list = if is_git_repo {
                worktrees::refresh(options, id, name, path)
            } else {
                Vec::new()
            };
            let worktree_label =
                is_git_repo.then(|| worktrees::label(&list, preferred_worktree_id.as_deref()));
            ProjectTruth {
                project_id: id.clone(),
                is_git_repo,
                worktree_label: worktree_label.flatten(),
                worktrees: list,
            }
        })
        .collect()
}
