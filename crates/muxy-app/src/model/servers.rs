//! The connection the app keeps with each server: this computer's, and each
//! one listed in `settings.toml`. Session and channel IDs are counters per
//! server, so what they key lives with the server too.

use std::collections::{BTreeMap, HashMap, HashSet};

use gpui::Context;
use muxy_app_core::settings::ServerEntry;
use muxy_app_core::{PaneId, Project, ServerId};
use muxy_client::SshTarget;
use muxy_protocol::{ChannelId, SessionId, SessionProgress};

use super::{AppModel, ConnectionState, Quitting, activity, catalog};
use crate::boot::{Target, Work, Worker, Workers};
use crate::views::terminal::pane::PaneState;

pub(crate) struct Runtime {
    pub(super) work: Option<Worker>,
    pub(super) connection: ConnectionState,
    pub(super) generation: u64,
    /// The server's build and instance, as its last connection reported them.
    pub(super) info: Option<muxy_protocol::ServerInfo>,
    /// Why the last connection attempt failed, for the server's own status.
    pub(super) error: Option<String>,
    /// Another computer's server is stopping or restarting at the user's request.
    pub(super) stopping: bool,
    /// What kind of failure `error` is, when ssh or the bridge said.
    pub(super) failure: Option<muxy_client::RemoteReason>,
    /// Installing Muxy there, at the user's request.
    pub(super) install: Option<super::remote_servers::Install>,
    /// How the worker reaches another computer.
    pub(super) target: Option<SshTarget>,
    /// Connecting again after a drop.
    pub(super) retry: super::reconnect::Retry,
    /// Checks of its project folders started, so only the latest applies.
    pub(super) status_checks: u64,
    /// It logs in with a password, which the app asks for before connecting.
    pub(super) password_login: bool,
    pub(super) catalog: catalog::Synchronization,
    pub(super) references: Option<Vec<SessionId>>,
    pub(super) discarding: HashSet<SessionId>,
    pub(super) sessions_revision: u64,
    pub(crate) progress: HashMap<SessionId, SessionProgress>,
    pub(crate) activity: activity::ActivityView,
}

impl Runtime {
    fn new(work: Option<Worker>, connection: ConnectionState, generation: u64) -> Self {
        Self {
            work,
            connection,
            generation,
            info: None,
            error: None,
            stopping: false,
            failure: None,
            install: None,
            target: None,
            retry: super::reconnect::Retry::default(),
            status_checks: 0,
            password_login: false,
            catalog: catalog::Synchronization::default(),
            references: None,
            discarding: HashSet::new(),
            sessions_revision: 0,
            progress: HashMap::new(),
            activity: activity::ActivityView::default(),
        }
    }
}

pub(crate) struct Servers {
    workers: Workers,
    /// Passwords typed this session, which ssh asks the app for.
    pub(super) passwords: crate::askpass::Passwords,
    /// This computer's server, which is always there.
    pub(crate) local: Runtime,
    remotes: BTreeMap<ServerId, Runtime>,
}

impl Servers {
    /// Boot already asked the local worker to connect, as generation 1.
    /// ssh asks for passwords on `askpass`, a socket path.
    pub(super) fn new(workers: Workers, local: Worker, askpass: std::path::PathBuf) -> Self {
        Self {
            workers,
            passwords: crate::askpass::Passwords::new(askpass),
            local: Runtime::new(Some(local), ConnectionState::Connecting, 1),
            remotes: BTreeMap::new(),
        }
    }

    pub(crate) fn get(&self, server: ServerId) -> Option<&Runtime> {
        if server.is_local() {
            Some(&self.local)
        } else {
            self.remotes.get(&server)
        }
    }

    pub(super) fn get_mut(&mut self, server: ServerId) -> Option<&mut Runtime> {
        if server.is_local() {
            Some(&mut self.local)
        } else {
            self.remotes.get_mut(&server)
        }
    }

