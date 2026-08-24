use std::collections::BTreeMap;

use crate::scrollbar::ScrollbarMetrics;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeTarget {
    App,
    Surface(Option<u64>),
    Unknown(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TerminalColorSlot {
    Foreground,
    Background,
    Cursor,
    Palette(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MouseShape {
    Default,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    AllScroll,
    ColumnResize,
    RowResize,
    NorthResize,
    EastResize,
    SouthResize,
    WestResize,
    NorthEastResize,
    NorthWestResize,
    SouthEastResize,
    SouthWestResize,
    EastWestResize,
    NorthSouthResize,
    NorthEastSouthWestResize,
    NorthWestSouthEastResize,
    ZoomIn,
    ZoomOut,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressKind {
    Set,
    Error,
    Indeterminate,
    Paused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalProgress {
    pub kind: ProgressKind,
    pub percent: Option<u8>,
}

impl TerminalProgress {
    pub fn new(kind: ProgressKind, percent: Option<u8>) -> Self {
        Self {
            kind,
            percent: percent.map(|value| value.min(100)),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeSearchState {
    pub visible: bool,
    pub needle: Option<String>,
    pub total: Option<usize>,
    pub selected: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalStateAction {
    SetTitle(String),
    SetTabTitle(String),
    WorkingDirectory(String),
    Bell,
    MouseShape(MouseShape),
    MouseVisibility(bool),
    MouseOverLink(Option<String>),
    OpenUrl(String),
    SearchStart(Option<String>),
    SearchEnd,
    SearchTotal(Option<usize>),
    SearchSelected(Option<usize>),
    Progress(Option<TerminalProgress>),
    Scrollbar(ScrollbarMetrics),
    ColorChange {
        slot: TerminalColorSlot,
        color: TerminalColor,
    },
    Unsupported {
        tag: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalActionEvent {
    pub target: RuntimeTarget,
    pub action: TerminalStateAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplyActionResult {
    Changed,
    IgnoredTarget,
    Unsupported(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSurfaceState {
    pub surface_id: u64,
    pub title: Option<String>,
    pub tab_title: Option<String>,
    pub working_directory: Option<String>,
    pub bell_count: u64,
    pub mouse_shape: MouseShape,
    pub mouse_visible: bool,
    pub link_under_pointer: Option<String>,
    pub last_open_url: Option<String>,
    pub progress: Option<TerminalProgress>,
    pub colors: BTreeMap<TerminalColorSlot, TerminalColor>,
    pub search: RuntimeSearchState,
    pub scrollbar: ScrollbarMetrics,
}

impl TerminalSurfaceState {
    pub fn new(surface_id: u64) -> Self {
        Self {
            surface_id,
            title: None,
            tab_title: None,
            working_directory: None,
            bell_count: 0,
            mouse_shape: MouseShape::Default,
            mouse_visible: true,
            link_under_pointer: None,
            last_open_url: None,
            progress: None,
            colors: BTreeMap::new(),
            search: RuntimeSearchState::default(),
            scrollbar: ScrollbarMetrics::default(),
        }
    }

    pub fn apply_event(&mut self, event: TerminalActionEvent) -> ApplyActionResult {
        if event.target != RuntimeTarget::Surface(Some(self.surface_id)) {
            return ApplyActionResult::IgnoredTarget;
        }
        self.apply_action(event.action)
    }

    pub fn apply_action(&mut self, action: TerminalStateAction) -> ApplyActionResult {
        match action {
            TerminalStateAction::SetTitle(value) => self.title = Some(value),
            TerminalStateAction::SetTabTitle(value) => self.tab_title = Some(value),
            TerminalStateAction::WorkingDirectory(value) => self.working_directory = Some(value),
            TerminalStateAction::Bell => self.bell_count = self.bell_count.saturating_add(1),
            TerminalStateAction::MouseShape(value) => self.mouse_shape = value,
            TerminalStateAction::MouseVisibility(value) => self.mouse_visible = value,
            TerminalStateAction::MouseOverLink(value) => self.link_under_pointer = value,
            TerminalStateAction::OpenUrl(value) => self.last_open_url = Some(value),
            TerminalStateAction::SearchStart(needle) => {
                self.search = RuntimeSearchState {
                    visible: true,
                    needle,
                    total: None,
                    selected: None,
                };
            }
            TerminalStateAction::SearchEnd => self.search = RuntimeSearchState::default(),
            TerminalStateAction::SearchTotal(value) => self.search.total = value,
            TerminalStateAction::SearchSelected(value) => self.search.selected = value,
            TerminalStateAction::Progress(value) => self.progress = value,
            TerminalStateAction::Scrollbar(value) => {
                self.scrollbar = ScrollbarMetrics::new(value.total, value.offset, value.visible);
            }
            TerminalStateAction::ColorChange { slot, color } => {
                self.colors.insert(slot, color);
            }
            TerminalStateAction::Unsupported { tag } => {
                return ApplyActionResult::Unsupported(tag);
            }
        }
        ApplyActionResult::Changed
    }

    pub fn effective_title(&self) -> &str {
        self.title
            .as_deref()
            .filter(|title| !title.is_empty())
            .or_else(|| self.tab_title.as_deref().filter(|title| !title.is_empty()))
            .unwrap_or("Muxy")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(target: RuntimeTarget, action: TerminalStateAction) -> TerminalActionEvent {
        TerminalActionEvent { target, action }
    }

    #[test]
    fn filters_app_unknown_null_and_other_surface_targets() {
        let mut state = TerminalSurfaceState::new(7);
        for target in [
            RuntimeTarget::App,
            RuntimeTarget::Unknown(9),
            RuntimeTarget::Surface(None),
            RuntimeTarget::Surface(Some(8)),
        ] {
            assert_eq!(
                state.apply_event(event(target, TerminalStateAction::Bell)),
                ApplyActionResult::IgnoredTarget
            );
        }
        assert_eq!(state.bell_count, 0);
        assert_eq!(
            state.apply_event(event(
                RuntimeTarget::Surface(Some(7)),
                TerminalStateAction::Bell
            )),
            ApplyActionResult::Changed
        );
        assert_eq!(state.bell_count, 1);
    }

    #[test]
    fn updates_metadata_colors_progress_and_link_state() {
        let mut state = TerminalSurfaceState::new(1);
        assert_eq!(state.effective_title(), "Muxy");
        state.apply_action(TerminalStateAction::SetTitle("shell".into()));
        state.apply_action(TerminalStateAction::SetTabTitle("tab".into()));
        state.apply_action(TerminalStateAction::WorkingDirectory("/tmp".into()));
        state.apply_action(TerminalStateAction::MouseShape(MouseShape::Pointer));
        state.apply_action(TerminalStateAction::MouseVisibility(false));
        state.apply_action(TerminalStateAction::MouseOverLink(Some(
            "https://muxy.app".into(),
        )));
        state.apply_action(TerminalStateAction::OpenUrl("https://muxy.app/docs".into()));
        state.apply_action(TerminalStateAction::Progress(Some(TerminalProgress::new(
            ProgressKind::Set,
            Some(250),
        ))));
        let color = TerminalColor {
            red: 1,
            green: 2,
            blue: 3,
        };
        state.apply_action(TerminalStateAction::ColorChange {
            slot: TerminalColorSlot::Foreground,
            color,
        });

        assert_eq!(state.title.as_deref(), Some("shell"));
        assert_eq!(state.tab_title.as_deref(), Some("tab"));
        assert_eq!(state.effective_title(), "shell");
        state.apply_action(TerminalStateAction::SetTitle(String::new()));
        assert_eq!(state.effective_title(), "tab");
        assert_eq!(state.working_directory.as_deref(), Some("/tmp"));
        assert_eq!(state.mouse_shape, MouseShape::Pointer);
        assert!(!state.mouse_visible);
        assert_eq!(
            state.link_under_pointer.as_deref(),
            Some("https://muxy.app")
        );
        assert_eq!(state.progress.expect("progress").percent, Some(100));
        assert_eq!(state.colors[&TerminalColorSlot::Foreground], color);
    }

    #[test]
    fn search_scrollbar_and_unsupported_transitions_are_explicit() {
        let mut state = TerminalSurfaceState::new(1);
        state.apply_action(TerminalStateAction::SearchStart(Some("needle".into())));
        state.apply_action(TerminalStateAction::SearchTotal(Some(8)));
        state.apply_action(TerminalStateAction::SearchSelected(Some(3)));
        state.apply_action(TerminalStateAction::Scrollbar(ScrollbarMetrics {
            total: 10,
            offset: 99,
            visible: 4,
        }));
        assert_eq!(state.search.total, Some(8));
        assert_eq!(state.search.selected, Some(3));
        assert_eq!(state.scrollbar.offset, 6);
        assert_eq!(
            state.apply_action(TerminalStateAction::Unsupported { tag: 404 }),
            ApplyActionResult::Unsupported(404)
        );
        state.apply_action(TerminalStateAction::SearchEnd);
        assert_eq!(state.search, RuntimeSearchState::default());
    }

    #[test]
    fn bell_generations_saturate() {
        let mut state = TerminalSurfaceState::new(1);
        state.bell_count = u64::MAX;
        state.apply_action(TerminalStateAction::Bell);
        assert_eq!(state.bell_count, u64::MAX);
    }
}
