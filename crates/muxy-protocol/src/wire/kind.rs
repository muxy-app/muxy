use crate::Message;

use crate::wire::WireError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MessageKind {
    Hello = 1,
    Request = 2,
    FrameAck = 3,
    HelloReply = 4,
    VersionUnsupported = 5,
    Reply = 6,
    SessionEnded = 7,
    Fatal = 8,
    Input = 9,
    Frame = 10,
    Metadata = 11,
    Mouse = 12,
    ServerRestarting = 13,
    CellSize = 14,
    Changed = 15,
    // 16 and 19 are retired; see `Changed`.
    GitChanged = 17,
    Progress = 18,
    SessionMetadata = 20,
    FilesChanged = 21,
    // 22 is retired; see `Changed`.
}

impl MessageKind {
    /// `None` is a kind from a newer build, which this build ignores.
    pub fn from_u8(value: u8) -> Result<Option<Self>, WireError> {
        if value & 0xc0 != 0 {
            return Err(WireError::FlagsSet(value));
        }
        Ok(Some(match value {
            1 => Self::Hello,
            2 => Self::Request,
            3 => Self::FrameAck,
            4 => Self::HelloReply,
            5 => Self::VersionUnsupported,
            6 => Self::Reply,
            7 => Self::SessionEnded,
            8 => Self::Fatal,
            9 => Self::Input,
            10 => Self::Frame,
            11 => Self::Metadata,
            12 => Self::Mouse,
            13 => Self::ServerRestarting,
            14 => Self::CellSize,
            15 => Self::Changed,
            17 => Self::GitChanged,
            18 => Self::Progress,
            20 => Self::SessionMetadata,
            21 => Self::FilesChanged,
            _ => return Ok(None),
        }))
    }
}

impl From<&Message> for MessageKind {
    fn from(message: &Message) -> Self {
        match message {
            Message::FilesChanged { .. } => Self::FilesChanged,
            Message::SessionMetadata { .. } => Self::SessionMetadata,
            Message::GitChanged { .. } => Self::GitChanged,
            Message::Progress { .. } => Self::Progress,
            Message::Changed { .. } => Self::Changed,
            Message::Hello { .. } => Self::Hello,
            Message::Request { .. } | Message::UnsupportedRequest { .. } => Self::Request,
            Message::FrameAck { .. } => Self::FrameAck,
            Message::HelloReply { .. } => Self::HelloReply,
            Message::VersionUnsupported => Self::VersionUnsupported,
            Message::Reply { .. } | Message::UnreadableReply { .. } => Self::Reply,
            Message::SessionEnded { .. } => Self::SessionEnded,
            Message::Fatal(_) => Self::Fatal,
            Message::Input(_) => Self::Input,
            Message::Frame(_) => Self::Frame,
            Message::Metadata(_) => Self::Metadata,
            Message::Mouse(_) => Self::Mouse,
            Message::CellSize(_) => Self::CellSize,
            Message::ServerRestarting => Self::ServerRestarting,
        }
    }
}
