mod io;
pub(crate) use io::{InputWriter, Shared, View, lock};

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use muxy_app_core::{Direction, PaneId};
use muxy_client::{Client, ClientError, Start};
use muxy_protocol::{
    CatalogPage, ErrorCode, ProjectId, ProjectSession, ServerIdentity, SessionId, Size,
};
use ratatui::layout::Rect;

use crate::input::Input;
use crate::state::{Discard, Result, Store};
use crate::target::Target;

/// How long the worker waits for a request before checking whether to quit.
const POLL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
pub(crate) enum Action {
    Open(ProjectId),
    New(Option<Direction>),
    SelectTab(usize),
    SelectPane(ProjectId, PaneId),
    CycleTab(bool),
    Focus(Direction),
    Resize(Direction),
    Zoom,
    CheckClose,
    Close(PaneId),
    Existing(ProjectSession),
    ListSessions,
    Detach,
    Input(Input),
}

pub(crate) struct Worker {
    pub shared: Arc<Mutex<Shared>>,
    pub input: Arc<InputWriter>,
    pub viewport: Arc<Mutex<Rect>>,
    sender: SyncSender<Action>,
    stop: Arc<AtomicBool>,
    connection: Arc<Mutex<Option<Client>>>,
    thread: Option<JoinHandle<()>>,
    ordered: Arc<AtomicUsize>,
    queued_bytes: Arc<AtomicUsize>,
}

impl Worker {
    /// Starts serving `target`, beginning with `first` if it is already
    /// connected.
    pub(crate) fn start(target: Target, first: Option<Client>, viewport: Rect) -> Result<Self> {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let input = Arc::new(InputWriter::new(Arc::clone(&shared))?);
        let ordered = Arc::new(AtomicUsize::new(0));
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let viewport = Arc::new(Mutex::new(viewport));
        let stop = Arc::new(AtomicBool::new(false));
        let connection = Arc::new(Mutex::new(None));
        let (sender, receiver) = mpsc::sync_channel(1024);
        let mut core = Core {
            target,
            shared: Arc::clone(&shared),
            viewport: Arc::clone(&viewport),
            stop: Arc::clone(&stop),
            connection: Arc::clone(&connection),
            store: None,
            references: None,
            input: Arc::clone(&input),
            ordered: Arc::clone(&ordered),
            queued_bytes: Arc::clone(&queued_bytes),
        };
        let worker = thread::Builder::new()
            .name("muxy-tui-requests".into())
            .spawn(move || {
                core.run(first, &receiver);
                lock(&core.shared).exit.get_or_insert(Ok(()));
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            shared,
            input,
            viewport,
            sender,
            stop,
            connection,
            thread: Some(worker),
            ordered,
            queued_bytes,
        })
    }

    pub(crate) fn send(&self, action: Action) -> Result {
        if !matches!(action, Action::Detach)
            && lock(&self.shared)
                .client
                .as_ref()
                .is_none_or(|client| !client.is_connected())
        {
            return Err("Server is disconnected; the action was not applied".into());
        }
        let action = if let Action::SelectTab(index) = action {
            let shared = lock(&self.shared);
            let (project, pane) = shared
                .state
                .as_ref()
                .and_then(|state| state.tab_selection(index))
                .ok_or("Tab is no longer available")?;
            Action::SelectPane(project, pane.ok_or("Tab has no pane")?)
        } else {
            action
        };
        let ordered = !matches!(action, Action::ListSessions);
        let bytes = match &action {
            Action::Input(input) => input.length(),
            _ => 0,
        };
        self.queued_bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count
                    .checked_add(bytes)
                    .filter(|count| *count <= 16 * muxy_protocol::MAX_INPUT)
            })
            .map_err(|_| "Input buffer is full; wait before sending more text")?;
        if ordered {
            self.ordered.fetch_add(1, Ordering::AcqRel);
        }
        if self.sender.try_send(action).is_err() {
            if ordered {
                self.ordered.fetch_sub(1, Ordering::AcqRel);
            }
            self.queued_bytes.fetch_sub(bytes, Ordering::AcqRel);
            return Err("TUI is busy; wait for the pending action".into());
        }
        Ok(())
    }

    pub(crate) fn typing(&self, input: Input) -> Result {
        if self.ordered.load(Ordering::Acquire) > 0 {
            return self.send(Action::Input(input));
        }
        send_input(&self.shared, &self.input, input)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(client) = lock(&self.connection).take() {
            client.disconnect();
        }
        if let Some(worker) = self.thread.take() {
            let _ = worker.join();
        }
    }
}

