use super::{Catalog, Receipt, ServerError, State, bad, unknown};
use muxy_protocol::{MAX_PROJECTS, ProjectId, ProjectIntent, ProjectMutation, SessionId};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

impl Catalog {
    pub(crate) fn begin_mutation(&self, intent: &ProjectIntent) -> Result<bool, ServerError> {
        let receipt = self.lock().receipts.get(&intent.operation).cloned();
        if let Some(receipt) = receipt {
            self.ensure_durable()?;
            if receipt.intent != *intent {
                return Err(bad(
                    "operation token reused with different project mutation",
                ));
            }
            if let Some(error) = receipt.error {
                return Err(ServerError::new(error.code, error.message));
            }
            return Ok(receipt.complete);
        }
        intent
            .mutation
            .validate()
            .map_err(|_| bad("invalid project mutation"))?;
        self.update(|state| {
            let result = match &intent.mutation {
                ProjectMutation::Create(project) => {
                    if state.projects.contains_key(&project.id)
                        || state.projects.len() == MAX_PROJECTS
                    {
                        Err(bad("project identity already exists or catalog is full"))
                    } else if !Path::new(OsStr::from_bytes(&project.directory.0)).is_dir() {
                        Err(ServerError::new(
                            muxy_protocol::ErrorCode::BadPath,
                            "project folder does not exist",
                        ))
                    } else {
                        state.validate_parent(project).and_then(|()| {
                            if project
                                .parent_id
                                .is_some_and(|id| state.deleting.contains(&id))
                            {
                                return Err(unknown());
                            }
                            state.projects.insert(project.id, project.clone());
                            Ok(())
                        })
                    }
                }
                ProjectMutation::Patch { project, patch } => {
                    if state.deleting.contains(project) {
                        Err(unknown())
                    } else if let Some(project) = state.projects.get_mut(project) {
                        patch.apply(project);
                        Ok(())
                    } else {
                        Err(unknown())
                    }
                }
                ProjectMutation::Delete(project) => {
                    if *project == state.home {
                        Err(bad("Home cannot be deleted"))
                    } else if !state.projects.contains_key(project) {
                        Err(unknown())
                    } else {
                        state.deleting.insert(*project);
                        state.deleting.extend(
                            state
                                .projects
                                .values()
                                .filter(|child| child.parent_id == Some(*project))
                                .map(|child| child.id),
                        );
                        Ok(())
                    }
                }
                ProjectMutation::PruneWorktree(id) => {
                    if state.can_prune_worktree(*id) {
                        state.projects.remove(id);
                    }
                    Ok(())
                }
            };
            state.receipts.insert(
                intent.operation,
                Receipt {
                    intent: intent.clone(),
                    error: result.as_ref().err().map(ServerError::to_reply),
                    complete: !matches!(intent.mutation, ProjectMutation::Delete(_))
                        || result.is_err(),
                },
            );
            Ok(result)
        })??;
        Ok(!matches!(intent.mutation, ProjectMutation::Delete(_)))
    }

    pub(crate) fn deleting(&self) -> Vec<ProjectId> {
        self.lock().deleting.iter().copied().collect()
    }

    pub(crate) fn owned(&self, project: ProjectId) -> Vec<SessionId> {
        self.lock()
            .sessions
            .values()
            .filter(|record| record.info.project == project)
            .map(|record| record.info.id)
            .collect()
    }

    pub(crate) fn finish_deletions(&self) -> Result<(), ServerError> {
        self.update(|state| {
            for project in &state.deleting {
                if state
                    .sessions
                    .values()
                    .any(|session| session.info.project == *project)
                {
                    return Err(bad("project still owns session content"));
                }
                state.projects.remove(project);
            }
            for receipt in state.receipts.values_mut() {
                if let ProjectMutation::Delete(project) = receipt.intent.mutation
                    && state.deleting.contains(&project)
                {
                    receipt.complete = true;
                }
            }
            state.deleting.clear();
            Ok(())
        })
    }
}

impl State {
    fn can_prune_worktree(&self, id: ProjectId) -> bool {
        let Some(project) = self.projects.get(&id) else {
            return false;
        };
        !project.home
            && project.kind == Some(muxy_protocol::ProjectKind::Worktree)
            && !self.deleting.contains(&id)
            && project
                .parent_id
                .and_then(|id| self.projects.get(&id))
                .is_some_and(|parent| Path::new(OsStr::from_bytes(&parent.directory.0)).is_dir())
            && Path::new(OsStr::from_bytes(&project.directory.0))
                .try_exists()
                .is_ok_and(|exists| !exists)
            && !self
                .sessions
                .values()
                .any(|session| session.info.project == id)
    }
}
