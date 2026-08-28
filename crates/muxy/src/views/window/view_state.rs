use crate::views::overlay::Overlay;
use gpui::{Bounds, Entity, FocusHandle, Pixels, Point, Subscription, Task};
use muxy_ui::scrollbar::ScrollbarRevealState;
use muxy_ui::text_input::TextInput;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CreateRequestIdentity {
    pub(super) project_id: String,
    pub(super) generation: u64,
    pub(super) request_id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RemovalRequestIdentity {
    pub(super) project_id: String,
    pub(super) worktree_id: String,
    pub(super) generation: u64,
    pub(super) request_id: u64,
}

impl CreateRequestIdentity {
    pub(super) fn new(project_id: &str, generation: u64, request_id: u64) -> Self {
        Self {
            project_id: project_id.to_owned(),
            generation,
            request_id,
        }
    }
}

#[derive(Default)]
pub(super) struct WorktreeViewState {
    expanded: HashSet<String>,
    create_request: Option<CreateRequestIdentity>,
    removal_request: Option<RemovalRequestIdentity>,
    next_request_id: u64,
}

impl WorktreeViewState {
    #[cfg(test)]
    pub(super) fn is_expanded(&self, project_id: &str) -> bool {
        self.expanded.contains(project_id)
    }

    pub(super) fn toggle(&mut self, project_id: &str) {
        if !self.expanded.remove(project_id) {
            self.expanded.insert(project_id.to_owned());
        }
    }

    pub(super) fn expand(&mut self, project_id: &str) {
        self.expanded.insert(project_id.to_owned());
    }

    pub(super) fn expanded_projects(&self) -> &HashSet<String> {
        &self.expanded
    }

    pub(super) fn begin_create(&mut self, request: CreateRequestIdentity) {
        self.create_request = Some(request);
    }

    pub(super) fn begin_create_for(
        &mut self,
        project_id: &str,
        generation: u64,
    ) -> CreateRequestIdentity {
        self.next_request_id = self.next_request_id.wrapping_add(1);
        let request = CreateRequestIdentity::new(project_id, generation, self.next_request_id);
        self.begin_create(request.clone());
        request
    }

    pub(super) fn matches_create(&self, request: &CreateRequestIdentity) -> bool {
        self.create_request.as_ref() == Some(request)
    }

    pub(super) fn create_request(&self) -> Option<&CreateRequestIdentity> {
        self.create_request.as_ref()
    }

    pub(super) fn clear_create(&mut self) {
        self.create_request = None;
    }

    pub(super) fn begin_removal(
        &mut self,
        project_id: &str,
        worktree_id: &str,
        generation: u64,
    ) -> RemovalRequestIdentity {
        self.next_request_id = self.next_request_id.wrapping_add(1);
        let request = RemovalRequestIdentity {
            project_id: project_id.to_owned(),
            worktree_id: worktree_id.to_owned(),
            generation,
            request_id: self.next_request_id,
        };
        self.removal_request = Some(request.clone());
        request
    }

    pub(super) fn matches_removal(&self, request: &RemovalRequestIdentity) -> bool {
        self.removal_request.as_ref() == Some(request)
    }

    pub(super) fn clear_removal(&mut self) {
        self.removal_request = None;
    }

    pub(super) fn clear_removal_if(&mut self, request: &RemovalRequestIdentity) {
        if self.matches_removal(request) {
            self.clear_removal();
        }
    }

    pub(super) fn rebase_create(&mut self, project_id: &str, generation: u64) {
        if let Some(request) = &mut self.create_request
            && request.project_id.eq_ignore_ascii_case(project_id)
        {
            request.generation = generation;
        }
    }

    pub(super) fn clear_project(&mut self, project_id: &str) {
        self.expanded.remove(project_id);
        if self
            .create_request
            .as_ref()
            .is_some_and(|request| request.project_id.eq_ignore_ascii_case(project_id))
        {
            self.create_request = None;
        }
        if self
            .removal_request
            .as_ref()
            .is_some_and(|request| request.project_id.eq_ignore_ascii_case(project_id))
        {
            self.removal_request = None;
        }
    }

    pub(super) fn retain_projects(&mut self, project_ids: &HashSet<String>) {
        let removed = self
            .expanded
            .iter()
            .filter(|project_id| !project_ids.contains(*project_id))
            .cloned()
            .collect::<Vec<_>>();
        for project_id in removed {
            self.clear_project(&project_id);
        }
        if self
            .create_request
            .as_ref()
            .is_some_and(|request| !project_ids.contains(&request.project_id))
        {
            self.create_request = None;
        }
        if self
            .removal_request
            .as_ref()
            .is_some_and(|request| !project_ids.contains(&request.project_id))
        {
            self.removal_request = None;
        }
    }
}

pub(super) enum WorkspaceGesture {
    Tab {
        tab_id: String,
        group_id: String,
        drag: muxy_core::workspace::DragCoordinator,
        target: Option<(String, muxy_core::workspace::DropZone)>,
    },
    Pane {
        tab_id: String,
        drag: muxy_core::workspace::DragCoordinator,
        enabled: bool,
        target: Option<(String, muxy_core::workspace::DropZone)>,
    },
    Resize {
        split_id: String,
        top_level: bool,
        axis: muxy_core::workspace::Axis,
        initial_ratio: f32,
        origin: Point<Pixels>,
    },
}

pub(super) struct ScrollbarDrag {
    pub(super) tab_id: String,
    pub(super) area_id: String,
    pub(super) grab: f64,
    pub(super) origin: f64,
    pub(super) last_row: Option<u64>,
}

pub(super) struct WorkspaceInteractionState {
    pub(super) gesture: Option<WorkspaceGesture>,
    pub(super) tab_bounds: HashMap<String, Bounds<Pixels>>,
    pub(super) area_bounds: HashMap<String, Bounds<Pixels>>,
    pub(super) group_bounds: HashMap<String, Bounds<Pixels>>,
    pub(super) split_bounds: HashMap<String, Bounds<Pixels>>,
}

impl WorkspaceInteractionState {
    fn new() -> Self {
        Self {
            gesture: None,
            tab_bounds: HashMap::new(),
            area_bounds: HashMap::new(),
            group_bounds: HashMap::new(),
            split_bounds: HashMap::new(),
        }
    }
}

pub(super) struct TerminalViewState {
    pub(super) overlay_was_open: bool,
    pub(super) search_inputs: HashMap<String, Entity<TextInput>>,
    pub(super) search_subscriptions: HashMap<String, Subscription>,
    pub(super) search_debounce: Option<Task<()>>,
    pub(super) pending_search_focus: Option<String>,
    pub(super) scrollbar_reveal: HashMap<String, ScrollbarRevealState>,
    pub(super) scrollbar_drag: Option<ScrollbarDrag>,
    pub(super) scrollbar_expiry: Option<Task<()>>,
    pub(super) attention: HashSet<String>,
    pub(super) bell_flashes: HashMap<String, Duration>,
    pub(super) bell_expiry: Option<Task<()>>,
    pub(super) started_at: Instant,
}

impl TerminalViewState {
    fn new() -> Self {
        Self {
            overlay_was_open: false,
            search_inputs: HashMap::new(),
            search_subscriptions: HashMap::new(),
            search_debounce: None,
            pending_search_focus: None,
            scrollbar_reveal: HashMap::new(),
            scrollbar_drag: None,
            scrollbar_expiry: None,
            attention: HashSet::new(),
            bell_flashes: HashMap::new(),
            bell_expiry: None,
            started_at: Instant::now(),
        }
    }
}

pub(super) struct ViewState {
    pub(super) overlay: Overlay,
    pub(super) menu_focus: FocusHandle,
    pub(super) workspace_focus: FocusHandle,
    pub(super) pending_focus: Option<FocusHandle>,
    pub(super) subscriptions: Vec<Subscription>,
    pub(super) activation_subscription: Option<Subscription>,
    pub(super) theme_picker_anchor: Option<Bounds<Pixels>>,
    pub(super) sidebar_expanded: bool,
    pub(super) repository: super::repository::RepositoryViewState,
    pub(super) worktrees: WorktreeViewState,
    pub(super) workspace: WorkspaceInteractionState,
    pub(super) terminal: TerminalViewState,
}

impl ViewState {
    pub(super) fn new(
        menu_focus: FocusHandle,
        workspace_focus: FocusHandle,
        sidebar_expanded: bool,
    ) -> Self {
        Self {
            overlay: Overlay::None,
            menu_focus,
            pending_focus: Some(workspace_focus.clone()),
            workspace_focus,
            subscriptions: Vec::new(),
            activation_subscription: None,
            theme_picker_anchor: None,
            sidebar_expanded,
            repository: super::repository::RepositoryViewState::new(),
            worktrees: WorktreeViewState::default(),
            workspace: WorkspaceInteractionState::new(),
            terminal: TerminalViewState::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_interaction_state_starts_empty() {
        let workspace = WorkspaceInteractionState::new();
        assert!(workspace.gesture.is_none());
        assert!(workspace.tab_bounds.is_empty());
        assert!(workspace.area_bounds.is_empty());
        assert!(workspace.group_bounds.is_empty());
        assert!(workspace.split_bounds.is_empty());

        let terminal = TerminalViewState::new();
        assert!(!terminal.overlay_was_open);
        assert!(terminal.search_inputs.is_empty());
        assert!(terminal.search_subscriptions.is_empty());
        assert!(terminal.search_debounce.is_none());
        assert!(terminal.pending_search_focus.is_none());
        assert!(terminal.scrollbar_reveal.is_empty());
        assert!(terminal.scrollbar_drag.is_none());
        assert!(terminal.scrollbar_expiry.is_none());
        assert!(terminal.attention.is_empty());
        assert!(terminal.bell_flashes.is_empty());
        assert!(terminal.bell_expiry.is_none());
    }

    #[test]
    fn worktree_rows_view_state_tracks_expansion_and_rejects_stale_request_identity() {
        let mut state = WorktreeViewState::default();
        assert!(!state.is_expanded("PROJECT"));
        state.toggle("PROJECT");
        assert!(state.is_expanded("PROJECT"));
        state.toggle("PROJECT");
        assert!(!state.is_expanded("PROJECT"));

        let current = CreateRequestIdentity::new("PROJECT", 4, 7);
        state.begin_create(current.clone());
        assert!(state.matches_create(&current));
        assert!(!state.matches_create(&CreateRequestIdentity::new("PROJECT", 4, 6)));
        assert!(!state.matches_create(&CreateRequestIdentity::new("OTHER", 4, 7)));
        state.rebase_create("PROJECT", 5);
        assert!(state.matches_create(&CreateRequestIdentity::new("PROJECT", 5, 7)));
        assert!(!state.matches_create(&current));
        let removal = state.begin_removal("PROJECT", "SECONDARY", 5);
        assert!(state.matches_removal(&removal));
        assert!(!state.matches_removal(&RemovalRequestIdentity {
            request_id: removal.request_id.wrapping_add(1),
            ..removal.clone()
        }));
        let newer = state.begin_removal("PROJECT", "THIRD", 5);
        state.clear_removal_if(&removal);
        assert!(state.matches_removal(&newer));
        state.clear_project("PROJECT");
        assert!(state.removal_request.is_none());
        assert!(state.create_request().is_none());
        assert!(!state.is_expanded("PROJECT"));
    }
}