    /// This computer's server first, then the others.
    pub(super) fn ids(&self) -> Vec<ServerId> {
        std::iter::once(ServerId::local())
            .chain(self.remotes.keys().copied())
            .collect()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (ServerId, &Runtime)> {
        std::iter::once((ServerId::local(), &self.local))
            .chain(self.remotes.iter().map(|(id, runtime)| (*id, runtime)))
    }

    /// Starts the worker for a server reached over SSH. An invalid entry
    /// leaves the server without a worker, and its error says why. A server
    /// that had a worker before keeps counting from its `generation`, so
    /// updates from that worker are never taken for the new one's.
    fn add(&mut self, entry: &ServerEntry, generation: u64) {
        let mut runtime = Runtime::new(None, ConnectionState::Disconnected, generation);
        runtime.password_login = entry.password_login;
        match self.target(entry).and_then(|target| {
            runtime.target = Some(target.clone());
            self.workers.start(entry.id, Target::Ssh(target))
        }) {
            Ok(work) => runtime.work = Some(work),
            Err(error) => runtime.error = Some(format!("{}: {error}", entry.ssh)),
        }
        self.remotes.insert(entry.id, runtime);
    }

    /// How ssh reaches `entry`: its key file, and asking the app for a
    /// password when it logs in with one.
    pub(super) fn target(&mut self, entry: &ServerEntry) -> std::io::Result<SshTarget> {
        let mut target = SshTarget::new(&entry.ssh)?;
        if let Some(identity) = &entry.identity_file {
            target = target.with_identity(identity);
        }
        if entry.password_login {
            let login = target.login()?;
            target = target.with_askpass(self.passwords.askpass(entry.id, login)?);
        }
        Ok(target)
    }

    /// Stops another computer's worker and drops what its connection kept.
    /// Returns the generation it reached.
    pub(super) fn remove(&mut self, server: ServerId) -> Option<u64> {
        let runtime = self.remotes.remove(&server)?;
        if let Some(worker) = &runtime.work {
            let _ = worker.send((runtime.generation, Work::Stop));
        }
        Some(runtime.generation)
    }

    /// Starts a server listed after launch, or one whose way in changed. The
    /// new worker starts a generation past the old one's, so the old one's
    /// updates are dropped even before the new one connects.
    pub(super) fn restart(&mut self, entry: &ServerEntry) {
        let generation = self.remove(entry.id).map_or(0, |generation| generation + 1);
        self.add(entry, generation);
    }

    pub(super) fn probe(&self) -> crate::boot::Probe {
        self.workers.probe()
    }

    pub(super) fn run(&self) -> crate::boot::Run {
        self.workers.run()
    }
}

impl AppModel {
    /// Connects every server listed in settings, each on its own worker, so
    /// SSH never blocks the window.
    /// Servers that log in with a password wait until one of their projects
    /// is opened or they are connected by hand, so launching never asks.
    pub(super) fn start_servers(&mut self, cx: &mut Context<Self>) {
        for entry in self.settings.servers.clone() {
            self.servers.add(&entry, 0);
            if !entry.password_login {
                self.connect_server(entry.id, cx);
            }
        }
    }

    pub(super) fn connection(&self, server: ServerId) -> ConnectionState {
        self.servers
            .get(server)
            .map_or(ConnectionState::Disconnected, |runtime| runtime.connection)
    }

    pub(super) fn ready(&self, server: ServerId) -> bool {
        self.connection(server) == ConnectionState::Ready
    }

    pub(crate) fn server_ready(&self, server: ServerId) -> bool {
        self.ready(server)
    }

    /// Whether another computer's catalog has shown, on this connection, that
    /// it is still the server the app knew. Until then its session numbers
    /// may belong to someone else's sessions, so none are closed or read.
    /// This computer's server is trusted as before.
    pub(crate) fn confirmed(&self, server: ServerId) -> bool {
        server.is_local()
            || self
                .servers
                .get(server)
                .is_some_and(|runtime| runtime.catalog.restore.is_none())
    }

