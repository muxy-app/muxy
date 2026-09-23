use super::{AppModel, ConnectionState, Work};
use gpui::Context;
use muxy_client::ClientError;
use muxy_protocol::{CatalogPage, ErrorCode, OperationId, ProjectId, ProjectMutation};

impl AppModel {
    pub(super) fn project_creation_pending(&self, project: ProjectId) -> bool {
        self.state.project_intents().iter().any(|intent| {
            matches!(&intent.mutation, ProjectMutation::Create(record) if record.id == project)
        })
    }

    pub(super) fn refresh_catalog(&mut self, cx: &mut Context<Self>) {
        if self.connection == ConnectionState::Ready && !self.catalog.pending {
            self.catalog.pending = self.send(Work::ReadCatalog, cx);
        }
    }

    pub(super) fn replay_projects(&mut self, cx: &mut Context<Self>) {
        if self.connection != ConnectionState::Ready || self.catalog.replaying {
            return;
        }
        if let Some(intent) = self.state.project_intents().first().cloned() {
            self.catalog.replaying = self.send(Work::MutateProject(intent), cx);
        }
    }

    pub(super) fn receive_project_mutation(
        &mut self,
        operation: OperationId,
        result: Result<u64, ClientError>,
        cx: &mut Context<Self>,
    ) {
        let accepted = result.is_ok();
        self.catalog.replaying = false;
        match result {
            Ok(revision) => self.catalog.dirty = self.catalog.dirty.max(revision),
            Err(ClientError::Server(error)) if error.code != ErrorCode::PersistenceFailed => {
                self.fail(error.message, cx);
            }
            Err(error) => {
                self.fail(format!("Project edit is pending: {error}"), cx);
                return;
            }
        }
        let previous = self.state.clone();
        if let Err(error) = self.state.complete_project_intent(operation) {
            self.fail(error.to_string(), cx);
            return;
        }
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        if accepted {
            self.sync_git(cx);
        }
        if self.state.project_intents().is_empty() {
            self.refresh_catalog(cx);
        } else {
            self.replay_projects(cx);
        }
    }

    pub(super) fn receive_catalog(
        &mut self,
        result: Result<CatalogPage, ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.catalog.pending = false;
        let page = match result {
            Ok(page) => page,
            Err(error) => {
                self.fail(format!("Could not refresh projects: {error}"), cx);
                return;
            }
        };
        if page.revision < self.state.catalog_revision() {
            return;
        }
        let previous = self.state.clone();
        if let Err(error) = self.state.apply_catalog(&page) {
            self.fail(error.to_string(), cx);
            return;
        }
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        if !self.state.project_intents().is_empty() {
            self.replay_projects(cx);
        } else if let Some(sessions) = self.catalog.restore.take() {
            self.apply_restore(&sessions, cx);
            self.resume_update_attaches(cx);
        }
        if let Some((project, context)) = self.git.select_after_catalog.take()
            && context == self.git.interaction
            && self.state.project(project).is_some()
        {
            if let Some(parent) = self
                .state
                .project(project)
                .and_then(|project| project.parent_id)
            {
                self.expanded_worktrees.insert(parent);
            }
            self.select_project(project, cx);
        }
        self.sync_visible(cx);
        if self.catalog.dirty > page.revision {
            self.refresh_catalog(cx);
        }
        self.resume_activity_navigation(cx);
        cx.notify();
    }
}

#[derive(Default)]
pub(super) struct Synchronization {
    pub(super) pending: bool,
    pub(super) dirty: u64,
    pub(super) restore: Option<Vec<muxy_protocol::SessionInfo>>,
    pub(super) replaying: bool,
    pub(super) cancelling: std::collections::HashSet<OperationId>,
}

impl AppModel {
    pub(super) fn receive_creation_cancelled(
        &mut self,
        operation: OperationId,
        result: Result<(), ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.catalog.cancelling.remove(&operation);
        match result {
            Ok(()) => {
                let previous = self.state.clone();
                self.state.complete_cancellation(operation);
                if !self.save(cx) {
                    self.state = previous;
                }
            }
            Err(error) => self.fail(format!("Closed pane cleanup is pending: {error}"), cx),
        }
    }
    pub(super) fn receive_discarded(
        &mut self,
        session: muxy_protocol::SessionId,
        result: Result<(), ClientError>,
        cx: &mut Context<Self>,
    ) {
        self.discarding.remove(&session);
        match result {
            Ok(()) => {
                let previous = self.state.clone();
                self.state.complete_discard(session);
                if !self.save(cx) {
                    self.state = previous;
                }
            }
            Err(ClientError::Disconnected) => self.disconnect(cx),
            Err(error) => self.fail(
                format!("Tab closed; server cleanup is pending: {error}"),
                cx,
            ),
        }
    }
}

impl AppModel {
    pub(crate) fn session_listing_ready(&self) -> bool {
        self.connection == ConnectionState::Ready
    }

    pub(crate) fn send_session_request(&mut self, work: Work, cx: &mut Context<Self>) -> bool {
        self.send(work, cx)
    }

    pub(crate) fn open_existing_session(
        &mut self,
        project: ProjectId,
        session: &muxy_protocol::ProjectSession,
        cx: &mut Context<Self>,
    ) {
        if self.state.session_references().contains(&session.info.id) || session.attached {
            return;
        }
        if session.info.project != project {
            self.fail("Session belongs to a different project".into(), cx);
            return;
        }
        if !matches!(
            session.status,
            muxy_protocol::SessionStatus::Live | muxy_protocol::SessionStatus::Starting
        ) {
            return;
        }
        let previous = self.state.clone();
        let result = self.state.open_terminal_tab(project).and_then(|_| {
            let pane = self.state.window().active_pane.ok_or_else(|| {
                muxy_app_core::AppError::InvalidState("new terminal has no pane".into())
            })?;
            self.state.set_pane_session(pane, Some(session.info.id))?;
            Ok(pane)
        });
        if let Err(error) = result {
            self.state = previous;
            self.fail(error.to_string(), cx);
            return;
        }
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        self.dismiss_overlay(cx);
        self.sync_visible(cx);
    }
}
