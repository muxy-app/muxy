use crate::terminal::Backend;
use gpui::{AnyElement, App, Window};
use muxy_core::workspace::{CloseMode, TabId, TabKind};
use muxy_core::workspace_store::WorkspaceStore;
use muxy_terminal::backend::{LaunchCommand, PointerInput, SurfaceAction, TerminalSurfaceHandle};
use muxy_terminal::confirmation::{ConfirmationId, ConfirmationKind};
use std::collections::{HashMap, HashSet};
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

pub struct TerminalSurfaces {
    backend: Backend,
    handles: HashMap<TabId, Box<dyn AppSurfaceHandle>>,
    pending_cwd: HashMap<TabId, PathBuf>,
    pending_command: HashMap<TabId, LaunchCommand>,
    materialization_requested: HashSet<TabId>,
    socket_path: Option<String>,
    pointer_tab: Option<TabId>,
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
            socket_path: None,
            pointer_tab: None,
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
    pub fn route(
        &mut self,
        event: crate::terminal::TerminalEvent,
        cx: &mut App,
    ) -> Option<(TabId, crate::terminal::SurfaceSignal)> {
        self.backend.route(crate::terminal::unwrap_event(event), cx)
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

    pub fn has_native_scrollbar(&self, tab_id: &str) -> bool {
        self.handle(tab_id)
            .is_some_and(|handle| handle.has_native_scrollbar())
    }

    pub fn send_text(&self, tab_id: &str, text: &str) -> bool {
        self.handle(tab_id)
            .is_some_and(|handle| handle.send_text(text))
    }

    pub fn send_bytes(&self, tab_id: &str, bytes: &[u8]) -> bool {
        self.handle(tab_id)
            .is_some_and(|handle| handle.send_bytes(bytes))
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
        let Some(handle) = self.handles.get_mut(tab_id) else {
            return false;
        };
        handle.perform(action);
        handle.refresh()
    }

    pub fn forward_pointer(&mut self, tab_id: &str, input: PointerInput) -> bool {
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
        for (tab_id, handle) in &self.handles {
            handle.set_focused(Some(tab_id.as_str()) == focused);
        }
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
            if let Some(handle) = self
                .backend
                .spawn(&tab_id, directory, command, &context, window, cx)
            {
                self.pending_cwd.remove(&tab_id);
                self.pending_command.remove(&tab_id);
                self.materialization_requested.remove(&tab_id);
                self.handles.insert(tab_id, handle);
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
        self.handles.retain(|tab_id, _| tab_is_known(store, tab_id));
        self.pending_cwd
            .retain(|tab_id, _| tab_is_known(store, tab_id));
        self.pending_command
            .retain(|tab_id, _| tab_is_known(store, tab_id));
        self.materialization_requested
            .retain(|tab_id| tab_is_known(store, tab_id));
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
            .filter(|tab_id| !visible.contains(tab_id))
            .cloned()
            .collect::<Vec<_>>();
        requested.sort_by_key(|tab_id| tab_id.to_ascii_uppercase());
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
    use std::cell::Cell;
    use std::rc::Rc;

    struct FakeHandle {
        occluded: Rc<Cell<bool>>,
        pointer_inside: Rc<Cell<bool>>,
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
        fn has_selection(&self) -> bool {
            false
        }
        fn foreground_pid(&self) -> Option<u64> {
            self.foreground_pid
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
                metadata: SurfaceMetadata::default(),
                foreground_pid: None,
            }),
        );
        (occluded, pointer_inside)
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
    fn handle_exit_closes_a_tab_in_a_non_active_workspace() {
        let (mut store, tabs) = store_with(&[("p1", "/tmp/p1"), ("p2", "/tmp/p2")]);
        let mut surfaces = TerminalSurfaces::new();

        assert!(surfaces.handle_exit(&mut store, &tabs[1]));

        assert!(!tab_is_known(&store, &tabs[1]));
        assert!(tab_is_known(&store, &tabs[0]));
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
