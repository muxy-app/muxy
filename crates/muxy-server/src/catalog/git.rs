use super::{Catalog, ServerError, bad, unknown};
use muxy_protocol::{GitReply, OperationId, ProjectDescriptor, ProjectId, WorktreeIntent};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitReceipt {
    pub(crate) owner: ProjectId,
    pub(crate) intent: WorktreeIntent,
    pub(crate) project: ProjectDescriptor,
    pub(crate) device: u64,
    pub(crate) inode: u64,
    pub(crate) applied: bool,
    #[serde(default)]
    pub(crate) hooks_started: bool,
    #[serde(default)]
    pub(crate) hooks_finished: bool,
    #[serde(default)]
    pub(crate) hook_error: Option<String>,
    #[serde(default)]
    pub(crate) failed: Option<muxy_protocol::ErrorReply>,
    pub(crate) reply: Option<GitReply>,
}

impl Catalog {
    pub(crate) fn project(&self, id: ProjectId) -> Result<ProjectDescriptor, ServerError> {
        let state = self.lock();
        if state.deleting.contains(&id) {
            return Err(unknown());
        }
        state.projects.get(&id).cloned().ok_or_else(unknown)
    }
    pub(crate) fn child_at(&self, parent: ProjectId, path: &Path) -> Option<ProjectId> {
        use std::os::unix::ffi::OsStrExt;
        let canonical = path.canonicalize().ok()?;
        let projects: Vec<_> = self
            .lock()
            .projects
            .values()
            .filter(|p| p.parent_id == Some(parent))
            .cloned()
            .collect();
        projects
            .iter()
            .find(|p| {
                Path::new(std::ffi::OsStr::from_bytes(&p.directory.0))
                    .canonicalize()
                    .is_ok_and(|p| p == canonical)
            })
            .map(|p| p.id)
    }
    pub(crate) fn has_project_receipt(&self, operation: OperationId) -> bool {
        self.lock().receipts.contains_key(&operation)
    }
    pub(crate) fn git_receipt(&self, operation: OperationId) -> Option<GitReceipt> {
        self.lock().git.get(&operation).cloned()
    }
    pub(crate) fn pending_git(&self) -> Vec<GitReceipt> {
        self.lock()
            .git
            .values()
            .filter(|r| r.reply.is_none() && r.failed.is_none())
            .cloned()
            .collect()
    }
    pub(crate) fn save_git(&self, receipt: GitReceipt) -> Result<(), ServerError> {
        self.update(|state| {
            state.git.insert(receipt.intent.operation, receipt);
            Ok(())
        })
    }
    pub(crate) fn finish_git_project(&self, receipt: &GitReceipt) -> Result<(), ServerError> {
        self.update(|state| {
            if let Some(project) = state.projects.get(&receipt.project.id) {
                if project != &receipt.project {
                    return Err(bad("Worktree identity already exists"));
                }
            } else {
                state.validate_parent(&receipt.project)?;
                if state.projects.len() >= muxy_protocol::MAX_PROJECTS {
                    return Err(bad("Project catalog is full"));
                }
                state
                    .projects
                    .insert(receipt.project.id, receipt.project.clone());
            }
            let mut done = receipt.clone();
            done.reply = Some(receipt.hook_error.as_ref().map_or_else(
                || GitReply::Project(receipt.project.clone()),
                |message| GitReply::WorktreeSetupFailed {
                    project: receipt.project.clone(),
                    message: message.clone(),
                },
            ));
            state.git.insert(receipt.intent.operation, done);
            Ok(())
        })
    }
}
