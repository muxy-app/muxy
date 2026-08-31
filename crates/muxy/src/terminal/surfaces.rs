use crate::terminal::Backend;
use crate::terminal::idle::{
    IdleActivity, IdleMaterialization, InputDisposition, TerminalIdleCoordinator,
};
use gpui::{AnyElement, App, Window};
use muxy_core::workspace::{CloseMode, TabId, TabKind};
use muxy_core::workspace_store::WorkspaceStore;
use muxy_terminal::backend::{LaunchCommand, PointerInput, SurfaceAction, TerminalSurfaceHandle};
use muxy_terminal::confirmation::{ConfirmationId, ConfirmationKind};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

pub trait AppSurfaceHandle: TerminalSurfaceHandle {
    fn element(&self, visible: bool) -> AnyElement;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneLaunchContext {
    pane_id: String,
    project_id: String,
    worktree_id: String,
    socket_path: String,
}

impl PaneLaunchContext {
    pub(crate) fn new(
        pane_id: impl Into<String>,
        project_id: impl Into<String>,
        worktree_id: impl Into<String>,
        socket_path: impl Into<String>,
    ) -> Self {
        Self {
            pane_id: pane_id.into(),
            project_id: project_id.into(),
            worktree_id: worktree_id.into(),
            socket_path: socket_path.into(),
        }
    }

