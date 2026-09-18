use serde::{Deserialize, Serialize};

use crate::{
    ChannelId, ErrorReply, ExitReason, MetadataEvent, MouseEvent, ReplyBody, RequestBody,
    RequestId, ScreenFrame, SessionId, Version,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChannelKind {
    Control,
    Session,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Message {
    SessionMetadata {
        session: SessionId,
        metadata: crate::SessionMetadata,
    },
    ActivityChanged {
        revision: u64,
    },
    Progress {
        session: SessionId,
        progress: crate::SessionProgress,
    },
    SessionsChanged {
        revision: u64,
    },
    CatalogChanged {
        revision: u64,
    },
    Hello {
        compatibility: u64,
        versions: Vec<Version>,
    },
    Request {
        id: RequestId,
        body: RequestBody,
    },
    FrameAck {
        channel: ChannelId,
        seq: u64,
    },
    HelloReply {
        server: crate::ServerInfo,
        versions: Vec<Version>,
    },
    VersionUnsupported,
    ServerRestarting,
    Reply {
        id: RequestId,
        body: ReplyBody,
    },
    SessionEnded {
        session: SessionId,
        reason: ExitReason,
    },
    Fatal(ErrorReply),
    Input(Vec<u8>),
    Frame(ScreenFrame),
    Metadata(MetadataEvent),
    Mouse(MouseEvent),
    CellSize(crate::CellSize),
    GitChanged {
        project: crate::ProjectId,
    },
}

impl Message {
    pub fn channel_kind(&self) -> ChannelKind {
        match self {
            Self::SessionMetadata { .. }
            | Self::ActivityChanged { .. }
            | Self::Progress { .. }
            | Self::GitChanged { .. }
            | Self::SessionsChanged { .. }
            | Self::CatalogChanged { .. }
            | Self::Hello { .. }
            | Self::Request { .. }
            | Self::FrameAck { .. }
            | Self::HelloReply { .. }
            | Self::ServerRestarting
            | Self::VersionUnsupported
            | Self::Reply { .. }
            | Self::SessionEnded { .. }
            | Self::Fatal(_) => ChannelKind::Control,
            Self::Input(_)
            | Self::Frame(_)
            | Self::Metadata(_)
            | Self::Mouse(_)
            | Self::CellSize(_) => ChannelKind::Session,
        }
    }
}
