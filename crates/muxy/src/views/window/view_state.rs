use crate::views::overlay::Overlay;
use gpui::{Bounds, Entity, FocusHandle, Pixels, Point, Subscription, Task};
use muxy_ui::scrollbar::ScrollbarRevealState;
use muxy_ui::text_input::TextInput;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

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
    pub(super) sidebar_expanded: bool,
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
            sidebar_expanded,
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
}
