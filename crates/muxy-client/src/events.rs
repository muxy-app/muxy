use std::io::Read;
use std::sync::mpsc::{Receiver, Sender};

use muxy_protocol::transport::StreamCancellation;
use muxy_protocol::wire::Decoder;
use muxy_protocol::{
    CONTROL, ChannelId, ErrorCode, ErrorReply, ExitReason, Feature, Message, MetadataEvent,
    ReplyBody, RequestId, ScreenFrame, SessionId, Topic,
};

use crate::handshake;
use crate::requests::Pending;
use crate::{Attachment, ClientError};

mod clipboard;

pub use clipboard::ClipboardWrite;
use clipboard::ClipboardWrites;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientEvent {
    /// The reply to `Client::attach` with this `tag`; the channel's frames follow it.
    Attached {
        tag: u64,
        session: SessionId,
        attachment: Box<Attachment>,
    },
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
    ClipboardWrite {
        channel: ChannelId,
        write: ClipboardWrite,
    },
    SessionEnded {
        session: SessionId,
        reason: ExitReason,
    },
    Disconnected,
    ServerRestarting,
}

/// Takes the terminal that `Client::attach` announced with `tag`, for a caller
/// that reads the events itself. Earlier events are dropped.
pub fn take_attached(events: &Receiver<ClientEvent>, tag: u64) -> Result<Attachment, ClientError> {
    events
        .try_iter()
        .find_map(|event| match event {
            ClientEvent::Attached {
                tag: attached,
                attachment,
                ..
            } if attached == tag => Some(*attachment),
            _ => None,
        })
        .ok_or(ClientError::Disconnected)
}

pub(crate) fn route(
    decoder: &mut Decoder<impl Read>,
    pending: &Pending,
    events: &Sender<ClientEvent>,
    connected: &Sender<Result<(muxy_protocol::ServerInfo, Vec<Feature>), ClientError>>,
    cancellation: &dyn StreamCancellation,
) {
    let handshake = handshake::accept(decoder.next());
    let server_input = handshake
        .as_ref()
        .ok()
        .map(|(_, features)| features.contains(&Feature::TERMINAL_INPUT));
    let _ = connected.send(handshake);
    let mut router = Router {
        pending,
        clipboard: ClipboardWrites::default(),
        server_input: server_input.unwrap_or_default(),
    };
    if server_input.is_some() {
        while let Some((event, attached)) = router.next(decoder) {
            if events.send(event).is_err() {
                break;
            }
            // The caller learns of the attach only once its terminal is queued.
            if let Some((id, channel)) = attached {
                pending.resolve_attached(id, channel);
            }
        }
    }
    pending.close();
    cancellation.cancel();
    let _ = events.send(ClientEvent::Disconnected);
}

struct Router<'a> {
    pending: &'a Pending,
    clipboard: ClipboardWrites,
    server_input: bool,
}

impl Router<'_> {
    /// The next event, and the attach request it answers.
    fn next(
        &mut self,
        decoder: &mut Decoder<impl Read>,
    ) -> Option<(ClientEvent, Option<(RequestId, ChannelId)>)> {
        loop {
            let (channel, message) = decoder.next().ok()?;
            if message.validate().is_err() {
                return None;
            }
            let event = match (channel, message) {
                (CONTROL, Message::SessionMetadata { session, metadata }) => {
                    ClientEvent::SessionMetadata { session, metadata }
                }
                (CONTROL, Message::Changed { topic, revision }) => match topic {
                    Topic::Catalog => ClientEvent::CatalogChanged { revision },
                    Topic::Sessions => ClientEvent::SessionsChanged { revision },
                    Topic::Activity => ClientEvent::ActivityChanged { revision },
                    Topic::RemoteAccess => ClientEvent::RemoteAccessChanged { revision },
                    Topic::Unrecognized(_) => continue,
                },
                (CONTROL, Message::Progress { session, progress }) => {
                    ClientEvent::Progress { session, progress }
                }
                (CONTROL, Message::FilesChanged { project, changes }) => {
                    ClientEvent::FilesChanged { project, changes }
                }
                (CONTROL, Message::GitChanged { project }) => ClientEvent::GitChanged { project },
                (CONTROL, Message::ServerRestarting) => ClientEvent::ServerRestarting,
                (CONTROL, Message::Reply { id, body }) => match self.reply(id, body) {
                    Some(attached) => return Some(attached),
                    None => continue,
                },
                (CONTROL, Message::UnreadableReply { id }) => {
                    self.pending
                        .resolve(id, ReplyBody::Error(unreadable_reply()));
                    continue;
                }
                (CONTROL, Message::SessionEnded { session, reason }) => {
                    ClientEvent::SessionEnded { session, reason }
                }
                (channel, Message::Frame(frame)) if channel != CONTROL => {
                    ClientEvent::Frame { channel, frame }
                }
                (channel, Message::Metadata(MetadataEvent::ClipboardWrite(text)))
                    if channel != CONTROL =>
                {
                    match self.clipboard.push(channel, text) {
                        Some(write) => ClientEvent::ClipboardWrite { channel, write },
                        None => continue,
                    }
                }
                (channel, Message::Metadata(event)) if channel != CONTROL => {
                    ClientEvent::Metadata { channel, event }
                }
                _ => return None,
            };
            return Some((event, None));
        }
    }

    /// An attach reply becomes its terminal's event; any other goes to whoever waits.
    fn reply(
        &self,
        id: RequestId,
        body: ReplyBody,
    ) -> Option<(ClientEvent, Option<(RequestId, ChannelId)>)> {
        let attaching = match &body {
            ReplyBody::Attached { .. } => self.pending.take_attaching(id),
            _ => None,
        };
        match (body, attaching) {
            (ReplyBody::Attached { snapshot, process }, Some(attaching)) => {
                let channel = snapshot.channel;
                let attachment = Attachment::new(*snapshot, process, self.server_input);
                let event = ClientEvent::Attached {
                    tag: attaching.tag,
                    session: attaching.session,
                    attachment: Box::new(attachment),
                };
                Some((event, Some((id, channel))))
            }
            (body, _) => {
                self.pending.resolve(id, body);
                None
            }
        }
    }
}

fn unreadable_reply() -> ErrorReply {
    ErrorReply {
        code: ErrorCode::Unsupported,
        message: "The server replied in a form this app can't read. Update Muxy.".into(),
    }
}