    /// Whether another computer's catalog has shown, on this connection, that
    /// it is the server the app knew, so the project edits waiting for that
    /// server can go to it. A server that answers as another one never gets
    /// them. This computer's server is trusted as before.
    pub(super) fn identified(&self, server: ServerId) -> bool {
        server.is_local()
            || self
                .servers
                .get(server)
                .is_some_and(|runtime| runtime.catalog.identified)
    }

    /// The project's server; this computer's for a project that is gone.
    pub(super) fn project_server_or_local(&self, project: muxy_protocol::ProjectId) -> ServerId {
        self.state
            .project_server(project)
            .unwrap_or_else(ServerId::local)
    }

    pub(super) fn generation(&self, server: ServerId) -> u64 {
        self.servers
            .get(server)
            .map_or(0, |runtime| runtime.generation)
    }

    /// The connection generation of the project's server, which work for the
    /// project keeps so a reconnect expires it.
    pub(super) fn project_generation(&self, project: muxy_protocol::ProjectId) -> u64 {
        self.state
            .project_server(project)
            .map_or(0, |server| self.generation(server))
    }

    /// Projects of servers that are no longer in settings stay in the state,
    /// hidden, until their server returns.
    pub(crate) fn project_shown(&self, project: &Project) -> bool {
        project.server_id.is_local() || self.settings.server(project.server_id).is_some()
    }

    /// The Home folder of another computer's server, from its catalog; `None`
    /// for this computer's, whose Home is `$HOME`.
    pub(crate) fn remote_home(&self, server: ServerId) -> Option<&std::path::Path> {
        if server.is_local() {
            return None;
        }
        self.state
            .server_home(server)
            .map(|home| home.directory.as_path())
    }

    pub(crate) fn project_is_local(&self, project: muxy_protocol::ProjectId) -> bool {
        self.state
            .project_server(project)
            .is_none_or(ServerId::is_local)
    }

    /// The Muxy version the server runs, once it connected.
    pub(crate) fn server_version(&self, server: ServerId) -> Option<&str> {
        self.servers
            .get(server)
            .and_then(|runtime| runtime.info.as_ref())
            .map(|info| info.build.version.as_str())
    }

    /// Why the server's last connection attempt failed.
    pub(crate) fn server_error(&self, server: ServerId) -> Option<&str> {
        self.servers
            .get(server)
            .and_then(|runtime| runtime.error.as_deref())
    }

    /// Projects the sidebar and Switch Project list: shown ones, except
    /// another computer's Home, which only backs browsing its folders.
    pub(crate) fn project_listed(&self, project: &Project) -> bool {
        self.project_shown(project) && (project.server_id.is_local() || !project.home)
    }

    /// The name settings give another computer's server.
    pub(crate) fn server_name(&self, server: ServerId) -> Option<&str> {
        self.settings
            .server(server)
            .map(|entry| entry.name.as_str())
    }

    /// A failure nobody asked about: this computer's server alerts; another
    /// computer's shows beside it in the sidebar's Remote section.
    pub(super) fn server_problem(
        &mut self,
        server: ServerId,
        message: String,
        cx: &mut Context<Self>,
    ) {
        if server.is_local() {
            self.fail(message, cx);
        } else if let Some(runtime) = self.servers.get_mut(server) {
            runtime.error = Some(message);
            cx.notify();
        }
    }

    /// Names another computer's server in a message about it.
    pub(super) fn server_message(&self, server: ServerId, message: &str) -> String {
        match self.server_name(server) {
            Some(name) if !server.is_local() => format!("{name}: {message}"),
            _ => message.to_owned(),
        }
    }

    /// The server that produced the pane's attachment, and its channel there.
    pub(super) fn attachment(&self, pane: PaneId, cx: &gpui::App) -> Option<(ServerId, ChannelId)> {
        self.terminal(&pane)
            .and_then(|view| view.view.read(cx).attachment())
    }

    fn on_server(&self, pane: PaneId, server: ServerId) -> bool {
        self.state.pane_server(pane) == Some(server)
    }

