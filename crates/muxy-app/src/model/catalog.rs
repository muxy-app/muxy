use super::{AppModel, Work};
use gpui::Context;
use muxy_app_core::{AppError, ServerId};
use muxy_client::ClientError;
use muxy_protocol::{CatalogPage, ErrorCode, OperationId, ProjectId, ProjectMutation};

impl AppModel {
    fn created_project(&self, server: ServerId, operation: OperationId) -> Option<ProjectId> {
        self.state
            .project_intents(server)
            .iter()
            .find_map(|intent| match &intent.mutation {
                ProjectMutation::Create(record) if intent.operation == operation => Some(record.id),
                _ => None,
            })
    }

    pub(super) fn refresh_catalog(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if self.ready(server)
            && self
                .servers
                .get(server)
                .is_some_and(|runtime| !runtime.catalog.pending)
        {
            let pending = self.send(server, Work::ReadCatalog, cx);
            if let Some(runtime) = self.servers.get_mut(server) {
                runtime.catalog.pending = pending;
            }
        }
    }

    /// Sends each connected server its first project edit still waiting.
    pub(super) fn replay_projects(&mut self, cx: &mut Context<Self>) {
        for server in self.servers.ids() {
            self.replay_server_projects(server, cx);
        }
    }

    fn replay_server_projects(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if !self.ready(server)
            || self
                .servers
                .get(server)
                .is_none_or(|runtime| runtime.catalog.replaying)
        {
            return;
        }
        if let Some(intent) = self.state.project_intents(server).first().cloned() {
            let replaying = self.send(server, Work::MutateProject(intent), cx);
            if let Some(runtime) = self.servers.get_mut(server) {
                runtime.catalog.replaying = replaying;
            }
        }
    }

    pub(super) fn receive_project_mutation(
        &mut self,
        server: ServerId,
        operation: OperationId,
        result: Result<u64, ClientError>,
        cx: &mut Context<Self>,
    ) {
        let accepted = result.is_ok();
        let created = self.created_project(server, operation);
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        runtime.catalog.replaying = false;
        match result {
            Ok(revision) => runtime.catalog.dirty = runtime.catalog.dirty.max(revision),
            Err(ClientError::Server(error)) if error.code != ErrorCode::PersistenceFailed => {
                self.fail(self.server_message(server, &error.message), cx);
            }
            Err(error) => {
                self.fail(format!("Project edit is pending: {error}"), cx);
                return;
            }
        }
        let previous = self.state.clone();
        if let Err(error) = self.state.complete_project_intent(server, operation) {
            self.fail(error.to_string(), cx);
            return;
        }
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        if accepted {
            self.sync_git(cx);
            if let Some(project) = created {
                self.sync_worktrees(project, cx);
            }
        }
        if self.state.project_intents(server).is_empty() {
            self.refresh_catalog(server, cx);
        } else {
            self.replay_server_projects(server, cx);
        }
    }

