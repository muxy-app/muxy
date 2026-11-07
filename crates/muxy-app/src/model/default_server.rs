//! The server the app works with. The sidebar lists its projects, new
//! projects and tabs go to it, and terminals other clients start there open
//! here as tabs.

use std::collections::{BTreeSet, HashSet};

use gpui::Context;
use muxy_app_core::{AppError, ProjectId, ProjectStatus, ServerId};
use muxy_protocol::{ProjectSession, SessionId, SessionStatus};

use super::{AppModel, Quitting};

impl AppModel {
    pub(crate) fn default_server(&self) -> ServerId {
        self.state.default_server()
    }

    /// A default server no longer in settings, removed by hand, gives way to
    /// this computer's. Returns whether it did.
    pub(super) fn sync_default_server(&mut self) -> bool {
        let server = self.state.default_server();
        if server.is_local() || self.settings.server(server).is_some() {
            return false;
        }
        self.state.set_default_server(ServerId::local())
    }

    /// Switches the whole app to `server`: the sidebar lists its projects and
    /// the window shows its Home once its catalog is known, connecting first
    /// when it is offline.
    pub(crate) fn set_default_server(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if !server.is_local() && self.settings.server(server).is_none() {
            return;
        }
        if server == self.state.default_server() {
            self.connect_on_demand(server, cx);
            return;
        }
        match self.transact(
            |state| -> Result<(), std::convert::Infallible> {
                state.set_default_server(server);
                Ok(())
            },
            cx,
        ) {
            Ok(Some(())) => {}
            Ok(None) | Err(_) => return,
        }
        if self
            .pending_remote
            .is_some_and(|pending| pending.server != server)
        {
            self.pending_remote = None;
        }
        if server.is_local() {
            if self.connection(server) == super::ConnectionState::Disconnected {
                self.connect(cx);
            }
        } else {
            self.connect_on_demand(server, cx);
        }
        self.changed(cx);
    }

    /// The server of `project` when it can open a terminal, so failing work
    /// never switches the app.
    pub(crate) fn available_project_server(&self, project: ProjectId) -> Option<ServerId> {
        self.state
            .project(project)
            .filter(|project| project.status() == ProjectStatus::Available)
            .map(|project| project.server_id)
    }

    /// Work aimed at another server than the default one switches the app
    /// to it first, so what it opens is listed.
    pub(crate) fn use_server(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if server != self.state.default_server() {
            self.set_default_server(server, cx);
        }
    }

    /// The default server's listed projects whose terminals other clients
    /// show, once that server has shown, on this connection, that its
    /// projects are the ones the app knows.
    pub(crate) fn adoptable_projects(&self) -> Vec<ProjectId> {
        let server = self.state.default_server();
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
        let server = self.state.default_server();
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

    /// Remembers the sessions on `server` that this app stopped showing
    /// since it last looked, so they aren't opened here again. The first
    /// look only notes what is shown.
    pub(super) fn note_shown_sessions(
        &mut self,
        server: ServerId,
        shown: &[SessionId],
        cx: &mut Context<Self>,
    ) {
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

    /// A session left running when its pane went away before it attached.
    pub(super) fn dismiss_session(
        &mut self,
        server: ServerId,
        session: SessionId,
        cx: &mut Context<Self>,
    ) {
        if self.state.dismiss_sessions(server, [session]) {
            self.save(cx);
        }
    }
}
