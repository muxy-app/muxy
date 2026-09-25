//! Plain records the app reads; converted from the protocol's types.

use muxy_client::RunGrid;
use muxy_protocol::{ProjectDescriptor, ProjectKind, ProjectSession, Row, Run};

/// Everything the app keeps to reconnect; store it in the Keychain or Keystore.
#[derive(Clone, Eq, PartialEq, uniffi::Record)]
pub struct ServerCredential {
    pub server_id: String,
    pub server_name: String,
    pub hosts: Vec<String>,
    pub port: u16,
    /// SHA-256 of the server's certificate.
    pub fingerprint: Vec<u8>,
    pub device_id: String,
    pub token: Vec<u8>,
}

impl std::fmt::Debug for ServerCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ServerCredential")
            .field("server_id", &self.server_id)
            .field("server_name", &self.server_name)
            .field("hosts", &self.hosts)
            .field("port", &self.port)
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

/// What a pairing link points at, for confirming before pairing.
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct PairingLink {
    pub hosts: Vec<String>,
    pub port: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub color: String,
    /// An emoji, or an SF Symbol name prefixed with `sf:`.
    pub icon: Option<String>,
    /// A square PNG.
    pub logo: Option<Vec<u8>>,
    pub parent_id: Option<String>,
    pub is_home: bool,
    pub is_worktree: bool,
}