    pub(super) fn receive_catalog(
        &mut self,
        server: ServerId,
        result: Result<CatalogPage, ClientError>,
        cx: &mut Context<Self>,
    ) {
        let local = ServerId::local();
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        runtime.catalog.pending = false;
        let page = match result {
            Ok(page) => page,
            Err(error) => {
                self.server_problem(server, format!("Could not refresh projects: {error}"), cx);
                return;
            }
        };
        if page.revision < self.state.catalog_revision(server) {
            return;
        }
        // An entry that reaches this computer must not claim its server before
        // the local catalog does; it is read again once that is known.
        if !server.is_local() && self.state.server_identity(local).is_none() {
            return;
        }
        let first = self.state.server_identity(server).is_none();
        let previous = self.state.clone();
        if let Err(error) = self.state.apply_catalog(server, &page) {
            self.server_failed(server, &error, cx);
            return;
        }
        if !self.save(cx) {
            self.state = previous;
            return;
        }
        if server.is_local() && first {
            for remote in self.servers.ids() {
                if self.state.server_identity(remote).is_none() {
                    self.refresh_catalog(remote, cx);
                }
            }
        }
        if !self.state.project_intents(server).is_empty() {
            self.replay_server_projects(server, cx);
        } else if let Some(sessions) = self
            .servers
            .get_mut(server)
            .and_then(|runtime| runtime.catalog.restore.take())
        {
            self.apply_restore(server, &sessions, cx);
            if server.is_local() {
                self.resume_update_attaches(cx);
            }
            self.sync_all_worktrees(cx);
        }
        if let Some((_, project, context)) = self
            .git
            .select_after_catalog
            .take_if(|(owner, _, _)| *owner == server)
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
        if self
            .servers
            .get(server)
            .is_some_and(|runtime| runtime.catalog.dirty > page.revision)
        {
            self.refresh_catalog(server, cx);
        }
        self.resume_activity_navigation(server, cx);
        self.resume_remote_picker(server, cx);
        cx.notify();
    }

    /// A catalog this app can't take keeps the state as it was. For another
    /// computer, the reason shows in the sidebar's Remote section instead.
    fn server_failed(&mut self, server: ServerId, error: &AppError, cx: &mut Context<Self>) {
        self.server_problem(server, error.to_string(), cx);
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
        server: ServerId,
        operation: OperationId,
        result: Result<(), ClientError>,
        cx: &mut Context<Self>,
    ) {
        if let Some(runtime) = self.servers.get_mut(server) {
            runtime.catalog.cancelling.remove(&operation);
        }
        match result {
            Ok(()) => {
                let previous = self.state.clone();
                self.state.complete_cancellation(server, operation);
                if !self.save(cx) {
                    self.state = previous;
                }
            }
            Err(error) => self.fail(format!("Closed pane cleanup is pending: {error}"), cx),
        }
    }
    pub(super) fn receive_discarded(
        &mut self,
        server: ServerId,
        session: muxy_protocol::SessionId,
        result: Result<(), ClientError>,
        cx: &mut Context<Self>,
    ) {
        if let Some(runtime) = self.servers.get_mut(server) {
            runtime.discarding.remove(&session);
        }
        match result {
            Ok(()) => {
                let previous = self.state.clone();
                self.state.complete_discard(server, session);
                if !self.save(cx) {
                    self.state = previous;
                }
            }
            Err(ClientError::Disconnected) => self.disconnect(server, cx),
            Err(error) => self.fail(
                format!("Tab closed; server cleanup is pending: {error}"),
                cx,
            ),
        }
    }
}

impl AppModel {
    /// Whether the current project's server can answer.
    pub(crate) fn session_listing_ready(&self) -> bool {
        self.ready(self.state.current_project().server_id)
    }

    /// The latest session list revision of the project's server, while it is
    /// connected.
    pub(crate) fn sessions_revision(&self, project: ProjectId) -> Option<u64> {
        let server = self.state.project_server(project)?;
        self.servers
            .get(server)
            .filter(|_| self.ready(server))
            .map(|runtime| runtime.sessions_revision)
    }

    pub(crate) fn send_session_request(
        &mut self,
        project: ProjectId,
        work: Work,
        cx: &mut Context<Self>,
    ) -> bool {
        let server = self
            .state
            .project_server(project)
            .unwrap_or_else(ServerId::local);
        self.send(server, work, cx)
    }

    pub(crate) fn open_existing_session(
        &mut self,
        project: ProjectId,
        session: &muxy_protocol::ProjectSession,
        cx: &mut Context<Self>,
    ) {
        let shown = self.state.project_server(project).is_some_and(|server| {
            self.state
                .session_references(server)
                .contains(&session.info.id)
        });
        if shown || session.attached {
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
            let pane = self
                .state
                .window()
                .active_pane
                .ok_or_else(|| AppError::InvalidState("new terminal has no pane".into()))?;
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
