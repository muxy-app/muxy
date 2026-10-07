use super::{Catalog, Creation, Membership, ServerError, bad, check_revision, unknown};
use muxy_protocol::{
    CATALOG_PAGE_SIZE, OperationId, ProjectId, ProjectSession, ProjectSessions, SessionId,
    SessionInfo, SessionStatus,
};

impl Catalog {
    pub(crate) fn cancel_creation(
        &self,
        operation: OperationId,
    ) -> Result<Option<SessionId>, ServerError> {
        self.update(|state| {
            state.cancelled.insert(operation);
            Ok(state
                .creations
                .get(&operation)
                .map(|creation| creation.info.id))
        })
    }

    pub(crate) fn creation(
        &self,
        operation: OperationId,
        project: ProjectId,
        directory: &muxy_protocol::ServerPath,
        sandbox: Option<&muxy_protocol::SandboxSpec>,
    ) -> Result<Option<SessionInfo>, ServerError> {
        let (cancelled, creation) = {
            let state = self.lock();
            (
                state.cancelled.contains(&operation),
                state.creations.get(&operation).cloned(),
            )
        };
        if cancelled || creation.is_some() {
            self.ensure_durable()?;
        }
        if cancelled {
            return Err(bad("session creation was cancelled"));
        }
        let Some(creation) = creation else {
            return Ok(None);
        };
        if creation.info.project != project
            || creation.info.directory != *directory
            || creation.info.sandbox.as_ref().map(|info| &info.spec) != sandbox
        {
            return Err(bad(
                "session operation token reused with different arguments",
            ));
        }
        if let Some(error) = &creation.error {
            return Err(ServerError::new(error.code, &error.message));
        }
        Ok(Some(creation.info.clone()))
    }

    pub(crate) fn reserve(
        &self,
        operation: OperationId,
        info: &SessionInfo,
    ) -> Result<(), ServerError> {
        self.update(|state| {
            if !state.projects.contains_key(&info.project) || state.deleting.contains(&info.project)
            {
                return Err(unknown());
            }
            if state.sessions.contains_key(&info.id)
                || state
                    .creations
                    .values()
                    .any(|creation| creation.info.id == info.id)
            {
                return Err(bad("session identity already reserved"));
            }
            state.sessions.insert(
                info.id,
                Membership {
                    info: info.clone(),
                    status: SessionStatus::Starting,
                    ended_at: None,
                },
            );
            state.creations.insert(
                operation,
                Creation {
                    info: info.clone(),
                    error: None,
                },
            );
            Ok(())
        })
    }

    pub(crate) fn set_status(
        &self,
        session: SessionId,
        status: SessionStatus,
    ) -> Result<(), ServerError> {
        self.update(|state| {
            if let Some(membership) = state.sessions.get_mut(&session) {
                // A fast process may exit before the spawning thread publishes its handle.
                if status != SessionStatus::Live || membership.status == SessionStatus::Starting {
                    membership.status = status;
                }
            }
            Ok(())
        })
    }

    pub(crate) fn fail_creation(
        &self,
        operation: OperationId,
        error: &ServerError,
    ) -> Result<(), ServerError> {
        self.update(|state| {
            if let Some(creation) = state.creations.get_mut(&operation) {
                creation.error = Some(error.to_reply());
                if let Some(session) = state.sessions.get_mut(&creation.info.id) {
                    session.status = SessionStatus::Unavailable;
                    session.ended_at.get_or_insert(super::retention::now());
                }
            }
            Ok(())
        })
    }

    pub(crate) fn close_receipt(
        &self,
        operation: OperationId,
        session: SessionId,
    ) -> Result<Option<bool>, ServerError> {
        let receipt = self.lock().closes.get(&operation).copied();
        if let Some((recorded, _)) = receipt {
            if recorded != session {
                return Err(bad("close operation reused for another session"));
            }
            self.ensure_durable()?;
        }
        Ok(receipt.map(|(_, discard)| discard))
    }

