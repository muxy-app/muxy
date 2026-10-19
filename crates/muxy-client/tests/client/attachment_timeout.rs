use super::*;
use muxy_protocol::{ClientKind, ReplyBody};
use std::io::{self, Cursor, Read, Write};

#[test]
fn lost_attach_reply_releases_ownership_and_preserves_the_session_for_reopening() -> TestResult {
    for shared in [false, true] {
        let fixture = Fixture::new()?;
        let (installed, received) = mpsc::channel();
        let desktop = fixture
            .connect_stream(|socket| Box::new(WithoutAttachReplies { socket, installed }))?;
        let owner = desktop.client.identify(ClientKind::Desktop)?;
        desktop.client.sync_session_references(vec![])?;
        let session = fixture.create(&desktop.client)?;
        let observer = fixture.connect()?;
        let observer_id = observer.client.identify(ClientKind::Tui)?;
        if shared {
            observer.client.attach(session.id, SIZE)?;
        }
        assert_eq!(
            observer
                .client
                .project_sessions(session.project, None, None)?
                .sessions[0]
                .owner,
            Some(owner)
        );
        let attached = desktop
            .client
            .clone()
            .with_timeout(Duration::from_secs(1))
            .attach(session.id, SIZE);
        received.recv_timeout(TIMEOUT)?;
        assert!(matches!(attached, Err(ClientError::Timeout)));
        assert!(!desktop.client.is_connected());
        desktop.finished.recv_timeout(TIMEOUT)??;
        let reopened = fixture.connect()?;
        let available = reopened
            .client
            .available_project_sessions(session.project)?
            .sessions;
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].info.id, session.id);
        assert_eq!(available[0].owner, shared.then_some(observer_id));
        assert!(!available[0].attached);
        let mut attachment = reopened.client.attach(session.id, SIZE)?;
        reopened.client.send_input(
            attachment.channel,
            &print_joined("AFTER_ATTACH", "_TIMEOUT"),
        )?;
        reopened.frame_containing(&mut attachment, "AFTER_ATTACH_TIMEOUT")?;
    }
    Ok(())
}

struct WithoutAttachReplies {
    socket: LocalStream,
    installed: Sender<ChannelId>,
}

impl ByteStream for WithoutAttachReplies {
    fn cancellation(&self) -> io::Result<Box<dyn StreamCancellation>> {
        self.socket.cancellation()
    }

    fn split(self: Box<Self>) -> io::Result<(Box<dyn Read + Send>, Box<dyn Write + Send>)> {
        let reader = ReplyFilter {
            decoder: Decoder::new(self.socket.try_clone()?),
            buffer: Cursor::new(Vec::new()),
            installed: self.installed,
        };
        Ok((Box::new(reader), Box::new(self.socket)))
    }
}

struct ReplyFilter {
    decoder: Decoder<LocalStream>,
    buffer: Cursor<Vec<u8>>,
    installed: Sender<ChannelId>,
}

impl Read for ReplyFilter {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let count = self.buffer.read(output)?;
            if count > 0 {
                return Ok(count);
            }
            let (channel, message) = self.decoder.next().map_err(io::Error::other)?;
            if let Message::Reply {
                body: ReplyBody::Attached { snapshot, .. },
                ..
            } = &message
            {
                self.installed
                    .send(snapshot.channel)
                    .map_err(io::Error::other)?;
                continue;
            }
            muxy_protocol::wire::encode(&message, channel, self.buffer.get_mut())
                .map_err(io::Error::other)?;
            self.buffer.set_position(0);
        }
    }
}
