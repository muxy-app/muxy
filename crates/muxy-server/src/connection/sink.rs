use std::sync::{Arc, Weak};

use muxy_protocol::{ChannelId, SessionId};

use crate::{AttachmentEvent, ServerError};

use super::Connection;

/// Writes one attached channel's events straight into its connection's outbox.
///
/// Dropping it without an `Ended` event still fails the channel, so the
/// client hears about a session that went away.
pub(crate) struct OutboxSink {
    connection: Weak<Connection>,
    channel: ChannelId,
    session: SessionId,
}

impl OutboxSink {
    pub(super) fn new(
        connection: &Arc<Connection>,
        channel: ChannelId,
        session: SessionId,
    ) -> Self {
        Self {
            connection: Arc::downgrade(connection),
            channel,
            session,
        }
    }

    pub(crate) fn send(&mut self, event: AttachmentEvent) -> bool {
        let Some(connection) = self.connection.upgrade() else {
            return false;
        };
        match event {
            AttachmentEvent::Snapshot { snapshot, process } => {
                connection.snapshot(self.channel, snapshot, process)
            }
            AttachmentEvent::Metadata(event) => connection.push_metadata(self.channel, event),
            AttachmentEvent::Frame(frame) => connection.push_frame(self.channel, frame),
            AttachmentEvent::Resized(frame) => connection.resized(self.channel, frame),
            AttachmentEvent::Ended(_) => {
                self.end();
                false
            }
        }
    }

    fn end(&mut self) {
        if let Some(connection) = std::mem::take(&mut self.connection).upgrade() {
            connection.attach_failed(self.channel, &ServerError::unknown_session(self.session));
        }
    }
}

impl Drop for OutboxSink {
    fn drop(&mut self) {
        self.end();
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Arc;
    use std::sync::mpsc::Receiver;

    use muxy_protocol::{
        AttachSnapshot, CONTROL, ChannelId, ErrorCode, ExitReason, Message, MetadataEvent,
        ReplyBody, RequestId, ScreenFrame,
    };

    use super::OutboxSink;
    use crate::connection::Connection;
    use crate::{AttachmentEvent, AttachmentId, SessionCommand, SessionHandle};

    type TestResult = Result<(), Box<dyn Error>>;

    const CHANNEL: ChannelId = ChannelId(1);
    const REQUEST: RequestId = RequestId(7);

    /// A connection with `CHANNEL` attached and waiting on `REQUEST`, and that channel's sink.
    fn attached() -> Result<(Arc<Connection>, Receiver<impl Sized>, OutboxSink), Box<dyn Error>> {
        let connection = Arc::new(Connection::new(Arc::default()));
        let (handle, commands) = SessionHandle::fake();
        let sink = OutboxSink::new(&connection, CHANNEL, handle.id());
        let id = AttachmentId(1);
        connection.attach(CHANNEL, id, REQUEST, handle, SessionCommand::Detach(id))?;
        Ok((connection, commands, sink))
    }

    fn snapshot() -> Result<AttachSnapshot, Box<dyn Error>> {
        Message::samples()
            .into_iter()
            .find_map(|message| match message {
                Message::Reply {
                    body: ReplyBody::Attached { snapshot, .. },
                    ..
                } => Some(*snapshot),
                _ => None,
            })
            .ok_or_else(|| "no attach sample".into())
    }

    fn is_failed_attach(message: Option<(ChannelId, Message)>) -> bool {
        matches!(
            message,
            Some((CONTROL, Message::Reply { id: REQUEST, body: ReplyBody::Error(error) }))
                if error.code == ErrorCode::UnknownSession
        )
    }

    #[test]
    fn events_reach_the_outbox_until_the_channel_is_detached() -> TestResult {
        let (connection, _commands, mut sink) = attached()?;
        let snapshot = snapshot()?;
        let frame = ScreenFrame {
            size: snapshot.size,
            graphics: None,
            seq: 1,
            reset: false,
            rows: snapshot.rows.clone(),
            cursor: snapshot.cursor,
            modes: snapshot.modes,
        };

        assert!(sink.send(AttachmentEvent::Snapshot {
            snapshot,
            process: None,
        }));
        assert!(matches!(
            connection.next(),
            Some((
                CONTROL,
                Message::Reply {
                    id: REQUEST,
                    body: ReplyBody::Attached { .. }
                }
            ))
        ));
        assert!(sink.send(AttachmentEvent::Metadata(MetadataEvent::Bell)));
        assert_eq!(
            connection.next(),
            Some((CHANNEL, Message::Metadata(MetadataEvent::Bell)))
        );
        assert!(sink.send(AttachmentEvent::Frame(frame.clone())));
        assert_eq!(
            connection.next(),
            Some((CHANNEL, Message::Frame(frame.clone())))
        );

        connection.detach(CHANNEL)?;
        assert!(!sink.send(AttachmentEvent::Frame(frame)));
        drop(sink);
        connection.close();
        assert_eq!(connection.next(), None);
        Ok(())
    }

    #[test]
    fn a_session_that_goes_away_fails_the_pending_attach_once() -> TestResult {
        for ended in [false, true] {
            let (connection, _commands, mut sink) = attached()?;
            if ended {
                assert!(!sink.send(AttachmentEvent::Ended(ExitReason::Ended)));
            }
            drop(sink);
            connection.close();
            assert!(is_failed_attach(connection.next()), "ended: {ended}");
            assert_eq!(connection.next(), None);
        }
        Ok(())
    }
}