    pub(super) fn pending_on(&self, server: ServerId) -> bool {
        self.pending.values().any(|owner| *owner == server)
    }

    pub(crate) fn connect(&mut self, cx: &mut Context<Self>) {
        self.connect_server(ServerId::local(), cx);
    }

    pub(crate) fn connect_server(&mut self, server: ServerId, cx: &mut Context<Self>) {
        self.connect_to_server(server, false, cx);
    }

    /// Reconnects a remote server when one of its projects or tabs is shown.
    /// This computer's server keeps connecting only when a terminal needs it,
    /// so a server stopped from Settings stays stopped.
    pub(crate) fn connect_on_demand(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if !server.is_local() && self.connection(server) == ConnectionState::Disconnected {
            self.connect_server(server, cx);
        }
    }

    pub(super) fn connect_to_server(
        &mut self,
        server: ServerId,
        after_update: bool,
        cx: &mut Context<Self>,
    ) {
        if self.quitting != Quitting::Idle
            || (server.is_local() && self.server_preferences.control_busy)
        {
            return;
        }
        if self.server_needs_password(server) {
            if self.connection(server) == ConnectionState::Disconnected {
                self.ask_password(server, cx);
            }
            return;
        }
        let instance = self
            .updates
            .server
            .as_ref()
            .map_or(0, |server| server.instance);
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        if runtime.connection != ConnectionState::Disconnected {
            return;
        }
        let Some(generation) = runtime.generation.checked_add(1) else {
            return;
        };
        runtime.generation = generation;
        runtime.connection = ConnectionState::Connecting;
        runtime.stopping = false;
        runtime.retry.connecting();
        runtime.discarding.clear();
        runtime.catalog.cancelling.clear();
        let work = if after_update && server.is_local() {
            Work::ReconnectAfterUpdate(instance)
        } else {
            Work::Connect
        };
        let sent = runtime
            .work
            .as_ref()
            .is_some_and(|worker| worker.send((generation, work)).is_ok());
        self.forget_requests(server);
        self.sync_preferences(cx);
        self.set_server_panes(server, PaneState::Connecting, cx);
        if !sent {
            self.disconnect(server, cx);
            if server.is_local() {
                self.fail("The server connection worker stopped".into(), cx);
            }
        }
    }

    /// Drops what was waiting on `server`'s connection.
    fn forget_requests(&mut self, server: ServerId) {
        self.pending.retain(|_, owner| *owner != server);
        self.detached_pending
            .retain(|pane| self.pending.contains_key(pane));
        let closing = self
            .close_request
            .as_ref()
            .and_then(|request| self.tab_project(request.tab))
            .is_some_and(|project| project.server_id == server);
        if closing {
            self.pending_close = None;
            if self.close_prompt.is_none() {
                self.close_request = None;
            }
        }
    }

    fn set_server_panes(&self, server: ServerId, state: PaneState, cx: &mut Context<Self>) {
        for (id, pane) in &self.grids {
            if self.on_server(*id, server) {
                pane.view.update(cx, |pane, cx| pane.set_state(state, cx));
            }
        }
    }