    pub(crate) fn record_close(
        &self,
        operation: OperationId,
        session: SessionId,
        discard: bool,
    ) -> Result<(), ServerError> {
        self.update(|state| {
            state.closes.insert(operation, (session, discard));
            if discard {
                state.discarding.insert(session);
            }
            Ok(())
        })
    }

    pub(crate) fn begin_discard(&self, session: SessionId) -> Result<(), ServerError> {
        self.update(|state| {
            state.discarding.insert(session);
            Ok(())
        })
    }

    pub(crate) fn finish_discard(&self, session: SessionId) -> Result<(), ServerError> {
        self.update(|state| {
            state.sessions.remove(&session);
            state.discarding.remove(&session);
            Ok(())
        })
    }

    /// Every session the catalog keeps, live or saved, except those being
    /// discarded.
    pub(crate) fn sessions(&self) -> std::collections::BTreeSet<SessionId> {
        let state = self.lock();
        state
            .sessions
            .keys()
            .filter(|id| !state.discarding.contains(id))
            .copied()
            .collect()
    }

    pub(crate) fn discarding(&self) -> Vec<SessionId> {
        let state = self.lock();
        state
            .discarding
            .iter()
            .copied()
            .chain(
                state
                    .cancelled
                    .iter()
                    .filter_map(|operation| state.creations.get(operation))
                    .map(|creation| creation.info.id)
                    .filter(|id| state.sessions.contains_key(id)),
            )
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub(crate) fn list_project(
        &self,
        project: ProjectId,
        after: Option<SessionId>,
        revision: Option<u64>,
    ) -> Result<ProjectSessions, ServerError> {
        self.retry_exits();
        let state = self.lock();
        check_revision(&state, revision)?;
        if !state.projects.contains_key(&project) || state.deleting.contains(&project) {
            return Err(unknown());
        }
        let mut entries = state.sessions.values().filter(|entry| {
            entry.info.project == project
                && !state.discarding.contains(&entry.info.id)
                && after.is_none_or(|after| entry.info.id > after)
        });
        let sessions: Vec<_> = entries
            .by_ref()
            .take(CATALOG_PAGE_SIZE)
            .map(|entry| ProjectSession {
                info: entry.info.clone(),
                status: entry.status,
                owner: None,
                attached: false,
            })
            .collect();
        let next = entries
            .next()
            .and_then(|_| sessions.last().map(|session| session.info.id));
        Ok(ProjectSessions {
            revision: state.revision,
            sessions,
            next,
        })
    }
}

#[cfg(test)]
mod sandbox_tests {
    use super::*;

    #[test]
    fn a_creation_token_cannot_change_its_sandbox() -> Result<(), Box<dyn std::error::Error>> {
        let catalog = Catalog::memory();
        let operation = OperationId::new();
        let spec = muxy_protocol::SandboxSpec {
            workspace: muxy_protocol::ServerPath(b"/workspace".to_vec()),
            policy: muxy_protocol::SandboxPolicy::default(),
        };
        let info = SessionInfo {
            id: SessionId::new(10).ok_or("session ID")?,
            project: catalog.home(),
            directory: spec.workspace.clone(),
            sandbox: Some(muxy_protocol::SandboxInfo {
                spec: spec.clone(),
                backend_version: "0.79.0".into(),
            }),
        };
        catalog.reserve(operation, &info)?;
        assert_eq!(
            catalog.creation(operation, info.project, &info.directory, Some(&spec))?,
            Some(info.clone())
        );
        assert!(
            catalog
                .creation(operation, info.project, &info.directory, None)
                .is_err()
        );
        let mut changed = spec;
        changed.policy.environment.push("EXAMPLE_API_KEY".into());
        assert!(
            catalog
                .creation(operation, info.project, &info.directory, Some(&changed))
                .is_err()
        );
        Ok(())
    }
}
