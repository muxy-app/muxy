use std::collections::{BTreeMap, HashSet};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use muxy_protocol::{
    ErrorCode, ExitReason, HistoryCursor, HistoryPage, SavedScreen, ServerPath, SessionId,
    SessionInfo, Size, TerminalColors, validate_size,
};

use crate::archive::Archive;
use crate::error::ServerError;
use crate::session::{self, SessionHandle};
use crate::settings::ServerSettings;
use crate::spawn::spawn_shell;

type Sessions = Arc<Mutex<State>>;

#[derive(Debug, Default)]
struct State {
    sessions: BTreeMap<SessionId, SessionHandle>,
    starting: HashSet<SessionId>,
    stopping: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerEvent {
    SessionEnded { id: SessionId, reason: ExitReason },
    StopRequested,
    RestartRequested,
}

#[derive(Debug)]
pub struct Registry {
    pub(crate) activity: Arc<crate::activity::Activity>,
    pub(crate) catalog: Arc<crate::catalog::Catalog>,
    pub(crate) git: crate::git::Git,
    pub(crate) files: crate::files::Files,
    pub(crate) operations: Mutex<()>,
    connections: Mutex<Vec<Weak<crate::connection::Outbox>>>,
    pub(crate) attachment_changes: Arc<AtomicU64>,
    settings: Mutex<ServerSettings>,
    settings_write: Mutex<()>,
    persist: crate::settings::Persistence,
    shell_integration: Option<crate::ShellIntegration>,
    events: Sender<ServerEvent>,
    sessions: Sessions,
    completed: Arc<Condvar>,
    archive: Archive,
    pub(crate) remote: crate::RemoteAccess,
}

impl Registry {
    pub fn new(settings: ServerSettings, events: Sender<ServerEvent>) -> Self {
        Self {
            activity: Arc::default(),
            catalog: Arc::new(crate::catalog::Catalog::memory()),
            operations: Mutex::new(()),
            git: crate::git::Git::default(),
            files: crate::files::Files::default(),
            connections: Mutex::default(),
            attachment_changes: Arc::default(),
            archive: Archive::memory(settings.history_budget_bytes),
            settings: Mutex::new(settings),
            settings_write: Mutex::new(()),
            persist: crate::settings::Persistence(Box::new(|_| Ok(()))),
            shell_integration: None,
            events,
            sessions: Sessions::default(),
            completed: Arc::default(),
            remote: crate::RemoteAccess::memory(),
        }
    }

    pub fn persistent(
        settings: ServerSettings,
        events: Sender<ServerEvent>,
        directory: &Path,
    ) -> io::Result<Self> {
        Self::persistent_with_import(settings, events, directory, crate::LegacyImport::default())
    }

    pub fn persistent_with_import(
        settings: ServerSettings,
        events: Sender<ServerEvent>,
        directory: &Path,
        legacy: crate::LegacyImport,
    ) -> io::Result<Self> {
        let archive = Archive::open(directory, settings.history_budget_bytes)?;
        let catalog = Arc::new(crate::catalog::Catalog::open(directory, &archive, legacy)?);
        let registry = Self {
            activity: Arc::new(crate::activity::Activity::open(directory)?),
            catalog,
            archive,
            ..Self::new(settings, events)
        };
        registry.resume_git();
        registry.resume_cleanup().map_err(io::Error::other)?;
        Ok(registry)
    }

    pub(crate) fn session_observations(
        &self,
        outbox: &crate::connection::Outbox,
    ) -> BTreeMap<
        SessionId,
        (
            muxy_protocol::SessionProgress,
            muxy_protocol::SessionMetadata,
        ),
    > {
        let sessions = outbox.referenced_sessions();
        let handles: Vec<_> = {
            let state = lock(&self.sessions);
            sessions
                .iter()
                .filter_map(|id| state.sessions.get(id).cloned())
                .collect()
        };
        handles
            .into_iter()
            .map(|handle| (handle.id(), (handle.progress(), handle.metadata())))
            .collect()
    }

    pub fn home_project(&self) -> muxy_protocol::ProjectId {
        self.catalog.home()
    }
    pub fn catalog_revision(&self) -> u64 {
        self.catalog.revision()
    }

