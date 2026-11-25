mod bar;
pub(crate) mod layout;
pub(crate) mod menu;
mod overlay;
pub(crate) mod panes;
mod sidebar;
mod tabs;
mod theme;

pub(crate) use overlay::{help_lines, project_choices};

use std::time::{Duration, Instant};

use muxy_app_core::PaneId;
use muxy_app_core::activity::{self, ActivityIndicator};
use muxy_protocol::{ProjectId, SessionId};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    text::Span,
    widgets::Paragraph,
};

use crate::selection::Selection;
use crate::state::{Pane, State, Tab};
use crate::worker::{Closing, Shared};
use layout::{Chrome, Divider};

const MESSAGE_TIME: Duration = Duration::from_secs(8);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Mode {
    #[default]
    Normal,
    Prefix,
    Scroll,
    Resize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum Overlay {
    #[default]
    None,
    Projects {
        filter: String,
        selected: usize,
    },
    Sessions(usize),
    Help(usize),
    Confirm(Closing),
    Menu(menu::Menu),
    Rename {
        pane: PaneId,
        text: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    Project(ProjectId),
    Agent(SessionId, ProjectId),
    SidebarList,
    SidebarEdge,
    HideSidebar,
    ShowSidebar,
    Help,
    TabBar,
    Tab(PaneId),
    NewTab,
    Existing,
    Pane(PaneId),
    Content(PaneId),
    Divider(Divider),
    Item(usize),
    Panel,
    Backdrop,
    Confirm(bool),
}

#[derive(Debug)]
pub(crate) struct Ui {
    pub mode: Mode,
    pub overlay: Overlay,
    pub sidebar_shown: bool,
    pub sidebar_width: u16,
    pub mouse: bool,
    pub sidebar_scroll: usize,
    revealed: Option<ProjectId>,
    pub chrome: Chrome,
    pub hits: Vec<(Rect, Target)>,
    pub selection: Option<Selection>,
    pub hover: Option<Position>,
    message: Option<(u64, Instant)>,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            overlay: Overlay::None,
            sidebar_shown: true,
            sidebar_width: layout::SIDEBAR_WIDTH,
            mouse: true,
            sidebar_scroll: 0,
            revealed: None,
            chrome: Chrome::default(),
            hits: Vec::new(),
            selection: None,
            hover: None,
            message: None,
        }
    }
}

impl Ui {
    pub(crate) fn sidebar(&self) -> Option<u16> {
        self.sidebar_shown.then_some(self.sidebar_width)
    }

    pub(crate) fn target(&self, column: u16, row: u16) -> Option<Target> {
        let position = Position::new(column, row);
        self.hits
            .iter()
            .rev()
            .find(|(area, _)| area.contains(position))
            .map(|(_, target)| *target)
    }

    pub(crate) fn target_beneath_dialog(&self, column: u16, row: u16) -> Option<Target> {
        let position = Position::new(column, row);
        let end = self
            .hits
            .iter()
            .position(|(_, target)| *target == Target::Backdrop)
            .unwrap_or(self.hits.len());
        self.hits[..end]
            .iter()
            .rev()
            .find(|(area, _)| area.contains(position))
            .map(|(_, target)| *target)
    }

    pub(crate) fn area(&self, target: Target) -> Option<Rect> {
        self.hits
            .iter()
            .rev()
            .find(|(_, drawn)| *drawn == target)
            .map(|(area, _)| *area)
    }

    fn hovers(&self, area: Rect) -> bool {
        self.hover.is_some_and(|position| area.contains(position))
    }

    fn message(&mut self, shared: &Shared) -> Option<String> {
        if shared.message.is_empty() {
            self.message = None;
            return None;
        }
        if self.message.is_none_or(|(id, _)| id != shared.message_id) {
            self.message = Some((shared.message_id, Instant::now()));
        }
        let online = shared
            .client
            .as_ref()
            .is_some_and(muxy_client::Client::is_connected);
        self.message
            .filter(|(_, since)| !online || since.elapsed() < MESSAGE_TIME)
            .map(|_| clean(&shared.message))
    }
}

pub(crate) fn draw(frame: &mut Frame<'_>, shared: &Shared, ui: &mut Ui) {
    ui.hits.clear();
    let area = frame.area();
    if area.is_empty() {
        return;
    }
    let Some(state) = &shared.state else {
        frame.render_widget(Paragraph::new(clean(&shared.message)), area);
        return;
    };
    ui.chrome = Chrome::new(area, ui.sidebar());
    let chrome = ui.chrome;
    sidebar::draw(frame, shared, state, ui, chrome.sidebar);
    tabs::draw(frame, shared, state, ui, chrome.tabs);
    panes::draw(frame, shared, state, ui, chrome.body);
    if ui.overlay == Overlay::None {
        bar::draw(frame, shared, state, ui, chrome.body);
    }
    overlay::draw(frame, shared, ui);
}

pub(crate) fn clean(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .collect()
}

pub(crate) fn truncate(text: &str, width: usize) -> String {
    if Span::raw(text).width() <= width {
        return text.to_owned();
    }
    let mut kept = String::new();
    let mut used = 0;
    for character in text.chars() {
        let mut buffer = [0; 4];
        let size = Span::raw(&*character.encode_utf8(&mut buffer)).width();
        if used + size + 1 > width {
            break;
        }
        kept.push(character);
        used += size;
    }
    if width > 0 {
        kept.push('…');
    }
    kept
}

fn title_of(shared: &Shared, id: PaneId, pane: &Pane) -> String {
    let metadata = pane
        .session
        .and_then(|session| shared.metadata.get(&session));
    let directory = metadata.map_or(&pane.directory, |metadata| &metadata.directory);
    let title = match (shared.views.get(&id), metadata) {
        (Some(view), _) => {
            muxy_app_core::title::derive(&view.title, view.process.as_ref(), directory)
        }
        (None, Some(metadata)) => {
            muxy_app_core::title::derive(&metadata.title, metadata.process.as_ref(), directory)
        }
        (None, None) => muxy_app_core::title::derive("", None, directory),
    };
    clean(&title)
}

fn pane_title(shared: &Shared, state: &State, id: PaneId) -> String {
    state
        .tab()
        .and_then(|tab| tab.panes.get(&id))
        .map_or_else(String::new, |pane| title_of(shared, id, pane))
}

fn session_activity(shared: &Shared, session: SessionId) -> ActivityIndicator {
    activity::indicator(&shared.activity, |candidate| candidate == session)
}

fn tab_activity(shared: &Shared, tab: &Tab) -> ActivityIndicator {
    activity::indicator(&shared.activity, |session| {
        tab.panes.values().any(|pane| pane.session == Some(session))
    })
}

fn project_activity(shared: &Shared, project: ProjectId) -> ActivityIndicator {
    let snapshot = &shared.activity;
    activity::indicator(snapshot, |session| {
        snapshot
            .agents
            .iter()
            .any(|agent| agent.session == session && agent.project == project)
            || snapshot
                .events
                .iter()
                .any(|event| event.session == session && event.project == project)
    })
}
