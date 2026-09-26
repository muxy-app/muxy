use minicbor::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::wire::cbor::open::{self, Variant};
use crate::wire::cbor::open_enum;

open_enum! {
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
    pub enum ClientKind {
        Desktop = 0,
        Tui = 1,
        #[default]
        Cli = 2,
        Mobile = 3,
    }
}

impl std::fmt::Display for ClientKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Desktop => "Desktop",
            Self::Tui => "TUI",
            Self::Cli => "CLI",
            Self::Mobile => "Mobile",
            Self::Unrecognized(_) => "Other",
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SessionClient {
    #[n(0)]
    pub id: crate::ClientId,
    #[n(1)]
    pub kind: ClientKind,
}

impl std::fmt::Display for SessionClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} · {:.6}", self.kind, self.id.to_string())
    }
}

use crate::{ChannelId, Cursor, Modes, Row, ServerPath, SessionId, Size};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SessionInfo {
    #[n(0)]
    pub project: crate::ProjectId,
    #[n(1)]
    pub id: SessionId,
    #[n(2)]
    pub directory: ServerPath,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ExitReason {
    Exited(i32),
    Signaled(i32),
    Ended,
    ServerStopped,
    /// A reason from a newer build, kept by its number.
    Unrecognized(u32),
}

impl<C> Encode<C> for ExitReason {
    fn encode<W: minicbor::encode::Write>(
        &self,
        encoder: &mut minicbor::Encoder<W>,
        ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        match self {
            Self::Exited(code) => open::encode_value(0, code, encoder, ctx),
            Self::Signaled(signal) => open::encode_value(1, signal, encoder, ctx),
            Self::Ended => open::encode_unit(2, encoder),
            Self::ServerStopped => open::encode_unit(3, encoder),
            Self::Unrecognized(index) => open::encode_unit(*index, encoder),
        }
    }
}

impl<'b, C> Decode<'b, C> for ExitReason {
    fn decode(
        decoder: &mut minicbor::Decoder<'b>,
        ctx: &mut C,
    ) -> Result<Self, minicbor::decode::Error> {
        Ok(match open::variant(decoder)? {
            Variant::Value(0) => Self::Exited(decoder.decode_with(ctx)?),
            Variant::Value(1) => Self::Signaled(decoder.decode_with(ctx)?),
            Variant::Unit(2) => Self::Ended,
            Variant::Unit(3) => Self::ServerStopped,
            other => Self::Unrecognized(open::unrecognized(decoder, other)?),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct AttachSnapshot {
    #[n(0)]
    pub graphics: crate::Graphics,
    /// Prompt starts in history-then-screen row coordinates.
    #[n(1)]
    pub prompts: Vec<u16>,
    #[n(2)]
    pub channel: ChannelId,
    #[n(3)]
    pub size: Size,
    #[n(4)]
    #[cbor(with = "crate::wire::cbor::rows")]
    pub rows: Vec<Row>,
    #[n(5)]
    pub cursor: Cursor,
    #[n(6)]
    pub modes: Modes,
    #[n(7)]
    pub title: String,
    #[n(8)]
    pub directory: ServerPath,
    #[n(9)]
    #[cbor(with = "crate::wire::cbor::rows")]
    pub history: Vec<Row>,
    #[n(10)]
    pub history_cursor: Option<HistoryCursor>,
    #[n(11)]
    pub history_total: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
#[cbor(transparent)]
pub struct HistoryCursor(#[n(0)] pub u64);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct HistoryPage {
    /// Prompt starts in page-history-then-optional-screen row coordinates.
    #[n(0)]
    pub prompts: Vec<u16>,
    #[n(1)]
    #[cbor(with = "crate::wire::cbor::rows")]
    pub rows: Vec<Row>,
    #[n(2)]
    pub next: Option<HistoryCursor>,
    #[n(3)]
    pub total_rows: u64,
    #[n(4)]
    pub screen: Option<SavedScreen>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum SearchSource {
    #[n(0)]
    Live(#[n(0)] ChannelId),
    #[n(1)]
    Saved(#[n(0)] SessionId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SearchMatch {
    #[n(0)]
    pub row: u64,
    #[n(1)]
    pub start: u16,
    #[n(2)]
    pub end: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SearchPage {
    #[n(0)]
    pub matches: Vec<SearchMatch>,
    #[n(1)]
    pub next: Option<HistoryCursor>,
    #[n(2)]
    pub total_rows: u64,
    #[n(3)]
    pub scanned_rows: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct ForegroundProcess {
    #[n(0)]
    pub name: String,
    #[n(1)]
    pub is_shell: bool,
}

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum ProgressState {
        Running = 0,
        Error = 1,
        Indeterminate = 2,
        Paused = 3,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct TerminalProgress {
    #[n(0)]
    pub state: ProgressState,
    #[n(1)]
    pub percent: Option<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SessionProgress {
    #[n(0)]
    pub progress: Option<TerminalProgress>,
    #[n(1)]
    pub completed: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SessionMetadata {
    #[n(0)]
    pub title: String,
    #[n(1)]
    pub directory: ServerPath,
    #[n(2)]
    pub process: Option<ForegroundProcess>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum MetadataEvent {
    #[n(0)]
    Title(#[n(0)] String),
    #[n(1)]
    Directory(#[n(0)] ServerPath),
    #[n(2)]
    ForegroundProcess {
        #[n(0)]
        name: String,
        #[n(1)]
        is_shell: bool,
    },
    #[n(3)]
    Bell,
    #[n(4)]
    History {
        #[n(0)]
        total_rows: u64,
    },
    #[n(5)]
    InputModes(#[n(0)] InputModes),
    #[n(6)]
    CursorBlinking(#[n(0)] bool),
    #[n(7)]
    Links {
        #[n(0)]
        seq: u64,
        #[n(1)]
        rows: Vec<LinkRow>,
    },
    #[n(8)]
    ScreenPrompts {
        #[n(0)]
        seq: u64,
        #[n(1)]
        rows: Vec<u16>,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct InputModes {
    #[n(0)]
    pub mouse_tracking: bool,
    #[n(1)]
    pub alternate_scroll: bool,
    #[n(2)]
    pub focus_events: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct MouseEvent {
    #[n(0)]
    pub action: MouseAction,
    #[n(1)]
    pub button: Option<MouseButton>,
    #[n(2)]
    pub column: u16,
    #[n(3)]
    pub row: u16,
    #[n(4)]
    pub scroll: Option<ScrollDirection>,
    #[n(5)]
    pub modifiers: Modifiers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub enum MouseAction {
    #[n(0)]
    Press,
    #[n(1)]
    Release,
    #[n(2)]
    Motion,
    #[n(3)]
    Scroll,
}

// Open because an optional field would otherwise read a newer value as absent,
// which fails validation instead of being ignored.
open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum MouseButton {
        Left = 0,
        Middle = 1,
        Right = 2,
        Back = 3,
        Forward = 4,
    }
}

open_enum! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub enum ScrollDirection {
        Up = 0,
        Down = 1,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct Modifiers {
    #[n(0)]
    pub shift: bool,
    #[n(1)]
    pub alt: bool,
    #[n(2)]
    pub ctrl: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SavedScreen {
    #[n(0)]
    pub graphics: crate::Graphics,
    #[n(1)]
    pub size: Size,
    #[n(2)]
    #[cbor(with = "crate::wire::cbor::rows")]
    pub rows: Vec<Row>,
    #[n(3)]
    pub cursor: Cursor,
    #[n(4)]
    pub reason: Option<ExitReason>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct LinkRow {
    #[n(0)]
    pub row: u16,
    #[n(1)]
    pub spans: Vec<LinkSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct LinkSpan {
    #[n(0)]
    pub start: u16,
    #[n(1)]
    pub end: u16,
    #[n(2)]
    pub uri: String,
}
