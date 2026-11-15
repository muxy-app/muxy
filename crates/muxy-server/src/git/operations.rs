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

impl Active {
    fn check_projects(&self, projects: impl IntoIterator<Item = ProjectId>) -> Result<()> {
        if projects.into_iter().any(|id| self.projects.contains(&id)) {
            return Err(error(
                "Project has an active worktree operation; retry when it completes",
            ));
        }
        Ok(())
    }
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
        active.check_projects(projects.iter().copied())?;
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
            ProjectMutation::Patch { project, .. }
            | ProjectMutation::Delete(project)
            | ProjectMutation::PruneWorktree(project) => [Some(*project), None],
        };
        let active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        active.check_projects(projects.into_iter().flatten())
    }

    pub(crate) fn check_projects(&self, projects: &[ProjectId]) -> Result<()> {
        let active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        active.check_projects(projects.iter().copied())
    }

    pub(crate) fn check_session(&self, directory: &Path) -> Result<()> {
        let active = self.active.lock().unwrap_or_else(PoisonError::into_inner);
        if active
            .removals
            .iter()
            .any(|path| muxy_core::paths::path_starts_with(directory, path))
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

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_protocol::{ProjectDescriptor, ProjectPatch, ServerPath};

    #[test]
    fn sessions_stay_put_while_their_project_has_a_worktree_operation() {
        let operations = Operations::default();
        let (busy, other) = (ProjectId::new(), ProjectId::new());
        assert!(operations.check_projects(&[busy, other]).is_ok());
        let reservation = operations.reserve(vec![busy], None);
        assert!(reservation.is_ok());
        assert!(operations.check_projects(&[other, busy]).is_err());
        assert!(operations.check_projects(&[other]).is_ok());
        drop(reservation);
        assert!(operations.check_projects(&[busy]).is_ok());
    }

    #[test]
    fn rejected_reservations_do_not_claim_projects_or_removal_paths() -> Result<()> {
        let operations = Operations::default();
        let (busy, other) = (ProjectId::new(), ProjectId::new());
        let directory = PathBuf::from("worktree");
        let reservation = operations.reserve(vec![busy], None)?;
        let rejected = operations.reserve(vec![other, busy], Some(directory.clone()));
        assert_eq!(
            rejected.err(),
            Some(error(
                "Project has an active worktree operation; retry when it completes"
            ))
        );
        operations.check_projects(&[other])?;
        operations.check_session(&directory)?;
        drop(reservation);

        let reservation = operations.reserve(vec![other, busy], Some(directory.clone()))?;
        assert!(operations.check_projects(&[busy]).is_err());
        assert!(operations.check_projects(&[other]).is_err());
        assert!(operations.check_session(&directory.join("child")).is_err());
        drop(reservation);
        operations.check_projects(&[busy, other])?;
        operations.check_session(&directory.join("child"))
    }

    #[test]
    fn mutations_check_their_project_and_creation_parent_until_release() -> Result<()> {
        let operations = Operations::default();
        let (busy, other) = (ProjectId::new(), ProjectId::new());
        let reservation = operations.reserve(vec![busy], None)?;
        let mut mutations = Vec::new();
        for project in [busy, other] {
            let descriptor = ProjectDescriptor {
                id: project,
                ..ProjectDescriptor::new(ServerPath::from(Path::new("worktree")), [])
            };
            let child = ProjectDescriptor {
                id: ProjectId::new(),
                parent_id: Some(project),
                ..descriptor.clone()
            };
            for mutation in [
                ProjectMutation::Create(descriptor),
                ProjectMutation::Create(child),
                ProjectMutation::Patch {
                    project,
                    patch: ProjectPatch::Name("renamed".into()),
                },
                ProjectMutation::Delete(project),
                ProjectMutation::PruneWorktree(project),
            ] {
                let expected = if project == busy {
                    Err(error(
                        "Project has an active worktree operation; retry when it completes",
                    ))
                } else {
                    Ok(())
                };
                assert_eq!(
                    operations.check_mutation(&mutation),
                    expected,
                    "{mutation:?}"
                );
                mutations.push(mutation);
            }
        }
        drop(reservation);
        for mutation in mutations {
            operations.check_mutation(&mutation)?;
        }
        Ok(())
    }

    #[test]
    fn concurrent_reservations_have_one_owner_until_release() {
        let operations = Operations::default();
        let project = ProjectId::new();
        let barrier = std::sync::Barrier::new(2);
        let owners = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..2 {
                scope.spawn(|| {
                    barrier.wait();
                    let reservation = operations.reserve(vec![project], None);
                    if reservation.is_ok() {
                        owners.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    barrier.wait();
                    drop(reservation);
                });
            }
        });
        assert_eq!(owners.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(operations.reserve(vec![project], None).is_ok());
    }
}