impl From<&ProjectDescriptor> for Project {
    fn from(project: &ProjectDescriptor) -> Self {
        Self {
            id: project.id.to_string(),
            name: project.name.clone(),
            directory: String::from_utf8_lossy(&project.directory.0).into_owned(),
            color: project.color.clone(),
            icon: project.icon.clone(),
            logo: project.logo.as_deref().map(<[u8]>::to_vec),
            parent_id: project.parent_id.map(|parent| parent.to_string()),
            is_home: project.home,
            is_worktree: project.kind == Some(ProjectKind::Worktree),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum SessionStatus {
    Starting,
    Live,
    Ended,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum ClientKind {
    Desktop,
    Tui,
    Cli,
    Mobile,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct Session {
    pub id: u64,
    pub project_id: String,
    pub directory: String,
    pub status: SessionStatus,
    /// The earliest attached client, if any.
    pub owner: Option<ClientKind>,
    /// Whether this device is attached.
    pub attached: bool,
}

impl From<&ProjectSession> for Session {
    fn from(session: &ProjectSession) -> Self {
        Self {
            id: session.info.id.get(),
            project_id: session.info.project.to_string(),
            directory: String::from_utf8_lossy(&session.info.directory.0).into_owned(),
            status: match session.status {
                muxy_protocol::SessionStatus::Starting => SessionStatus::Starting,
                muxy_protocol::SessionStatus::Live => SessionStatus::Live,
                muxy_protocol::SessionStatus::Ended => SessionStatus::Ended,
                muxy_protocol::SessionStatus::Unavailable => SessionStatus::Unavailable,
            },
            owner: session.owner.map(|owner| match owner.kind {
                muxy_protocol::ClientKind::Desktop => ClientKind::Desktop,
                muxy_protocol::ClientKind::Tui => ClientKind::Tui,
                muxy_protocol::ClientKind::Cli => ClientKind::Cli,
                muxy_protocol::ClientKind::Mobile => ClientKind::Mobile,
            }),
            attached: session.attached,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum AgentState {
    Unknown,
    Idle,
    Working,
    Blocked,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct Agent {
    pub session_id: u64,
    pub project_id: String,
    pub provider: String,
    pub state: AgentState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum ActivityKind {
    Attention,
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct ActivityEvent {
    pub id: u64,
    pub session_id: u64,
    pub project_id: String,
    pub provider: String,
    /// Unix seconds.
    pub timestamp: u64,
    pub kind: ActivityKind,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct Activity {
    pub agents: Vec<Agent>,
    pub events: Vec<ActivityEvent>,
}

impl From<muxy_protocol::ActivitySnapshot> for Activity {
    fn from(snapshot: muxy_protocol::ActivitySnapshot) -> Self {
        Self {
            agents: snapshot
                .agents
                .into_iter()
                .map(|agent| Agent {
                    session_id: agent.session.get(),
                    project_id: agent.project.to_string(),
                    provider: agent.provider.name().into(),
                    state: match agent.state {
                        muxy_protocol::AgentState::Unknown => AgentState::Unknown,
                        muxy_protocol::AgentState::Idle => AgentState::Idle,
                        muxy_protocol::AgentState::Working => AgentState::Working,
                        muxy_protocol::AgentState::Blocked => AgentState::Blocked,
                    },
                })
                .collect(),
            events: snapshot
                .events
                .into_iter()
                .map(|event| ActivityEvent {
                    id: event.id,
                    session_id: event.session.get(),
                    project_id: event.project.to_string(),
                    provider: event.provider.name().into(),
                    timestamp: event.timestamp,
                    kind: match event.kind {
                        muxy_protocol::ActivityKind::Attention => ActivityKind::Attention,
                        muxy_protocol::ActivityKind::Completed => ActivityKind::Completed,
                    },
                })
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum TerminalColor {
    /// The theme's default foreground or background.
    Default,
    /// One of the 256 palette entries.
    Indexed {
        index: u8,
    },
    Rgb {
        red: u8,
        green: u8,
        blue: u8,
    },
}

impl From<muxy_protocol::Color> for TerminalColor {
    fn from(color: muxy_protocol::Color) -> Self {
        match color {
            muxy_protocol::Color::Default => Self::Default,
            muxy_protocol::Color::Indexed(index) => Self::Indexed { index },
            muxy_protocol::Color::Rgb(red, green, blue) => Self::Rgb { red, green, blue },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum Underline {
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Record)]
pub struct Style {
    pub foreground: TerminalColor,
    pub background: TerminalColor,
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub underline: Underline,
    pub underline_color: TerminalColor,
    pub strikethrough: bool,
    pub overline: bool,
    /// Swap foreground and background when drawing.
    pub inverse: bool,
    pub invisible: bool,
}

impl From<muxy_protocol::Style> for Style {
    fn from(style: muxy_protocol::Style) -> Self {
        Self {
            foreground: style.fg.into(),
            background: style.bg.into(),
            bold: style.bold,
            italic: style.italic,
            faint: style.faint,
            underline: match style.underline {
                muxy_protocol::Underline::None => Underline::None,
                muxy_protocol::Underline::Single => Underline::Single,
                muxy_protocol::Underline::Double => Underline::Double,
                muxy_protocol::Underline::Curly => Underline::Curly,
                muxy_protocol::Underline::Dotted => Underline::Dotted,
                muxy_protocol::Underline::Dashed => Underline::Dashed,
            },
            underline_color: style.underline_color.into(),
            strikethrough: style.strikethrough,
            overline: style.overline,
            inverse: style.inverse,
            invisible: style.invisible,
        }
    }
}

/// Text sharing one style; `width` counts terminal cells, which may differ from characters.
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct Span {
    pub text: String,
    pub width: u16,
    pub style: Style,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    pub(crate) fn from_runs(runs: &[Run]) -> Self {
        Self {
            spans: runs
                .iter()
                .map(|run| Span {
                    text: run.text.clone(),
                    width: run.width,
                    style: run.style.into(),
                })
                .collect(),
        }
    }

    pub(crate) fn from_row(row: &Row) -> Self {
        Self::from_runs(&row.runs)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
    Hollow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Record)]
pub struct Cursor {
    pub row: u16,
    pub column: u16,
    pub visible: bool,
    pub shape: CursorShape,
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct Screen {
    pub columns: u16,
    pub rows: u16,
    pub lines: Vec<Line>,
    pub cursor: Cursor,
    pub title: String,
    pub directory: String,
    /// Rows retained above the screen on the server; read them through `Terminal::scrollback`.
    pub history_rows: u64,
    pub application_cursor_keys: bool,
    pub bracketed_paste: bool,
}

impl Screen {
    pub(crate) fn new(grid: &RunGrid, title: &str, directory: &[u8]) -> Self {
        Self {
            columns: grid.size.cols,
            rows: grid.size.rows,
            lines: grid.rows.iter().map(|runs| Line::from_runs(runs)).collect(),
            cursor: Cursor {
                row: grid.cursor.row,
                column: grid.cursor.col,
                visible: grid.cursor.visible,
                shape: match grid.cursor.shape {
                    muxy_protocol::CursorShape::Block => CursorShape::Block,
                    muxy_protocol::CursorShape::Bar => CursorShape::Bar,
                    muxy_protocol::CursorShape::Underline => CursorShape::Underline,
                    muxy_protocol::CursorShape::Hollow => CursorShape::Hollow,
                },
            },
            title: title.into(),
            directory: String::from_utf8_lossy(directory).into_owned(),
            history_rows: grid.history_total,
            application_cursor_keys: grid.modes.application_cursor_keys,
            bracketed_paste: grid.modes.bracketed_paste,
        }
    }
}
