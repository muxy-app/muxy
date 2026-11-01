use std::os::unix::net::UnixStream;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use muxy_protocol::wire::{Decoder, Encoder};
use muxy_protocol::{
    AttachSnapshot, CONTROL, ChannelId, Cursor, Message, Modes, ReplyBody, RequestBody, RequestId,
    ScreenFrame, ServerInfo, ServerPath, SessionId, Size,
};

use super::Client;
use crate::{ClientError, ClientEvent};

const SIZE: Size = Size { cols: 10, rows: 2 };
const CHANNEL: ChannelId = ChannelId(1);

struct Server {
    decoder: Decoder<UnixStream>,
    encoder: Encoder<UnixStream>,
}

impl Server {
    fn start(timeout: Duration) -> (Self, Client, Receiver<ClientEvent>) {
        let (client, server) = muxy_protocol::transport::socket_pair().expect("socket pair");
        let mut server = Self {
            decoder: Decoder::new(server.try_clone().expect("clone")),
            encoder: Encoder::new(server),
        };
        let replying = std::thread::spawn(move || {
            server.decoder.next().expect("hello");
            server
                .encoder
                .send(
                    CONTROL,
                    &Message::HelloReply {
                        versions: muxy_protocol::SUPPORTED.to_vec(),
                        server: ServerInfo::current(),
                        features: Vec::new(),
                    },
                )
                .expect("hello reply");
            server
        });
        let client = Client::from_stream(Box::new(client))
            .expect("connect")
            .with_timeout(timeout);
        let events = client.events().expect("events");
        (replying.join().expect("server"), client, events)
    }

    fn attach_request(&mut self) -> RequestId {
        match self.decoder.next().expect("request") {
            (
                CONTROL,
                Message::Request {
                    id,
                    body: RequestBody::Attach { .. },
                },
            ) => id,
            other => panic!("expected an attach, got {other:?}"),
        }
    }

    fn attach_reply(&mut self, id: RequestId) {
        self.encoder
            .send(
                CONTROL,
                &Message::Reply {
                    id,
                    body: ReplyBody::Attached {
                        snapshot: Box::new(snapshot()),
                        process: None,
                    },
                },
            )
            .expect("reply");
    }

    fn frame(&mut self, seq: u64) {
        self.encoder
            .send(CHANNEL, &Message::Frame(frame(seq)))
            .expect("frame");
    }
}

fn snapshot() -> AttachSnapshot {
    AttachSnapshot {
        links: None,
        graphics: muxy_protocol::Graphics::default(),
        prompts: Vec::new(),
        channel: CHANNEL,
        size: SIZE,
        rows: Vec::new(),
        cursor: cursor(),
        modes: Modes::default(),
        title: "shell".into(),
        directory: ServerPath(b"/tmp".to_vec()),
        history: Vec::new(),
        history_cursor: None,
        history_total: 0,
    }
}

fn frame(seq: u64) -> ScreenFrame {
    ScreenFrame {
        size: SIZE,
        graphics: None,
        seq,
        reset: false,
        rows: Vec::new(),
        cursor: cursor(),
        modes: Modes::default(),
    }
}

fn cursor() -> Cursor {
    Cursor {
        shape: muxy_protocol::CursorShape::default(),
        row: 0,
        col: 0,
        visible: true,
    }
}

fn session() -> SessionId {
    SessionId::new(7).expect("session")
}

#[test]
fn the_attached_terminal_arrives_before_its_frames_and_before_attach_returns() {
    let (mut server, client, events) = Server::start(Duration::from_secs(5));
    let serving = std::thread::spawn(move || {
        let id = server.attach_request();
        server.attach_reply(id);
        server.frame(1);
        server
    });
    assert!(matches!(client.attach(42, session(), SIZE), Ok(CHANNEL)));
    match events.try_recv() {
        Ok(ClientEvent::Attached {
            tag: 42,
            session: attached,
            attachment,
        }) => {
            assert_eq!(attached, session());
            assert_eq!(attachment.terminal.channel(), CHANNEL);
            assert_eq!(attachment.title, "shell");
        }
        other => panic!("expected the terminal first, got {other:?}"),
    }
    let _server = serving.join().expect("server");
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(5)),
        Ok(ClientEvent::Frame { channel: CHANNEL, frame }) if frame.seq == 1
    ));
}

#[test]
fn a_reply_after_the_caller_gave_up_still_announces_the_terminal() {
    let (mut server, client, events) = Server::start(Duration::from_millis(50));
    let id = std::thread::scope(|scope| {
        let waiting = scope.spawn(|| client.attach(9, session(), SIZE));
        let id = server.attach_request();
        assert!(matches!(
            waiting.join().expect("attach"),
            Err(ClientError::Timeout)
        ));
        id
    });
    server.attach_reply(id);
    assert!(matches!(
        events.recv_timeout(Duration::from_secs(5)),
        Ok(ClientEvent::Attached { tag: 9, .. })
    ));
    assert!(client.is_connected());
}