    pub fn read_catalog(
        &self,
        after: Option<muxy_protocol::ProjectId>,
        revision: Option<u64>,
    ) -> Result<muxy_protocol::CatalogPage, ServerError> {
        self.catalog.page(after, revision)
    }

    pub fn list_project_sessions(
        &self,
        project: muxy_protocol::ProjectId,
        after: Option<SessionId>,
        revision: Option<u64>,
    ) -> Result<muxy_protocol::ProjectSessions, ServerError> {
        self.project_sessions_for(project, after, revision, None)
    }

    pub(crate) fn sessions_revision(&self) -> u64 {
        self.catalog_revision()
            .saturating_add(self.attachment_changes.load(Ordering::Acquire))
    }

    pub(crate) fn project_sessions_for(
        &self,
        project: muxy_protocol::ProjectId,
        after: Option<SessionId>,
        revision: Option<u64>,
        requester: Option<&crate::connection::Outbox>,
    ) -> Result<muxy_protocol::ProjectSessions, ServerError> {
        let _operation = self.session_operation();
        let current = self.sessions_revision();
        let changed = || {
            ServerError::new(
                ErrorCode::CatalogChanged,
                "sessions changed; restart the page fetch",
            )
        };
        if revision.is_some_and(|revision| revision != current) {
            return Err(changed());
        }
        let mut page = self.catalog.list_project(project, after, None)?;
        let connections: Vec<_> = self
            .connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        for session in &mut page.sessions {
            if matches!(
                session.status,
                muxy_protocol::SessionStatus::Live | muxy_protocol::SessionStatus::Starting
            ) {
                session.owner = connections
                    .iter()
                    .filter_map(|connection| connection.participation(session.info.id))
                    .min_by_key(|(order, _)| *order)
                    .map(|(_, client)| client);
                session.attached = requester
                    .is_some_and(|connection| connection.participation(session.info.id).is_some());
            }
        }
        if current != self.sessions_revision() {
            return Err(changed());
        }
        page.revision = current;
        Ok(page)
    }

    pub fn mutate_project(
        &self,
        intent: &muxy_protocol::ProjectIntent,
    ) -> Result<u64, ServerError> {
        let _operation = loop {
            let revision = self.catalog.revision();
            let validate = if let muxy_protocol::ProjectMutation::Create(project) = &intent.mutation
                && project.kind == Some(muxy_protocol::ProjectKind::Worktree)
                && !self.catalog.has_project_receipt(intent.operation)
            {
                self.validate_worktree_registration(project)?;
                true
            } else {
                false
            };
            let operation = self.session_operation();
            if !validate || self.catalog.revision() == revision {
                break operation;
            }
        };
        if !self.catalog.has_project_receipt(intent.operation) {
            self.git.operations.check_mutation(&intent.mutation)?;
        }
        if !self.catalog.begin_mutation(intent)? {
            self.resume_cleanup()?;
        }
        Ok(self.catalog.revision())
    }