    pub fn environment(&self) -> [(&'static str, &str); 4] {
        [
            ("MUXY_PANE_ID", &self.pane_id),
            ("MUXY_PROJECT_ID", &self.project_id),
            ("MUXY_WORKTREE_ID", &self.worktree_id),
            ("MUXY_SOCKET_PATH", &self.socket_path),
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneLaunchContext {
    working_directory: PathBuf,
    socket_path: String,
}

impl StandaloneLaunchContext {
    pub fn new(working_directory: PathBuf, socket_path: &Path) -> Self {
        Self {
            working_directory,
            socket_path: socket_path.to_string_lossy().into_owned(),
        }
    }

    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }
}

pub struct StandaloneTerminal {
    backend: Backend,
}

impl Default for StandaloneTerminal {
    fn default() -> Self {
        Self::new()
    }
}

impl StandaloneTerminal {
    pub fn new() -> Self {
        Self {
            backend: Backend::new(),
        }
    }

    pub fn attach(
        &mut self,
        mode: muxy_core::environment::BuildMode,
        socket_path: &Path,
        window: &mut Window,
    ) -> Result<(), String> {
        self.backend.attach_standalone(mode, socket_path, window)
    }

    pub fn spawn(
        &mut self,
        context: &StandaloneLaunchContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<Box<dyn AppSurfaceHandle>, String> {
        self.backend.spawn_standalone(context, window, cx)
    }

    #[cfg(target_os = "macos")]
    pub fn wakeups(&self) -> Option<crate::terminal::TerminalWakeups> {
        self.backend
            .wakeup_receiver()
            .map(crate::terminal::wrap_wakeups)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn wakeups(&self) -> Option<crate::terminal::TerminalWakeups> {
        None
    }

    #[cfg(target_os = "macos")]
    pub fn events(&self) -> Option<crate::terminal::TerminalEvents> {
        self.backend
            .event_receiver()
            .map(crate::terminal::wrap_events)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn events(&self) -> Option<crate::terminal::TerminalEvents> {
        None
    }

    #[cfg(target_os = "macos")]
    pub fn shortcuts(&self) -> Option<async_channel::Receiver<()>> {
        Some(self.backend.standalone_shortcut_receiver())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn shortcuts(&self) -> Option<async_channel::Receiver<()>> {
        None
    }

    #[cfg(target_os = "macos")]
    pub fn route(
        &mut self,
        event: crate::terminal::TerminalEvent,
        cx: &mut App,
    ) -> Option<crate::terminal::SurfaceSignal> {
        match self.backend.route(crate::terminal::unwrap_event(event), cx) {
            Some(crate::terminal::RoutedTerminalEvent::Standalone(signal)) => Some(signal),
            Some(crate::terminal::RoutedTerminalEvent::Workspace(_, _)) | None => None,
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn route(
        &mut self,
        _event: crate::terminal::TerminalEvent,
        _cx: &mut App,
    ) -> Option<crate::terminal::SurfaceSignal> {
        None
    }

    pub fn tick(&self) {
        self.backend.tick();
    }

    pub fn reload_config(&mut self) {
        self.backend.reload_config();
    }

    pub fn set_backdrop(&self, backdrop: gpui::Rgba) {
        self.backend.set_backdrop(backdrop);
    }

    pub fn set_color_scheme(&mut self, scheme: crate::terminal::TerminalColorScheme) {
        self.backend.set_color_scheme(scheme);
    }

    pub fn set_window_active(&self, active: bool) {
        self.backend.set_window_active(active);
    }
}

pub struct PersistentReconciliation<'a> {
    pub visible: &'a [TabId],
    pub persistent_tabs: &'a HashSet<TabId>,
    pub session_identities: &'a HashMap<TabId, String>,
    pub attachments: &'a HashMap<TabId, String>,
    pub unavailable: HashMap<TabId, String>,
    pub retryable: HashSet<TabId>,
}

pub struct TerminalSurfaces {
    backend: Backend,
    handles: HashMap<TabId, Box<dyn AppSurfaceHandle>>,
    pending_cwd: HashMap<TabId, PathBuf>,
    pending_command: HashMap<TabId, LaunchCommand>,
    materialization_requested: HashSet<TabId>,
    session_attachments: HashMap<TabId, String>,
    session_unavailable: HashMap<TabId, String>,
    session_retryable: HashSet<TabId>,
    socket_path: Option<String>,
    pointer_tab: Option<TabId>,
    pub(crate) input_queue_generation: u64,
    pub(crate) input_queues: HashMap<TabId, crate::terminal::input_queue::PaneInputQueue>,
    pub(crate) pasteboard_owner: Option<crate::terminal::input_queue::PasteboardInputId>,
    pub(crate) pasteboard_waiting: VecDeque<crate::terminal::input_queue::PasteboardInputId>,
    staged_input_bytes: RefCell<Option<HashMap<TabId, Vec<Vec<u8>>>>>,
    staged_image_failure: RefCell<Option<TabId>>,
    idle: RefCell<TerminalIdleCoordinator>,
    idle_started_at: std::time::Instant,
}

fn captures_staged_input_bytes() -> bool {
    muxy_core::prefs::is_test_process()
        && matches!(
            std::env::var("MUXY_TEST_P7_COMPOSER_CASE").ok().as_deref(),
            Some("phase-5" | "phase-6" | "phase-7")
        )
        && std::env::var_os("MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY").is_some()
}

impl Default for TerminalSurfaces {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalSurfaces {
    pub fn new() -> Self {
        Self {
            backend: Backend::new(),
            handles: HashMap::new(),
            pending_cwd: HashMap::new(),
            pending_command: HashMap::new(),
            materialization_requested: HashSet::new(),
            session_attachments: HashMap::new(),
            session_unavailable: HashMap::new(),
            session_retryable: HashSet::new(),
            socket_path: None,
            pointer_tab: None,
            input_queue_generation: 0,
            input_queues: HashMap::new(),
            pasteboard_owner: None,
            pasteboard_waiting: VecDeque::new(),
            staged_input_bytes: RefCell::new(captures_staged_input_bytes().then(HashMap::new)),
            staged_image_failure: RefCell::new(None),
            idle: RefCell::new(TerminalIdleCoordinator::new()),
            idle_started_at: std::time::Instant::now(),
        }
    }

    pub fn with_socket_path(socket_path: &Path) -> Self {
        let mut surfaces = Self::new();
        surfaces.socket_path = Some(socket_path.to_string_lossy().into_owned());
        surfaces
    }

    pub fn backend_mut(&mut self) -> &mut Backend {
        &mut self.backend
    }

    #[cfg(target_os = "macos")]
    pub fn wakeups(&self) -> Option<crate::terminal::TerminalWakeups> {
        self.backend
            .wakeup_receiver()
            .map(crate::terminal::wrap_wakeups)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn wakeups(&self) -> Option<crate::terminal::TerminalWakeups> {
        None
    }

    #[cfg(target_os = "macos")]
    pub fn events(&self) -> Option<crate::terminal::TerminalEvents> {
        self.backend
            .event_receiver()
            .map(crate::terminal::wrap_events)
    }

    #[cfg(not(target_os = "macos"))]
    pub fn events(&self) -> Option<crate::terminal::TerminalEvents> {
        None
    }

    #[cfg(target_os = "macos")]
    pub fn navigation_events(
        &self,
    ) -> Option<async_channel::Receiver<muxy_core::navigation::Direction>> {
        Some(self.backend.navigation_event_receiver())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn navigation_events(
        &self,
    ) -> Option<async_channel::Receiver<muxy_core::navigation::Direction>> {
        None
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn external_drop_events(
        &self,
    ) -> Option<
        async_channel::Receiver<(
            crate::terminal::SurfaceIdentity,
            muxy_terminal::backend::ExternalDrop,
        )>,
    > {
        Some(self.backend.external_drop_receiver())
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn external_drop_events(
        &self,
    ) -> Option<
        async_channel::Receiver<(
            crate::terminal::SurfaceIdentity,
            muxy_terminal::backend::ExternalDrop,
        )>,
    > {
        None
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn inject_staged_external_drop(
        &self,
        tab_id: &str,
        dropped: muxy_terminal::backend::ExternalDrop,
    ) -> bool {
        self.backend.inject_staged_external_drop(
            crate::terminal::SurfaceIdentity::Workspace(tab_id.to_owned()),
            dropped,
        )
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn inject_staged_external_drop(
        &self,
        _tab_id: &str,
        _dropped: muxy_terminal::backend::ExternalDrop,
    ) -> bool {
        false
    }

    #[cfg(target_os = "macos")]
    pub fn route(
        &mut self,
        event: crate::terminal::TerminalEvent,
        cx: &mut App,
    ) -> Option<(TabId, crate::terminal::SurfaceSignal)> {
        match self.backend.route(crate::terminal::unwrap_event(event), cx) {
            Some(crate::terminal::RoutedTerminalEvent::Workspace(tab_id, signal)) => {
                Some((tab_id, signal))
            }
            Some(crate::terminal::RoutedTerminalEvent::Standalone(_)) | None => None,
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn route(
        &mut self,
        _event: crate::terminal::TerminalEvent,
        _cx: &mut App,
    ) -> Option<(TabId, crate::terminal::SurfaceSignal)> {
        None
    }

    pub fn tick(&self) {
        self.backend.tick();
    }

    pub fn apply_idle_settings(&self, enabled: bool, timeout_seconds: u64) {
        self.idle
            .borrow_mut()
            .set_settings(enabled, timeout_seconds);
    }

    pub fn poll_idle(&mut self, store: &WorkspaceStore) -> Vec<TabId> {
        let now = self.idle_now();
        let facts = self
            .handles
            .iter()
            .map(|(tab_id, handle)| {
                (
                    tab_id.clone(),
                    handle.lifecycle_facts(),
                    handle
                        .metadata()
                        .working_directory
                        .as_ref()
                        .map(PathBuf::from),
                )
            })
            .collect::<Vec<_>>();
        let requests = facts
            .into_iter()
            .filter_map(|(tab_id, facts, working_directory)| {
                let mut idle = self.idle.borrow_mut();
                idle.update_working_directory(&tab_id, working_directory);
                idle.observe_lifecycle(&tab_id, facts, now)
            })
            .collect::<Vec<_>>();
        let mut slept = Vec::new();
        for request in requests {
            if self.handles.remove(&request.tab_id).is_none() {
                continue;
            }
            if muxy_core::prefs::is_test_process()
                && std::env::var_os("MUXY_TEST_APPLICATION_SUPPORT_DIRECTORY").is_some()
            {
                eprintln!("P8_PHASE5_SLEEP {}", request.tab_id);
            }
            if !request.persistent {
                let directory = request
                    .working_directory
                    .or_else(|| self.launch_directory(store, &request.tab_id));
                if let Some(directory) = directory {
                    self.pending_cwd.insert(request.tab_id.clone(), directory);
                }
                self.pending_command.remove(&request.tab_id);
            }
            slept.push(request.tab_id);
        }
        slept
    }

    pub fn idle_reconcile_requested(&self) -> bool {
        !self.idle.borrow().wake_requested_tabs().is_empty()
    }

    pub(crate) fn idle_now(&self) -> u64 {
        u64::try_from(self.idle_started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    pub(crate) fn set_idle_input_transaction(&self, tab_id: &str, active: bool) {
        self.idle
            .borrow_mut()
            .set_input_transaction_active(tab_id, active, self.idle_now());
    }

    pub fn record_grid_resize(&self, tab_id: &str) {
        let Some((columns, rows)) = self.handle(tab_id).and_then(|handle| handle.grid_size())
        else {
            return;
        };
        let now = self.idle_now();
        let mut idle = self.idle.borrow_mut();
        idle.set_resize_active(tab_id, true, now);
        let _ = idle.queue_resize(tab_id, columns, rows, now);
        idle.set_resize_active(tab_id, false, now);
    }

    pub fn set_window_active(&self, active: bool) {
        self.backend.set_window_active(active);
    }

    pub fn set_overlay_active(&self, active: bool) {
        self.backend.set_overlay_active(active);
    }

    pub fn queue_launch_directory(&mut self, tab_id: impl Into<TabId>, directory: PathBuf) {
        self.pending_cwd.insert(tab_id.into(), directory);
    }

    pub fn queue_launch_command(&mut self, tab_id: impl Into<TabId>, command: LaunchCommand) {
        self.pending_command.insert(tab_id.into(), command);
    }

    pub fn pending_session_launches(
        &self,
        store: &WorkspaceStore,
    ) -> HashMap<TabId, crate::sessions::PendingSessionLaunch> {
        let mut tab_ids = self.pending_cwd.keys().cloned().collect::<HashSet<_>>();
        tab_ids.extend(self.pending_command.keys().cloned());
        tab_ids
            .into_iter()
            .filter_map(|tab_id| {
                let directory = self.launch_directory(store, &tab_id)?;
                Some((
                    tab_id.clone(),
                    crate::sessions::PendingSessionLaunch {
                        directory,
                        command: self.pending_command.get(&tab_id).cloned(),
                    },
                ))
            })
            .collect()
    }

    pub fn finish_session_reconciliation(&mut self, store: &WorkspaceStore) {
        let linked = store
            .states()
            .iter()
            .flat_map(|workspace| workspace.root.iter().flat_map(|root| root.tabs()))
            .filter(|tab| tab.session_id.is_some())
            .map(|tab| tab.id.clone())
            .collect::<Vec<_>>();
        for tab_id in linked {
            self.pending_cwd.remove(&tab_id);
            self.pending_command.remove(&tab_id);
            self.materialization_requested.remove(&tab_id);
        }
    }

    pub fn unavailable_reason(&self, tab_id: &str) -> Option<&str> {
        self.session_unavailable.get(tab_id).map(String::as_str)
    }

    pub fn attachment_retry_available(&self, tab_id: &str) -> bool {
        self.session_retryable.contains(tab_id)
    }

    pub fn start_new_available(&self, tab_id: &str) -> bool {
        matches!(
            self.unavailable_reason(tab_id),
            Some("Background session is missing" | "Background session has ended")
        )
    }

    pub fn take_session_attachment(&mut self, tab_id: &str) -> bool {
        let attached = self.session_attachments.remove(tab_id).is_some();
        if attached {
            self.handles.remove(tab_id);
        }
        attached
    }

    pub fn set_shortcut_combos(&mut self, combos: Vec<muxy_core::shortcuts::KeyCombo>) {
        self.backend.set_shortcut_combos(combos);
    }

    pub fn handle(&self, tab_id: &str) -> Option<&dyn AppSurfaceHandle> {
        self.handles.get(tab_id).map(Box::as_ref)
    }

    pub fn request_materialization(&mut self, store: &WorkspaceStore, tab_id: &str) -> bool {
        if self.handles.contains_key(tab_id) {
            return true;
        }
        if self.launch_context(store, tab_id).is_none() {
            return false;
        }
        self.materialization_requested.insert(tab_id.to_owned());
        true
    }

    fn begin_idle_materialization(
        &self,
        tab_id: &str,
        backing_identity: Option<String>,
        persistent: bool,
    ) {
        self.idle.borrow_mut().begin_materialization(
            tab_id,
            backing_identity,
            persistent,
            self.idle_now(),
        );
    }

    fn install_handle(
        &mut self,
        tab_id: TabId,
        handle: Box<dyn AppSurfaceHandle>,
        persistent: bool,
        backing_identity: Option<String>,
        working_directory: Option<PathBuf>,
    ) {
        let facts = handle.lifecycle_facts();
        let now_milliseconds = self.idle_now();
        let operations = self.idle.borrow_mut().take_wake_operations(&tab_id);
        let mut operations = operations.into_iter();
        while let Some(operation) = operations.next() {
            let applied = match &operation {
                muxy_terminal::offline::state::WakeOperation::Input(bytes) => {
                    handle.send_bytes(bytes)
                }
                muxy_terminal::offline::state::WakeOperation::Resize { columns, rows } => {
                    handle.resize_grid(*columns, *rows)
                }
            };
            if !applied {
                let pending = std::iter::once(operation).chain(operations).collect();
                self.idle
                    .borrow_mut()
                    .restore_wake_operations(&tab_id, pending)
                    .expect("drained wake operations must fit when restored");
                return;
            }
        }
        let remaining = self.idle.borrow_mut().materialized(
            &tab_id,
            IdleMaterialization {
                backing_identity,
                persistent,
                surface_identity: facts.surface_identity,
                working_directory,
                host_activity_generation: facts.activity_generation,
                now_milliseconds,
            },
        );
        debug_assert!(remaining.is_empty());
        self.handles.insert(tab_id, handle);
    }

    pub fn has_native_scrollbar(&self, tab_id: &str) -> bool {
        self.handle(tab_id)
            .is_some_and(|handle| handle.has_native_scrollbar())
    }

    pub fn send_text(&self, tab_id: &str, text: &str) -> bool {
        let disposition =
            self.idle
                .borrow_mut()
                .queue_input(tab_id, text.as_bytes(), self.idle_now());
        match disposition {
            Ok(InputDisposition::QueuedForWake) => true,
            Ok(InputDisposition::SendNow) => self
                .handle(tab_id)
                .is_some_and(|handle| handle.send_text(text)),
            Err(_) => false,
        }
    }

    pub fn send_bytes(&self, tab_id: &str, bytes: &[u8]) -> bool {
        if bytes == muxy_terminal::input::PASTE_SHORTCUT
            && self.staged_input_bytes.borrow().is_some()
            && self.staged_image_failure.borrow().as_deref() == Some(tab_id)
        {
            self.staged_image_failure.borrow_mut().take();
            return false;
        }
        let disposition = self
            .idle
            .borrow_mut()
            .queue_input(tab_id, bytes, self.idle_now());
        let sent = match disposition {
            Ok(InputDisposition::QueuedForWake) => true,
            Ok(InputDisposition::SendNow) => self
                .handle(tab_id)
                .is_some_and(|handle| handle.send_bytes(bytes)),
            Err(_) => false,
        };
        if sent && let Some(captured) = self.staged_input_bytes.borrow_mut().as_mut() {
            captured
                .entry(tab_id.to_owned())
                .or_default()
                .push(bytes.to_vec());
        }
        sent
    }

    pub(crate) fn reset_staged_input_bytes(&self) {
        if let Some(captured) = self.staged_input_bytes.borrow_mut().as_mut() {
            captured.clear();
        }
    }

    pub(crate) fn arm_staged_image_failure(&self, tab_id: &str) {
        if self.staged_input_bytes.borrow().is_some() {
            self.staged_image_failure.replace(Some(tab_id.to_owned()));
        }
    }

    pub(crate) fn take_staged_input_bytes(&self, tab_id: &str) -> Vec<Vec<u8>> {
        self.staged_input_bytes
            .borrow_mut()
            .as_mut()
            .and_then(|captured| captured.remove(tab_id))
            .unwrap_or_default()
    }

    pub fn read_screen_text(&self, tab_id: &str, last_lines: usize) -> Option<String> {
        self.handle(tab_id)?.read_screen_text(last_lines)
    }

    pub fn panes_matching_foreground_pid(&self, pid: u64) -> Vec<TabId> {
        let mut panes = self
            .handles
            .iter()
            .filter(|(_, handle)| handle.foreground_pid() == Some(pid))
            .map(|(tab_id, _)| tab_id.clone())
            .collect::<Vec<_>>();
        panes.sort_by_key(|pane_id| pane_id.to_ascii_uppercase());
        panes
    }

    pub fn active_confirmation(&self) -> Option<(TabId, ConfirmationId, ConfirmationKind)> {
        self.backend.active_confirmation()
    }

    pub fn perform(&mut self, tab_id: &str, action: SurfaceAction) -> bool {
        self.idle
            .borrow_mut()
            .record_activity(tab_id, IdleActivity::Action, self.idle_now());
        let Some(handle) = self.handles.get_mut(tab_id) else {
            return false;
        };
        handle.perform(action);
        handle.refresh()
    }

    pub fn forward_pointer(&mut self, tab_id: &str, input: PointerInput) -> bool {
        self.idle
            .borrow_mut()
            .record_activity(tab_id, IdleActivity::Input, self.idle_now());
        if matches!(input, PointerInput::Moved { .. }) {
            self.set_pointer_tab(Some(tab_id));
        }
        self.handles
            .get(tab_id)
            .is_some_and(|handle| handle.forward_pointer(input))
    }

    pub fn pointer_tab(&self) -> Option<&str> {
        self.pointer_tab.as_deref()
    }

    pub fn clear_pointer_tab(&mut self) {
        self.set_pointer_tab(None);
    }

    fn set_pointer_tab(&mut self, tab_id: Option<&str>) {
        if self.pointer_tab.as_deref() == tab_id {
            return;
        }
        if let Some(previous) = self.pointer_tab.take()
            && let Some(handle) = self.handles.get(&previous)
        {
            handle.set_pointer_inside(false);
        }
        let Some(tab_id) = tab_id else {
            return;
        };
        if let Some(handle) = self.handles.get(tab_id) {
            handle.set_pointer_inside(true);
            self.pointer_tab = Some(tab_id.to_owned());
        }
    }

    pub fn apply(&mut self, tab_id: &str, signal: crate::terminal::SurfaceSignal) -> bool {
        self.idle
            .borrow_mut()
            .record_activity(tab_id, IdleActivity::Output, self.idle_now());
        self.handles
            .get_mut(tab_id)
            .is_some_and(|handle| handle.apply(signal))
    }

    pub fn element(&self, tab_id: &str, visible: bool) -> Option<AnyElement> {
        self.handles
            .get(tab_id)
            .map(|handle| handle.element(visible))
    }

    pub fn set_focused_tab(&self, focused: Option<&str>) {
        self.idle.borrow_mut().set_focused(focused, self.idle_now());
        for (tab_id, handle) in &self.handles {
            handle.set_focused(Some(tab_id.as_str()) == focused);
        }
    }

    pub fn reconcile_persistent(
        &mut self,
        store: &WorkspaceStore,
        reconciliation: PersistentReconciliation<'_>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<TabId> {
        let PersistentReconciliation {
            visible,
            persistent_tabs,
            session_identities,
            attachments,
            unavailable,
            retryable,
        } = reconciliation;
        self.session_unavailable = unavailable;
        self.session_retryable = retryable;
        let stale = self
            .session_attachments
            .iter()
            .filter(|(tab_id, command)| attachments.get(*tab_id) != Some(*command))
            .map(|(tab_id, _)| tab_id.clone())
            .collect::<Vec<_>>();
        for tab_id in stale {
            self.session_attachments.remove(&tab_id);
            self.handles.remove(&tab_id);
        }
        let mut candidates = visible.to_vec();
        let mut requested = self
            .materialization_requested
            .iter()
            .filter(|tab_id| !persistent_tabs.contains(*tab_id) && !visible.contains(tab_id))
            .cloned()
            .collect::<Vec<_>>();
        requested.extend(
            self.idle
                .borrow()
                .wake_requested_tabs()
                .into_iter()
                .filter(|tab_id| !visible.contains(tab_id)),
        );
        requested.sort_by_key(|tab_id| tab_id.to_ascii_uppercase());
        requested.dedup();
        candidates.extend(requested);
        let mut failed = Vec::new();
        for tab_id in candidates {
            if self.handles.contains_key(&tab_id) {
                self.materialization_requested.remove(&tab_id);
                continue;
            }
            if self.session_unavailable.contains_key(&tab_id) {
                continue;
            }
            let Some(directory) = self.launch_directory(store, &tab_id) else {
                continue;
            };
            let Some(context) = self.launch_context(store, &tab_id) else {
                continue;
            };
            if persistent_tabs.contains(&tab_id) {
                let Some(command) = attachments.get(&tab_id) else {
                    continue;
                };
                let Some(session_identity) = session_identities.get(&tab_id) else {
                    continue;
                };
                self.begin_idle_materialization(&tab_id, Some(session_identity.clone()), true);
                match self.backend.spawn_attachment(
                    &tab_id,
                    directory.clone(),
                    command.clone(),
                    &context,
                    window,
                    cx,
                ) {
                    Some(handle) => {
                        self.materialization_requested.remove(&tab_id);
                        self.session_attachments
                            .insert(tab_id.clone(), command.clone());
                        self.install_handle(
                            tab_id,
                            handle,
                            true,
                            Some(session_identity.clone()),
                            Some(directory),
                        );
                    }
                    None => failed.push(tab_id),
                }
                continue;
            }
            let command = self.pending_command.get(&tab_id).cloned();
            self.begin_idle_materialization(&tab_id, None, false);
            if let Some(handle) =
                self.backend
                    .spawn(&tab_id, directory.clone(), command, &context, window, cx)
            {
                self.pending_cwd.remove(&tab_id);
                self.pending_command.remove(&tab_id);
                self.materialization_requested.remove(&tab_id);
                self.install_handle(tab_id, handle, false, None, Some(directory));
            }
        }
        self.occlude_hidden(visible);
        self.retain_known(store);
        failed
    }

    pub fn reconcile(
        &mut self,
        store: &WorkspaceStore,
        visible: &[TabId],
        window: &mut Window,
        cx: &mut App,
    ) {
        let candidates = self.materialization_candidates(visible);
        for tab_id in candidates {
            if self.handles.contains_key(&tab_id) {
                self.materialization_requested.remove(&tab_id);
                continue;
            }
            let Some(directory) = self.launch_directory(store, &tab_id) else {
                continue;
            };
            let Some(context) = self.launch_context(store, &tab_id) else {
                continue;
            };
            let command = self.pending_command.get(&tab_id).cloned();
            self.begin_idle_materialization(&tab_id, None, false);
            if let Some(handle) =
                self.backend
                    .spawn(&tab_id, directory.clone(), command, &context, window, cx)
            {
                self.pending_cwd.remove(&tab_id);
                self.pending_command.remove(&tab_id);
                self.materialization_requested.remove(&tab_id);
                self.install_handle(tab_id, handle, false, None, Some(directory));
            }
        }
        self.occlude_hidden(visible);
        self.retain_known(store);
    }

    pub fn handle_exit(&mut self, store: &mut WorkspaceStore, tab_id: &str) -> bool {
        let Some(state) = store
            .states_mut()
            .iter_mut()
            .find(|state| state.tab(tab_id).is_some())
        else {
            return false;
        };
        !state.close_tab(tab_id, CloseMode::Single).is_empty()
    }

    fn occlude_hidden(&mut self, visible: &[TabId]) {
        self.idle
            .borrow_mut()
            .sync_visibility(visible, self.idle_now());
        if self
            .pointer_tab
            .as_ref()
            .is_some_and(|tab_id| !visible.contains(tab_id))
        {
            self.set_pointer_tab(None);
        }
        for (tab_id, handle) in &self.handles {
            handle.set_occluded(!visible.contains(tab_id));
        }
    }

    fn retain_known(&mut self, store: &WorkspaceStore) {
        let removed_queues = self
            .input_queues
            .keys()
            .filter(|tab_id| !tab_is_known(store, tab_id))
            .cloned()
            .collect::<Vec<_>>();
        for tab_id in removed_queues {
            self.cancel_input_queue(&tab_id);
        }
        self.handles.retain(|tab_id, _| tab_is_known(store, tab_id));
        self.pending_cwd
            .retain(|tab_id, _| tab_is_known(store, tab_id));
        self.pending_command
            .retain(|tab_id, _| tab_is_known(store, tab_id));
        self.materialization_requested
            .retain(|tab_id| tab_is_known(store, tab_id));
        self.session_attachments
            .retain(|tab_id, _| tab_is_known(store, tab_id));
        self.session_unavailable
            .retain(|tab_id, _| tab_is_known(store, tab_id));
        self.session_retryable
            .retain(|tab_id| tab_is_known(store, tab_id));
        self.idle
            .borrow_mut()
            .remove_unknown(|tab_id| tab_is_known(store, tab_id));
        if self
            .pointer_tab
            .as_ref()
            .is_some_and(|tab_id| !self.handles.contains_key(tab_id))
        {
            self.pointer_tab = None;
        }
    }

    fn materialization_candidates(&self, visible: &[TabId]) -> Vec<TabId> {
        let mut candidates = visible.to_vec();
        let mut requested = self
            .materialization_requested
            .iter()
            .cloned()
            .chain(self.idle.borrow().wake_requested_tabs())
            .filter(|tab_id| !visible.contains(tab_id))
            .collect::<Vec<_>>();
        requested.sort_by_key(|tab_id| tab_id.to_ascii_uppercase());
        requested.dedup();
        candidates.extend(requested);
        candidates
    }

    fn launch_context(&self, store: &WorkspaceStore, tab_id: &str) -> Option<PaneLaunchContext> {
        let socket_path = self.socket_path.clone()?;
        let (state, tab) = store
            .states()
            .iter()
            .find_map(|state| state.tab(tab_id).map(|tab| (state, tab)))?;
        if tab.kind != TabKind::Terminal {
            return None;
        }
        Some(PaneLaunchContext::new(
            canonical_uuid(&tab.id)?,
            canonical_uuid(&state.project_id)?,
            canonical_uuid(state.worktree_id.as_deref()?)?,
            socket_path,
        ))
    }

    fn launch_directory(&self, store: &WorkspaceStore, tab_id: &str) -> Option<PathBuf> {
        if let Some(directory) = self.pending_cwd.get(tab_id) {
            return Some(directory.clone());
        }
        let tab = store
            .states()
            .iter()
            .find_map(|state| state.tab(tab_id))
            .filter(|tab| tab.kind == TabKind::Terminal)?;
        tab.project_path.as_ref().map(PathBuf::from)
    }
}

fn canonical_uuid(value: &str) -> Option<String> {
    (value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        }))
    .then(|| value.to_ascii_uppercase())
}

fn tab_is_known(store: &WorkspaceStore, tab_id: &str) -> bool {
    store
        .states()
        .iter()
        .any(|state| state.tab(tab_id).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use muxy_terminal::backend::{PointerInput, SurfaceAction, SurfaceMetadata, SurfaceSignal};
    use muxy_terminal::input::{
        TerminalInputError, TerminalInputStep, TerminalInputTransaction, bracketed_text_bytes,
        clear_input_bytes,
    };
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    #[derive(Default)]
    struct FakeInput {
        bytes: RefCell<Vec<Vec<u8>>>,
        transaction_states: RefCell<Vec<bool>>,
        cancellations: Cell<usize>,
        send_fails: Cell<bool>,
    }

    struct FakeHandle {
        occluded: Rc<Cell<bool>>,
        pointer_inside: Rc<Cell<bool>>,
        input: Rc<FakeInput>,
        metadata: SurfaceMetadata,
        foreground_pid: Option<u64>,
    }

    impl AppSurfaceHandle for FakeHandle {
        fn element(&self, _visible: bool) -> AnyElement {
            unreachable!("registry tests never render")
        }
    }

    impl TerminalSurfaceHandle for FakeHandle {
        fn set_focused(&self, _focused: bool) {}
        fn set_occluded(&self, occluded: bool) {
            self.occluded.set(occluded);
        }
        fn set_pointer_inside(&self, inside: bool) {
            self.pointer_inside.set(inside);
        }
        fn set_input_transaction_active(&self, active: bool) {
            self.input.transaction_states.borrow_mut().push(active);
        }
        fn cancel_input_transaction(&self) {
            self.input
                .cancellations
                .set(self.input.cancellations.get() + 1);
        }
        fn has_selection(&self) -> bool {
            false
        }
        fn send_bytes(&self, bytes: &[u8]) -> bool {
            self.input.bytes.borrow_mut().push(bytes.to_vec());
            !self.input.send_fails.get()
        }
        fn foreground_pid(&self) -> Option<u64> {
            self.foreground_pid
        }
        fn lifecycle_facts(&self) -> muxy_terminal::backend::TerminalLifecycleFacts {
            use muxy_terminal::offline::policy::{
                ForegroundState, ProcessSafety, TerminalSafetyFacts,
            };
            match self.foreground_pid {
                Some(surface_identity) => muxy_terminal::backend::TerminalLifecycleFacts {
                    surface_identity,
                    activity_generation: 1,
                    safety: TerminalSafetyFacts {
                        foreground: ForegroundState::Idle,
                        process_safety: ProcessSafety::SafeToLoseOrdinaryShell,
                        alternate_screen: false,
                    },
                },
                None => muxy_terminal::backend::TerminalLifecycleFacts::unknown(0, 0),
            }
        }
        fn metadata(&self) -> &SurfaceMetadata {
            &self.metadata
        }
        fn perform(&self, _action: SurfaceAction) -> bool {
            false
        }
        fn forward_pointer(&self, _input: PointerInput) -> bool {
            false
        }
        fn apply(&mut self, _signal: SurfaceSignal) -> bool {
            false
        }
        fn refresh(&mut self) -> bool {
            false
        }
        fn needs_confirm_close(&self) -> bool {
            false
        }
        fn request_close(&self) {}
    }

    fn store_with(projects: &[(&str, &str)]) -> (WorkspaceStore, Vec<TabId>) {
        let directory = std::env::temp_dir().join(format!("muxy-surfaces-{}", std::process::id()));
        let mut store = WorkspaceStore::load_from(directory.join("workspaces.json"));
        let mut tabs = Vec::new();
        for (id, path) in projects {
            let state = store.ensure_project(*id, *path);
            tabs.push(state.top_level_order[0].clone());
        }
        (store, tabs)
    }

    fn contextual_store() -> (WorkspaceStore, String, String, TabId) {
        let directory = tempfile::tempdir().unwrap();
        let mut store = WorkspaceStore::load_from(directory.path().join("workspaces.json"));
        let project_id = muxy_core::store::new_uuid();
        let worktree_id = muxy_core::store::new_uuid();
        let state = store.ensure_worktree(&project_id, &worktree_id, "/tmp/context");
        let tab_id = state.top_level_order[0].clone();
        (store, project_id, worktree_id, tab_id)
    }

    fn insert_fake(surfaces: &mut TerminalSurfaces, tab_id: &str) -> Rc<Cell<bool>> {
        insert_fake_pair(surfaces, tab_id).0
    }

    fn insert_fake_pair(
        surfaces: &mut TerminalSurfaces,
        tab_id: &str,
    ) -> (Rc<Cell<bool>>, Rc<Cell<bool>>) {
        let occluded = Rc::new(Cell::new(false));
        let pointer_inside = Rc::new(Cell::new(false));
        surfaces.handles.insert(
            tab_id.to_owned(),
            Box::new(FakeHandle {
                occluded: occluded.clone(),
                pointer_inside: pointer_inside.clone(),
                input: Rc::new(FakeInput::default()),
                metadata: SurfaceMetadata::default(),
                foreground_pid: None,
            }),
        );
        (occluded, pointer_inside)
    }

    fn insert_input_fake(surfaces: &mut TerminalSurfaces, tab_id: &str) -> Rc<FakeInput> {
        let input = Rc::new(FakeInput::default());
        surfaces.handles.insert(
            tab_id.to_owned(),
            Box::new(FakeHandle {
                occluded: Rc::new(Cell::new(false)),
                pointer_inside: Rc::new(Cell::new(false)),
                input: input.clone(),
                metadata: SurfaceMetadata::default(),
                foreground_pid: None,
            }),
        );
        input
    }

    #[test]
    fn terminal_idle_persistent_sleep_drops_only_renderer_and_retains_attachment() {
        let (store, _, _, tab_id) = contextual_store();
        let mut surfaces = TerminalSurfaces::with_socket_path(Path::new("/tmp/selected.socket"));
        surfaces.apply_idle_settings(true, 0);
        surfaces
            .session_attachments
            .insert(tab_id.clone(), "attach-command".to_owned());
        let handle = Box::new(FakeHandle {
            occluded: Rc::new(Cell::new(true)),
            pointer_inside: Rc::new(Cell::new(false)),
            input: Rc::new(FakeInput::default()),
            metadata: SurfaceMetadata::default(),
            foreground_pid: Some(7),
        });
        surfaces.begin_idle_materialization(&tab_id, Some("session-one".to_owned()), true);
        surfaces.install_handle(
            tab_id.clone(),
            handle,
            true,
            Some("session-one".to_owned()),
            Some(PathBuf::from("/tmp/runtime-cwd")),
        );

        assert_eq!(surfaces.poll_idle(&store), vec![tab_id.clone()]);
        assert!(!surfaces.handles.contains_key(&tab_id));
        assert_eq!(
            surfaces
                .session_attachments
                .get(&tab_id)
                .map(String::as_str),
            Some("attach-command")
        );
    }

    #[test]
    fn terminal_idle_ordinary_sleep_restores_runtime_cwd_without_startup_command() {
        let (store, _, _, tab_id) = contextual_store();
        let mut surfaces = TerminalSurfaces::with_socket_path(Path::new("/tmp/selected.socket"));
        surfaces.apply_idle_settings(true, 0);
        surfaces.pending_command.insert(
            tab_id.clone(),
            LaunchCommand {
                command: "printf startup".to_owned(),
                keeps_shell_open: true,
            },
        );
        let handle = Box::new(FakeHandle {
            occluded: Rc::new(Cell::new(true)),
            pointer_inside: Rc::new(Cell::new(false)),
            input: Rc::new(FakeInput::default()),
            metadata: SurfaceMetadata {
                working_directory: Some("/tmp/runtime-cwd".to_owned()),
                ..SurfaceMetadata::default()
            },
            foreground_pid: Some(7),
        });
        surfaces.begin_idle_materialization(&tab_id, None, false);
        surfaces.install_handle(
            tab_id.clone(),
            handle,
            false,
            None,
            Some(PathBuf::from("/tmp/launch-cwd")),
        );

        assert_eq!(surfaces.poll_idle(&store), vec![tab_id.clone()]);
        assert!(!surfaces.handles.contains_key(&tab_id));
        assert_eq!(
            surfaces.pending_cwd.get(&tab_id),
            Some(&PathBuf::from("/tmp/runtime-cwd"))
        );
        assert!(!surfaces.pending_command.contains_key(&tab_id));
    }

    #[test]
    fn pane_launch_context_uses_real_uppercase_ids_and_selected_socket() {
        let (store, project_id, worktree_id, tab_id) = contextual_store();
        let surfaces = TerminalSurfaces::with_socket_path(Path::new("/tmp/selected.socket"));
        let context = surfaces.launch_context(&store, &tab_id).unwrap();

        assert_eq!(
            context.environment(),
            [
                ("MUXY_PANE_ID", tab_id.as_str()),
                ("MUXY_PROJECT_ID", project_id.as_str()),
                ("MUXY_WORKTREE_ID", worktree_id.as_str()),
                ("MUXY_SOCKET_PATH", "/tmp/selected.socket"),
            ]
        );
    }

    #[test]
    fn hidden_materialization_is_scheduled_once_and_requires_context() {
        let (store, _, _, tab_id) = contextual_store();
        let mut surfaces = TerminalSurfaces::with_socket_path(Path::new("/tmp/selected.socket"));

        assert!(surfaces.request_materialization(&store, &tab_id));
        assert!(surfaces.request_materialization(&store, &tab_id));
        let candidates = surfaces.materialization_candidates(&[]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], tab_id);

        let (missing_context, tabs) = store_with(&[("not-a-uuid", "/tmp/missing")]);
        assert!(!surfaces.request_materialization(&missing_context, &tabs[0]));
    }

    #[test]
    fn pointer_entry_into_one_pane_releases_every_other_pane() {
        let mut surfaces = TerminalSurfaces::new();
        let (_, first) = insert_fake_pair(&mut surfaces, "a");
        let (_, second) = insert_fake_pair(&mut surfaces, "b");

        surfaces.set_pointer_tab(Some("a"));
        assert!(first.get());
        assert!(!second.get());

        surfaces.set_pointer_tab(Some("b"));
        assert!(!first.get());
        assert!(second.get());

        surfaces.clear_pointer_tab();
        assert!(!second.get());
        assert_eq!(surfaces.pointer_tab(), None);
    }

    #[test]
    fn pointer_tracking_forgets_a_destroyed_surface() {
        let (store, tabs) = store_with(&[("p1", "/tmp/p1")]);
        let mut surfaces = TerminalSurfaces::new();
        insert_fake(&mut surfaces, "orphan");
        surfaces.set_pointer_tab(Some("orphan"));
        assert_eq!(surfaces.pointer_tab(), Some("orphan"));

        surfaces.retain_known(&store);

        assert_eq!(surfaces.pointer_tab(), None);
        assert!(!tabs.is_empty());
    }

    #[test]
    fn a_tab_absent_from_every_workspace_is_destroyed() {
        let (store, tabs) = store_with(&[("p1", "/tmp/p1")]);
        let mut surfaces = TerminalSurfaces::new();
        insert_fake(&mut surfaces, &tabs[0]);
        insert_fake(&mut surfaces, "orphan");

        surfaces.retain_known(&store);

        assert!(surfaces.handles.contains_key(&tabs[0]));
        assert!(!surfaces.handles.contains_key("orphan"));
    }

    #[test]
    fn a_tab_in_a_non_active_workspace_is_occluded_not_destroyed() {
        let (store, tabs) = store_with(&[("p1", "/tmp/p1"), ("p2", "/tmp/p2")]);
        let mut surfaces = TerminalSurfaces::new();
        let (front, _) = insert_fake_pair(&mut surfaces, &tabs[0]);
        let (background, background_pointer) = insert_fake_pair(&mut surfaces, &tabs[1]);
        let visible = vec![tabs[0].clone()];
        surfaces.set_pointer_tab(Some(&tabs[1]));

        surfaces.occlude_hidden(&visible);
        surfaces.retain_known(&store);

        assert!(!front.get());
        assert!(background.get());
        assert!(!background_pointer.get());
        assert_eq!(surfaces.pointer_tab(), None);
        assert!(surfaces.handles.contains_key(&tabs[1]));
    }

    #[test]
    fn moving_a_tab_between_areas_does_not_recreate_its_surface() {
        let (store, tabs) = store_with(&[("p1", "/tmp/p1")]);
        let mut surfaces = TerminalSurfaces::new();
        insert_fake(&mut surfaces, &tabs[0]);
        let before = surfaces.handles.len();

        surfaces.occlude_hidden(&[tabs[0].clone()]);
        surfaces.retain_known(&store);

        assert_eq!(surfaces.handles.len(), before);
        assert!(surfaces.handles.contains_key(&tabs[0]));
    }

    #[test]
    fn queued_launch_state_is_retained_until_a_surface_spawns() {
        let (store, tabs) = store_with(&[("p1", "/tmp/p1")]);
        let mut surfaces = TerminalSurfaces::new();
        let command = LaunchCommand {
            command: "echo ready".to_owned(),
            keeps_shell_open: true,
        };
        surfaces.queue_launch_directory(tabs[0].clone(), PathBuf::from("/queued"));
        surfaces.queue_launch_command(tabs[0].clone(), command.clone());

        assert_eq!(
            surfaces.launch_directory(&store, &tabs[0]),
            Some(PathBuf::from("/queued"))
        );
        assert_eq!(
            surfaces.launch_directory(&store, &tabs[0]),
            Some(PathBuf::from("/queued"))
        );
        assert_eq!(surfaces.pending_command.get(&tabs[0]), Some(&command));

        surfaces.pending_cwd.remove(&tabs[0]);
        surfaces.pending_command.remove(&tabs[0]);
        assert_eq!(
            surfaces.launch_directory(&store, &tabs[0]),
            Some(PathBuf::from("/tmp/p1"))
        );
    }

    #[test]
    fn remove_worktree_reconciliation_drops_handles_and_all_pending_launch_state() {
        let (mut store, project_id, worktree_id, tab_id) = contextual_store();
        let mut surfaces = TerminalSurfaces::with_socket_path(Path::new("/tmp/selected.socket"));
        insert_fake(&mut surfaces, &tab_id);
        surfaces.queue_launch_directory(tab_id.clone(), PathBuf::from("/queued"));
        surfaces.queue_launch_command(
            tab_id.clone(),
            LaunchCommand {
                command: "echo pending".into(),
                keeps_shell_open: true,
            },
        );
        assert!(surfaces.request_materialization(&store, &tab_id));

        assert!(store.remove_worktree(&project_id, &worktree_id).is_some());
        surfaces.retain_known(&store);

        assert!(!surfaces.handles.contains_key(&tab_id));
        assert!(!surfaces.pending_cwd.contains_key(&tab_id));
        assert!(!surfaces.pending_command.contains_key(&tab_id));
        assert!(!surfaces.materialization_requested.contains(&tab_id));
    }

    #[test]
    fn session_attachment_exit_drops_only_the_proxy_surface() {
        let mut surfaces = TerminalSurfaces::new();
        insert_fake(&mut surfaces, "persistent-tab");
        surfaces
            .session_attachments
            .insert("persistent-tab".into(), "attach command".into());

        assert!(surfaces.take_session_attachment("persistent-tab"));
        assert!(!surfaces.handles.contains_key("persistent-tab"));
        assert!(!surfaces.session_attachments.contains_key("persistent-tab"));
        assert!(!surfaces.take_session_attachment("persistent-tab"));
    }

    #[test]
    fn handle_exit_closes_a_tab_in_a_non_active_workspace() {
        let (mut store, tabs) = store_with(&[("p1", "/tmp/p1"), ("p2", "/tmp/p2")]);
        let mut surfaces = TerminalSurfaces::new();

        assert!(surfaces.handle_exit(&mut store, &tabs[1]));

        assert!(!tab_is_known(&store, &tabs[1]));
        assert!(tab_is_known(&store, &tabs[0]));
    }

    #[test]
    fn pane_input_queue_serializes_exact_bytes_failures_and_idle_release() {
        let mut surfaces = TerminalSurfaces::new();
        let input = insert_input_fake(&mut surfaces, "pane");
        let first = TerminalInputTransaction::new(
            vec![
                TerminalInputStep::ClearInput { submitted_lines: 0 },
                TerminalInputStep::BracketedText("first".to_owned()),
            ],
            true,
        );
        let second = TerminalInputTransaction::new(
            vec![TerminalInputStep::BracketedText("second".to_owned())],
            false,
        );
        let (first_completion, first_worker) = surfaces.enqueue_input_transaction("pane", first);
        let generation = first_worker.unwrap();
        let (second_completion, second_worker) = surfaces.enqueue_input_transaction("pane", second);
        assert_eq!(second_worker, None);
        let active = surfaces
            .active_input_transaction("pane", generation)
            .unwrap();
        for step in &active.transaction.steps {
            surfaces
                .send_input_step("pane", generation, active.id, step)
                .unwrap();
        }
        surfaces
            .send_input_return("pane", generation, active.id)
            .unwrap();
        assert!(surfaces.complete_input_transaction("pane", generation, active.id, Ok(())));
        assert_eq!(first_completion.try_recv(), Ok(Ok(())));
        input.send_fails.set(true);
        let active = surfaces
            .active_input_transaction("pane", generation)
            .unwrap();
        let result =
            surfaces.send_input_step("pane", generation, active.id, &active.transaction.steps[0]);
        assert_eq!(result, Err(TerminalInputError::SendFailed));
        assert!(!surfaces.complete_input_transaction("pane", generation, active.id, result));
        assert_eq!(
            second_completion.try_recv(),
            Ok(Err(TerminalInputError::SendFailed))
        );
        assert!(!surfaces.input_queues.contains_key("pane"));
        assert_eq!(*input.transaction_states.borrow(), [true, false]);
        assert_eq!(
            *input.bytes.borrow(),
            [
                clear_input_bytes(0),
                bracketed_text_bytes("first"),
                b"\r".to_vec(),
                bracketed_text_bytes("second"),
            ]
        );
    }

    #[test]
    fn pane_input_queue_cancellation_rejects_stale_workers_and_restarts_cleanly() {
        let mut surfaces = TerminalSurfaces::new();
        let input = insert_input_fake(&mut surfaces, "pane");
        let transaction =
            TerminalInputTransaction::new(vec![TerminalInputStep::RawBytes(vec![1])], false);
        let (completion, worker) = surfaces.enqueue_input_transaction("pane", transaction.clone());
        let stale_generation = worker.unwrap();
        surfaces.cancel_input_queue("pane");
        assert_eq!(
            completion.try_recv(),
            Ok(Err(TerminalInputError::Cancelled))
        );
        assert_eq!(input.cancellations.get(), 1);
        let (_, worker) = surfaces.enqueue_input_transaction("pane", transaction);
        let current_generation = worker.unwrap();
        assert_ne!(stale_generation, current_generation);
        assert!(
            surfaces
                .active_input_transaction("pane", stale_generation)
                .is_none()
        );
        assert_eq!(
            surfaces.send_input_return("pane", stale_generation, 1),
            Err(TerminalInputError::Cancelled)
        );
        surfaces.cancel_input_queue("pane");
        assert_eq!(input.cancellations.get(), 2);
    }

    #[test]
    fn image_pasteboard_windows_are_fifo_across_panes() {
        let mut surfaces = TerminalSurfaces::new();
        insert_input_fake(&mut surfaces, "pane-a");
        insert_input_fake(&mut surfaces, "pane-b");
        let transaction =
            || TerminalInputTransaction::new(vec![TerminalInputStep::PastePng(vec![1])], false);
        let (_, first_worker) = surfaces.enqueue_input_transaction("pane-a", transaction());
        let (_, second_worker) = surfaces.enqueue_input_transaction("pane-b", transaction());
        let first_generation = first_worker.unwrap();
        let second_generation = second_worker.unwrap();
        let first = surfaces
            .active_input_transaction("pane-a", first_generation)
            .unwrap();
        let second = surfaces
            .active_input_transaction("pane-b", second_generation)
            .unwrap();
        assert_eq!(
            surfaces.begin_pasteboard_step("pane-a", first_generation, first.id),
            crate::terminal::input_queue::PasteboardStepState::Acquired
        );
        assert_eq!(
            surfaces.begin_pasteboard_step("pane-b", second_generation, second.id),
            crate::terminal::input_queue::PasteboardStepState::Waiting
        );
        assert_eq!(
            surfaces.begin_pasteboard_step("pane-b", second_generation, second.id),
            crate::terminal::input_queue::PasteboardStepState::Waiting
        );
        surfaces.finish_pasteboard_step("pane-a", first_generation, first.id);
        assert_eq!(
            surfaces.begin_pasteboard_step("pane-b", second_generation, second.id),
            crate::terminal::input_queue::PasteboardStepState::Acquired
        );
        surfaces.finish_pasteboard_step("pane-b", second_generation, second.id);
        assert!(surfaces.pasteboard_owner.is_none());
        assert!(surfaces.pasteboard_waiting.is_empty());
        surfaces.cancel_input_queue("pane-a");
        surfaces.cancel_input_queue("pane-b");
    }

    #[test]
    fn image_window_cancellation_defers_completion_and_native_release_until_restore() {
        let mut surfaces = TerminalSurfaces::new();
        let input = insert_input_fake(&mut surfaces, "pane");
        let transaction =
            TerminalInputTransaction::new(vec![TerminalInputStep::PastePng(vec![1, 2, 3])], false)
                .with_rollback_on_failure();
        let (completion, worker) = surfaces.enqueue_input_transaction("pane", transaction);
        let generation = worker.unwrap();
        let active = surfaces
            .active_input_transaction("pane", generation)
            .unwrap();
        assert_eq!(
            surfaces.begin_pasteboard_step("pane", generation, active.id),
            crate::terminal::input_queue::PasteboardStepState::Acquired
        );
        surfaces.cancel_input_queue("pane");
        assert!(completion.try_recv().is_err());
        assert_eq!(input.cancellations.get(), 0);
        assert!(surfaces.input_transaction_cancelled("pane", generation, active.id));
        surfaces.finish_pasteboard_step("pane", generation, active.id);
        assert!(!surfaces.complete_input_transaction(
            "pane",
            generation,
            active.id,
            Err(TerminalInputError::Cancelled),
        ));
        assert_eq!(
            completion.try_recv(),
            Ok(Err(TerminalInputError::Cancelled))
        );
        assert_eq!(*input.transaction_states.borrow(), [true, false]);
    }

    #[test]
    fn image_failure_rollback_clears_every_submitted_line_without_return() {
        let mut surfaces = TerminalSurfaces::new();
        let input = insert_input_fake(&mut surfaces, "pane");
        let transaction =
            TerminalInputTransaction::new(Vec::new(), true).with_rollback_on_failure();
        let (_, worker) = surfaces.enqueue_input_transaction("pane", transaction);
        let generation = worker.unwrap();
        let active = surfaces
            .active_input_transaction("pane", generation)
            .unwrap();
        surfaces
            .send_input_rollback("pane", generation, active.id, 2)
            .unwrap();
        assert_eq!(*input.bytes.borrow(), [clear_input_bytes(2)]);
        surfaces.cancel_input_queue("pane");
    }

    #[test]
    fn foreground_pid_matches_are_sorted_by_uppercase_pane_id() {
        let mut surfaces = TerminalSurfaces::new();
        for pane_id in ["B-pane", "a-pane"] {
            surfaces.handles.insert(
                pane_id.to_owned(),
                Box::new(FakeHandle {
                    occluded: Rc::new(Cell::new(false)),
                    pointer_inside: Rc::new(Cell::new(false)),
                    input: Rc::new(FakeInput::default()),
                    metadata: SurfaceMetadata::default(),
                    foreground_pid: Some(42),
                }),
            );
        }
        assert_eq!(
            surfaces.panes_matching_foreground_pid(42),
            ["a-pane", "B-pane"]
        );
        assert!(surfaces.panes_matching_foreground_pid(7).is_empty());
    }

    #[test]
    fn handle_exit_leaves_a_pinned_tab_alive() {
        let (mut store, tabs) = store_with(&[("p1", "/tmp/p1")]);
        store.states_mut()[0].tab_mut(&tabs[0]).unwrap().pinned = true;
        let mut surfaces = TerminalSurfaces::new();

        assert!(!surfaces.handle_exit(&mut store, &tabs[0]));
        assert!(tab_is_known(&store, &tabs[0]));
    }
}