    /// Marks only `server`'s panes and requests as disconnected.
    pub(super) fn disconnect(&mut self, server: ServerId, cx: &mut Context<Self>) {
        if server.is_local() {
            self.stop_extension_tasks(cx);
        }
        self.disconnect_extensions(server);
        let current = self.state.current_project().server_id == server;
        if current {
            self.git.reset_context();
        }
        for (project, repository) in &mut self.git.projects {
            if self.state.project_server(*project) == Some(server) {
                repository.disconnect();
            }
        }
        self.state.forget_remote_statuses(server);
        let Some(runtime) = self.servers.get_mut(server) else {
            return;
        };
        runtime.connection = ConnectionState::Disconnected;
        for progress in runtime.progress.values_mut() {
            progress.progress = None;
        }
        runtime.activity.snapshot.agents.clear();
        runtime.activity.loaded = false;
        runtime.activity.pending = false;
        runtime.references = None;
        runtime.sessions_revision = 0;
        runtime.discarding.clear();
        let panes: Vec<_> = self
            .completions
            .iter()
            .copied()
            .filter(|pane| self.on_server(*pane, server))
            .collect();
        for pane in panes {
            self.completions.remove(&pane);
        }
        self.forget_server_sessions(server, cx);
        self.forget_server_layouts(server, cx);
        if server.is_local() {
            self.quick.closing = None;
            self.refresh_quick_terminal(cx);
            if self.server_preferences.busy || !self.server_preferences.pending.is_empty() {
                let message = "Disconnected before settings were confirmed. Reconnect and reload before retrying.";
                self.preference_result("server", Some(message), cx);
                for id in self.pending_server_fields() {
                    self.preference_result(&id, Some(message), cx);
                }
            }
            self.server_preferences.busy = false;
            self.server_preferences.pending.clear();
            self.mobile = super::mobile::MobileAccess::default();
        }
        self.sync_preferences(cx);
        self.forget_requests(server);
        self.loaded
            .retain(|pane| self.state.pane_server(*pane) != Some(server));
        self.set_server_panes(server, PaneState::Disconnected, cx);
        cx.notify();
    }

    pub(super) fn send(&mut self, server: ServerId, work: Work, cx: &mut Context<Self>) -> bool {
        let Some(runtime) = self.servers.get(server) else {
            if server.is_local() {
                self.fail("Server disconnected".into(), cx);
            }
            return false;
        };
        if !matches!(
            work,
            Work::Input(..)
                | Work::TerminalInput(..)
                | Work::Mouse(..)
                | Work::CellSize(..)
                | Work::Ack(..)
                | Work::Flush
        ) {
            crate::diagnostics::event(
                "model.send",
                format_args!(
                    "server={server} generation={} kind={} connection={:?}",
                    runtime.generation,
                    work.name(),
                    runtime.connection
                ),
            );
        }
        if runtime.connection != ConnectionState::Ready && !matches!(work, Work::Flush) {
            if server.is_local() {
                self.fail("Server disconnected".into(), cx);
            }
            return false;
        }
        let sent = runtime
            .work
            .as_ref()
            .is_some_and(|worker| worker.send((runtime.generation, work)).is_ok());
        if !sent {
            self.disconnect(server, cx);
            self.server_problem(server, "The server connection worker stopped".into(), cx);
        }
        sent
    }

    /// Asks the workers to finish their work in flight before quitting: this
    /// computer's always, and each other server that is connected.
    pub(super) fn flush_servers(&mut self, cx: &mut Context<Self>) -> bool {
        self.flushing.clear();
        for server in self.servers.ids() {
            if server.is_local() || self.ready(server) {
                if !self.send(server, Work::Flush, cx) {
                    if server.is_local() {
                        return false;
                    }
                    continue;
                }
                self.flushing.insert(server);
            }
        }
        true
    }

    /// Whether every server has finished. A server with work left is asked to
    /// flush again.
    pub(super) fn flushed(&mut self, server: ServerId, cx: &mut Context<Self>) -> bool {
        let busy = self.pending_on(server)
            || self
                .servers
                .get(server)
                .is_some_and(|runtime| !runtime.discarding.is_empty());
        if busy {
            if !self.send(server, Work::Flush, cx) {
                if !server.is_local() {
                    self.flushing.remove(&server);
                    return self.flushing.is_empty();
                }
                if self.quitting == Quitting::Update {
                    self.quitting = Quitting::Idle;
                }
            }
            return false;
        }
        self.flushing.remove(&server);
        self.flushing.is_empty()
    }

    pub(super) fn stop_workers(&self) {
        for (_, runtime) in self.servers.iter() {
            if let Some(worker) = &runtime.work {
                let _ = worker.send((runtime.generation, Work::Stop));
            }
        }
    }
}
