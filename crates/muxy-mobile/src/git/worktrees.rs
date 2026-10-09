//! Worktrees belong to a top-level project. Called on a worktree project, the
//! calls that list and add worktrees act on its parent.

use std::ffi::OsStr;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

use muxy_protocol::{
    GitAction, GitReply, OperationId, ProjectDescriptor, ProjectId, ServerPath, WorktreeAction,
    WorktreeIntent,
};

use super::GitRepository;
use super::records::{GitWorktree, WorktreeRemoval, collect};
use super::repository::unexpected;
use crate::MobileError;
use crate::connection::find_project;
use crate::records::{Project, server_path};

#[uniffi::export]
impl GitRepository {
    /// The repository's worktrees; `registered` names the project of each.
    pub fn worktrees(&self) -> Result<Vec<GitWorktree>, MobileError> {
        let root = self.root()?;
        match self.request_in(root.id, GitAction::Worktrees)? {
            GitReply::Worktrees(worktrees) => Ok(collect(worktrees)),
            other => Err(unexpected(other)),
        }
    }

    /// Checks out `branch` in a new worktree and returns its project. `base`
    /// creates the branch from that ref, such as `HEAD` or `main`; `None`
    /// checks out an existing branch. Without a `directory`, the worktree goes
    /// next to the project's folder, named `<project>-<branch>`, as on the desktop.
    pub fn create_worktree(
        &self,
        branch: String,
        base: Option<String>,
        directory: Option<String>,
    ) -> Result<Project, MobileError> {
        let root = self.root()?;
        let directory = directory.map_or_else(|| suggested_folder(&root, &branch), server_path);
        self.add_worktree(
            &root,
            WorktreeAction::Create {
                project: ProjectId::new(),
                directory,
                branch,
                base,
            },
        )
    }

    /// Adds a worktree made outside Muxy, at its absolute `directory`.
    pub fn register_worktree(&self, directory: String) -> Result<Project, MobileError> {
        let root = self.root()?;
        self.add_worktree(
            &root,
            WorktreeAction::Register {
                project: ProjectId::new(),
                directory: server_path(directory),
            },
        )
    }

    /// Checks out a pull request in a new worktree, by default next to the
    /// project's folder, named `<project>-pr-<number>`.
    pub fn checkout_pull_request_worktree(
        &self,
        number: u64,
        directory: Option<String>,
    ) -> Result<Project, MobileError> {
        let root = self.root()?;
        let directory = directory.map_or_else(
            || suggested_folder(&root, &format!("pr-{number}")),
            server_path,
        );
        self.add_worktree(
            &root,
            WorktreeAction::CheckoutPullRequest {
                project: ProjectId::new(),
                directory,
                number,
            },
        )
    }

    /// Describes this worktree project for confirming its removal, such as
    /// whether it has uncommitted changes.
    pub fn inspect_worktree_removal(&self) -> Result<WorktreeRemoval, MobileError> {
        match self.request(GitAction::InspectRemoval)? {
            GitReply::Removal(removal) => Ok(removal.into()),
            other => Err(unexpected(other)),
        }
    }

    /// Deletes this worktree project and its folder, and ends its terminals,
    /// if nothing changed since `inspect_worktree_removal`.
    pub fn remove_worktree(&self, expected: WorktreeRemoval) -> Result<(), MobileError> {
        self.run(GitAction::Worktree(WorktreeIntent {
            options: None,
            operation: OperationId::new(),
            action: WorktreeAction::Remove {
                expected: expected.into(),
            },
        }))
    }
}

impl GitRepository {
    /// The project that owns the worktrees: this one, or its parent.
    fn root(&self) -> Result<ProjectDescriptor, MobileError> {
        let catalog = self.client.catalog()?;
        let project = find_project(&catalog.projects, self.project)?;
        let root = match project.parent_id {
            Some(parent) => find_project(&catalog.projects, parent)?,
            None => project,
        };
        Ok(root.clone())
    }

    fn add_worktree(
        &self,
        root: &ProjectDescriptor,
        action: WorktreeAction,
    ) -> Result<Project, MobileError> {
        let intent = WorktreeIntent {
            options: None,
            operation: OperationId::new(),
            action,
        };
        match self.request_in(root.id, GitAction::Worktree(intent))? {
            GitReply::Project(project) => Ok(Project::from(&project)),
            other => Err(unexpected(other)),
        }
    }
}

/// Next to the project's folder, named after the project and `suffix`.
fn suggested_folder(project: &ProjectDescriptor, suffix: &str) -> ServerPath {
    let directory = Path::new(OsStr::from_bytes(&project.directory.0));
    let name = format!("{}-{}", folder_name(&project.name), folder_name(suffix));
    let folder = directory.parent().unwrap_or(directory).join(name);
    ServerPath(folder.into_os_string().into_vec())
}

/// Slashes would nest folders, so they become dashes.
fn folder_name(name: &str) -> String {
    name.trim().replace(['/', '\\'], "-")
}
