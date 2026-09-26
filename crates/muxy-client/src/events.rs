use std::io::Read;
use std::sync::mpsc::Sender;

use muxy_protocol::transport::StreamCancellation;
use muxy_protocol::wire::Decoder;
use muxy_protocol::{
    CONTROL, ChannelId, ErrorCode, ErrorReply, ExitReason, Feature, Message, MetadataEvent,
    ReplyBody, ScreenFrame, SessionId, Topic,
};

use crate::ClientError;
use crate::handshake;
use crate::requests::Pending;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientEvent {
    SessionMetadata {
        session: SessionId,
        metadata: muxy_protocol::SessionMetadata,
    },
    ActivityChanged {
        revision: u64,
    },
    Progress {
        session: SessionId,
        progress: muxy_protocol::SessionProgress,
    },
    FilesChanged {
        project: muxy_protocol::ProjectId,
        changes: muxy_protocol::FileChanges,
    },
    GitChanged {
        project: muxy_protocol::ProjectId,
    },
    SessionsChanged {
        revision: u64,
    },
    CatalogChanged {
        revision: u64,
    },
    RemoteAccessChanged {
        revision: u64,
    },
    Frame {
        channel: ChannelId,
        frame: ScreenFrame,
    },
    Metadata {
        channel: ChannelId,
        event: MetadataEvent,
    },
    SessionEnded {
        session: SessionId,
        reason: ExitReason,
    },
    Disconnected,
    ServerRestarting,
}

pub(crate) fn route(
    decoder: &mut Decoder<impl Read>,
    pending: &Pending,
    events: &Sender<ClientEvent>,
    connected: &Sender<Result<(muxy_protocol::ServerInfo, Vec<Feature>), ClientError>>,
    cancellation: &dyn StreamCancellation,
) {
    let handshake = handshake::accept(decoder.next());
    let accepted = handshake.is_ok();
    let _ = connected.send(handshake);
    if accepted {
        while let Some(event) = next_event(decoder, pending) {
            if events.send(event).is_err() {
                break;
            }
        }
    }
    pending.close();
    cancellation.cancel();
    let _ = events.send(ClientEvent::Disconnected);
}

fn next_event(decoder: &mut Decoder<impl Read>, pending: &Pending) -> Option<ClientEvent> {
    loop {
        let (channel, message) = decoder.next().ok()?;
        if message.validate().is_err() {
            return None;
        }
        match (channel, message) {
            (CONTROL, Message::SessionMetadata { session, metadata }) => {
                return Some(ClientEvent::SessionMetadata { session, metadata });
            }
            (CONTROL, Message::Changed { topic, revision }) => {
                return Some(match topic {
                    Topic::Catalog => ClientEvent::CatalogChanged { revision },
                    Topic::Sessions => ClientEvent::SessionsChanged { revision },
                    Topic::Activity => ClientEvent::ActivityChanged { revision },
                    Topic::RemoteAccess => ClientEvent::RemoteAccessChanged { revision },
                    Topic::Unrecognized(_) => continue,
                });
            }
            (CONTROL, Message::Progress { session, progress }) => {
                return Some(ClientEvent::Progress { session, progress });
            }
            (CONTROL, Message::FilesChanged { project, changes }) => {
                return Some(ClientEvent::FilesChanged { project, changes });
            }
            (CONTROL, Message::GitChanged { project }) => {
                return Some(ClientEvent::GitChanged { project });
            }
            (CONTROL, Message::ServerRestarting) => return Some(ClientEvent::ServerRestarting),
            (CONTROL, Message::Reply { id, body }) => pending.resolve(id, body),
            (CONTROL, Message::UnreadableReply { id }) => {
                pending.resolve(id, ReplyBody::Error(unreadable_reply()));
            }
            (CONTROL, Message::SessionEnded { session, reason }) => {
                return Some(ClientEvent::SessionEnded { session, reason });
            }
            (channel, Message::Frame(frame)) if channel != CONTROL => {
                return Some(ClientEvent::Frame { channel, frame });
            }
            (channel, Message::Metadata(event)) if channel != CONTROL => {
                return Some(ClientEvent::Metadata { channel, event });
            }
            _ => return None,
        }
    }
}

fn unreadable_reply() -> ErrorReply {
    ErrorReply {
        code: ErrorCode::Unsupported,
        message: "The server replied in a form this app can't read. Update Muxy.".into(),
    }
}
