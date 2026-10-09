use std::sync::{Arc, Weak};

use muxy_protocol::{ChannelId, SessionId};

use crate::{AttachmentEvent, ServerError};

use super::Outbox;

/// Writes one attached channel's events straight into its connection's outbox.
///
/// Dropping it without an `Ended` event still fails the channel, so the
/// client hears about a session that went away.
pub(crate) struct OutboxSink {
    outbox: Weak<Outbox>,
    channel: ChannelId,
    session: SessionId,
}

impl OutboxSink {
    pub(super) fn new(outbox: &Arc<Outbox>, channel: ChannelId, session: SessionId) -> Self {
        Self {
            outbox: Arc::downgrade(outbox),
            channel,
            session,
        }
    }

    pub(crate) fn send(&mut self, event: AttachmentEvent) -> bool {
        let Some(outbox) = self.outbox.upgrade() else {
            return false;
        };
        match event {
            AttachmentEvent::Snapshot { snapshot, process } => {
                outbox.snapshot(self.channel, snapshot, process)
            }
            AttachmentEvent::Metadata(event) => outbox.push_metadata(self.channel, event),
            AttachmentEvent::Frame(frame) => outbox.push_frame(self.channel, frame),
            AttachmentEvent::Resized(frame) => outbox.resized(self.channel, frame),
            AttachmentEvent::Ended(_) => {
                self.end();
                false
            }
        }
    }

    fn end(&mut self) {
        if let Some(outbox) = std::mem::take(&mut self.outbox).upgrade() {
            outbox.attach_failed(self.channel, &ServerError::unknown_session(self.session));
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
    use std::sync::mpsc::{self, Receiver};
    use std::thread;
    use std::time::Duration;

    use muxy_protocol::{
        AttachSnapshot, CONTROL, ChannelId, ErrorCode, ExitReason, Message, MetadataEvent,
        ReplyBody, RequestId, ScreenFrame, TerminalColors,
    };

    use super::OutboxSink;
    use crate::connection::Outbox;
    use crate::{AttachmentEvent, AttachmentId, AttachmentSink, SessionCommand, SessionHandle};

    type TestResult = Result<(), Box<dyn Error>>;

    const CHANNEL: ChannelId = ChannelId(1);
    const REQUEST: RequestId = RequestId(7);

    /// An outbox with `CHANNEL` attached and waiting on `REQUEST`, and that channel's sink.
    fn attached() -> Result<(Arc<Outbox>, Receiver<impl Sized>, OutboxSink), Box<dyn Error>> {
        let outbox = Arc::new(Outbox::new(Arc::default()));
        let (handle, commands) = SessionHandle::fake();
        let sink = OutboxSink::new(&outbox, CHANNEL, handle.id());
        let id = AttachmentId(1);
        outbox.attach(CHANNEL, id, REQUEST, handle, SessionCommand::Detach(id))?;
        Ok((outbox, commands, sink))
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
        let (outbox, _commands, mut sink) = attached()?;
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
            outbox.next(),
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
            outbox.next(),
            Some((CHANNEL, Message::Metadata(MetadataEvent::Bell)))
        );
        assert!(sink.send(AttachmentEvent::Frame(frame.clone())));
        assert_eq!(
            outbox.next(),
            Some((CHANNEL, Message::Frame(frame.clone())))
        );

        outbox.detach(CHANNEL)?;
        assert!(!sink.send(AttachmentEvent::Frame(frame)));
        drop(sink);
        outbox.close();
        assert_eq!(outbox.next(), None);
        Ok(())
    }

    #[test]
    fn a_session_that_goes_away_fails_the_pending_attach_once() -> TestResult {
        for ended in [false, true] {
            let (outbox, _commands, mut sink) = attached()?;
            if ended {
                assert!(!sink.send(AttachmentEvent::Ended(ExitReason::Ended)));
            }
            drop(sink);
            outbox.close();
            assert!(is_failed_attach(outbox.next()), "ended: {ended}");
            assert_eq!(outbox.next(), None);
        }
        Ok(())
    }

    #[test]
    fn attaching_to_a_stopped_session_fails_without_holding_the_outbox() -> TestResult {
        for colors in [None, Some(TerminalColors::default())] {
            let outbox = Arc::new(Outbox::new(Arc::default()));
            if let Some(colors) = colors {
                outbox.set_colors(colors);
            }
            let (handle, commands) = SessionHandle::fake();
            drop(commands);
            let attaching = Arc::clone(&outbox);
            let (done, result) = mpsc::channel();
            thread::spawn(move || {
                let id = AttachmentId(1);
                let sink = OutboxSink::new(&attaching, CHANNEL, handle.id());
                let command = SessionCommand::AttachWithoutResize {
                    id,
                    channel: CHANNEL,
                    sink: AttachmentSink::outbox(sink),
                };
                let _ = done.send(attaching.attach(CHANNEL, id, REQUEST, handle, command));
            });
            let result = result.recv_timeout(Duration::from_secs(5))?;
            assert!(matches!(result, Err(error) if error.code() == ErrorCode::UnknownSession));
            assert!(outbox.handle(CHANNEL).is_none());
            outbox.close();
            assert_eq!(
                outbox.next(),
                None,
                "the request's own error is the only reply"
            );
        }
        Ok(())
    }
}