    pub(crate) fn resume_cleanup(&self) -> Result<(), ServerError> {
        for session in self.catalog.discarding() {
            self.discard_owned(session)?;
        }
        let deleting = self.catalog.deleting();
        for project in &deleting {
            for session in self.catalog.owned(*project) {
                self.discard_owned(session)?;
            }
        }
        if !deleting.is_empty() {
            self.catalog.finish_deletions()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn with_remote_access(mut self, remote: crate::RemoteAccess) -> Self {
        self.remote = remote;
        self
    }

    #[must_use]
    pub fn with_remote_listener(
        mut self,
        listener: impl Fn(
            Option<(u16, muxy_protocol::transport::tls::TlsIdentity)>,
        ) -> muxy_protocol::ListenerStatus
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.remote.set_listener(Box::new(listener));
        self
    }

    /// Starts listening for paired devices if mobile access was left on.
    pub fn resume_remote_access(&self) {
        self.remote.resume();
    }

    /// Reserves a slot for a network connection that has not authenticated yet.
    pub fn admit_remote(&self) -> Option<crate::Admission> {
        self.remote.admit()
    }

    pub fn remote_access_state(&self) -> muxy_protocol::RemoteAccessState {
        let connected = self
            .live_connections()
            .iter()
            .filter_map(|connection| connection.device())
            .collect();
        self.remote.state(&connected)
    }

    pub(crate) fn write_remote_access(
        &self,
        settings: muxy_protocol::RemoteAccessSettings,
    ) -> Result<muxy_protocol::RemoteAccessState, ServerError> {
        self.remote.configure(settings)?;
        if !settings.enabled {
            self.close_devices(None);
        }
        Ok(self.remote_access_state())
    }

    pub(crate) fn revoke_device(
        &self,
        device: muxy_protocol::DeviceId,
    ) -> Result<muxy_protocol::RemoteAccessState, ServerError> {
        self.remote.revoke(device)?;
        self.close_devices(Some(device));
        Ok(self.remote_access_state())
    }

    pub(crate) fn pair_device(
        &self,
        request: &muxy_protocol::PairRequest,
    ) -> Option<muxy_protocol::Paired> {
        self.remote.pair(request, self.catalog.identity())
    }

    pub(crate) fn connection_closed(&self, outbox: &crate::connection::Outbox) {
        if outbox.device().is_some() {
            self.remote.changed();
        } else {
            self.remote.release_pairing(outbox.client().id);
        }
    }

    /// Closes the connections of one paired device, or of every paired device.
    fn close_devices(&self, device: Option<muxy_protocol::DeviceId>) {
        for connection in self.live_connections() {
            if connection
                .device()
                .is_some_and(|connected| device.is_none_or(|device| device == connected))
            {
                connection.close();
            }
        }
    }

    fn live_connections(&self) -> Vec<Arc<crate::connection::Outbox>> {
        self.connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter_map(Weak::upgrade)
            .filter(|connection| !connection.is_closed())
            .collect()
    }

    #[must_use]
    pub fn with_shell_integration(mut self, integration: crate::ShellIntegration) -> Self {
        self.shell_integration = Some(integration);
        self
    }

    pub fn settings(&self) -> ServerSettings {
        self.settings
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    #[must_use]
    pub fn with_settings_persistence(
        mut self,
        persist: impl Fn(&ServerSettings) -> io::Result<()> + Send + Sync + 'static,
    ) -> Self {
        self.persist = crate::settings::Persistence(Box::new(persist));
        self
    }

    pub fn write_settings(&self, settings: ServerSettings) -> Result<(), ServerError> {
        settings.validate()?;
        let _write = self
            .settings_write
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        (self.persist.0)(&settings).map_err(|error| {
            ServerError::new(
                ErrorCode::BadRequest,
                format!("Could not save server settings: {error}"),
            )
        })?;
        *self.settings.lock().unwrap_or_else(PoisonError::into_inner) = settings;
        Ok(())
    }

    /// Reserves shutdown against all concurrent session creation.
    pub fn stop_if_idle(&self) -> bool {
        let mut state = lock(&self.sessions);
        if state.stopping || !state.sessions.is_empty() || !state.starting.is_empty() {
            return false;
        }
        state.stopping = true;
        true
    }

    pub fn request_restart(&self) {
        let _ = self.events.send(ServerEvent::RestartRequested);
    }

    pub fn request_stop(&self) {
        let _ = self.events.send(ServerEvent::StopRequested);
    }

    pub fn create(&self, directory: &Path, size: Size) -> Result<SessionInfo, ServerError> {
        self.create_project_session(
            self.home_project(),
            muxy_protocol::OperationId::new(),
            directory,
            size,
        )
    }

    pub fn create_project_session(
        &self,
        project: muxy_protocol::ProjectId,
        operation: muxy_protocol::OperationId,
        directory: &Path,
        size: Size,
    ) -> Result<SessionInfo, ServerError> {
        self.create_with_colors(project, operation, directory, size, None, None)
    }

    pub(crate) fn create_with_colors(
        &self,
        project: muxy_protocol::ProjectId,
        operation: muxy_protocol::OperationId,
        directory: &Path,
        size: Size,
        colors: Option<TerminalColors>,
        requester: Option<&crate::connection::Outbox>,
    ) -> Result<SessionInfo, ServerError> {
        let _operation = self.session_operation();
        let directory_bytes = session_directory(directory)?;
        self.git
            .operations
            .check_session(Path::new(std::ffi::OsStr::from_bytes(&directory_bytes.0)))?;
        if let Some(info) = self
            .catalog
            .creation(operation, project, &directory_bytes)?
        {
            self.attach_creator(info.id, requester);
            return Ok(info);
        }
        validate_size(size).map_err(|code| {
            ServerError::new(
                code,
                format!("{}x{} is not a valid size", size.cols, size.rows),
            )
        })?;
        let id = self.reserve_start()?;
        let info = SessionInfo {
            id,
            project,
            directory: directory_bytes,
        };
        if let Err(error) = self.catalog.reserve(operation, &info) {
            lock(&self.sessions).starting.remove(&id);
            self.completed.notify_all();
            return Err(error);
        }
        let settings = self.settings();
        let budget = usize::try_from(settings.history_budget_bytes).unwrap_or(usize::MAX);
        let listing = Arc::clone(&self.sessions);
        let events = self.events.clone();
        let completed = Arc::clone(&self.completed);
        let catalog = Arc::clone(&self.catalog);
        let saved = self.archive.clone();
        let handle = spawn_shell(
            &settings,
            self.shell_integration.as_ref(),
            directory,
            size.into(),
            (self.catalog.identity(), id),
        )
        .and_then(|pty| {
            session::start(
                info.clone(),
                pty,
                size,
                budget,
                self.archive.clone(),
                colors,
                Arc::clone(&self.activity),
                move |reason| {
                    let status = if saved.contains(id) {
                        muxy_protocol::SessionStatus::Ended
                    } else {
                        muxy_protocol::SessionStatus::Unavailable
                    };
                    catalog.record_exit(id, status);
                    let mut state = lock(&listing);
                    state.sessions.remove(&id);
                    state.starting.remove(&id);
                    let _ = events.send(ServerEvent::SessionEnded { id, reason });
                    completed.notify_all();
                },
            )
        });
        let handle = match handle {
            Ok(handle) => handle,
            Err(error) => {
                lock(&self.sessions).starting.remove(&id);
                self.completed.notify_all();
                self.catalog.fail_creation(operation, &error)?;
                return Err(error);
            }
        };
        let published = self
            .catalog
            .set_status(id, muxy_protocol::SessionStatus::Live);
        let mut state = lock(&self.sessions);
        let starting = state.starting.remove(&id);
        self.completed.notify_all();
        if starting {
            if state.stopping {
                let _ = handle.send(session::SessionCommand::Stop);
            }
            state.sessions.insert(id, handle);
        }
        drop(state);
        if let Err(error) = published {
            if let Some(handle) = self.handle(id) {
                let _ = handle.send(session::SessionCommand::End);
            }
            self.wait_ended(id)?;
            return Err(error);
        }
        log::info!("session created: {}", id.get());
        self.attach_creator(id, requester);
        Ok(info)
    }

    fn attach_creator(&self, id: SessionId, requester: Option<&crate::connection::Outbox>) {
        if let Some(requester) = requester
            && self.handle(id).is_some()
        {
            requester.created_session(id);
        }
    }

    fn reserve_start(&self) -> Result<SessionId, ServerError> {
        loop {
            let id = fresh_id(&self.archive)?;
            let mut state = lock(&self.sessions);
            if state.stopping {
                return Err(ServerError::new(
                    ErrorCode::SpawnFailed,
                    "server is stopping",
                ));
            }
            if !state.sessions.contains_key(&id) && state.starting.insert(id) {
                return Ok(id);
            }
        }
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        lock(&self.sessions)
            .sessions
            .values()
            .map(|handle| handle.info().clone())
            .collect()
    }

    pub fn handle(&self, id: SessionId) -> Option<SessionHandle> {
        lock(&self.sessions).sessions.get(&id).cloned()
    }

    pub fn end(&self, id: SessionId) -> Result<(), ServerError> {
        let handle = self
            .handle(id)
            .ok_or_else(|| ServerError::unknown_session(id))?;
        let _ = handle.send(session::SessionCommand::End);
        self.wait_ended(id)
    }

    pub fn cancel_creation(
        &self,
        operation: muxy_protocol::OperationId,
    ) -> Result<(), ServerError> {
        let _operation = self
            .operations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(session) = self.catalog.cancel_creation(operation)? {
            self.discard_owned(session)?;
        }
        Ok(())
    }

    pub(crate) fn session_operation(&self) -> MutexGuard<'_, ()> {
        self.operations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn claim_activity(
        &self,
        requester: &crate::connection::Outbox,
        ids: &[u64],
    ) -> Vec<u64> {
        let connections = self
            .connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let owner = connections
            .iter()
            .filter_map(Weak::upgrade)
            .find(|connection| {
                !connection.is_closed()
                    && connection.client().kind == muxy_protocol::ClientKind::Desktop
            });
        if owner.is_some_and(|owner| owner.client().id == requester.client().id) {
            self.activity.claim(ids)
        } else {
            Vec::new()
        }
    }

    pub(crate) fn register_connection(&self, outbox: &Arc<crate::connection::Outbox>) {
        let mut connections = self
            .connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        connections.retain(|connection| connection.strong_count() > 0);
        connections.push(Arc::downgrade(outbox));
    }

    pub(crate) fn sync_references(
        &self,
        requester: &crate::connection::Outbox,
        owner: Option<muxy_protocol::OperationId>,
        revision: u64,
        sessions: &[SessionId],
    ) {
        let _operation = self.session_operation();
        let connections: Vec<_> = self
            .connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter_map(Weak::upgrade)
            .collect();
        let mut references: crate::connection::References = owner
            .and_then(|owner| {
                connections
                    .iter()
                    .filter_map(|connection| connection.reference_state())
                    .filter(|references| references.owner == Some(owner))
                    .max_by_key(|references| references.revision)
            })
            .or_else(|| requester.reference_state())
            .unwrap_or_default();
        let accepted =
            owner.is_none() || references.owner != owner || revision >= references.revision;
        if accepted {
            references.update(owner, revision, sessions.to_vec());
        }
        requester.set_references(references.clone(), Some(sessions), accepted);
        if let Some(owner) = owner {
            for connection in connections {
                if connection
                    .reference_state()
                    .is_some_and(|state| state.owner == Some(owner))
                {
                    connection.set_references(references.clone(), None, false);
                }
            }
        }
    }

    pub(crate) fn session_shared(
        &self,
        session: SessionId,
        requester: &crate::connection::Outbox,
    ) -> bool {
        self.connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter_map(Weak::upgrade)
            .any(|connection| {
                !std::ptr::eq(connection.as_ref(), requester)
                    && connection.references(session, true)
            })
    }

    pub(crate) fn can_attach(&self, session: SessionId) -> bool {
        !self.catalog.discarding().contains(&session)
    }

    pub(crate) fn close_session(
        &self,
        session: SessionId,
        operation: muxy_protocol::OperationId,
        requester: &crate::connection::Outbox,
    ) -> Result<(), ServerError> {
        let _operation = self.session_operation();
        let discard = if let Some(discard) = self.catalog.close_receipt(operation, session)? {
            discard
        } else {
            let local = requester.references(session, false);
            if !local {
                requester.detach_session(session);
            }
            let discard = !local && !self.session_shared(session, requester);
            self.catalog.record_close(operation, session, discard)?;
            discard
        };
        if discard {
            self.discard_owned(session)?;
        }
        Ok(())
    }

    pub fn discard(&self, id: SessionId) -> Result<(), ServerError> {
        let _operation = self
            .operations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        self.discard_owned(id)
    }

    pub(crate) fn discard_owned(&self, id: SessionId) -> Result<(), ServerError> {
        self.catalog.begin_discard(id)?;
        if let Some(handle) = self.handle(id) {
            let _ = handle.send(session::SessionCommand::End);
            self.wait_ended(id)?;
        }
        self.archive
            .discard(id)
            .map_err(|error| saved_content_error(&error))?;
        self.catalog.finish_discard(id)
    }

    pub fn read_saved_screen(&self, id: SessionId) -> Result<SavedScreen, ServerError> {
        self.archive
            .read(id)
            .map_err(|error| saved_content_error(&error))
    }

    pub fn saved_history_page(
        &self,
        id: SessionId,
        before: HistoryCursor,
        max_rows: u16,
    ) -> Result<HistoryPage, ServerError> {
        self.archive.history_page(id, before, max_rows)
    }

    pub(crate) fn archive(&self) -> Archive {
        self.archive.clone()
    }

    fn wait_ended(&self, id: SessionId) -> Result<(), ServerError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut state = lock(&self.sessions);
        while state.sessions.contains_key(&id) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ServerError::new(
                    ErrorCode::BadRequest,
                    "session termination timed out",
                ));
            }
            (state, _) = self
                .completed
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner);
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        let mut state = lock(&self.sessions);
        if !state.stopping {
            state.stopping = true;
            for handle in state.sessions.values() {
                let _ = handle.send(session::SessionCommand::Stop);
            }
        }
    }

