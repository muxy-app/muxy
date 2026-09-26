use serde::{Deserialize, Serialize};

use crate::wire::cbor::open_enum;
use crate::{
    ChannelId, ErrorReply, ExitReason, Feature, MetadataEvent, MouseEvent, ReplyBody, RequestBody,
    RequestId, ScreenFrame, SessionId, Version,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ChannelKind {
    Control,
    Session,
}

open_enum! {
    /// What a [`Message::Changed`] notification tells clients to refetch.
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
    pub enum Topic {
        Catalog = 0,
        Sessions = 1,
        Activity = 2,
        RemoteAccess = 3,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Message {
    SessionMetadata {
        session: SessionId,
        metadata: crate::SessionMetadata,
    },
    Progress {
        session: SessionId,
        progress: crate::SessionProgress,
    },
    /// Coalesced invalidation: the topic reached `revision`.
    Changed {
        topic: Topic,
        revision: u64,
    },
    Hello {
        versions: Vec<Version>,
    },
    Request {
        id: RequestId,
        body: RequestBody,
    },
    /// A request whose body this build can't read, such as a method from a newer client.
    UnsupportedRequest {
        id: RequestId,
    },
    FrameAck {
        channel: ChannelId,
        seq: u64,
    },
    HelloReply {
        versions: Vec<Version>,
        server: crate::ServerInfo,
        features: Vec<Feature>,
    },
    VersionUnsupported,
    ServerRestarting,
    Reply {
        id: RequestId,
        body: ReplyBody,
    },
    /// A reply whose body this build can't read, such as a newer server's shape.
    UnreadableReply {
        id: RequestId,
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
    FilesChanged {
        project: crate::ProjectId,
        changes: crate::FileChanges,
    },
    GitChanged {
        project: crate::ProjectId,
    },
}

impl Message {
    pub fn channel_kind(&self) -> ChannelKind {
        match self {
            Self::FilesChanged { .. }
            | Self::SessionMetadata { .. }
            | Self::Progress { .. }
            | Self::GitChanged { .. }
            | Self::Changed { .. }
            | Self::Hello { .. }
            | Self::Request { .. }
            | Self::UnsupportedRequest { .. }
            | Self::FrameAck { .. }
            | Self::HelloReply { .. }
            | Self::ServerRestarting
            | Self::VersionUnsupported
            | Self::Reply { .. }
            | Self::UnreadableReply { .. }
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