struct Core {
    target: Target,
    shared: Arc<Mutex<Shared>>,
    viewport: Arc<Mutex<Rect>>,
    stop: Arc<AtomicBool>,
    connection: Arc<Mutex<Option<Client>>>,
    store: Option<Store>,
    references: Option<(u64, Vec<SessionId>)>,
    input: Arc<InputWriter>,
    ordered: Arc<AtomicUsize>,
    queued_bytes: Arc<AtomicUsize>,
}

impl Core {
    fn run(&mut self, mut first: Option<Client>, requests: &Receiver<Action>) {
        let mut failures = 0;
        while !self.stop.load(Ordering::Acquire) {
            let client = if let Some(client) = first.take() {
                client
            } else {
                self.message(&format!("Connecting to {}…", self.target.describe()));
                match self.connect(requests) {
                    Some(Ok(client)) => client,
                    Some(Err(error)) if !error.recoverable() => {
                        self.fail(error.to_string());
                        break;
                    }
                    Some(Err(error)) => {
                        self.message(&self.target.explain(error).to_string());
                        failures += 1;
                        if self.pause(requests, self.target.retry_delay(failures)) {
                            continue;
                        }
                        break;
                    }
                    None => break,
                }
            };
            failures = 0;
            *lock(&self.connection) = Some(client.clone());
            let reader = match io::reader(client.clone(), Arc::clone(&self.shared)) {
                Ok(reader) => reader,
                Err(error) => {
                    self.message(&error);
                    break;
                }
            };
            let result = self.connected(&client, requests);
            client.disconnect();
            let _ = reader.join();
            lock(&self.shared).disconnected();
            *lock(&self.connection) = None;
            match result {
                Ok(true) => break,
                Ok(false) => {}
                Err(error) => {
                    self.fail(error);
                    break;
                }
            }
        }
    }

    /// Ends the TUI with `error`.
    fn fail(&self, error: String) {
        let mut shared = lock(&self.shared);
        shared.message.clone_from(&error);
        shared.exit = Some(Err(error));
    }

    /// Connects on another thread, so Detach and quitting never wait for a
    /// slow host. Returns `None` if the TUI stops first.
    fn connect(
        &self,
        requests: &Receiver<Action>,
    ) -> Option<std::result::Result<Client, ClientError>> {
        let (sender, connected) = mpsc::sync_channel(1);
        let target = self.target.clone();
        let spawned = thread::Builder::new()
            .name("muxy-tui-connect".into())
            .spawn(move || {
                let _ = sender.send(target.connect(Start::IfNeeded));
            });
        if let Err(error) = spawned {
            return Some(Err(error.into()));
        }
        loop {
            match connected.recv_timeout(POLL) {
                Ok(result) => return Some(result),
                Err(RecvTimeoutError::Disconnected) => return Some(Err(ClientError::Disconnected)),
                Err(RecvTimeoutError::Timeout) => {}
            }
            if !self.idle(requests, Duration::ZERO) {
                return None;
            }
        }
    }

