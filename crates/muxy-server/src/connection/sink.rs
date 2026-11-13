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
                connection.snapshot(self.channel, *snapshot, process)
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
            snapshot: Box::new(snapshot),
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
    fn a_connection_that_defers_images_gets_screens_without_their_pixels() -> TestResult {
        let image = muxy_protocol::GraphicImage {
            id: 1,
            generation: 1,
            width: 1,
            height: 1,
            rgba: vec![255; 4].into(),
            ..muxy_protocol::GraphicImage::default()
        };
        let graphics = muxy_protocol::Graphics {
            images: vec![image.clone()],
            ..muxy_protocol::Graphics::default()
        };
        let listed = muxy_protocol::GraphicImage {
            rgba: Arc::default(),
            ..image
        };
        for defers in [false, true] {
            let (connection, _commands, mut sink) = attached()?;
            if defers {
                connection.defer_images();
            }
            let mut snapshot = snapshot()?;
            snapshot.graphics = graphics.clone();
            let frame = ScreenFrame {
                size: snapshot.size,
                graphics: Some(graphics.clone()),
                seq: 1,
                reset: false,
                rows: Vec::new(),
                cursor: snapshot.cursor,
                modes: snapshot.modes,
            };
            assert!(sink.send(AttachmentEvent::Snapshot {
                snapshot: Box::new(snapshot),
                process: None,
            }));
            let resized = frame.clone();
            assert!(sink.send(AttachmentEvent::Frame(frame)));
            let expected = if defers { &listed } else { &graphics.images[0] };
            let Some((
                CONTROL,
                Message::Reply {
                    body: ReplyBody::Attached { snapshot, .. },
                    ..
                },
            )) = connection.next()
            else {
                return Err("no attach reply".into());
            };
            assert_eq!(&snapshot.graphics.images[0], expected, "defers: {defers}");
            let Some((CHANNEL, Message::Frame(frame))) = connection.next() else {
                return Err("no frame".into());
            };
            let sent = frame.graphics.ok_or("no graphics")?;
            assert_eq!(&sent.images[0], expected, "defers: {defers}");
            assert_eq!(sent.validate(), Ok(()));

            // Two frames that merge while the app is busy, then a screen read with history.
            for seq in [2, 3] {
                assert!(sink.send(AttachmentEvent::Frame(ScreenFrame {
                    seq,
                    graphics: (seq == 2).then(|| graphics.clone()),
                    ..resized.clone()
                })));
            }
            connection.ack(CHANNEL, 1);
            let Some((CHANNEL, Message::Frame(merged))) = connection.next() else {
                return Err("no merged frame".into());
            };
            assert_eq!(merged.seq, 3);
            let merged = merged.graphics.ok_or("no merged graphics")?;
            assert_eq!(&merged.images[0], expected, "defers: {defers}");
            let mut page = muxy_protocol::HistoryPage {
                prompts: Vec::new(),
                rows: Vec::new(),
                next: None,
                total_rows: 0,
                screen: Some(muxy_protocol::SavedScreen {
                    graphics: graphics.clone(),
                    size: resized.size,
                    rows: Vec::new(),
                    cursor: resized.cursor,
                    reason: None,
                }),
                links: None,
                graphics: Some(graphics.clone()),
            };
            connection.defer_page(&mut page);
            let screen = page.screen.ok_or("no screen")?;
            assert_eq!(&screen.graphics.images[0], expected, "defers: {defers}");
            let history = page.graphics.ok_or("no history images")?;
            assert_eq!(&history.images[0], expected, "defers: {defers}");
        }
        Ok(())
    }

    #[test]
    fn a_connection_that_streams_images_is_sent_each_one_once() -> TestResult {
        let image = |generation| muxy_protocol::GraphicImage {
            id: 1,
            generation,
            width: 1,
            height: 1,
            rgba: vec![255; 4].into(),
            ..muxy_protocol::GraphicImage::default()
        };
        let graphics = |generation| muxy_protocol::Graphics {
            images: vec![image(generation)],
            ..muxy_protocol::Graphics::default()
        };
        let (connection, _commands, mut sink) = attached()?;
        connection.stream_images();
        let mut snapshot = snapshot()?;
        snapshot.graphics = graphics(1);
        let frame = ScreenFrame {
            size: snapshot.size,
            graphics: Some(graphics(1)),
            seq: 1,
            reset: false,
            rows: Vec::new(),
            cursor: snapshot.cursor,
            modes: snapshot.modes,
        };
        assert!(sink.send(AttachmentEvent::Snapshot {
            snapshot: Box::new(snapshot),
            process: None,
        }));
        let Some((
            CONTROL,
            Message::Reply {
                body: ReplyBody::Attached { snapshot, .. },
                ..
            },
        )) = connection.next()
        else {
            return Err("no attach reply".into());
        };
        assert_eq!(snapshot.graphics.images[0], image(1));
        let mut changed = frame.clone();
        changed.seq = 2;
        changed.graphics = Some(graphics(2));
        for (frame, pixels) in [(frame, false), (changed, true)] {
            let seq = frame.seq;
            assert!(sink.send(AttachmentEvent::Frame(frame)));
            let Some((CHANNEL, Message::Frame(sent))) = connection.next() else {
                return Err("no frame".into());
            };
            let sent = sent.graphics.ok_or("no graphics")?;
            assert_eq!(!sent.images[0].rgba.is_empty(), pixels, "frame {seq}");
            assert_eq!(sent.validate(), Ok(()));
            connection.ack(CHANNEL, seq);
        }
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
