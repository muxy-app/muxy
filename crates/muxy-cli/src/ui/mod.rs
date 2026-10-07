//! What the terminal UI draws, and the screen state that isn't part of the
//! shared layout: modes, dialogs, the sidebar, the mouse, and selection.

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

/// How long a message stays in the tab bar, unless the server is offline.
const MESSAGE_TIME: Duration = Duration::from_secs(8);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum Mode {
    /// Keys go to the focused terminal.
    #[default]
    Normal,
    /// Ctrl-B was pressed; the next key is a command.
    Prefix,
    /// Keys scroll the focused pane's history.
    Scroll,
    /// Arrow keys resize the focused pane.
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
    /// The first line shown.
    Help(usize),
    Confirm(Closing),
    Menu(menu::Menu),
    /// Naming the tab that holds `pane`.
    Rename {
        pane: PaneId,
        text: String,
    },
}

/// Something on screen the mouse can act on.
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
    /// A tab, named by the pane focused in it when drawn.
    Tab(PaneId),
    NewTab,
    /// The count of this project's terminals that no tab shows.
    Existing,
    /// A pane, including its frame.
    Pane(PaneId),
    /// Where a pane's terminal is drawn.
    Content(PaneId),
    Divider(Divider),
    Item(usize),
    Panel,
    /// Outside a dialog.
    Backdrop,
    Confirm(bool),
}

#[derive(Debug)]
pub(crate) struct Ui {
    pub mode: Mode,
    pub overlay: Overlay,
    pub sidebar_shown: bool,
    pub sidebar_width: u16,
    /// Whether the mouse acts on this UI; off, the terminal selects text.
    pub mouse: bool,
    pub sidebar_scroll: usize,
    /// The project last scrolled into view in the sidebar.
    revealed: Option<ProjectId>,
    pub chrome: Chrome,
    /// What can be clicked, in drawing order: later entries are on top.
    pub hits: Vec<(Rect, Target)>,
    pub selection: Option<Selection>,
    pub hover: Option<Position>,
    /// The message last shown and when it appeared.
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
    /// The sidebar's width, or `None` while it is hidden.
    pub(crate) fn sidebar(&self) -> Option<u16> {
        self.sidebar_shown.then_some(self.sidebar_width)
    }

    /// The topmost target under a cell, as last drawn.
    pub(crate) fn target(&self, column: u16, row: u16) -> Option<Target> {
        let position = Position::new(column, row);
        self.hits
            .iter()
            .rev()
            .find(|(area, _)| area.contains(position))
            .map(|(_, target)| *target)
    }

    /// The topmost target under a cell, as if the open dialog weren't there.
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

    /// Where `target` was last drawn.
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

    /// The worker's latest message, for a while after it appears.
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

/// `text` cut to `width` columns, ending in `…` if anything was cut.
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

/// A terminal's title: the one its program set, else the program, else its
/// folder.
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

#[cfg(test)]
mod tests {
    use muxy_protocol::{
        ActivityEvent, ActivityKind, ActivitySnapshot, AgentActivity, AgentProvider, AgentState,
        CatalogPage, ProjectDescriptor, ProjectKind, ServerIdentity, ServerPath,
    };
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    fn project(name: &str, home: bool, parent: Option<ProjectId>) -> ProjectDescriptor {
        ProjectDescriptor {
            id: ProjectId::new(),
            home,
            directory: ServerPath(if home {
                b"/home/me".to_vec()
            } else {
                format!("/home/me/{name}").into_bytes()
            }),
            name: name.into(),
            icon: None,
            logo: None,
            color: "#336699".into(),
            kind: parent.map(|_| ProjectKind::Worktree),
            parent_id: parent,
        }
    }

    #[test]
    fn the_sidebar_lists_projects_with_worktrees_and_agents_needing_attention_first()
    -> crate::state::Result {
        let home = project("Home", true, None);
        let app = project("app", false, None);
        let feature = project("feature", false, Some(app.id));
        let catalog = CatalogPage {
            server: ServerIdentity::new(),
            home: home.id,
            revision: 0,
            next: None,
            legacy_home: None,
            projects: vec![app.clone(), feature, home.clone()],
        };
        let session = |id| SessionId::new(id).ok_or("session");
        let mut state = State::new(&catalog);
        state.reconcile(&catalog)?;
        state.open(app.id, &catalog)?;
        state.new_pane(None, app.directory.clone(), Some(session(7)?))?;
        let mut shared = Shared::default();
        shared.server = "local server".into();
        shared.catalog = Some(catalog);
        shared.state = Some(state);
        shared.activity = ActivitySnapshot {
            revision: 1,
            agents: vec![AgentActivity {
                session: session(7)?,
                project: app.id,
                provider: AgentProvider::Claude,
                state: AgentState::Working,
            }],
            events: vec![ActivityEvent {
                id: 1,
                session: session(9)?,
                project: home.id,
                provider: AgentProvider::Codex,
                timestamp: 0,
                kind: ActivityKind::Attention,
                read: false,
            }],
        };
        shared.activity.agents.push(AgentActivity {
            session: session(9)?,
            project: home.id,
            provider: AgentProvider::Codex,
            state: AgentState::Blocked,
        });
        let mut terminal =
            Terminal::new(TestBackend::new(100, 24)).map_err(|error| error.to_string())?;
        let mut ui = Ui::default();
        terminal
            .draw(|frame| draw(frame, &shared, &mut ui))
            .map_err(|error| error.to_string())?;
        let buffer = terminal.backend().buffer();
        let rows: Vec<String> = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .filter_map(|x| buffer.cell((x, y)).map(|cell| cell.symbol().to_owned()))
                    .collect()
            })
            .collect();
        let text = rows.join("\n");
        for expected in [
            "● Home",
            "▌● app",
            "▌  ~/app",
            "  └─ feature",
            " agents",
            "● Home",
            "Codex · needs you",
            "▌● app · 2",
            "Claude Code · working",
            " 1 app ",
            " 2 app ● ",
            "● local server",
            "Ctrl-B ? help",
        ] {
            assert!(text.contains(expected), "{expected:?} missing from\n{text}");
        }
        let blocked = rows.iter().position(|row| row.contains("needs you"));
        let working = rows.iter().position(|row| row.contains("working"));
        assert!(blocked < working, "{text}");
        let feature_row = rows
            .iter()
            .position(|row| row.contains("feature"))
            .and_then(|row| u16::try_from(row).ok())
            .ok_or("feature row")?;
        assert_eq!(
            ui.target(8, feature_row),
            shared
                .catalog
                .as_ref()
                .map(|catalog| Target::Project(catalog.projects[1].id))
        );
        assert_eq!(ui.target(25, 5), Some(Target::SidebarEdge));
        Ok(())
    }
}