    /// Waits before connecting again. Returns false if the TUI stops first.
    fn pause(&self, requests: &Receiver<Action>, delay: Duration) -> bool {
        let deadline = Instant::now() + delay;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return true;
            }
            if !self.idle(requests, left.min(POLL)) {
                return false;
            }
        }
    }

    /// Answers a request that arrives within `timeout` while disconnected.
    /// Returns false once the TUI should stop: on Detach, or when it closes.
    fn idle(&self, requests: &Receiver<Action>, timeout: Duration) -> bool {
        match requests.recv_timeout(timeout) {
            Ok(Action::Detach) | Err(RecvTimeoutError::Disconnected) => return false,
            Ok(action) => {
                if !matches!(action, Action::ListSessions) {
                    self.ordered.fetch_sub(1, Ordering::AcqRel);
                }
                if let Action::Input(input) = action {
                    self.queued_bytes
                        .fetch_sub(input.length(), Ordering::AcqRel);
                }
                self.message("Server is disconnected; the pending action was not applied");
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        !self.stop.load(Ordering::Acquire)
    }

    fn connected(&mut self, client: &Client, requests: &Receiver<Action>) -> Result<bool> {
        self.references = None;
        client
            .identify(muxy_protocol::ClientKind::Tui)
            .map_err(|error| error.to_string())?;
        let mut catalog = client.catalog().map_err(|error| error.to_string())?;
        self.load_layout(&catalog)?;
        self.store_mut()?
            .change(|state| state.reconcile(&catalog))?;
        let references = self.store_mut()?.state.session_references();
        let live = client.list_sessions().map_err(|error| error.to_string())?;
        let ended: Vec<_> = references
            .into_iter()
            .filter(|id| !live.iter().any(|session| session.id == *id))
            .collect();
        for session in ended {
            lock(&self.shared).session_ended(session);
        }
        self.close_ended_sessions(&catalog)?;
        self.publish(&catalog);
        lock(&self.shared).client = Some(client.clone());
        lock(&self.shared).refresh = true;
        self.message("");
        while !self.stop.load(Ordering::Acquire) && client.is_connected() {
            self.close_ended_sessions(&catalog)?;
            let refresh = {
                let mut shared = lock(&self.shared);
                std::mem::take(&mut shared.refresh)
            };
            if refresh || self.store_mut()?.state.catalog_revision > catalog.revision {
                match client.catalog() {
                    Ok(next) => {
                        self.store_mut()?.change(|state| state.reconcile(&next))?;
                        catalog = next;
                        self.publish(&catalog);
                        lock(&self.shared).sessions_dirty = true;
                    }
                    Err(error) => self.message(&error.to_string()),
                }
            }
            if let Err(error) = self.synchronize(client, &catalog) {
                if self.store_mut()?.ready().is_err() {
                    return Err(error);
                }
                self.message(&error);
            }
            let sessions_dirty = {
                let project = self.store_mut()?.state.active;
                let mut shared = lock(&self.shared);
                std::mem::take(&mut shared.sessions_dirty)
                    || shared.sessions_project != Some(project)
            };
            if sessions_dirty && let Err(error) = self.list_sessions(client, &catalog) {
                self.message(&error);
            }
            match requests.recv_timeout(POLL) {
                Ok(Action::Detach) | Err(RecvTimeoutError::Disconnected) => return Ok(true),
                Ok(action) => {
                    let ordered = !matches!(action, Action::ListSessions);
                    let bytes = match &action {
                        Action::Input(input) => input.length(),
                        _ => 0,
                    };
                    if let Err(error) = self.action(action, client, &catalog) {
                        self.message(&error);
                    }
                    self.publish(&catalog);
                    self.queued_bytes.fetch_sub(bytes, Ordering::AcqRel);
                    if ordered {
                        self.ordered.fetch_sub(1, Ordering::AcqRel);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
        Ok(self.stop.load(Ordering::Acquire))
    }

    /// Loads the layout once the server is known. A remote's layout belongs
    /// to the server that first answered there, so a different one answering
    /// after a reconnect needs a fresh start.
    fn load_layout(&mut self, catalog: &CatalogPage) -> Result {
        match (&self.store, &self.target) {
            (None, _) => {}
            (Some(store), Target::Ssh { host, .. }) if store.state.server != catalog.server => {
                return Err(format!(
                    "A different Muxy server now runs on {host}. Run muxy --host {host} again to use it."
                ));
            }
            (Some(_), _) => return Ok(()),
        }
        self.store = Some(Store::load(
            &self.target.layout_directory(catalog.server),
            catalog,
        )?);
        Ok(())
    }

    fn action(&mut self, action: Action, client: &Client, catalog: &CatalogPage) -> Result {
        self.close_ended_sessions(catalog)?;
        if matches!(action, Action::CheckClose) {
            return self.check_close(client, catalog);
        }
        if let Action::Input(input) = action {
            return send_input(&self.shared, &self.input, input);
        }
        if matches!(action, Action::ListSessions) {
            return self.list_sessions(client, catalog);
        }
        self.input.flush()?;
        let host = hosting_session(catalog.server);
        let (selection, selected_tab) = {
            let shared = lock(&self.shared);
            let state = shared.state.as_ref().ok_or("TUI state is not ready")?;
            let selected = match action {
                Action::SelectPane(project, pane) => Some((project, Some(pane))),
                Action::SelectTab(index) => state.tab_selection(index),
                Action::CycleTab(forward) => state.cycle_selection(forward),
                _ => None,
            };
            (state.selection(), selected)
        };
        self.store_mut()?.change(|state| {
            if !matches!(action, Action::Open(_)) {
                state.select(selection)?;
            }
            match action {
                Action::Open(id) => state.open(id, catalog)?,
                Action::New(split) => {
                    let directory = catalog
                        .projects
                        .iter()
                        .find(|project| project.id == state.active)
                        .ok_or("Project is no longer available")?
                        .directory
                        .clone();
                    state.new_pane(split, directory, None)?;
                }
                Action::Existing(session) => {
                    if state.session_references().contains(&session.info.id) {
                        return Ok(());
                    }
                    if !matches!(
                        session.status,
                        muxy_protocol::SessionStatus::Live | muxy_protocol::SessionStatus::Starting
                    ) {
                        return Err("Terminal has ended".into());
                    }
                    if Some(session.info.id) == host {
                        return Err("Cannot attach the terminal hosting this TUI".into());
                    }
                    if session.info.project != state.active {
                        return Err("Terminal belongs to another project".into());
                    }
                    state.new_pane(None, session.info.directory, Some(session.info.id))?;
                }
                Action::Close(id) => {
                    if state
                        .tab()
                        .and_then(|tab| tab.panes.get(&id))
                        .is_some_and(|pane| pane.session.is_some() && pane.session == host)
                    {
                        return Err("Cannot close the terminal hosting this TUI".into());
                    }
                    state.close(id)?;
                }
                Action::Focus(direction) => state.focus(direction),
                Action::Resize(direction) => state.resize(direction)?,
                Action::Zoom => {
                    if let Some(tab) = state.tab_mut() {
                        tab.zoom = !tab.zoom;
                    }
                }
                Action::SelectPane(_, _) | Action::SelectTab(_) | Action::CycleTab(_) => {
                    if let Some(selected) = selected_tab {
                        state.select(selected)?;
                    }
                }
                Action::ListSessions | Action::Detach | Action::Input(_) | Action::CheckClose => {}
            }
            Ok(())
        })?;
        self.message("");
        Ok(())
    }

    fn check_close(&mut self, client: &Client, catalog: &CatalogPage) -> Result {
        self.input.flush()?;
        let tab = self.store_mut()?.state.tab().ok_or("No pane to close")?;
        let id = tab.focus;
        let session = tab.panes[&id].session;
        if session.is_some() && session == hosting_session(catalog.server) {
            return Err("Cannot close the terminal hosting this TUI".into());
        }
        if let Some(session) = session {
            let local = self
                .store_mut()?
                .state
                .projects
                .values()
                .flat_map(|project| &project.tabs)
                .flat_map(|tab| &tab.panes)
                .any(|(pane, value)| *pane != id && value.session == Some(session));
            if local {
                return self.action(Action::Close(id), client, catalog);
            }
            let size = lock(&self.shared)
                .views
                .get(&id)
                .map_or(Size { cols: 80, rows: 24 }, |view| view.viewport);
            match client.attach(session, size) {
                Ok(attachment) => {
                    let detached = client.detach(attachment.channel);
                    lock(&self.shared).retire(attachment.channel);
                    match detached {
                        Ok(()) => {}
                        Err(ClientError::Server(error))
                            if error.code == ErrorCode::UnknownChannel => {}
                        Err(error) => return Err(error.to_string()),
                    }
                    if attachment.process.is_none_or(|process| !process.is_shell) {
                        lock(&self.shared).confirm = Some(id);
                        return Ok(());
                    }
                }
                Err(ClientError::Server(error)) if error.code == ErrorCode::UnknownSession => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        self.action(Action::Close(id), client, catalog)
    }

    fn synchronize(&mut self, client: &Client, catalog: &CatalogPage) -> Result {
        self.store_mut()?.ready()?;
        self.sync_references(client)?;
        while let Some(discard) = self.store_mut()?.state.discards.first().copied() {
            match discard {
                Discard::Session(session) => client
                    .close_session(session, self.store_mut()?.state.close_operations[&session]),
                Discard::Creation(operation) => client.cancel_creation(operation),
            }
            .map_err(|error| error.to_string())?;
            self.store_mut()?.change(|state| {
                state.discards.retain(|pending| *pending != discard);
                Ok(())
            })?;
        }
        let viewport = *lock(&self.viewport);
        self.create_pending(client, viewport)?;
        self.close_ended_sessions(catalog)?;
        self.sync_references(client)?;
        let state = self.store_mut()?.state.clone();
        let regions = crate::render::regions(&state, viewport);
        let desired: BTreeMap<_, _> = regions
            .iter()
            .filter_map(|(id, rect)| {
                let size = crate::render::terminal_size(*rect)?;
                Some((*id, size))
            })
            .collect();
        let obsolete: Vec<_> = lock(&self.shared)
            .views
            .iter()
            .filter(|(id, _)| !desired.contains_key(id))
            .map(|(id, view)| (*id, view.channel))
            .collect();
        for (id, channel) in obsolete {
            if let Some(channel) = channel {
                let focused = {
                    let mut shared = lock(&self.shared);
                    let focused = shared.focus == Some(channel);
                    if focused {
                        shared.focus = None;
                    }
                    focused
                };
                if focused {
                    self.input
                        .send(client.clone(), channel, b"\x1b[O".to_vec())?;
                }
                self.input.flush()?;
                match client.detach(channel) {
                    Ok(()) => {}
                    Err(ClientError::Server(error)) if error.code == ErrorCode::UnknownChannel => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
            lock(&self.shared).views.remove(&id);
        }
        for (id, size) in desired {
            if self.stop.load(Ordering::Acquire) {
                break;
            }
            self.synchronize_pane(client, catalog, id, size)?;
        }
        self.publish(catalog);
        Ok(())
    }

    fn sync_references(&mut self, client: &Client) -> Result {
        let state = &self.store_mut()?.state;
        let owner = state.layout_id.ok_or("TUI layout identity is missing")?;
        let revision = state.reference_revision;
        let references = state.session_references();
        if self.references.as_ref() != Some(&(revision, references.clone())) {
            if let Err(error) =
                client.sync_layout_references(Some(owner), revision, references.clone())
            {
                client.disconnect();
                return Err(error.to_string());
            }
            self.references = Some((revision, references));
        }
        Ok(())
    }

    fn create_pending(&mut self, client: &Client, viewport: Rect) -> Result {
        let state = self.store_mut()?.state.clone();
        let regions = crate::render::regions(&state, viewport);
        for (project, layout) in state.projects {
            for tab in layout.tabs {
                for (id, mut pane) in tab.panes {
                    if self.stop.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    if pane.creation.is_none() || pane.error.is_some() {
                        continue;
                    }
                    let size = regions
                        .iter()
                        .find(|(pane, _)| *pane == id)
                        .and_then(|(_, rect)| crate::render::terminal_size(*rect))
                        .unwrap_or(Size { cols: 80, rows: 24 });
                    self.create_pane(client, project, id, size, &mut pane)?;
                }
            }
        }
        Ok(())
    }

    fn synchronize_pane(
        &mut self,
        client: &Client,
        catalog: &CatalogPage,
        id: PaneId,
        size: Size,
    ) -> Result {
        let Some(pane) = self.store_mut()?.state.pane_mut(id).cloned() else {
            return Ok(());
        };
        if pane.error.is_some() || pane.creation.is_some() {
            return Ok(());
        }
        let Some(session) = pane.session else {
            return Ok(());
        };
        if Some(session) == hosting_session(catalog.server) {
            return Ok(());
        }
        let existing = lock(&self.shared)
            .views
            .get(&id)
            .map(|view| (view.ended, view.channel, view.viewport));
        if let Some((ended, channel, viewport)) = existing {
            if ended {
                return Ok(());
            }
            if let Some(channel) = channel {
                if viewport != size {
                    if let Some(view) = lock(&self.shared).views.get_mut(&id) {
                        view.viewport = size;
                        view.grid.resize(size);
                    }
                    client
                        .resize(channel, size)
                        .map_err(|error| error.to_string())?;
                }
                return Ok(());
            }
        }
        let mut view = match client.attach(session, size) {
            Ok(attachment) => View::attached(session, size, attachment),
            Err(ClientError::Server(error)) if error.code == ErrorCode::UnknownSession => {
                lock(&self.shared).session_ended(session);
                self.close_ended_sessions(catalog)?;
                return Ok(());
            }
            Err(error) => return Err(error.to_string()),
        };
        view.grid.graphics = muxy_protocol::Graphics::default();
        let ack = lock(&self.shared).insert(id, view);
        if let Some((channel, seq)) = ack {
            client
                .ack(channel, seq)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn create_pane(
        &mut self,
        client: &Client,
        project: ProjectId,
        id: PaneId,
        size: Size,
        pane: &mut crate::state::Pane,
    ) -> Result<bool> {
        if let Some(operation) = pane.creation {
            let directory = PathBuf::from(OsString::from_vec(pane.directory.0.clone()));
            match client.create_project_session(project, operation, &directory, size) {
                Ok(info) => {
                    self.store_mut()?.change(|state| {
                        if let Some(pane) = state.pane_mut(id)
                            && pane.creation == Some(operation)
                        {
                            pane.session = Some(info.id);
                            pane.creation = None;
                        }
                        Ok(())
                    })?;
                    pane.session = Some(info.id);
                }
                Err(ClientError::Server(error)) if error.code != ErrorCode::PersistenceFailed => {
                    self.store_mut()?.change(|state| {
                        if let Some(pane) = state.pane_mut(id)
                            && pane.creation == Some(operation)
                        {
                            pane.creation = None;
                            pane.error = Some(error.message.chars().take(512).collect());
                        }
                        Ok(())
                    })?;
                    return Ok(false);
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(true)
    }

    fn list_sessions(&self, client: &Client, catalog: &CatalogPage) -> Result {
        let project = self
            .store
            .as_ref()
            .ok_or("TUI state is not ready")?
            .state
            .active;
        let references = self
            .store
            .as_ref()
            .ok_or("TUI state is not ready")?
            .state
            .session_references();
        let mut page = client
            .available_project_sessions(project)
            .map_err(|error| error.to_string())?;
        page.sessions.retain(|session| {
            Some(session.info.id) != hosting_session(catalog.server)
                && !references.contains(&session.info.id)
        });
        let mut shared = lock(&self.shared);
        shared.sessions = page.sessions;
        shared.sessions_project = Some(project);
        Ok(())
    }

    fn store_mut(&mut self) -> Result<&mut Store> {
        self.store
            .as_mut()
            .ok_or_else(|| "TUI state is not ready".into())
    }
    fn close_ended_sessions(&mut self, catalog: &CatalogPage) -> Result {
        let ended = lock(&self.shared).ended.clone();
        if ended.is_empty() {
            return Ok(());
        }
        let closed = self
            .store_mut()?
            .change(|state| Ok(state.close_ended_sessions(&ended)))?;
        lock(&self.shared)
            .ended
            .retain(|session| !closed.contains(session));
        self.publish(catalog);
        Ok(())
    }
    fn message(&self, text: &str) {
        lock(&self.shared).message = text.into();
    }
    fn publish(&self, catalog: &CatalogPage) {
        let mut shared = lock(&self.shared);
        shared.state = self.store.as_ref().map(|store| store.state.clone());
        let ended = shared.ended.clone();
        if let Some(state) = &mut shared.state {
            state.close_sessions(&ended);
        }
        let references = shared
            .state
            .as_ref()
            .map(crate::state::State::session_references)
            .unwrap_or_default();
        shared
            .sessions
            .retain(|session| !references.contains(&session.info.id));
        if shared
            .state
            .as_ref()
            .is_none_or(|state| shared.sessions_project != Some(state.active))
        {
            shared.sessions.clear();
        }
        shared.catalog = Some(catalog.clone());
    }
}

fn send_input(shared: &Mutex<Shared>, writer: &InputWriter, input: Input) -> Result {
    let shared = lock(shared);
    let Some(view) = shared
        .state
        .as_ref()
        .and_then(|state| state.tab())
        .and_then(|tab| shared.views.get(&tab.focus))
        .filter(|view| !view.ended)
    else {
        return Ok(());
    };
    let Some((client, channel)) = shared.client.clone().zip(view.channel) else {
        return Ok(());
    };
    let modes = view.grid.modes;
    drop(shared);
    let bytes = input.encode(modes);
    if !bytes.is_empty() {
        writer.send(client, channel, bytes)?;
    }
    Ok(())
}

pub(crate) fn hosting_session(server: ServerIdentity) -> Option<SessionId> {
    let inherited: ServerIdentity = std::env::var("MUXY_SERVER_ID").ok()?.parse().ok()?;
    (inherited == server).then_some(())?;
    SessionId::new(std::env::var("MUXY_SESSION_ID").ok()?.parse().ok()?)
}
