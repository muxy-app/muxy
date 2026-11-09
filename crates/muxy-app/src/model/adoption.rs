//! Terminals other clients start on this computer's server, such as a phone
//! or the terminal UI, open here as background tabs in their project.

use std::collections::{BTreeSet, HashSet};

use gpui::Context;
use muxy_app_core::{AppError, ProjectId, ProjectStatus, ServerId};
use muxy_protocol::{ProjectSession, SessionId, SessionStatus};

use super::{AppModel, Quitting};

impl AppModel {
    /// This computer's listed projects whose terminals other clients show,
    /// once its server has shown, on this connection, that its projects are
    /// the ones the app knows.
    pub(crate) fn adoptable_projects(&self) -> Vec<ProjectId> {
        let server = ServerId::local();
        if !self.adopting(server) {
            return Vec::new();
        }
        self.state
            .projects()
            .iter()
            .filter(|project| {
                project.server_id == server
                    && self.project_listed(project)
                    && project.status() == ProjectStatus::Available
            })
            .map(|project| project.id)
            .collect()
    }

    fn adopting(&self, server: ServerId) -> bool {
        self.quitting == Quitting::Idle
            && self.ready(server)
            && self.confirmed(server)
            && self.identified(server)
            && self.servers.get(server).is_some_and(|runtime| {
                runtime.catalog.restore.is_none() && !runtime.catalog.pending
            })
    }

    /// Opens, as background tabs, the terminals of `project` that another
    /// client shows and this app neither shows nor stopped showing. Terminals
    /// nobody shows stay in Existing Terminals.
    pub(crate) fn adopt_sessions(
        &mut self,
        project: ProjectId,
        sessions: &[ProjectSession],
        cx: &mut Context<Self>,
    ) {
        if !self.adoptable_projects().contains(&project) {
            return;
        }
        let server = ServerId::local();
        let shown: HashSet<_> = self.state.session_references(server).into_iter().collect();
        let dismissed = self.state.dismissed_sessions(server);
        let others: Vec<_> = sessions
            .iter()
            .filter(|session| {
                session.owner.is_some()
                    && !session.attached
                    && session.info.project == project
                    && matches!(
                        session.status,
                        SessionStatus::Live | SessionStatus::Starting
                    )
                    && !shown.contains(&session.info.id)
                    && !dismissed.contains(&session.info.id)
            })
            .map(|session| session.info.id)
            .collect();
        if others.is_empty() {
            return;
        }
        let opened = self.transact(
            |state| {
                for session in &others {
                    let pane = state.open_background_terminal_tab(project)?;
                    state.set_pane_session(pane, Some(*session))?;
                }
                Ok::<_, AppError>(())
            },
            cx,
        );
        match opened {
            Ok(Some(())) => {
                self.sync_visible(cx);
                cx.notify();
            }
            Ok(None) => {}
            Err(error) => self.fail(error.to_string(), cx),
        }
    }

    /// Forgets the dismissed sessions that no longer run on `server`, from
    /// the list its connection reported, once its catalog showed it is the
    /// server the app knew.
    pub(super) fn prune_dismissed(&mut self, server: ServerId, cx: &mut Context<Self>) {
        let Some(live) = self
            .servers
            .get(server)
            .and_then(|runtime| runtime.catalog.restore.as_ref())
        else {
            return;
        };
        let live: HashSet<_> = live.iter().map(|session| session.id).collect();
        if self.state.retain_dismissed(server, &live) {
            self.save(cx);
        }
    }

    /// Remembers the sessions on this computer's server that this app
    /// stopped showing since it last looked, so they aren't opened here
    /// again. The first look only notes what is shown.
    pub(super) fn note_shown_sessions(
        &mut self,
        server: ServerId,
        shown: &[SessionId],
        cx: &mut Context<Self>,
    ) {
        if !server.is_local() {
            return;
        }
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        let current: BTreeSet<_> = shown.iter().copied().collect();
        let Some(previous) = runtime.shown.replace(current.clone()) else {
            return;
        };
        if self
            .state
            .dismiss_sessions(server, previous.difference(&current).copied())
        {
            self.save(cx);
        }
    }

    /// A session on this computer's server left running when its pane went
    /// away before it attached.
    pub(super) fn dismiss_session(
        &mut self,
        server: ServerId,
        session: SessionId,
        cx: &mut Context<Self>,
    ) {
        if server.is_local() && self.state.dismiss_sessions(server, [session]) {
            self.save(cx);
        }
    }
}