    pub fn is_stopped(&self) -> bool {
        let state = lock(&self.sessions);
        state.stopping && state.sessions.is_empty() && state.starting.is_empty()
    }
}

fn lock(sessions: &Sessions) -> MutexGuard<'_, State> {
    sessions.lock().unwrap_or_else(PoisonError::into_inner)
}

fn fresh_id(archive: &Archive) -> Result<SessionId, ServerError> {
    loop {
        let value = getrandom::u64().map_err(|error| {
            ServerError::new(ErrorCode::SpawnFailed, format!("no randomness: {error}"))
        })?;
        if let Some(id) = SessionId::new(value)
            && !archive.contains(id)
        {
            return Ok(id);
        }
    }
}

fn saved_content_error(error: &io::Error) -> ServerError {
    ServerError::new(
        ErrorCode::SavedContentUnavailable,
        format!("saved terminal content: {error}"),
    )
}

#[cfg(test)]
mod update_tests {
    use super::*;

    #[test]
    fn shutdown_reservation_includes_in_progress_spawns() {
        let (events, _receiver) = std::sync::mpsc::channel();
        let registry = Registry::new(ServerSettings::default(), events);
        let session = SessionId::new(1).expect("session");
        lock(&registry.sessions).starting.insert(session);
        assert!(!registry.stop_if_idle());
        assert!(!lock(&registry.sessions).stopping);
        lock(&registry.sessions).starting.remove(&session);
        assert!(registry.stop_if_idle());
        assert!(!registry.stop_if_idle());
    }

