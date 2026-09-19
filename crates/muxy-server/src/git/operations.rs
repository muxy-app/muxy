use super::{Result, error};
use muxy_protocol::{OperationId, ProjectId, ProjectMutation};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};

#[derive(Debug, Default)]
pub(crate) struct Operations {
    tokens: Mutex<HashMap<OperationId, Weak<Mutex<()>>>>,
    active: Mutex<Active>,
}

#[derive(Debug, Default)]
struct Active {
    projects: HashSet<ProjectId>,
    removals: HashSet<PathBuf>,
}

pub(super) struct Reservation<'a> {
    operations: &'a Operations,
    projects: Vec<ProjectId>,
    removal: Option<PathBuf>,
}

impl Operations {
    pub(super) fn token(&self, operation: OperationId) -> Arc<Mutex<()>> {
        let mut tokens = self.tokens.lock().unwrap_or_else(PoisonError::into_inner);
        tokens.retain(|_, token| token.strong_count() > 0);
        let token = tokens
            .get(&operation)
            .and_then(Weak::upgrade)
            .unwrap_or_default();
        tokens.insert(operation, Arc::downgrade(&token));
        token
    }

    pub(super) fn reserve(
        &self,
        projects: Vec<ProjectId>,
        removal: Option<PathBuf>,
    ) -> Result<Reservation<'_>> {
        let mut active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if projects.iter().any(|id| active.projects.contains(id)) {
            return Err(error(
                "Project has an active worktree operation; retry when it completes",
            ));
        }
        active.projects.extend(&projects);
        if let Some(path) = &removal {
            active.removals.insert(path.clone());
        }
        Ok(Reservation {
            operations: self,
            projects,
            removal,
        })
    }

    pub(crate) fn check_mutation(&self, mutation: &ProjectMutation) -> Result<()> {
        let projects = match mutation {
            ProjectMutation::Create(project) => [Some(project.id), project.parent_id],
            ProjectMutation::Patch { project, .. } | ProjectMutation::Delete(project) => {
                [Some(*project), None]
            }
        };
        let active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if projects
            .into_iter()
            .flatten()
            .any(|id| active.projects.contains(&id))
        {
            return Err(error(
                "Project has an active worktree operation; retry when it completes",
            ));
        }
        Ok(())
    }

    pub(crate) fn check_session(&self, directory: &Path) -> Result<()> {
        let active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if active
            .removals
            .iter()
            .any(|path| directory.starts_with(path))
        {
            return Err(error("Worktree is being removed"));
        }
        Ok(())
    }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        let mut active = self
            .operations
            .active
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for id in &self.projects {
            active.projects.remove(id);
        }
        if let Some(path) = &self.removal {
            active.removals.remove(path);
        }
    }
}
