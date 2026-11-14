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

/// What a queued message stands for: a newer message with the same key folds
/// into it instead of waiting behind it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CoalesceKey {
    Changed(Topic),
    SessionMetadata(SessionId),
    Progress(SessionId),
    FilesChanged(crate::ProjectId),
    GitChanged(crate::ProjectId),
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
    TerminalInput(crate::TerminalInput),
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
    /// `None` for a message that is delivered on its own, however many follow it.
    pub fn coalesce_key(&self) -> Option<CoalesceKey> {
        match self {
            Self::Changed { topic, .. } => Some(CoalesceKey::Changed(*topic)),
            Self::SessionMetadata { session, .. } => Some(CoalesceKey::SessionMetadata(*session)),
            Self::Progress { session, .. } => Some(CoalesceKey::Progress(*session)),
            Self::FilesChanged { project, .. } => Some(CoalesceKey::FilesChanged(*project)),
            Self::GitChanged { project } => Some(CoalesceKey::GitChanged(*project)),
            _ => None,
        }
    }

    /// Folds `newer`, which has the same coalesce key, into this message.
    pub fn merge(&mut self, newer: Self) {
        match (self, newer) {
            (
                Self::Changed { revision, .. },
                Self::Changed {
                    revision: newer, ..
                },
            ) => *revision = (*revision).max(newer),
            (Self::FilesChanged { changes, .. }, Self::FilesChanged { changes: newer, .. }) => {
                changes.merge(&newer);
            }
            (Self::GitChanged { .. }, Self::GitChanged { .. }) => {}
            (older, newer) => *older = newer,
        }
    }

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
            | Self::TerminalInput(_)
            | Self::Frame(_)
            | Self::Metadata(_)
            | Self::Mouse(_)
            | Self::CellSize(_) => ChannelKind::Session,
        }
    }
}