    #[test]
    fn creation_racing_idle_shutdown_is_either_preserved_or_rejected() {
        for _ in 0..8 {
            let (events, _receiver) = std::sync::mpsc::channel();
            let registry = Arc::new(Registry::new(
                ServerSettings {
                    default_shell: Some("/bin/sh".into()),
                    ..ServerSettings::default()
                },
                events,
            ));
            let gate = Arc::new(std::sync::Barrier::new(2));
            let spawning = registry.clone();
            let start = gate.clone();
            let worker = std::thread::spawn(move || {
                start.wait();
                spawning.create(Path::new("/tmp"), Size { cols: 80, rows: 24 })
            });
            gate.wait();
            let stopped = registry.stop_if_idle();
            let created = worker.join().expect("spawn thread");
            if stopped {
                assert!(created.is_err());
                assert!(registry.list().is_empty());
            } else {
                let session = created.expect("session preserved");
                assert_eq!(registry.list(), vec![session.clone()]);
                registry.end(session.id).expect("end fixture");
            }
        }
    }
}

#[cfg(test)]
#[path = "registry/recovery.rs"]
mod recovery;

fn session_directory(directory: &Path) -> Result<ServerPath, ServerError> {
    let bytes = directory.as_os_str().as_bytes();
    if !directory.is_absolute() || bytes.len() > 4096 || bytes.contains(&0) {
        return Err(ServerError::new(
            ErrorCode::BadPath,
            "session directory must be an absolute Unix path",
        ));
    }
    Ok(ServerPath(bytes.into()))
}
