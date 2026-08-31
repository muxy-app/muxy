use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectOperationKind {
    Refresh,
    Create,
    Remove,
    RepositoryMutation,
}

const MUTATING_OPERATION_KINDS: [ProjectOperationKind; 3] = [
    ProjectOperationKind::Create,
    ProjectOperationKind::Remove,
    ProjectOperationKind::RepositoryMutation,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeToken {
    project_id: String,
    generation: u64,
    request_id: u64,
}

impl ProbeToken {
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn request_id(&self) -> u64 {
        self.request_id
    }

    pub fn matches(&self, project_id: &str, generation: u64, request_id: u64) -> bool {
        self.project_id == project_id
            && self.generation == generation
            && self.request_id == request_id
    }

    #[cfg(test)]
    fn for_test(project_id: String, generation: u64, request_id: u64) -> Self {
        Self {
            project_id,
            generation,
            request_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectOperationToken {
    project_id: String,
    generation: u64,
    request_id: u64,
    kind: ProjectOperationKind,
}

impl ProjectOperationToken {
    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn request_id(&self) -> u64 {
        self.request_id
    }

    pub fn matches(&self, project_id: &str, generation: u64, request_id: u64) -> bool {
        self.project_id == project_id
            && self.generation == generation
            && self.request_id == request_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeginOperationError {
    Busy(ProjectOperationKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitError {
    Stale,
    ProjectRemoved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommitOutcome {
    pub schedule_fresh_probe: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinishOutcome {
    pub schedule_fresh_probe: bool,
}

#[derive(Default)]
struct ProjectState {
    generation: u64,
    probe_request_id: Option<u64>,
    probe_queued: bool,
    operation: Option<(u64, ProjectOperationKind)>,
}

#[derive(Default)]
pub struct ProjectOperations {
    projects: HashMap<String, ProjectState>,
    next_request_id: u64,
}

impl ProjectOperations {
    pub fn begin_background_probe(&mut self, project_id: &str) -> Option<ProbeToken> {
        let state = self.projects.entry(project_id.to_owned()).or_default();
        if state.operation.is_some() {
            state.probe_queued = true;
            return None;
        }
        if state.probe_request_id.is_some() {
            state.probe_queued = true;
            return None;
        }
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        state.probe_request_id = Some(self.next_request_id);
        Some(ProbeToken {
            project_id: project_id.to_owned(),
            generation: state.generation,
            request_id: self.next_request_id,
        })
    }

    pub fn commit_background_probe(
        &mut self,
        token: &ProbeToken,
        project_exists: bool,
    ) -> Result<CommitOutcome, CommitError> {
        let Some(state) = self.projects.get_mut(&token.project_id) else {
            return Err(CommitError::Stale);
        };
        if state.generation != token.generation
            || state.operation.is_some()
            || state.probe_request_id != Some(token.request_id)
        {
            return Err(CommitError::Stale);
        }
        state.probe_request_id = None;
        let schedule_fresh_probe = std::mem::take(&mut state.probe_queued);
        if !project_exists {
            return Err(CommitError::ProjectRemoved);
        }
        Ok(CommitOutcome {
            schedule_fresh_probe,
        })
    }

    pub fn begin_operation(
        &mut self,
        project_id: &str,
        kind: ProjectOperationKind,
    ) -> Result<ProjectOperationToken, BeginOperationError> {
        let state = self.projects.entry(project_id.to_owned()).or_default();
        if let Some((_, active)) = state.operation {
            return Err(BeginOperationError::Busy(active));
        }
        state.generation = state.generation.wrapping_add(1);
        state.probe_request_id = None;
        state.probe_queued = false;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        state.operation = Some((self.next_request_id, kind));
        Ok(ProjectOperationToken {
            project_id: project_id.to_owned(),
            generation: state.generation,
            request_id: self.next_request_id,
            kind,
        })
    }

    pub fn commit_explicit_refresh(
        &self,
        token: &ProjectOperationToken,
        project_exists: bool,
    ) -> Result<CommitOutcome, CommitError> {
        let Some(state) = self.projects.get(&token.project_id) else {
            return Err(CommitError::Stale);
        };
        if token.kind != ProjectOperationKind::Refresh
            || state.generation != token.generation
            || state.operation != Some((token.request_id, token.kind))
        {
            return Err(CommitError::Stale);
        }
        if !project_exists {
            return Err(CommitError::ProjectRemoved);
        }
        Ok(CommitOutcome {
            schedule_fresh_probe: false,
        })
    }

    pub fn finish_operation(
        &mut self,
        token: &ProjectOperationToken,
    ) -> Result<FinishOutcome, CommitError> {
        let Some(state) = self.projects.get_mut(&token.project_id) else {
            return Err(CommitError::Stale);
        };
        if state.generation != token.generation
            || state.operation != Some((token.request_id, token.kind))
        {
            return Err(CommitError::Stale);
        }
        state.operation = None;
        state.generation = state.generation.wrapping_add(1);
        let probe_queued = std::mem::take(&mut state.probe_queued);
        Ok(FinishOutcome {
            schedule_fresh_probe: probe_queued
                || matches!(
                    token.kind,
                    ProjectOperationKind::Create | ProjectOperationKind::Remove
                ),
        })
    }

    pub fn matches_operation(&self, token: &ProjectOperationToken) -> bool {
        self.projects.get(&token.project_id).is_some_and(|state| {
            state.generation == token.generation
                && state.operation == Some((token.request_id, token.kind))
        })
    }

    pub fn is_mutating(&self, project_id: &str) -> bool {
        self.projects
            .get(project_id)
            .and_then(|state| state.operation.map(|(_, kind)| kind))
            .is_some_and(|kind| MUTATING_OPERATION_KINDS.contains(&kind))
    }

    pub fn is_busy(&self, project_id: &str) -> bool {
        self.projects
            .get(project_id)
            .is_some_and(|state| state.operation.is_some())
    }

    pub fn project_removed(&mut self, project_id: &str) {
        let state = self.projects.entry(project_id.to_owned()).or_default();
        state.generation = state.generation.wrapping_add(1);
        state.probe_request_id = None;
        state.probe_queued = false;
        state.operation = None;
    }

    pub fn generation(&self, project_id: &str) -> u64 {
        self.projects
            .get(project_id)
            .map_or(0, |state| state.generation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_operation_coordinator_rejects_stale_and_duplicate_requests() {
        let mut operations = ProjectOperations::default();
        assert_eq!(operations.generation("project"), 0);
        let old = operations.begin_background_probe("project").unwrap();
        let refresh = operations
            .begin_operation("project", ProjectOperationKind::Refresh)
            .unwrap();
        assert_eq!(operations.generation("project"), 1);

        assert!(operations.commit_background_probe(&old, true).is_err());
        assert!(matches!(
            operations.begin_operation("project", ProjectOperationKind::Refresh),
            Err(BeginOperationError::Busy(ProjectOperationKind::Refresh))
        ));
        assert!(operations.begin_background_probe("project").is_none());
        assert!(operations.commit_explicit_refresh(&refresh, true).is_ok());

        let finished = operations.finish_operation(&refresh).unwrap();
        assert!(finished.schedule_fresh_probe);
        assert_eq!(operations.generation("project"), 2);
    }

    #[test]
    fn explicit_refresh_schedules_follow_up_only_when_a_probe_was_coalesced() {
        let mut operations = ProjectOperations::default();
        let quiet = operations
            .begin_operation("quiet", ProjectOperationKind::Refresh)
            .unwrap();
        assert!(
            !operations
                .finish_operation(&quiet)
                .unwrap()
                .schedule_fresh_probe
        );

        let changed = operations
            .begin_operation("changed", ProjectOperationKind::Refresh)
            .unwrap();
        assert!(operations.begin_background_probe("changed").is_none());
        assert!(
            operations
                .finish_operation(&changed)
                .unwrap()
                .schedule_fresh_probe
        );
    }

    #[test]
    fn project_operation_coordinator_serializes_mutations_and_refreshes_after_finish() {
        let mut operations = ProjectOperations::default();
        let refresh = operations
            .begin_operation("refreshing", ProjectOperationKind::Refresh)
            .unwrap();
        assert!(operations.is_busy("refreshing"));
        operations.finish_operation(&refresh).unwrap();
        assert!(!operations.is_busy("refreshing"));
        for kind in [ProjectOperationKind::Create, ProjectOperationKind::Remove] {
            let token = operations.begin_operation("project", kind).unwrap();
            assert!(operations.is_mutating("project"));
            assert!(matches!(
                operations.begin_operation("project", ProjectOperationKind::Refresh),
                Err(BeginOperationError::Busy(active)) if active == kind
            ));
            let finished = operations.finish_operation(&token).unwrap();
            assert!(finished.schedule_fresh_probe);
            assert!(!operations.is_mutating("project"));
        }

        let repository = operations
            .begin_operation("repository", ProjectOperationKind::RepositoryMutation)
            .unwrap();
        assert!(operations.is_mutating("repository"));
        for kind in [
            ProjectOperationKind::Create,
            ProjectOperationKind::Remove,
            ProjectOperationKind::RepositoryMutation,
        ] {
            assert!(matches!(
                operations.begin_operation("repository", kind),
                Err(BeginOperationError::Busy(
                    ProjectOperationKind::RepositoryMutation
                ))
            ));
        }
        assert!(operations.finish_operation(&repository).is_ok());
        assert!(operations.finish_operation(&repository).is_err());
    }

    #[test]
    fn project_operation_coordinator_checks_request_identity_and_project_liveness() {
        let mut operations = ProjectOperations::default();
        let first = operations.begin_background_probe("project").unwrap();
        assert!(operations.commit_background_probe(&first, false).is_err());

        let second = operations.begin_background_probe("project").unwrap();
        let forged = ProbeToken::for_test(
            second.project_id().to_owned(),
            second.generation(),
            second.request_id() + 1,
        );
        assert!(operations.commit_background_probe(&forged, true).is_err());
        assert!(operations.commit_background_probe(&second, true).is_ok());
    }

    #[test]
    fn duplicate_background_probe_coalesces_into_one_fresh_follow_up() {
        let mut operations = ProjectOperations::default();
        let current = operations.begin_background_probe("project").unwrap();

        assert!(operations.begin_background_probe("project").is_none());
        let committed = operations.commit_background_probe(&current, true).unwrap();

        assert!(committed.schedule_fresh_probe);
        assert!(operations.begin_background_probe("project").is_some());
    }

    #[test]
    fn session_lifecycle_owner_deletion_token_survives_confirmation_and_rejects_replacement() {
        let mut operations = ProjectOperations::default();
        let cancelled = operations
            .begin_operation("project", ProjectOperationKind::Remove)
            .unwrap();
        assert!(operations.matches_operation(&cancelled));
        assert!(operations.begin_background_probe("project").is_none());
        assert!(operations.finish_operation(&cancelled).is_ok());
        assert!(!operations.matches_operation(&cancelled));

        let active = operations
            .begin_operation("project", ProjectOperationKind::Remove)
            .unwrap();
        assert!(operations.matches_operation(&active));
        operations.project_removed("project");
        assert!(!operations.matches_operation(&active));
        assert!(operations.finish_operation(&active).is_err());
    }

    #[test]
    fn removed_and_reintroduced_project_rejects_its_old_candidate() {
        let mut operations = ProjectOperations::default();
        let old = operations.begin_background_probe("project").unwrap();

        operations.project_removed("project");
        let replacement = operations.begin_background_probe("project").unwrap();

        assert!(operations.commit_background_probe(&old, true).is_err());
        assert!(
            operations
                .commit_background_probe(&replacement, true)
                .is_ok()
        );
    }

    #[test]
    fn stale_probe_after_mutation_cannot_regress_disk_or_applied_state() {
        let temp = tempfile::tempdir().unwrap();
        let mut operations = ProjectOperations::default();
        let stale_token = operations.begin_background_probe("project").unwrap();
        let mutation = operations
            .begin_operation("project", ProjectOperationKind::Create)
            .unwrap();
        let current =
            muxy_api::worktrees::RefreshCandidate::Updated(vec![worktree("CURRENT", "/current")]);
        muxy_api::worktrees::save_candidate(temp.path(), "project", &current).unwrap();
        let mut applied = current.worktrees().unwrap().to_vec();
        let stale =
            muxy_api::worktrees::RefreshCandidate::Updated(vec![worktree("STALE", "/stale")]);

        if operations
            .commit_background_probe(&stale_token, true)
            .is_ok()
        {
            muxy_api::worktrees::save_candidate(temp.path(), "project", &stale).unwrap();
            applied = stale.worktrees().unwrap().to_vec();
        }

        assert_eq!(applied[0].id, "CURRENT");
        let persisted = muxy_core::store::worktrees::load_from(temp.path(), "project");
        assert_eq!(persisted[0].id, "CURRENT");
        assert!(
            operations
                .finish_operation(&mutation)
                .unwrap()
                .schedule_fresh_probe
        );
        assert!(operations.begin_background_probe("project").is_some());
    }

    fn worktree(id: &str, path: &str) -> muxy_core::store::Worktree {
        muxy_core::store::Worktree {
            id: id.to_owned(),
            name: id.to_owned(),
            path: path.to_owned(),
            branch: None,
            source: muxy_core::store::WorktreeSource::Muxy,
            is_primary: true,
            created_at: 1.0,
            last_active_at: None,
        }
    }
}
